use super::{providers::ProviderError, types::*};
use super::{
    providers::{self, Inspection},
    CreditManager,
};
use std::collections::HashMap;
use std::{sync::Arc, time::Duration};
use tauri::Emitter;
use tokio::sync::watch;

pub(super) const MIN_BALANCE_GAP: i64 = 60_000;
pub(super) const PROXY_DELAY: i64 = 5_000;
pub(super) const CLAIM_RECHECK_DELAY: i64 = 15_000;
const FOREGROUND_INTERVAL: i64 = 5 * 60_000;
const BACKGROUND_INTERVAL: i64 = 15 * 60_000;
const MODELS_INTERVAL: i64 = 30 * 60_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Part {
    Balance = 0,
    Rewards = 1,
    Models = 2,
}
impl Part {
    pub const ALL: [Self; 3] = [Self::Balance, Self::Rewards, Self::Models];
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Scope(pub [bool; 3]);
impl Scope {
    pub const FULL: Self = Self([true; 3]);
    #[cfg(test)]
    pub const BALANCE: Self = Self([true, false, false]);
    pub fn contains(self, part: Part) -> bool {
        self.0[part as usize]
    }
    pub fn is_empty(self) -> bool {
        !self.0.iter().any(|value| *value)
    }
    pub fn without(self, other: Self) -> Self {
        Self(std::array::from_fn(|i| self.0[i] && !other.0[i]))
    }
}

#[derive(Default)]
pub(super) struct PartState {
    pub last_success: Option<i64>,
    pub last_attempt: Option<i64>,
    failures: usize,
    retry_at: Option<i64>,
    paused: bool,
    rate_limit_until: Option<i64>,
}

pub(super) struct AccountSchedule {
    pub parts: [PartState; 3],
    pub running: Option<Scope>,
    pub finished: watch::Sender<u64>,
    pub generation: u64,
    pending_balance: Option<i64>,
    claim_recheck_at: Option<i64>,
}
impl Default for AccountSchedule {
    fn default() -> Self {
        Self {
            parts: Default::default(),
            running: None,
            finished: watch::channel(0).0,
            generation: 0,
            pending_balance: None,
            claim_recheck_at: None,
        }
    }
}
impl AccountSchedule {
    pub fn load_times(&mut self, account: &Account) {
        if let Some(snapshot) = &account.snapshot {
            let times = [
                Some(snapshot.fetched_at),
                snapshot
                    .sync
                    .rewards_fetched_at
                    .or(Some(snapshot.fetched_at)),
                snapshot
                    .sync
                    .models_fetched_at
                    .or(Some(snapshot.fetched_at)),
            ];
            for (state, time) in self.parts.iter_mut().zip(times) {
                if state.last_attempt.is_none() {
                    state.last_success = time.filter(|time| *time > 0);
                }
            }
        }
    }
    pub fn rate_limit_until(&self) -> i64 {
        self.parts
            .iter()
            .filter_map(|state| state.rate_limit_until)
            .max()
            .unwrap_or(0)
    }
    pub fn begin(&mut self, scope: Scope, now: i64) -> u64 {
        self.running = Some(scope);
        for part in Part::ALL {
            if scope.contains(part) {
                self.parts[part as usize].last_attempt = Some(now);
            }
        }
        if scope.contains(Part::Balance) && self.pending_balance.is_some_and(|due| due <= now) {
            self.pending_balance = None;
        }
        if scope.contains(Part::Balance) && self.claim_recheck_at.is_some_and(|due| due <= now) {
            self.claim_recheck_at = None;
        }
        self.generation
    }
    pub fn finish_part(&mut self, part: Part, now: i64, error: Option<&ProviderError>) {
        let state = &mut self.parts[part as usize];
        if let Some(error) = error {
            let delays = [60_000, 120_000, 300_000, 900_000, 1_800_000];
            let delay = delays[state.failures.min(delays.len() - 1)];
            state.failures = state.failures.saturating_add(1);
            state.paused =
                error.pause_auto || (error.rejected && matches!(error.status, 401 | 403));
            if error.status == 429 {
                state.rate_limit_until =
                    Some(now.saturating_add(error.retry_after_ms.unwrap_or(delay).max(1_000)));
            }
            state.retry_at = Some(
                now.saturating_add(delay)
                    .max(state.rate_limit_until.unwrap_or(0)),
            );
        } else {
            *state = PartState {
                last_success: Some(now),
                last_attempt: state.last_attempt,
                ..Default::default()
            };
        }
    }
    pub fn dirty(&mut self, due: i64) {
        self.generation = self.generation.wrapping_add(1);
        self.pending_balance = Some(
            self.pending_balance
                .map_or(due, |previous| previous.min(due)),
        );
    }
    pub fn recheck_claim(&mut self, due: i64) {
        self.claim_recheck_at = Some(
            self.claim_recheck_at
                .map_or(due, |previous| previous.min(due)),
        );
    }
    pub fn has_pending_balance(&self) -> bool {
        self.pending_balance.is_some() || self.claim_recheck_at.is_some()
    }
    pub fn resume(&mut self) {
        for part in &mut self.parts {
            part.paused = false;
            part.retry_at = None;
        }
    }
}

pub(super) struct RefreshSchedule {
    pub accounts: HashMap<String, AccountSchedule>,
    pub foreground: bool,
    started_at: i64,
}
impl RefreshSchedule {
    pub fn new(now: i64) -> Self {
        Self {
            accounts: HashMap::new(),
            foreground: false,
            started_at: now,
        }
    }
    pub fn account(&mut self, account: &Account) -> &mut AccountSchedule {
        let state = self.accounts.entry(account.id.clone()).or_default();
        state.load_times(account);
        state
    }
    pub fn due(&mut self, account: &Account, now: i64, enabled: bool) -> Scope {
        if !enabled || !account.enabled || account.status == "expired" {
            return Scope::default();
        }
        let interval = if self.foreground {
            FOREGROUND_INTERVAL
        } else {
            BACKGROUND_INTERVAL
        };
        let startup = self.started_at + PROXY_DELAY + jitter(&account.id);
        let started_at = self.started_at;
        let state = self.account(account);
        if state.running.is_some() || state.rate_limit_until() > now {
            return Scope::default();
        }
        Scope(std::array::from_fn(|index| {
            let part = &state.parts[index];
            if part.paused {
                return false;
            }
            if let Some(retry) = part.retry_at {
                return now >= retry;
            }
            let cadence = [interval, BACKGROUND_INTERVAL, MODELS_INTERVAL][index];
            let mut due = part.last_success.map_or(startup, |last| {
                if part.last_attempt.is_none() && started_at.saturating_sub(last) >= MIN_BALANCE_GAP
                {
                    startup
                } else {
                    last + cadence + jitter(&account.id)
                }
            });
            if index == Part::Balance as usize {
                let priority = state.claim_recheck_at.is_some_and(|due| due <= now);
                if let Some(pending) = state.pending_balance {
                    due = due.min(pending);
                }
                if let Some(recheck) = state.claim_recheck_at {
                    due = due.min(recheck);
                }
                if !priority {
                    if let Some(last) = part.last_attempt {
                        due = due.max(last + MIN_BALANCE_GAP);
                    }
                }
            }
            now >= due
        }))
    }
    pub fn focus(&mut self, accounts: &[Account], now: i64) {
        for account in accounts.iter().filter(|account| account.enabled) {
            let state = self.account(account);
            if state.parts[0]
                .last_success
                .is_none_or(|last| now.saturating_sub(last) >= MIN_BALANCE_GAP)
            {
                state.dirty(now);
            }
        }
    }
    pub fn resume(&mut self) {
        for state in self.accounts.values_mut() {
            state.resume();
        }
    }
}

// Stable, positive staggering keeps accounts apart without shortening the cadence.
fn jitter(id: &str) -> i64 {
    id.bytes().fold(0_u64, |value, byte| {
        value.wrapping_mul(31).wrapping_add(byte as u64)
    }) as i64
        & 4095
}

struct RefreshFlight {
    manager: Arc<CreditManager>,
    id: String,
}
impl Drop for RefreshFlight {
    fn drop(&mut self) {
        if let Some(state) = self
            .manager
            .refresh_schedule
            .lock()
            .accounts
            .get_mut(&self.id)
        {
            state.running = None;
            state
                .finished
                .send_modify(|version| *version = version.wrapping_add(1));
        }
        self.manager.emit_credits_changed();
        self.manager.refresh_wake.notify_one();
    }
}
enum RefreshStep {
    Wait(watch::Receiver<u64>, Scope),
    Start(u64),
    Limited(i64),
}

impl CreditManager {
    pub fn attach_events(&self, app: tauri::AppHandle) {
        *self.event_sink.lock() = Some(app);
    }
    pub(super) fn emit_credits_changed(&self) {
        if let Some(app) = self.event_sink.lock().as_ref() {
            let _ = app.emit("credits-updated", ());
        }
    }
    pub fn set_credits_view_active(&self, active: bool) -> Result<(), String> {
        let accounts = self.accounts()?;
        let mut schedule = self.refresh_schedule.lock();
        if active && !schedule.foreground {
            schedule.focus(&accounts, now_ms());
        }
        schedule.foreground = active;
        drop(schedule);
        self.refresh_wake.notify_one();
        Ok(())
    }
    pub fn credits_network_restored(&self) -> Result<(), String> {
        let accounts = self.accounts()?;
        let mut schedule = self.refresh_schedule.lock();
        for state in schedule.accounts.values_mut() {
            for part in &mut state.parts {
                if !part.paused {
                    part.retry_at = None;
                }
            }
        }
        schedule.focus(&accounts, now_ms());
        drop(schedule);
        self.refresh_wake.notify_one();
        Ok(())
    }
    pub(super) fn start_refresh_scheduler(self: &Arc<Self>) {
        let manager = self.clone();
        tokio::spawn(async move {
            let mut timer = tokio::time::interval(Duration::from_secs(5));
            timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    biased;
                    _ = manager.cancellation.cancelled() => break,
                    _ = timer.tick() => {},
                    _ = manager.refresh_wake.notified() => {},
                }
                let Ok(settings) = manager.store.settings() else {
                    continue;
                };
                let Ok(accounts) = manager.accounts() else {
                    continue;
                };
                let plans = {
                    let mut schedule = manager.refresh_schedule.lock();
                    schedule
                        .accounts
                        .retain(|id, _| accounts.iter().any(|account| &account.id == id));
                    accounts
                        .iter()
                        .filter_map(|account| {
                            let scope = schedule.due(account, now_ms(), settings.auto_refresh);
                            (!scope.is_empty()).then(|| (account.id.clone(), scope))
                        })
                        .collect::<Vec<_>>()
                };
                for (id, scope) in plans {
                    let worker = manager.clone();
                    tokio::spawn(async move {
                        tokio::select! {
                            biased;
                            _ = worker.cancellation.cancelled() => {},
                            _ = worker.refresh_scope(&id, scope, false) => {},
                        }
                    });
                }
            }
        });
    }
    pub(super) async fn refresh_scope(
        self: &Arc<Self>,
        id: &str,
        mut scope: Scope,
        manual: bool,
    ) -> Result<Account, String> {
        if self.cancellation.is_cancelled() {
            return Err("应用正在退出".into());
        }
        let generation = loop {
            let account = self.store.account(id)?;
            let step = {
                let mut schedule = self.refresh_schedule.lock();
                let state = schedule.account(&account);
                if let Some(running) = state.running {
                    RefreshStep::Wait(state.finished.subscribe(), running)
                } else if state.rate_limit_until() > now_ms() {
                    RefreshStep::Limited(state.rate_limit_until())
                } else {
                    if manual {
                        state.resume();
                    }
                    RefreshStep::Start(state.begin(scope, now_ms()))
                }
            };
            match step {
                RefreshStep::Start(generation) => break generation,
                RefreshStep::Limited(until) => {
                    return Err(format!(
                        "平台请求限流，请在 {} 秒后重试",
                        (until - now_ms()).max(1000) / 1000
                    ))
                }
                RefreshStep::Wait(mut finished, running) => {
                    tokio::select! {
                        _ = self.cancellation.cancelled() => return Err("应用正在退出".into()),
                        _ = finished.changed() => {},
                    }
                    scope = scope.without(running);
                    if scope.is_empty() {
                        return self.store.account(id);
                    }
                }
            }
        };
        let _flight = RefreshFlight {
            manager: self.clone(),
            id: id.into(),
        };
        self.emit_credits_changed();
        let _slot = self
            .refresh_slots
            .acquire()
            .await
            .map_err(|_| "刷新已停止".to_string())?;
        let gate = self.gate(id);
        let _guard = gate.lock().await;
        let mut account = self.store.account(id)?;
        if self.cancellation.is_cancelled() {
            return Err("应用正在退出".into());
        }
        if !manual && (!account.enabled || !self.store.settings()?.auto_refresh) {
            return Ok(account);
        }
        {
            let mut schedule = self.refresh_schedule.lock();
            let state = schedule.account(&account);
            for part in Part::ALL {
                if scope.contains(part) {
                    state.parts[part as usize].last_attempt = Some(now_ms());
                }
            }
        }
        let inspect = async {
            let credentials = match self.credentials_unlocked(&account, false).await {
                Ok(credentials) => credentials,
                Err(error) => return Inspection::failed(scope, error),
            };
            let mut result = self
                .provider
                .inspect_parts(&account, &credentials, scope)
                .await;
            if result.auth_rejected() {
                result = match self.credentials_unlocked(&account, true).await {
                    Ok(credentials) => {
                        self.provider
                            .inspect_parts(&account, &credentials, scope)
                            .await
                    }
                    Err(error) => Inspection::failed(scope, error),
                };
            }
            result
        };
        let result = tokio::time::timeout(Duration::from_secs(60), inspect)
            .await
            .unwrap_or_else(|_| Inspection::failed(scope, ProviderError::new(504, "平台同步超时")));
        let now = now_ms();
        let updates = merge_inspection(&mut account, result, now);
        let mut schedule = self.refresh_schedule.lock();
        let state = schedule.account(&account);
        if state.generation != generation {
            if let Some(snapshot) = &mut account.snapshot {
                snapshot.stale = true;
            }
        }
        match self.store.save_account(&account) {
            Ok(()) => {
                let pause_account =
                    updates
                        .iter()
                        .filter_map(|(_, error)| error.as_ref())
                        .any(|error| {
                            error.pause_auto
                                || (error.rejected && matches!(error.status, 401 | 403))
                        });
                for (part, error) in updates {
                    state.finish_part(part, now, error.as_ref());
                }
                if pause_account {
                    for part in &mut state.parts {
                        part.paused = true;
                    }
                }
            }
            Err(message) => {
                let error = ProviderError::new(500, &message);
                for part in Part::ALL {
                    if scope.contains(part) {
                        state.finish_part(part, now, Some(&error));
                    }
                }
                return Err(message);
            }
        }
        Ok(account)
    }
}

fn merge_inspection(
    account: &mut Account,
    inspection: Inspection,
    now: i64,
) -> Vec<(Part, Option<ProviderError>)> {
    let mut updates = Vec::new();
    let snapshot = account.snapshot.get_or_insert_with(|| {
        let mut value = providers::snapshot(vec![], vec![], vec![], None);
        value.fetched_at = 0;
        value
    });
    // Freeze the legacy full-snapshot timestamps before a balance-only update.
    snapshot
        .sync
        .rewards_fetched_at
        .get_or_insert(snapshot.fetched_at);
    snapshot
        .sync
        .models_fetched_at
        .get_or_insert(snapshot.fetched_at);
    if let Some(result) = inspection.balance {
        match result {
            Ok(balance) => {
                snapshot.balance = balance.balance;
                snapshot.packs = balance.packs;
                snapshot.quota = balance.quota;
                snapshot.unit = balance.unit;
                snapshot.fetched_at = now;
                snapshot.stale = false;
                account.status = "ready".into();
                account.last_error = None;
                updates.push((Part::Balance, None));
            }
            Err(error) => {
                snapshot.stale = true;
                account.status = if matches!(error.status, 401 | 403) {
                    "expired"
                } else {
                    "error"
                }
                .into();
                account.last_error = Some(error.message.clone());
                updates.push((Part::Balance, Some(error)));
            }
        }
    }
    if let Some(result) = inspection.rewards {
        match result {
            Ok(rewards) => {
                snapshot.rewards = rewards;
                snapshot.sync.rewards_fetched_at = Some(now);
                snapshot.sync.rewards_error = None;
                updates.push((Part::Rewards, None));
            }
            Err(error) => {
                snapshot.sync.rewards_error = Some(error.message.clone());
                updates.push((Part::Rewards, Some(error)));
            }
        }
    }
    if let Some(result) = inspection.models {
        match result {
            Ok(mut models) => {
                for model in &mut models {
                    if let Some(old) = snapshot.models.iter().find(|old| {
                        old.id == model.id && old.canonical_id == model.canonical_id && old.verified
                    }) {
                        model.verified = true;
                        model.verification_note = old.verification_note.clone();
                        model.supports_tools = old.supports_tools;
                    }
                }
                snapshot.models = models;
                snapshot.sync.models_fetched_at = Some(now);
                snapshot.sync.models_error = None;
                updates.push((Part::Models, None));
            }
            Err(error) => {
                snapshot.sync.models_error = Some(error.message.clone());
                updates.push((Part::Models, Some(error)));
            }
        }
    }
    let diagnostics = [
        snapshot.sync.rewards_error.as_deref(),
        snapshot.sync.models_error.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    snapshot.diagnostic = (!diagnostics.is_empty()).then(|| diagnostics.join("；"));
    updates
}

#[cfg(test)]
mod tests {
    use super::*;
    fn account() -> Account {
        Account {
            id: "a".into(),
            platform: Platform::Qoder,
            user_id: "test".into(),
            label: "test".into(),
            enabled: true,
            status: "ready".into(),
            last_error: None,
            snapshot: Some(ProviderSnapshot {
                balance: Some(100.0),
                unit: "credits".into(),
                quota: None,
                packs: vec![],
                rewards: vec![],
                models: vec![],
                fetched_at: 100_000,
                stale: false,
                diagnostic: None,
                sync: SnapshotSync::default(),
            }),
        }
    }
    #[test]
    fn foreground_background_and_component_cadences() {
        let a = account();
        let mut schedule = RefreshSchedule::new(100_000);
        schedule.account(&a).begin(Scope::FULL, 100_000);
        schedule.account(&a).running = None;
        assert!(schedule.due(&a, 401_000, true).is_empty());
        schedule.foreground = true;
        assert_eq!(schedule.due(&a, 401_000, true), Scope::BALANCE);
        schedule.foreground = false;
        assert_eq!(
            schedule.due(&a, 1_001_000, true),
            Scope([true, true, false])
        );
        assert_eq!(schedule.due(&a, 1_901_000, true), Scope::FULL);
        assert!(schedule.due(&a, 1_901_000, false).is_empty());
        let mut disabled = a.clone();
        disabled.enabled = false;
        assert!(schedule.due(&disabled, 1_901_000, true).is_empty());
    }
    #[test]
    fn startup_and_wake_catch_up_only_once() {
        let a = account();
        let mut schedule = RefreshSchedule::new(200_000);
        assert!(schedule.due(&a, 204_000, true).is_empty());
        assert_eq!(schedule.due(&a, 210_000, true), Scope::FULL);
        let state = schedule.account(&a);
        state.begin(Scope::FULL, 7_000_000);
        for part in Part::ALL {
            state.finish_part(part, 7_000_000, None);
        }
        state.running = None;
        assert!(schedule.due(&a, 7_000_001, true).is_empty());
    }
    #[test]
    fn recent_startup_cache_waits_for_cadence_and_claim_recheck_bypasses_gap_once() {
        let a = account();
        let mut schedule = RefreshSchedule::new(120_000);
        assert!(schedule.due(&a, 170_000, true).is_empty());
        let state = schedule.account(&a);
        state.begin(Scope::BALANCE, 180_000);
        state.finish_part(Part::Balance, 180_000, None);
        state.running = None;
        state.dirty(185_000);
        state.recheck_claim(195_000);
        assert!(!schedule.due(&a, 185_000, true).contains(Part::Balance));
        assert!(schedule.due(&a, 195_000, true).contains(Part::Balance));
        let state = schedule.account(&a);
        state.begin(Scope::BALANCE, 195_000);
        state.finish_part(Part::Balance, 195_000, None);
        state.running = None;
        assert!(!state.has_pending_balance());
        assert!(!schedule.due(&a, 210_000, true).contains(Part::Balance));
    }
    #[test]
    fn burst_requests_coalesce_and_inflight_consumption_stays_pending() {
        let a = account();
        let mut schedule = RefreshSchedule::new(100_000);
        for _ in 0..100 {
            schedule.account(&a).dirty(110_000);
        }
        let state = schedule.account(&a);
        let generation = state.begin(Scope::BALANCE, 110_000);
        assert!(!state.has_pending_balance());
        state.dirty(116_000);
        assert_ne!(generation, state.generation);
        state.finish_part(Part::Balance, 112_000, None);
        state.running = None;
        assert!(schedule.due(&a, 116_000, true).is_empty());
        assert_eq!(schedule.due(&a, 170_000, true), Scope::BALANCE);
    }
    #[test]
    fn backoff_auth_pause_and_retry_after_apply_to_event_refreshes() {
        let a = account();
        let mut schedule = RefreshSchedule::new(100_000);
        let mut now = 110_000;
        for delay in [60_000, 120_000, 300_000, 900_000, 1_800_000, 1_800_000] {
            let state = schedule.account(&a);
            state.begin(Scope::BALANCE, now);
            state.finish_part(
                Part::Balance,
                now,
                Some(&ProviderError::new(502, "offline")),
            );
            state.running = None;
            state.dirty(now);
            assert!(!schedule
                .due(&a, now + delay - 1, true)
                .contains(Part::Balance));
            now += delay;
            assert!(schedule.due(&a, now, true).contains(Part::Balance));
        }
        let state = schedule.account(&a);
        state.finish_part(Part::Balance, now, Some(&providers::http_error(401)));
        assert!(!schedule
            .due(&a, now + 9_000_000, true)
            .contains(Part::Balance));
        schedule.resume();
        let mut limited = ProviderError::new(429, "limited");
        limited.retry_after_ms = Some(3_600_000);
        schedule
            .account(&a)
            .finish_part(Part::Balance, now, Some(&limited));
        schedule.resume(); // A manual retry cannot bypass the server's limit.
        assert!(schedule.due(&a, now + 3_599_999, true).is_empty());
    }

    fn inspection(balance: f64) -> Inspection {
        let mut value = providers::snapshot(vec![], vec![], vec![], None);
        value.balance = Some(balance);
        Inspection {
            balance: Some(Ok(value)),
            rewards: Some(Ok(vec![])),
            models: Some(Ok(vec![])),
        }
    }
    fn saved_account(manager: &Arc<CreditManager>, id: &str) -> Account {
        let mut a = account();
        a.id = id.into();
        a.user_id = id.into();
        manager.store.save_account(&a).unwrap();
        manager
            .save_credentials(
                id,
                &Credentials {
                    access_token: "synthetic-test-token".into(),
                    refresh_token: None,
                    user_id: None,
                    device_id: None,
                    expires_at: None,
                    extra: serde_json::Value::Null,
                },
            )
            .unwrap();
        a
    }
    async fn wait_for_calls(manager: &Arc<CreditManager>, count: usize) {
        tokio::time::timeout(Duration::from_secs(2), async {
            while manager.provider.test_inspection_calls().len() < count {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
    #[tokio::test]
    async fn concurrent_manual_refreshes_share_one_inspection() {
        let manager = CreditManager::test();
        saved_account(&manager, "one");
        let gate = Arc::new(tokio::sync::Semaphore::new(0));
        manager.provider.test_inspection_gate(gate.clone());
        manager
            .provider
            .queue_test_inspection("one", inspection(88.0));
        let worker = manager.clone();
        let requests = tokio::spawn(async move {
            futures::future::join_all((0..20).map(|_| worker.refresh_account("one"))).await
        });
        wait_for_calls(&manager, 1).await;
        assert_eq!(
            manager.dashboard().unwrap().refreshing_account_ids,
            vec!["one"]
        );
        gate.add_permits(1);
        for result in requests.await.unwrap() {
            assert_eq!(result.unwrap().snapshot.unwrap().balance, Some(88.0));
        }
        assert_eq!(manager.provider.test_inspection_calls().len(), 1);
        assert!(manager
            .dashboard()
            .unwrap()
            .refreshing_account_ids
            .is_empty());
    }
    #[tokio::test]
    async fn manual_full_refresh_during_balance_only_work_fetches_only_missing_parts() {
        let manager = CreditManager::test();
        saved_account(&manager, "one");
        let gate = Arc::new(tokio::sync::Semaphore::new(0));
        manager.provider.test_inspection_gate(gate.clone());
        manager
            .provider
            .queue_test_inspection("one", inspection(88.0));
        manager
            .provider
            .queue_test_inspection("one", inspection(77.0));
        let worker = manager.clone();
        let background =
            tokio::spawn(async move { worker.refresh_scope("one", Scope::BALANCE, false).await });
        wait_for_calls(&manager, 1).await;
        let worker = manager.clone();
        let manual = tokio::spawn(async move { worker.refresh_account("one").await });
        tokio::task::yield_now().await;
        gate.add_permits(2);
        background.await.unwrap().unwrap();
        manual.await.unwrap().unwrap();
        let calls = manager.provider.test_inspection_calls();
        assert_eq!(
            calls.iter().map(|(_, scope)| *scope).collect::<Vec<_>>(),
            vec![Scope::BALANCE, Scope([false, true, true])]
        );
        assert_eq!(
            manager
                .store
                .account("one")
                .unwrap()
                .snapshot
                .unwrap()
                .balance,
            Some(88.0)
        );
    }
    #[tokio::test]
    async fn global_concurrency_is_three_and_consumption_during_refresh_is_not_lost() {
        let manager = CreditManager::test();
        let gate = Arc::new(tokio::sync::Semaphore::new(0));
        manager.provider.test_inspection_gate(gate.clone());
        let mut tasks = Vec::new();
        for id in ["one", "two", "three", "four", "five", "six"] {
            saved_account(&manager, id);
            manager.provider.queue_test_inspection(id, inspection(88.0));
            let worker = manager.clone();
            tasks.push(tokio::spawn(
                async move { worker.refresh_account(id).await },
            ));
        }
        wait_for_calls(&manager, 3).await;
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }
        assert_eq!(manager.provider.test_inspection_calls().len(), 3);
        manager.mark_stale("one");
        gate.add_permits(6);
        for task in tasks {
            task.await.unwrap().unwrap();
        }
        assert_eq!(manager.provider.test_inspection_calls().len(), 6);
        assert!(
            manager
                .dashboard()
                .unwrap()
                .accounts
                .iter()
                .find(|a| a.id == "one")
                .unwrap()
                .snapshot
                .as_ref()
                .unwrap()
                .stale
        );
        assert!(manager.refresh_schedule.lock().accounts["one"].has_pending_balance());
    }
    #[test]
    fn partial_failure_preserves_other_values_and_independent_timestamps() {
        let mut a = account();
        let old = Reward {
            id: "daily".into(),
            name: "daily".into(),
            kind: "credits".into(),
            status: "claimable".into(),
            amount: Some(100.0),
            next_available_at: None,
            action_url: None,
        };
        a.snapshot.as_mut().unwrap().rewards = vec![old];
        let mut result = inspection(88.0);
        result.rewards = Some(Err(ProviderError::new(502, "rewards unavailable")));
        result.models = None;
        merge_inspection(&mut a, result, 200_000);
        let value = a.snapshot.as_ref().unwrap();
        assert_eq!(value.balance, Some(88.0));
        assert!(!value.stale);
        assert_eq!(value.fetched_at, 200_000);
        assert_eq!(value.sync.rewards_fetched_at, Some(100_000));
        assert_eq!(value.sync.models_fetched_at, Some(100_000));
        assert_eq!(value.rewards.len(), 1);
        assert_eq!(
            value.sync.rewards_error.as_deref(),
            Some("rewards unavailable")
        );
        let mut result = inspection(70.0);
        result.balance = Some(Err(ProviderError::new(502, "billing unavailable")));
        merge_inspection(&mut a, result, 300_000);
        let value = a.snapshot.as_ref().unwrap();
        assert_eq!(value.balance, Some(88.0));
        assert_eq!(value.fetched_at, 200_000);
        assert!(value.stale);
        assert_eq!(value.sync.models_fetched_at, Some(300_000));
    }
    #[test]
    fn catalog_failure_keeps_models_and_success_preserves_matching_verification() {
        let mut a = account();
        let model: ModelInfo = serde_json::from_value(serde_json::json!({
            "id":"glm-5.3", "name":"GLM-5.3", "canonicalId":"glm-5.3",
            "supportsTools":true, "contextWindow":1000000,
            "verified":true, "verificationNote":"verified by an earlier request"
        }))
        .unwrap();
        a.snapshot.as_mut().unwrap().models = vec![model.clone()];
        let mut result = inspection(80.0);
        result.models = Some(Err(ProviderError::new(502, "catalog unavailable")));
        merge_inspection(&mut a, result, 200_000);
        let value = a.snapshot.as_ref().unwrap();
        assert_eq!(value.models.len(), 1);
        assert!(value.models[0].verified);
        assert_eq!(value.sync.models_fetched_at, Some(100_000));
        assert_eq!(value.balance, Some(80.0));
        assert_eq!(a.status, "ready");
        let mut unverified = model;
        unverified.verified = false;
        unverified.supports_tools = false;
        let mut result = inspection(70.0);
        result.models = Some(Ok(vec![unverified]));
        merge_inspection(&mut a, result, 300_000);
        let value = a.snapshot.as_ref().unwrap();
        assert!(value.models[0].verified);
        assert!(value.models[0].supports_tools);
        assert!(value.sync.models_error.is_none());
        assert_eq!(value.sync.models_fetched_at, Some(300_000));
    }
    #[tokio::test]
    async fn cancellation_releases_running_state_and_refresh_slot() {
        let manager = CreditManager::test();
        saved_account(&manager, "one");
        manager
            .provider
            .test_inspection_gate(Arc::new(tokio::sync::Semaphore::new(0)));
        manager
            .provider
            .queue_test_inspection("one", inspection(88.0));
        let worker = manager.clone();
        let task = tokio::spawn(async move { worker.refresh_account("one").await });
        wait_for_calls(&manager, 1).await;
        task.abort();
        let _ = task.await;
        assert!(manager
            .dashboard()
            .unwrap()
            .refreshing_account_ids
            .is_empty());
        assert_eq!(manager.refresh_slots.available_permits(), 3);
    }
    #[tokio::test]
    async fn scheduler_updates_without_a_mounted_dashboard_and_stops_on_shutdown() {
        let manager = CreditManager::test();
        let a = saved_account(&manager, "one");
        manager
            .provider
            .queue_test_inspection("one", inspection(80.0));
        let mut completed = manager
            .refresh_schedule
            .lock()
            .account(&a)
            .finished
            .subscribe();
        manager.start_scheduler();
        manager.start_scheduler();
        manager.set_credits_view_active(true).unwrap();
        tokio::time::timeout(Duration::from_secs(2), completed.changed())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            manager
                .store
                .account("one")
                .unwrap()
                .snapshot
                .unwrap()
                .balance,
            Some(80.0)
        );
        assert_eq!(manager.provider.test_inspection_calls().len(), 1);
        manager.shutdown().await;
        assert!(manager.cancellation.is_cancelled());
    }
    #[tokio::test]
    async fn disabled_auto_refresh_does_not_query_but_manual_refresh_still_works() {
        let manager = CreditManager::test();
        saved_account(&manager, "one");
        manager
            .store
            .save_settings(&ProxySettings {
                auto_refresh: false,
                ..ProxySettings::default()
            })
            .unwrap();
        manager
            .provider
            .queue_test_inspection("one", inspection(80.0));
        manager
            .refresh_scope("one", Scope::BALANCE, false)
            .await
            .unwrap();
        assert!(manager.provider.test_inspection_calls().is_empty());
        manager.refresh_account("one").await.unwrap();
        assert_eq!(manager.provider.test_inspection_calls().len(), 1);
    }
    #[tokio::test]
    async fn rate_limit_blocks_manual_retries_and_keeps_previous_balance() {
        let manager = CreditManager::test();
        saved_account(&manager, "one");
        let mut error = ProviderError::new(429, "limited");
        error.retry_after_ms = Some(600_000);
        manager
            .provider
            .queue_test_inspection("one", Inspection::failed(Scope::FULL, error));
        manager.refresh_account("one").await.unwrap();
        manager.retry_secret_access().unwrap();
        assert!(manager
            .refresh_account("one")
            .await
            .unwrap_err()
            .contains("限流"));
        assert_eq!(manager.provider.test_inspection_calls().len(), 1);
        assert_eq!(
            manager
                .store
                .account("one")
                .unwrap()
                .snapshot
                .unwrap()
                .balance,
            Some(100.0)
        );
    }
    #[tokio::test]
    async fn reward_device_rejection_does_not_pause_balance_refresh() {
        let manager = CreditManager::test();
        saved_account(&manager, "one");
        let mut result = inspection(80.0);
        result.rewards = Some(Err(ProviderError::new(403, "device verification required")));
        manager.provider.queue_test_inspection("one", result);
        let a = manager.refresh_account("one").await.unwrap();
        assert_eq!(a.status, "ready");
        let schedule = manager.refresh_schedule.lock();
        assert!(schedule.accounts["one"]
            .parts
            .iter()
            .all(|part| !part.paused));
    }
}
