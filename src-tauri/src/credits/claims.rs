use super::{claim_schedule, providers::ProviderError, types::*, CreditManager};
use futures::StreamExt;
use std::{collections::HashMap, sync::Arc, time::Duration};

#[derive(Default)]
pub(super) struct ClaimSchedule {
    accounts: HashMap<String, ClaimState>,
}

struct ClaimState {
    day: i64,
    failures: usize,
    retry_at: Option<i64>,
    paused: bool,
}

impl ClaimSchedule {
    pub fn due(&self, id: &str, now: i64) -> bool {
        let (day, due) = claim_schedule(now);
        due && self.accounts.get(id).is_none_or(|state| {
            state.day != day || state.retry_at.is_some_and(|retry| retry <= now)
        })
    }

    pub fn finish(
        &mut self,
        id: &str,
        started_at: i64,
        now: i64,
        result: &Result<Vec<RewardOutcome>, ProviderError>,
    ) {
        let day = claim_schedule(started_at).0;
        let state = self.accounts.entry(id.into()).or_insert(ClaimState {
            day,
            failures: 0,
            retry_at: None,
            paused: false,
        });
        if state.day != day {
            *state = ClaimState {
                day,
                failures: 0,
                retry_at: None,
                paused: false,
            };
        }
        let retry = match result {
            Err(error) => error.claim_retry_after_ms(),
            Ok(outcomes) if outcomes.iter().any(|outcome| outcome.status == "pending") => None,
            Ok(outcomes) => outcomes
                .iter()
                .filter_map(|outcome| outcome.retry_after_ms)
                .max(),
        };
        state.paused = retry.is_none()
            && match result {
                Err(_) => true,
                Ok(outcomes) => {
                    outcomes.iter().any(|outcome| outcome.status == "failed")
                        && !outcomes.iter().any(|outcome| outcome.status == "pending")
                }
            };
        state.retry_at = retry.map(|minimum| {
            let delays = [60_000, 120_000, 300_000, 900_000, 1_800_000];
            let delay = delays[state.failures.min(delays.len() - 1)].max(minimum);
            state.failures = state.failures.saturating_add(1);
            now.saturating_add(delay)
        });
    }

    pub fn resume(&mut self, now: i64) {
        for state in self.accounts.values_mut().filter(|state| state.paused) {
            state.paused = false;
            state.retry_at = Some(now);
        }
    }
}

impl CreditManager {
    pub(super) fn start_claim_scheduler(self: &Arc<Self>) {
        let manager = self.clone();
        tokio::spawn(async move {
            let mut timer = tokio::time::interval(Duration::from_secs(30));
            timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    _ = manager.cancellation.cancelled() => break,
                    _ = timer.tick() => {},
                }
                if !manager
                    .store
                    .settings()
                    .is_ok_and(|settings| settings.auto_claim)
                {
                    continue;
                }
                let Ok(accounts) = manager.accounts() else {
                    continue;
                };
                let ids = {
                    let mut schedule = manager.claim_schedule.lock();
                    schedule
                        .accounts
                        .retain(|id, _| accounts.iter().any(|account| &account.id == id));
                    accounts
                        .iter()
                        .filter(|account| account.enabled && schedule.due(&account.id, now_ms()))
                        .map(|account| account.id.clone())
                        .collect::<Vec<_>>()
                };
                let work = futures::stream::iter(ids)
                    .map(|id| {
                        let worker = manager.clone();
                        async move {
                            let _ = worker.claim_account(&id).await;
                        }
                    })
                    .buffer_unordered(3)
                    .collect::<Vec<_>>();
                tokio::select! {
                    _ = manager.cancellation.cancelled() => break,
                    _ = work => {},
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const DUE: i64 = 9_000_000;

    #[test]
    fn transient_failures_retry_each_account_with_backoff_and_reset_next_day() {
        let mut schedule = ClaimSchedule::default();
        assert!(!schedule.due("a", DUE - 1));
        assert!(schedule.due("a", DUE));
        let error = Err(ProviderError::new(502, "temporary connection failure"));
        schedule.finish("a", DUE, DUE, &error);
        assert!(!schedule.due("a", DUE + 59_999));
        assert!(schedule.due("a", DUE + 60_000));
        assert!(schedule.due("b", DUE));
        schedule.finish("a", DUE + 60_000, DUE + 60_000, &error);
        assert!(!schedule.due("a", DUE + 179_999));
        assert!(schedule.due("a", DUE + 180_000));
        schedule.finish("a", DUE + 180_000, DUE + 180_000, &Ok(vec![]));
        assert!(!schedule.due("a", DUE + 3_600_000));
        assert!(schedule.due("a", DUE + 86_400_000));
    }

    #[test]
    fn credential_failures_wait_for_explicit_resume_and_completed_accounts_stay_done() {
        let mut schedule = ClaimSchedule::default();
        let mut error = ProviderError::new(500, "keychain unavailable");
        error.pause_auto = true;
        schedule.finish("auth", DUE, DUE, &Err(error));
        schedule.finish("done", DUE, DUE, &Ok(vec![]));
        assert!(!schedule.due("auth", DUE + 3_600_000));
        schedule.resume(DUE + 3_600_000);
        assert!(schedule.due("auth", DUE + 3_600_000));
        assert!(!schedule.due("done", DUE + 3_600_000));
    }

    #[test]
    fn upstream_retry_after_is_respected() {
        let mut schedule = ClaimSchedule::default();
        let mut error = super::super::providers::http_error(429);
        error.retry_after_ms = Some(600_000);
        schedule.finish("limited", DUE, DUE, &Err(error));
        assert!(!schedule.due("limited", DUE + 599_999));
        assert!(schedule.due("limited", DUE + 600_000));
    }

    #[test]
    fn pending_claims_are_not_resubmitted_even_with_a_rate_limited_sibling_reward() {
        let mut schedule = ClaimSchedule::default();
        let pending: RewardOutcome = serde_json::from_value(serde_json::json!({
            "accountId":"a", "rewardId":"daily", "name":"daily", "status":"pending",
            "amount":null, "kind":"credits", "message":"uncertain", "createdAt":DUE
        }))
        .unwrap();
        let mut limited = pending.clone();
        limited.status = "failed".into();
        limited.retry_after_ms = Some(60_000);
        schedule.finish("a", DUE, DUE, &Ok(vec![pending, limited]));
        assert!(!schedule.due("a", DUE + 3_600_000));
        schedule.resume(DUE + 3_600_000);
        assert!(!schedule.due("a", DUE + 3_600_000));
    }

    #[test]
    fn an_attempt_crossing_beijing_midnight_does_not_skip_the_next_day() {
        let mut schedule = ClaimSchedule::default();
        schedule.finish("a", 57_599_999, 57_600_001, &Ok(vec![]));
        assert!(schedule.due("a", DUE + 86_400_000));
    }
}
