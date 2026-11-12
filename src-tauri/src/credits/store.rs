use super::types::*;
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use std::sync::Arc;

#[derive(Clone)]
pub struct CreditStore {
    pub conn: Arc<Mutex<Connection>>,
}

impl CreditStore {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Result<Self, String> {
        let store = Self { conn };
        migrate_account_regions(&mut store.conn.lock())?;
        store
            .conn
            .lock()
            .execute_batch(
                "
            CREATE TABLE IF NOT EXISTS credit_accounts (
                id TEXT PRIMARY KEY, platform TEXT NOT NULL, region TEXT NOT NULL DEFAULT 'cn',
                user_id TEXT NOT NULL, metadata TEXT NOT NULL, UNIQUE(platform, region, user_id)
            );
            CREATE TABLE IF NOT EXISTS credit_snapshots (
                account_id TEXT PRIMARY KEY, snapshot TEXT NOT NULL, updated_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS credit_balance_history (
                account_id TEXT NOT NULL, fetched_at INTEGER NOT NULL,
                balance REAL NOT NULL, unit TEXT NOT NULL,
                PRIMARY KEY(account_id, fetched_at)
            );
            CREATE TABLE IF NOT EXISTS credit_rewards (
                id INTEGER PRIMARY KEY AUTOINCREMENT, account_id TEXT NOT NULL,
                outcome TEXT NOT NULL, created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS credit_proxy_settings (
                id INTEGER PRIMARY KEY CHECK(id=1), settings TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS credit_requests (
                id TEXT PRIMARY KEY, account_id TEXT NOT NULL, summary TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
        ",
            )
            .map_err(db_error)?;
        Ok(store)
    }

    pub fn accounts(&self) -> Result<Vec<Account>, String> {
        let conn = self.conn.lock();
        let mut query = conn.prepare("SELECT a.metadata,s.snapshot FROM credit_accounts a LEFT JOIN credit_snapshots s ON s.account_id=a.id ORDER BY a.platform,a.region,a.user_id").map_err(db_error)?;
        let rows = query
            .query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
            })
            .map_err(db_error)?;
        let mut accounts = Vec::new();
        for row in rows {
            let (meta, snapshot) = row.map_err(db_error)?;
            let mut account: Account = serde_json::from_str(&meta).map_err(json_error)?;
            account.snapshot = snapshot
                .map(|s| serde_json::from_str(&s))
                .transpose()
                .map_err(json_error)?;
            accounts.push(account);
        }
        Ok(accounts)
    }

    pub fn account(&self, id: &str) -> Result<Account, String> {
        self.accounts()?
            .into_iter()
            .find(|a| a.id == id)
            .ok_or_else(|| "账号不存在".into())
    }

    pub fn save_account(&self, account: &Account) -> Result<(), String> {
        let mut meta = account.clone();
        meta.snapshot = None;
        let metadata = serde_json::to_string(&meta).map_err(json_error)?;
        let mut conn = self.conn.lock();
        let tx = conn.transaction().map_err(db_error)?;
        tx.execute("INSERT INTO credit_accounts(id,platform,region,user_id,metadata) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET metadata=excluded.metadata WHERE credit_accounts.platform=excluded.platform AND credit_accounts.region=excluded.region AND credit_accounts.user_id=excluded.user_id", params![account.id, account.platform.key(), account.region.key(), account.user_id, metadata]).map_err(db_error)?;
        if tx.changes() != 1 { return Err("不能更改已保存账号的身份或区域".into()); }
        if let Some(snapshot) = &account.snapshot {
            tx.execute("INSERT INTO credit_snapshots(account_id,snapshot,updated_at) VALUES(?1,?2,?3) ON CONFLICT(account_id) DO UPDATE SET snapshot=excluded.snapshot,updated_at=excluded.updated_at", params![account.id, serde_json::to_string(snapshot).map_err(json_error)?, snapshot.fetched_at]).map_err(db_error)?;
            if let Some(observation) = BalanceObservation::from_snapshot(snapshot) {
                tx.execute("INSERT OR IGNORE INTO credit_balance_history(account_id,fetched_at,balance,unit) VALUES(?1,?2,?3,?4)",
                    params![account.id, observation.fetched_at, observation.balance, observation.unit]).map_err(db_error)?;
            }
        }
        tx.commit().map_err(db_error)
    }

    pub fn delete_account(&self, id: &str) -> Result<(), String> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction().map_err(db_error)?;
        for table in ["credit_snapshots", "credit_balance_history", "credit_rewards", "credit_requests"] {
            tx.execute(&format!("DELETE FROM {table} WHERE account_id=?1"), [id])
                .map_err(db_error)?;
        }
        tx.execute("DELETE FROM credit_accounts WHERE id=?1", [id])
            .map_err(db_error)?;
        tx.commit().map_err(db_error)
    }

    pub fn settings(&self) -> Result<ProxySettings, String> {
        let raw: Option<String> = self
            .conn
            .lock()
            .query_row(
                "SELECT settings FROM credit_proxy_settings WHERE id=1",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(db_error)?;
        raw.map(|s| serde_json::from_str(&s).map_err(json_error))
            .unwrap_or_else(|| Ok(ProxySettings::default()))
    }

    pub fn save_settings(&self, settings: &ProxySettings) -> Result<(), String> {
        if settings.port == 0 {
            return Err("端口必须在 1 到 65535 之间".into());
        }
        self.conn.lock().execute("INSERT INTO credit_proxy_settings(id,settings) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET settings=excluded.settings", [serde_json::to_string(settings).map_err(json_error)?]).map_err(db_error)?;
        Ok(())
    }

    pub fn record_reward(&self, outcome: &RewardOutcome) -> Result<(), String> {
        self.conn
            .lock()
            .execute(
                "INSERT INTO credit_rewards(account_id,outcome,created_at) VALUES(?1,?2,?3)",
                params![
                    outcome.account_id,
                    serde_json::to_string(outcome).map_err(json_error)?,
                    outcome.created_at
                ],
            )
            .map_err(db_error)?;
        Ok(())
    }

    pub fn record_request(&self, summary: &RequestSummary) -> Result<(), String> {
        self.conn
            .lock()
            .execute(
                "INSERT INTO credit_requests(id,account_id,summary,created_at) VALUES(?1,?2,?3,?4)",
                params![
                    summary.id,
                    summary.account_id,
                    serde_json::to_string(summary).map_err(json_error)?,
                    summary.created_at
                ],
            )
            .map_err(db_error)?;
        Ok(())
    }

    pub fn rewards(&self) -> Result<Vec<RewardOutcome>, String> {
        let mut outcomes: Vec<RewardOutcome> = self.history("credit_rewards", "outcome")?;
        for outcome in &mut outcomes {
            self.fill_balance_check(outcome)?;
        }
        Ok(outcomes)
    }

    pub fn fill_balance_check(&self, outcome: &mut RewardOutcome) -> Result<(), String> {
        let Some(check) = &mut outcome.balance_check else { return Ok(()) };
        // Keep the first post-claim observation stable as later usage changes the balance.
        check.after = self.conn.lock().query_row(
            "SELECT balance,unit,fetched_at FROM credit_balance_history WHERE account_id=?1 AND fetched_at>?2 ORDER BY fetched_at LIMIT 1",
            params![outcome.account_id, outcome.created_at],
            |row| Ok(BalanceObservation { balance: row.get(0)?, unit: row.get(1)?, fetched_at: row.get(2)? }),
        ).optional().map_err(db_error)?;
        Ok(())
    }
    pub fn requests(&self) -> Result<Vec<RequestSummary>, String> {
        self.history("credit_requests", "summary")
    }

    fn history<T: serde::de::DeserializeOwned>(
        &self,
        table: &str,
        column: &str,
    ) -> Result<Vec<T>, String> {
        let conn = self.conn.lock();
        let mut query = conn
            .prepare(&format!(
                "SELECT {column} FROM {table} ORDER BY created_at DESC LIMIT 100"
            ))
            .map_err(db_error)?;
        let rows = query
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(db_error)?;
        rows.map(|row| serde_json::from_str(&row.map_err(db_error)?).map_err(json_error))
            .collect()
    }
}

fn migrate_account_regions(conn: &mut Connection) -> Result<(), String> {
    let has_accounts: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='credit_accounts')",
        [], |row| row.get(0),
    ).map_err(db_error)?;
    if !has_accounts { return Ok(()); }
    let has_region = conn.prepare("PRAGMA table_info(credit_accounts)").map_err(db_error)?
        .query_map([], |row| row.get::<_, String>(1)).map_err(db_error)?
        .collect::<Result<Vec<_>, _>>().map_err(db_error)?.iter().any(|name| name == "region");
    if has_region { return Ok(()); }

    // VACUUM INTO includes WAL contents; copying the database file would not.
    if let Some(db_path) = conn.path().filter(|path| !path.is_empty() && *path != ":memory:") {
        let directory = std::path::Path::new(db_path).parent()
            .ok_or_else(|| "积分数据库路径无效".to_string())?.join("backups");
        let backup = directory.join(format!("credit-regions-v1-{}.db", uuid::Uuid::new_v4()));
        crate::core::private_fs::prepare_private_file(&backup)
            .map_err(|_| "无法创建积分区域迁移备份".to_string())?;
        if let Err(error) = conn.execute("VACUUM INTO ?1", [backup.to_string_lossy().as_ref()]) {
            let _ = std::fs::remove_file(&backup);
            return Err(db_error(error));
        }
        crate::core::private_fs::secure_file(&backup)
            .map_err(|_| "无法保护积分区域迁移备份".to_string())?;
    }
    let tx = conn.transaction().map_err(db_error)?;
    tx.execute_batch("CREATE TABLE credit_accounts_regions_v1 (
        id TEXT PRIMARY KEY, platform TEXT NOT NULL, region TEXT NOT NULL DEFAULT 'cn',
        user_id TEXT NOT NULL, metadata TEXT NOT NULL, UNIQUE(platform,region,user_id)
    );
    INSERT INTO credit_accounts_regions_v1(id,platform,region,user_id,metadata)
        SELECT id,platform,'cn',user_id,metadata FROM credit_accounts;
    DROP TABLE credit_accounts;
    ALTER TABLE credit_accounts_regions_v1 RENAME TO credit_accounts;").map_err(db_error)?;
    tx.commit().map_err(db_error)
}

fn db_error(_: rusqlite::Error) -> String {
    "积分数据库操作失败".into()
}
fn json_error(_: serde_json::Error) -> String {
    "积分数据格式错误".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn store() -> CreditStore {
        CreditStore::new(Arc::new(Mutex::new(Connection::open_in_memory().unwrap()))).unwrap()
    }
    fn account() -> Account {
        Account {
            id: "one".into(),
            platform: Platform::Workbuddy,
            region: CreditRegion::Cn,
            user_id: "u1".into(),
            label: "测试".into(),
            enabled: true,
            status: "ready".into(),
            last_error: None,
            snapshot: None,
        }
    }

    fn balance_account(balance: f64, fetched_at: i64) -> Account {
        let mut value = account();
        let mut snapshot = crate::credits::providers::snapshot(vec![], vec![], vec![], None);
        snapshot.balance = Some(balance);
        snapshot.fetched_at = fetched_at;
        value.snapshot = Some(snapshot);
        value
    }

    #[test]
    fn balance_history_preserves_observations_and_first_claim_readback_after_reopening() {
        let s = store();
        let before = balance_account(100.0, 1000);
        s.save_account(&before).unwrap();
        s.save_account(&before).unwrap();
        let mut outcome: RewardOutcome = serde_json::from_value(serde_json::json!({
            "accountId":"one", "rewardId":"daily", "name":"签到", "status":"claimed",
            "amount":100, "kind":"credits", "message":"confirmed", "createdAt":1500
        })).unwrap();
        assert!(outcome.balance_check.is_none());
        outcome.balance_check = Some(ClaimBalanceCheck {
            before: BalanceObservation::from_snapshot(before.snapshot.as_ref().unwrap()), after: None,
        });
        s.record_reward(&outcome).unwrap();
        assert!(s.rewards().unwrap()[0].balance_check.as_ref().unwrap().after.is_none());
        s.save_account(&balance_account(200.0, 2000)).unwrap();
        s.save_account(&balance_account(190.0, 3000)).unwrap();
        let reopened = CreditStore::new(s.conn.clone()).unwrap();
        let rewards = reopened.rewards().unwrap();
        let check = rewards[0].balance_check.as_ref().unwrap();
        assert_eq!(check.before.as_ref().unwrap().balance, 100.0);
        assert_eq!(check.after.as_ref().unwrap().balance, 200.0);
        assert_eq!(check.after.as_ref().unwrap().fetched_at, 2000);
        assert_eq!(reopened.conn.lock().query_row("SELECT count(*) FROM credit_balance_history", [], |row| row.get::<_, i64>(0)).unwrap(), 3);
        reopened.delete_account("one").unwrap();
        assert_eq!(reopened.conn.lock().query_row("SELECT count(*) FROM credit_balance_history", [], |row| row.get::<_, i64>(0)).unwrap(), 0);
    }

    #[test]
    fn stale_and_unknown_balances_are_excluded_and_history_writes_are_atomic() {
        let s = store();
        s.save_account(&balance_account(0.0, 1000)).unwrap();
        let mut stale = balance_account(100.0, 2000);
        stale.snapshot.as_mut().unwrap().stale = true;
        s.save_account(&stale).unwrap();
        stale.snapshot.as_mut().unwrap().stale = false;
        stale.snapshot.as_mut().unwrap().balance = None;
        s.save_account(&stale).unwrap();
        assert_eq!(s.conn.lock().query_row("SELECT count(*) FROM credit_balance_history", [], |row| row.get::<_, i64>(0)).unwrap(), 1);
        s.conn.lock().execute_batch("CREATE TRIGGER reject_history BEFORE INSERT ON credit_balance_history BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
        let mut changed = balance_account(50.0, 3000);
        changed.label = "partial".into();
        assert!(s.save_account(&changed).is_err());
        assert_eq!(s.account("one").unwrap().label, "测试");
        assert_eq!(s.account("one").unwrap().snapshot.unwrap().fetched_at, 2000);
    }
    #[test]
    fn migration_is_repeatable_and_does_not_touch_existing_tables() {
        let s = store();
        s.conn
            .lock()
            .execute_batch(
                "CREATE TABLE mcp_servers(id TEXT); INSERT INTO mcp_servers VALUES('existing');",
            )
            .unwrap();
        CreditStore::new(s.conn.clone()).unwrap();
        assert_eq!(
            s.conn
                .lock()
                .query_row("SELECT id FROM mcp_servers", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "existing"
        );
        assert_eq!(s.settings().unwrap().port, 17860);
        assert!(!s.settings().unwrap().auto_claim);
    }
    #[test]
    fn account_upsert_deduplication_and_delete() {
        let s = store();
        s.save_account(&account()).unwrap();
        let mut changed = account();
        changed.label = "changed".into();
        s.save_account(&changed).unwrap();
        assert_eq!(s.accounts().unwrap().len(), 1);
        assert_eq!(s.account("one").unwrap().label, "changed");
        let mut duplicate = account();
        duplicate.id = "two".into();
        assert!(s.save_account(&duplicate).is_err());
        s.delete_account("one").unwrap();
        assert!(s.accounts().unwrap().is_empty());
    }
    #[test]
    fn legacy_accounts_keep_ids_and_regions_are_independent() {
        let conn = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
        conn.lock().execute_batch("CREATE TABLE credit_accounts(id TEXT PRIMARY KEY,platform TEXT NOT NULL,user_id TEXT NOT NULL,metadata TEXT NOT NULL,UNIQUE(platform,user_id));").unwrap();
        let mut legacy = serde_json::to_value(account()).unwrap();
        legacy.as_object_mut().unwrap().remove("region");
        conn.lock().execute("INSERT INTO credit_accounts VALUES(?1,?2,?3,?4)",
            params!["one", "workbuddy", "u1", legacy.to_string()]).unwrap();
        conn.lock().execute_batch("CREATE TABLE credit_snapshots(account_id TEXT PRIMARY KEY,snapshot TEXT NOT NULL,updated_at INTEGER NOT NULL); CREATE TABLE credit_requests(id TEXT PRIMARY KEY,account_id TEXT NOT NULL,summary TEXT NOT NULL,created_at INTEGER NOT NULL);").unwrap();
        let old_snapshot = serde_json::json!({"balance":123.5,"unit":"credits","quota":null,"packs":[{"id":"original-pack","name":"Original","remaining":123.5,"expiresAt":null}],"rewards":[],"models":[],"fetchedAt":42,"stale":false,"diagnostic":null});
        let old_request = serde_json::json!({"id":"original-request","accountId":"one","platform":"workbuddy","model":"workbuddy/fixture","elapsedMs":10,"inputTokens":3,"outputTokens":4,"status":"success","createdAt":42});
        conn.lock().execute("INSERT INTO credit_snapshots VALUES('one',?1,42)", [old_snapshot.to_string()]).unwrap();
        conn.lock().execute("INSERT INTO credit_requests VALUES('original-request','one',?1,42)", [old_request.to_string()]).unwrap();
        let s = CreditStore::new(conn.clone()).unwrap();
        assert_eq!(s.account("one").unwrap().region, CreditRegion::Cn);
        assert_eq!(s.account("one").unwrap().snapshot.unwrap().balance, Some(123.5));
        assert_eq!(s.account("one").unwrap().snapshot.unwrap().packs[0].unit, "credits");
        assert_eq!(s.requests().unwrap()[0].id, "original-request");
        assert_eq!(s.requests().unwrap()[0].region, CreditRegion::Cn);
        let mut intl = account();
        intl.id = "international".into(); intl.region = CreditRegion::Intl;
        s.save_account(&intl).unwrap();
        assert_eq!(s.accounts().unwrap().len(), 2);
        intl.id = "duplicate-international".into();
        assert!(s.save_account(&intl).is_err());
        let repeated = CreditStore::new(conn).unwrap();
        assert_eq!(repeated.accounts().unwrap().len(), 2);
        let mut changed_identity = account(); changed_identity.region = CreditRegion::Intl;
        assert!(repeated.save_account(&changed_identity).is_err());
        assert_eq!(repeated.account("one").unwrap().region, CreditRegion::Cn);
    }

    #[test]
    fn file_migration_keeps_a_private_consistent_pre_migration_backup() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("credits.db");
        let conn = Arc::new(Mutex::new(Connection::open(&path).unwrap()));
        conn.lock().execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE credit_accounts(id TEXT PRIMARY KEY,platform TEXT NOT NULL,user_id TEXT NOT NULL,metadata TEXT NOT NULL,UNIQUE(platform,user_id)); CREATE TABLE unrelated(value TEXT); INSERT INTO unrelated VALUES('keep');").unwrap();
        conn.lock().execute("INSERT INTO credit_accounts VALUES(?1,?2,?3,?4)",
            params!["one", "workbuddy", "u1", serde_json::to_string(&account()).unwrap()]).unwrap();
        let s = CreditStore::new(conn.clone()).unwrap();
        CreditStore::new(conn).unwrap();
        let backups: Vec<_> = std::fs::read_dir(directory.path().join("backups")).unwrap().map(Result::unwrap).collect();
        assert_eq!(backups.len(), 1);
        let backup = Connection::open(backups[0].path()).unwrap();
        assert_eq!(backup.query_row("SELECT value FROM unrelated", [], |r| r.get::<_, String>(0)).unwrap(), "keep");
        assert_eq!(backup.query_row("SELECT id FROM credit_accounts", [], |r| r.get::<_, String>(0)).unwrap(), "one");
        assert!(backup.prepare("SELECT region FROM credit_accounts").is_err());
        assert_eq!(s.account("one").unwrap().id, "one");
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(backups[0].metadata().unwrap().permissions().mode() & 0o777, 0o600);
        }
    }
    #[test]
    fn settings_reject_zero_port() {
        assert!(store()
            .save_settings(&ProxySettings {
                port: 0,
                auto_claim: true,
                auto_refresh: true,
            })
            .is_err());
    }
    #[test]
    fn failed_snapshot_write_rolls_back_metadata() {
        let s = store();
        s.save_account(&account()).unwrap();
        s.conn.lock().execute_batch("CREATE TRIGGER reject_snapshot BEFORE INSERT ON credit_snapshots BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
        let mut changed = account();
        changed.label = "partial".into();
        changed.snapshot = Some(ProviderSnapshot {
            balance: Some(12.0),
            unit: "credits".into(),
            quota: None,
            packs: vec![],
            rewards: vec![],
            models: vec![],
            fetched_at: now_ms(),
            stale: false,
            diagnostic: None,
            sync: SnapshotSync::default(),
        });
        assert!(s.save_account(&changed).is_err());
        assert_eq!(s.account("one").unwrap().label, "测试");
        assert!(s.account("one").unwrap().snapshot.is_none());
    }
}
