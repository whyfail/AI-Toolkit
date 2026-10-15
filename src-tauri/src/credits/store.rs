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
        store
            .conn
            .lock()
            .execute_batch(
                "
            CREATE TABLE IF NOT EXISTS credit_accounts (
                id TEXT PRIMARY KEY, platform TEXT NOT NULL, user_id TEXT NOT NULL,
                metadata TEXT NOT NULL, UNIQUE(platform, user_id)
            );
            CREATE TABLE IF NOT EXISTS credit_snapshots (
                account_id TEXT PRIMARY KEY, snapshot TEXT NOT NULL, updated_at INTEGER NOT NULL
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
        let mut query = conn.prepare("SELECT a.metadata,s.snapshot FROM credit_accounts a LEFT JOIN credit_snapshots s ON s.account_id=a.id ORDER BY a.platform,a.user_id").map_err(db_error)?;
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
        tx.execute("INSERT INTO credit_accounts(id,platform,user_id,metadata) VALUES(?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET metadata=excluded.metadata", params![account.id, account.platform.key(), account.user_id, metadata]).map_err(db_error)?;
        if let Some(snapshot) = &account.snapshot {
            tx.execute("INSERT INTO credit_snapshots(account_id,snapshot,updated_at) VALUES(?1,?2,?3) ON CONFLICT(account_id) DO UPDATE SET snapshot=excluded.snapshot,updated_at=excluded.updated_at", params![account.id, serde_json::to_string(snapshot).map_err(json_error)?, snapshot.fetched_at]).map_err(db_error)?;
        }
        tx.commit().map_err(db_error)
    }

    pub fn delete_account(&self, id: &str) -> Result<(), String> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction().map_err(db_error)?;
        for table in ["credit_snapshots", "credit_rewards", "credit_requests"] {
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
        self.history("credit_rewards", "outcome")
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
            user_id: "u1".into(),
            label: "测试".into(),
            enabled: true,
            status: "ready".into(),
            last_error: None,
            snapshot: None,
        }
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
    fn settings_reject_zero_port() {
        assert!(store()
            .save_settings(&ProxySettings {
                port: 0,
                auto_claim: true
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
        });
        assert!(s.save_account(&changed).is_err());
        assert_eq!(s.account("one").unwrap().label, "测试");
        assert!(s.account("one").unwrap().snapshot.is_none());
    }
}
