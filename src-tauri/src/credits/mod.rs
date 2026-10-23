pub mod gateway;
pub mod providers;
mod secrets;
mod refresh;
pub mod store;
pub mod types;

use futures::StreamExt;
use parking_lot::Mutex;
use std::{collections::HashMap, sync::Arc, time::Duration};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{Mutex as AsyncMutex, Notify, Semaphore};
use tokio_util::sync::CancellationToken;
use types::*;

const GATEWAY_API_KEY_ID: &str = "gateway-api-key";

pub struct CreditManager {
    pub store: store::CreditStore,
    pub provider: providers::ProviderClient,
    pub gateway: gateway::GatewayManager,
    secrets: secrets::SecretStore,
    account_gates: Mutex<HashMap<String, Arc<AsyncMutex<()>>>>,
    import_gate: AsyncMutex<()>,
    key_gate: Mutex<()>,
    cancellation: CancellationToken,
    refresh_schedule: Mutex<refresh::RefreshSchedule>,
    refresh_slots: Semaphore,
    refresh_wake: Notify,
    scheduler_started: AtomicBool,
    event_sink: Mutex<Option<tauri::AppHandle>>,
}

impl CreditManager {
    pub fn new(conn: Arc<Mutex<rusqlite::Connection>>) -> Result<Arc<Self>, String> {
        Ok(Arc::new(Self {
            store: store::CreditStore::new(conn)?,
            provider: providers::ProviderClient::new(),
            gateway: gateway::GatewayManager::new(),
            secrets: secrets::SecretStore::new(),
            account_gates: Mutex::new(HashMap::new()),
            import_gate: AsyncMutex::new(()),
            key_gate: Mutex::new(()),
            cancellation: CancellationToken::new(),
            refresh_schedule: Mutex::new(refresh::RefreshSchedule::new(now_ms())),
            refresh_slots: Semaphore::new(3),
            refresh_wake: Notify::new(),
            scheduler_started: AtomicBool::new(false),
            event_sink: Mutex::new(None),
        }))
    }

    #[cfg(test)]
    pub fn test() -> Arc<Self> {
        Arc::new(Self {
            store: store::CreditStore::new(Arc::new(Mutex::new(
                rusqlite::Connection::open_in_memory().unwrap(),
            )))
            .unwrap(),
            provider: providers::ProviderClient::new(),
            gateway: gateway::GatewayManager::new(),
            secrets: secrets::SecretStore::memory(),
            account_gates: Mutex::new(HashMap::new()),
            import_gate: AsyncMutex::new(()),
            key_gate: Mutex::new(()),
            cancellation: CancellationToken::new(),
            refresh_schedule: Mutex::new(refresh::RefreshSchedule::new(now_ms())),
            refresh_slots: Semaphore::new(3),
            refresh_wake: Notify::new(),
            scheduler_started: AtomicBool::new(false),
            event_sink: Mutex::new(None),
        })
    }

    fn gate(&self, id: &str) -> Arc<AsyncMutex<()>> {
        self.account_gates
            .lock()
            .entry(id.into())
            .or_insert_with(|| Arc::new(AsyncMutex::new(())))
            .clone()
    }
    pub fn accounts(&self) -> Result<Vec<Account>, String> {
        self.store.accounts()
    }
    /// A user action may retry a cancelled/denied OS credential request.
    /// Background refreshes and proxy traffic must not clear this pause.
    pub fn retry_secret_access(&self) -> Result<(), String> {
        self.secrets.retry_access()?;
        self.refresh_schedule.lock().resume();
        Ok(())
    }
    pub fn retry_gateway_key_access(&self) -> Result<(), String> {
        self.secrets
            .retry_access_for(GATEWAY_API_KEY_ID)
            .map_err(|error| format!("本地代理密钥不可用：{error}"))
    }
    /// Only the explicit start command may request OS authorization.
    pub async fn start_proxy_for_user(self: &Arc<Self>, port: u16) -> Result<GatewayStatus, String> {
        self.retry_gateway_key_access()?;
        self.gateway.start(self.clone(), port).await
    }
    pub fn credential(&self, account: &Account) -> Result<Credentials, String> {
        let raw = self
            .secrets
            .get(&account.id)?
            .ok_or_else(|| "账号凭据不存在，请重新授权".to_string())?;
        serde_json::from_str(&raw).map_err(|_| "账号凭据格式无效，请重新授权".into())
    }
    fn save_credentials(&self, id: &str, credentials: &Credentials) -> Result<(), String> {
        self.secrets.set(
            id,
            &serde_json::to_string(credentials).map_err(|_| "凭据编码失败".to_string())?,
        )
    }

    pub async fn credentials_for(&self, account: &Account) -> Result<Credentials, String> {
        let credentials = self.credential(account)?;
        if credentials
            .expires_at
            .is_none_or(|expires| expires > now_ms() + 60_000)
        {
            return Ok(credentials);
        }
        let gate = self.gate(&account.id);
        let _guard = gate.lock().await;
        self.credentials_unlocked(account, false)
            .await
            .map_err(|error| error.message)
    }
    pub async fn force_refresh_credentials(
        &self,
        account: &Account,
    ) -> Result<Credentials, String> {
        let gate = self.gate(&account.id);
        let _guard = gate.lock().await;
        self.credentials_unlocked(account, true)
            .await
            .map_err(|error| error.message)
    }
    async fn credentials_unlocked(
        &self,
        account: &Account,
        force: bool,
    ) -> Result<Credentials, providers::ProviderError> {
        let credentials = self
            .credential(account)
            .map_err(|message| {
                let mut error = providers::ProviderError::new(500, message);
                error.pause_auto = true;
                error
            })?;
        if force
            || credentials
                .expires_at
                .is_some_and(|expires| expires <= now_ms() + 60_000)
        {
            let refreshed = self
                .provider
                .refresh_credentials(account.platform, &credentials)
                .await.map_err(|mut error| {
                    // An expired token with no refresh token is also a terminal
                    // credential failure, even without an upstream rejection.
                    if matches!(error.status, 401 | 403) { error.pause_auto = true; }
                    error
                })?;
            self.save_credentials(&account.id, &refreshed)
                .map_err(|message| {
                    let mut error = providers::ProviderError::new(500, message);
                    error.pause_auto = true;
                    error
                })?;
            Ok(refreshed)
        } else {
            Ok(credentials)
        }
    }

    pub async fn add_account(
        self: &Arc<Self>,
        platform: Platform,
        label: String,
        credentials: Credentials,
    ) -> Result<Account, String> {
        if credentials.access_token.trim().is_empty() || credentials.access_token.len() > 65536 {
            return Err("请输入有效的账号凭据".into());
        }
        let authentication = self.provider.authenticate(platform, &credentials).await;
        let (user_id, default_label, credentials) = match authentication {
            Err(error)
                if platform == Platform::Traework
                    && error.rejected
                    && matches!(error.status, 401 | 403)
                    && credentials.refresh_token.is_some() =>
            {
                let refreshed = self
                    .provider
                    .refresh_credentials(platform, &credentials)
                    .await
                    .map_err(|error| error.message)?;
                self.provider
                    .authenticate(platform, &refreshed)
                    .await
                    .map_err(|error| error.message)?
            }
            result => result.map_err(|error| error.message)?,
        };
        let _import = self.import_gate.lock().await;
        let existing = self
            .accounts()?
            .into_iter()
            .find(|a| a.platform == platform && a.user_id == user_id);
        let id = existing
            .as_ref()
            .map(|a| a.id.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let gate = self.gate(&id);
        let _guard = gate.lock().await;
        let previous = self.secrets.get_for_update(&id)?;
        self.save_credentials(&id, &credentials)?;
        let account = Account {
            id: id.clone(),
            platform,
            user_id,
            label: if label.trim().is_empty() {
                default_label
            } else {
                label.trim().chars().take(100).collect()
            },
            enabled: existing.as_ref().is_none_or(|a| a.enabled),
            status: "ready".into(),
            last_error: None,
            snapshot: None,
        };
        if let Err(error) = self.store.save_account(&account) {
            self.secrets.restore(&id, previous.as_deref())?;
            return Err(error);
        }
        drop(_guard);
        drop(_import);
        self.refresh_account(&id).await
    }

    pub async fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), String> {
        let gate = self.gate(id);
        let _guard = gate.lock().await;
        let mut account = self.store.account(id)?;
        account.enabled = enabled;
        self.store.save_account(&account)
    }
    pub async fn delete_account(&self, id: &str) -> Result<(), String> {
        let gate = self.gate(id);
        let _guard = gate.lock().await;
        self.store.account(id)?;
        // Keep a recoverable copy before deletion in case the database rejects it.
        let previous = self.secrets.get(id)?;
        self.secrets.delete(id)?;
        if let Err(error) = self.store.delete_account(id) {
            if let Some(previous) = previous {
                self.secrets.set(id, &previous)?;
            }
            return Err(error);
        }
        Ok(())
    }

    pub async fn refresh_account(self: &Arc<Self>, id: &str) -> Result<Account, String> {
        self.refresh_scope(id, refresh::Scope::FULL, true).await
    }

    pub async fn refresh(self: &Arc<Self>, id: Option<&str>) -> Result<Vec<Account>, String> {
        let accounts = if let Some(id) = id {
            vec![self.store.account(id)?]
        } else {
            self.accounts()?.into_iter().filter(|a| a.enabled).collect()
        };
        let results = futures::stream::iter(accounts)
            .map(|a| async move { self.refresh_account(&a.id).await })
            .buffer_unordered(3)
            .collect::<Vec<_>>()
            .await;
        results.into_iter().collect()
    }

    pub async fn claim(self: &Arc<Self>, id: Option<&str>) -> Result<Vec<RewardOutcome>, String> {
        let accounts = if let Some(id) = id {
            vec![self.store.account(id)?]
        } else {
            self.accounts()?.into_iter().filter(|a| a.enabled).collect()
        };
        let outcomes = futures::stream::iter(accounts)
            .map(|a| async move { self.claim_account(&a.id).await })
            .buffer_unordered(3)
            .collect::<Vec<_>>()
            .await;
        let mut all = Vec::new();
        for outcomes in outcomes {
            all.extend(outcomes?);
        }
        Ok(all)
    }

    async fn claim_account(self: &Arc<Self>, id: &str) -> Result<Vec<RewardOutcome>, String> {
        let gate = self.gate(id);
        let guard = gate.lock().await;
        let account = self.store.account(id)?;
        if !account.enabled {
            return Ok(Vec::new());
        }
        let result = match self.credentials_unlocked(&account, false).await {
            Ok(credentials) => {
                let mut result = self.provider.claim(&account, &credentials).await;
                if result
                    .as_ref()
                    .is_err_and(|e| e.rejected && matches!(e.status, 401 | 403))
                {
                    result = match self.credentials_unlocked(&account, true).await {
                        Ok(refreshed) => self.provider.claim(&account, &refreshed).await,
                        Err(error) => Err(error),
                    };
                }
                result.map_err(|e| e.message)
            }
            Err(error) => Err(error.message),
        };
        let outcomes = result.unwrap_or_else(|error| {
            vec![RewardOutcome {
                account_id: id.into(),
                reward_id: "claim".into(),
                name: "领取奖励".into(),
                status: "failed".into(),
                amount: None,
                kind: "credits".into(),
                message: error,
                created_at: now_ms(),
            }]
        });
        for outcome in &outcomes {
            self.store.record_reward(outcome)?;
        }
        drop(guard);
        // A delayed/rate-limited balance read must not turn a recorded claim into
        // an apparent claim failure that invites the user to submit it again.
        let updated = self.refresh_account(id).await.unwrap_or_else(|_| account.clone());
        if outcomes.iter().any(|outcome| outcome.status == "pending"
            || (matches!(outcome.status.as_str(), "claimed" | "success")
                && account.snapshot.as_ref().and_then(|snapshot| snapshot.balance)
                    == updated.snapshot.as_ref().and_then(|snapshot| snapshot.balance))) {
            self.refresh_schedule.lock().account(&updated).recheck_claim(now_ms() + refresh::CLAIM_RECHECK_DELAY);
            self.refresh_wake.notify_one();
        }
        Ok(outcomes)
    }

    pub fn gateway_key(&self) -> Result<String, String> {
        let _guard = self.key_gate.lock();
        if let Some(key) = self.secrets.get(GATEWAY_API_KEY_ID)? {
            return Ok(key);
        }
        let key = new_key();
        self.secrets.set(GATEWAY_API_KEY_ID, &key)?;
        Ok(key)
    }
    pub fn record_request(&self, summary: RequestSummary) -> Result<(), String> {
        self.store.record_request(&summary)
    }
    pub async fn invalidate_model(
        &self,
        id: &str,
        model_id: &str,
        reason: &str,
    ) -> Result<(), String> {
        let gate = self.gate(id);
        let _guard = gate.lock().await;
        let mut account = self.store.account(id)?;
        if let Some(snapshot) = &mut account.snapshot {
            if let Some(model) = snapshot.models.iter_mut().find(|m| m.id == model_id) {
                model.verified = false;
                model.verification_note = Some(reason.into());
            }
        }
        self.store.save_account(&account)
    }
    pub fn mark_stale(self: &Arc<Self>, id: &str) {
        #[cfg(test)]
        {
            // Offline fixtures must never dispatch their synthetic tokens to real providers.
            if let Ok(mut account) = self.store.account(id) {
                if let Some(snapshot) = &mut account.snapshot {
                    snapshot.stale = true;
                    let _ = self.store.save_account(&account);
                }
            }
        }
        self.refresh_schedule.lock().accounts.entry(id.into()).or_default()
            .dirty(now_ms() + refresh::PROXY_DELAY);
        self.refresh_wake.notify_one();
        self.emit_credits_changed();
    }
    pub fn dashboard(&self) -> Result<Dashboard, String> {
        let settings = self.store.settings()?;
        let mut accounts = self.accounts()?;
        let schedule = self.refresh_schedule.lock();
        for account in &mut accounts {
            if schedule.accounts.get(&account.id).is_some_and(|state| state.has_pending_balance()) {
                if let Some(snapshot) = &mut account.snapshot { snapshot.stale = true; }
            }
        }
        Ok(Dashboard {
            accounts,
            gateway: self.gateway.status(settings.port),
            settings,
            rewards: self.store.rewards()?,
            requests: self.store.requests()?,
            refreshing_account_ids: schedule.accounts.iter()
                .filter(|(_, state)| state.running.is_some()).map(|(id, _)| id.clone()).collect(),
        })
    }
    pub async fn save_settings(&self, settings: ProxySettings) -> Result<ProxySettings, String> {
        if self.gateway.status(settings.port).running
            && settings.port != self.store.settings()?.port
        {
            return Err("请先停止代理再修改端口".into());
        }
        self.store.save_settings(&settings)?;
        self.refresh_wake.notify_one();
        self.emit_credits_changed();
        Ok(settings)
    }

    pub async fn probe_proxy(&self) -> Result<String, String> {
        let status = self.gateway.status(self.store.settings()?.port);
        if !status.running {
            return Err("请先开启代理".into());
        }
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|_| "连接测试初始化失败".to_string())?;
        let response = client
            .get(format!("{}/models", status.base_url))
            .bearer_auth(self.gateway_key()?)
            .send()
            .await
            .map_err(|_| "无法连接本地代理".to_string())?;
        if !response.status().is_success() {
            return Err("代理认证测试失败".into());
        }
        let models: serde_json::Value = response
            .json()
            .await
            .map_err(|_| "代理返回格式错误".to_string())?;
        let count = models["data"].as_array().map_or(0, Vec::len);
        Ok(format!("代理连接正常，当前有 {count} 个可用模型入口"))
    }

    pub fn start_scheduler(self: &Arc<Self>) {
        if self.scheduler_started.swap(true, Ordering::SeqCst) { return; }
        self.start_refresh_scheduler();
        let manager = self.clone();
        tokio::spawn(async move {
            let mut last_day: Option<i64> = None;
            // The first tick is immediate, so startup catch-up follows the same schedule.
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    _ = manager.cancellation.cancelled() => break,
                    _ = interval.tick() => {
                        let now = now_ms();
                        let (day, due) = claim_schedule(now);
                        if due && last_day != Some(day) && manager.store.settings().is_ok_and(|s| s.auto_claim) {
                            last_day = Some(day);
                            tokio::select! {
                                _ = manager.cancellation.cancelled() => break,
                                _ = manager.claim(None) => {},
                            }
                        }
                    }
                }
            }
        });
    }
    pub async fn shutdown(&self) {
        self.cancellation.cancel();
        let _ = self.gateway.stop().await;
    }
}

fn new_key() -> String {
    format!(
        "ait_{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}
fn claim_schedule(now: i64) -> (i64, bool) {
    let beijing = now / 1000 + 8 * 3600;
    (
        beijing.div_euclid(86400),
        beijing.rem_euclid(86400) >= 10 * 3600 + 30 * 60,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn saved_snapshot_account(id: &str) -> Account {
        Account {
            id: id.into(),
            platform: Platform::Workbuddy,
            user_id: "test-user".into(),
            label: "test".into(),
            enabled: true,
            status: "ready".into(),
            last_error: None,
            snapshot: Some(ProviderSnapshot {
                balance: Some(123.0),
                unit: "credits".into(),
                quota: None,
                packs: vec![],
                rewards: vec![],
                models: vec![],
                fetched_at: 1,
                stale: false,
                diagnostic: None,
                sync: SnapshotSync::default(),
            }),
        }
    }

    #[tokio::test]
    async fn refresh_local_credential_failures_preserve_snapshot_as_error() {
        for (raw, expected) in [
            (None, "账号凭据不存在，请重新授权"),
            (Some("invalid-json"), "账号凭据格式无效，请重新授权"),
        ] {
            let manager = CreditManager::test();
            let account = saved_snapshot_account("local-credential-error");
            manager.store.save_account(&account).unwrap();
            if let Some(raw) = raw {
                manager.secrets.set(&account.id, raw).unwrap();
            }
            let updated = manager.refresh_account(&account.id).await.unwrap();
            assert_eq!(updated.status, "error");
            assert_eq!(updated.last_error.as_deref(), Some(expected));
            assert!(manager.refresh_schedule.lock().due(&updated, now_ms() + 86_400_000, true).is_empty());
            assert!(manager.provider.test_inspection_calls().is_empty());
            let snapshot = updated.snapshot.unwrap();
            assert!(snapshot.stale);
            assert_eq!(snapshot.balance, Some(123.0));
            assert_eq!(snapshot.fetched_at, 1);
            assert_eq!(manager.store.account(&account.id).unwrap().status, "error");
        }
    }

    #[tokio::test]
    async fn refresh_expired_credential_without_refresh_token_is_expired() {
        let manager = CreditManager::test();
        let account = saved_snapshot_account("expired-credential");
        manager.store.save_account(&account).unwrap();
        manager
            .save_credentials(
                &account.id,
                &Credentials {
                    access_token: "synthetic-expired-token".into(),
                    refresh_token: None,
                    user_id: None,
                    device_id: None,
                    expires_at: Some(now_ms() - 1),
                    extra: serde_json::Value::Null,
                },
            )
            .unwrap();
        // WorkBuddy rejects a missing refresh token before building any request.
        let updated = manager.refresh_account(&account.id).await.unwrap();
        assert_eq!(updated.status, "expired");
        assert_eq!(
            updated.last_error.as_deref(),
            Some("此凭证没有刷新令牌，请重新登录")
        );
        let snapshot = updated.snapshot.unwrap();
        assert!(snapshot.stale);
        assert_eq!(snapshot.balance, Some(123.0));
        assert_eq!(snapshot.fetched_at, 1);
        assert_eq!(
            manager.store.account(&account.id).unwrap().status,
            "expired"
        );
    }

    #[tokio::test]
    #[ignore = "uses a saved WorkBuddy account for one short capped inference; requires explicit opt-in"]
    async fn saved_workbuddy_messages_smoke() {
        assert_eq!(
            std::env::var("AI_TOOLKIT_PROXY_DIAGNOSTIC").as_deref(),
            Ok("real-inference")
        );
        let path = dirs::home_dir().unwrap().join(".ai-toolkit/ai-toolkit.db");
        let connection =
            rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .unwrap();
        let (metadata, snapshot): (String, String) = connection.query_row(
            "SELECT a.metadata,s.snapshot FROM credit_accounts a JOIN credit_snapshots s ON s.account_id=a.id WHERE a.platform='workbuddy' LIMIT 1",
            [], |row| Ok((row.get(0)?, row.get(1)?))
        ).expect("saved WorkBuddy account available");
        let mut account: Account = serde_json::from_str(&metadata).unwrap();
        account.snapshot = Some(serde_json::from_str(&snapshot).unwrap());
        assert!(account.enabled);
        assert!(account
            .snapshot
            .as_ref()
            .unwrap()
            .models
            .iter()
            .any(|model| model.id == "glm-5.3-flash"));
        let saved = secrets::SecretStore::new()
            .get(&account.id)
            .unwrap()
            .expect("saved system credential available");
        let credentials: Credentials = serde_json::from_str(&saved).unwrap();
        assert!(
            credentials
                .expires_at
                .is_none_or(|expiry| expiry > now_ms() + 60_000),
            "saved credential expired; do not refresh or alter the user's login for this test"
        );
        // All gateway state, keys and request history are isolated in memory.
        // A stale UI access error does not prevent this isolated read/inference
        // diagnostic after the system credential has actually been retrieved.
        account.status = "ready".into();
        account.last_error = None;
        let manager = CreditManager::test();
        manager.store.save_account(&account).unwrap();
        manager.secrets.set(&account.id, &saved).unwrap();
        let status = manager.gateway.start(manager.clone(), 0).await.unwrap();
        let http = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(60))
            .build()
            .unwrap();
        let response = http.post(format!("{}/messages", status.base_url))
            .header("x-api-key", manager.gateway_key().unwrap())
            .header("anthropic-version", "2023-06-01")
            .json(&serde_json::json!({"model":"workbuddy/glm-5.3-flash","max_tokens":128,"stream":false,
                "messages":[{"role":"user","content":"Reply exactly OK."}]})).send().await.unwrap();
        let status = response.status();
        let result: serde_json::Value = response.json().await.unwrap();
        manager.gateway.stop().await.unwrap();
        assert!(
            status.is_success(),
            "gateway response: {}",
            result.get("error").unwrap_or(&serde_json::Value::Null)
        );
        assert_eq!(result["type"], "message");
        assert_eq!(result["model"], "workbuddy/glm-5.3-flash");
        assert!(result["content"].as_array().is_some_and(|blocks| blocks
            .iter()
            .any(|block| block["text"].as_str().is_some_and(|text| !text.is_empty()))));
        println!("WorkBuddy Messages inference verified: HTTP {}; model=workbuddy/glm-5.3-flash; stop_reason={}", status.as_u16(), result["stop_reason"]);
    }

    #[test]
    fn scheduler_uses_beijing_ten_thirty() {
        assert_eq!(claim_schedule(3_600_000), (0, false));
        assert_eq!(claim_schedule(8_999_999), (0, false));
        assert_eq!(claim_schedule(9_000_000), (0, true));
        assert_eq!(claim_schedule(57_599_999), (0, true));
        assert_eq!(claim_schedule(57_600_000), (1, false));
        assert_eq!(claim_schedule(95_399_999), (1, false));
        assert_eq!(claim_schedule(95_400_000), (1, true));
    }
    #[test]
    fn gateway_keys_persist_and_do_not_enter_dashboard() {
        let manager = CreditManager::test();
        manager.retry_gateway_key_access().unwrap();
        let key = manager.gateway_key().unwrap();
        assert_eq!(manager.gateway_key().unwrap(), key);
        assert!(!serde_json::to_string(&manager.dashboard().unwrap())
            .unwrap()
            .contains(&key));
        assert!(manager.accounts().unwrap().is_empty());
    }
    #[tokio::test]
    async fn database_delete_failure_restores_credentials() {
        let manager = CreditManager::test();
        let account = Account {
            id: "restore-test".into(),
            platform: Platform::Qoder,
            user_id: "test".into(),
            label: "test".into(),
            enabled: true,
            status: "ready".into(),
            last_error: None,
            snapshot: None,
        };
        manager.store.save_account(&account).unwrap();
        manager.secrets.set(&account.id, "private-token").unwrap();
        manager.store.conn.lock().execute_batch("CREATE TRIGGER reject_delete BEFORE DELETE ON credit_accounts BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
        assert!(manager.delete_account(&account.id).await.is_err());
        assert_eq!(
            manager.secrets.get(&account.id).unwrap().as_deref(),
            Some("private-token")
        );
        assert!(manager.store.account(&account.id).is_ok());
    }
}
