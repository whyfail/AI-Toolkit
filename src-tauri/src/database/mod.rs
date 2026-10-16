mod dao;

use parking_lot::Mutex;
use rusqlite::Connection;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

pub use dao::mcp::*;
pub use dao::skill::*;

use crate::error::AppError;

const TRAE_UNIFICATION_VERSION: &str = "trae-unification-v1";

/// 数据库连接包装器（线程安全）
pub struct Database {
    pub conn: Arc<Mutex<Connection>>,
}

impl Database {
    #[cfg(test)]
    pub(crate) fn in_memory() -> Result<Self, AppError> {
        let conn = Connection::open_in_memory()
            .map_err(|e| AppError::Database(format!("Failed to open in-memory database: {}", e)))?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init_schema()?;
        Ok(db)
    }

    pub fn new() -> Result<Self, AppError> {
        let db_path = Self::get_db_path()?;
        Self::open_private(&db_path)
    }

    fn open_private(db_path: &Path) -> Result<Self, AppError> {
        crate::core::private_fs::prepare_private_file(db_path)
            .map_err(|e| AppError::Database(format!("保护数据库文件失败: {e:#}")))?;
        Self::secure_sidecars(db_path)?;
        let conn = Connection::open(&db_path)
            .map_err(|e| AppError::Database(format!("Failed to open database: {}", e)))?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        // 初始化表
        db.init_schema()?;
        Self::secure_sidecars(db_path)?;
        Ok(db)
    }

    fn secure_sidecars(db_path: &Path) -> Result<(), AppError> {
        for suffix in ["-journal", "-wal", "-shm"] {
            let mut name = db_path.as_os_str().to_os_string();
            name.push(suffix);
            let path = PathBuf::from(name);
            match std::fs::symlink_metadata(&path) {
                Ok(_) => crate::core::private_fs::secure_file(&path)
                    .map_err(|e| AppError::Database(format!("保护 SQLite 辅助文件失败: {e:#}")))?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(AppError::Database(error.to_string())),
            }
        }
        Ok(())
    }

    fn get_db_path() -> Result<PathBuf, AppError> {
        let config_dir = dirs::home_dir()
            .ok_or_else(|| AppError::Database("Could not find home directory".to_string()))?
            .join(".ai-toolkit");
        Ok(config_dir.join("ai-toolkit.db"))
    }

    fn init_schema(&self) -> Result<(), AppError> {
        let mut conn = self.conn.lock();
        let migration_backup = backup_before_trae_unification(&conn)?;

        // 创建表
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS mcp_servers (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                server_config TEXT NOT NULL,
                description TEXT,
                homepage TEXT,
                docs TEXT,
                tags TEXT DEFAULT '[]',
                enabled_qwen_code BOOLEAN DEFAULT FALSE,
                enabled_claude BOOLEAN DEFAULT FALSE,
                enabled_codex BOOLEAN DEFAULT FALSE,
                enabled_gemini BOOLEAN DEFAULT FALSE,
                enabled_opencode BOOLEAN DEFAULT FALSE,
                enabled_trae BOOLEAN DEFAULT FALSE,
                enabled_trae_cn BOOLEAN DEFAULT FALSE,
                enabled_trae_work BOOLEAN DEFAULT FALSE,
                enabled_trae_solo_cn BOOLEAN DEFAULT FALSE,
                enabled_qoder BOOLEAN DEFAULT FALSE,
                enabled_qodercli BOOLEAN DEFAULT FALSE,
                enabled_codebuddy BOOLEAN DEFAULT FALSE,
                enabled_hermes BOOLEAN DEFAULT FALSE,
                enabled_mimo_code BOOLEAN DEFAULT FALSE,
                enabled_workbuddy BOOLEAN DEFAULT FALSE,
                enabled_zcode BOOLEAN DEFAULT FALSE,
                enabled_cursor BOOLEAN DEFAULT FALSE,
                enabled_windsurf BOOLEAN DEFAULT FALSE,
                enabled_cline BOOLEAN DEFAULT FALSE,
                enabled_roo BOOLEAN DEFAULT FALSE,
                enabled_kilo BOOLEAN DEFAULT FALSE,
                enabled_continue BOOLEAN DEFAULT FALSE,
                enabled_goose BOOLEAN DEFAULT FALSE,
                enabled_kiro BOOLEAN DEFAULT FALSE,
                enabled_kimi_code BOOLEAN DEFAULT FALSE,
                enabled_minimax_code BOOLEAN DEFAULT FALSE,
                created_at INTEGER DEFAULT (strftime('%s', 'now') * 1000),
                updated_at INTEGER DEFAULT (strftime('%s', 'now') * 1000)
            );

            CREATE TABLE IF NOT EXISTS app_configs (
                id TEXT PRIMARY KEY,
                app_name TEXT NOT NULL,
                config_path TEXT NOT NULL,
                mcp_config TEXT,
                last_import_at INTEGER,
                UNIQUE(app_name)
            );

            CREATE TABLE IF NOT EXISTS managed_skills (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                source_type TEXT NOT NULL,
                source_ref TEXT,
                source_subpath TEXT,
                central_path TEXT NOT NULL,
                created_at INTEGER DEFAULT (strftime('%s', 'now') * 1000),
                updated_at INTEGER DEFAULT (strftime('%s', 'now') * 1000),
                last_sync_at INTEGER
            );

            CREATE TABLE IF NOT EXISTS skill_sync_targets (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                skill_id TEXT NOT NULL,
                tool_id TEXT NOT NULL,
                mode TEXT NOT NULL,
                status TEXT NOT NULL,
                target_path TEXT NOT NULL,
                synced_at INTEGER,
                FOREIGN KEY(skill_id) REFERENCES managed_skills(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at INTEGER DEFAULT (strftime('%s', 'now') * 1000)
            );
            ",
        )
        .map_err(|e| AppError::Database(format!("Failed to initialize schema: {}", e)))?;

        // 尝试添加新列（忽略已存在的错误）
        let _ = conn.execute(
            "ALTER TABLE mcp_servers ADD COLUMN enabled_trae BOOLEAN DEFAULT FALSE",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE mcp_servers ADD COLUMN enabled_trae_cn BOOLEAN DEFAULT FALSE",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE mcp_servers ADD COLUMN enabled_trae_work BOOLEAN DEFAULT FALSE",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE mcp_servers ADD COLUMN enabled_trae_solo_cn BOOLEAN DEFAULT FALSE",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE mcp_servers ADD COLUMN enabled_qoder BOOLEAN DEFAULT FALSE",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE mcp_servers ADD COLUMN enabled_qodercli BOOLEAN DEFAULT FALSE",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE mcp_servers ADD COLUMN enabled_codebuddy BOOLEAN DEFAULT FALSE",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE mcp_servers ADD COLUMN enabled_hermes BOOLEAN DEFAULT FALSE",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE mcp_servers ADD COLUMN enabled_mimo_code BOOLEAN DEFAULT FALSE",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE mcp_servers ADD COLUMN enabled_workbuddy BOOLEAN DEFAULT FALSE",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE mcp_servers ADD COLUMN enabled_zcode BOOLEAN DEFAULT FALSE",
            [],
        );
        for column in [
            "enabled_cursor",
            "enabled_windsurf",
            "enabled_cline",
            "enabled_roo",
            "enabled_kilo",
            "enabled_continue",
            "enabled_goose",
            "enabled_kiro",
        ] {
            let _ = conn.execute(
                &format!("ALTER TABLE mcp_servers ADD COLUMN {column} BOOLEAN DEFAULT FALSE"),
                [],
            );
        }
        for column in ["enabled_kimi_code", "enabled_minimax_code"] {
            let exists: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM pragma_table_info('mcp_servers') WHERE name = ?1)",
                    [column],
                    |row| row.get(0),
                )
                .map_err(|error| AppError::Database(error.to_string()))?;
            if !exists {
                conn.execute(
                    &format!("ALTER TABLE mcp_servers ADD COLUMN {column} BOOLEAN DEFAULT FALSE"),
                    [],
                )
                .map_err(|error| AppError::Database(error.to_string()))?;
            }
        }
        conn.execute(
            "UPDATE skill_sync_targets SET tool_id = 'workbuddy' WHERE tool_id = 'workbuddy-cn'",
            [],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        let _ = conn.execute(
            "ALTER TABLE managed_skills ADD COLUMN source_subpath TEXT",
            [],
        );

        let transaction = conn
            .transaction()
            .map_err(|e| AppError::Database(format!("Failed to begin TRAE migration: {}", e)))?;
        if !has_trae_unification_marker(&transaction)? {
            transaction
                .execute_batch(
                    "UPDATE mcp_servers SET
                        enabled_trae = COALESCE(enabled_trae, FALSE) OR COALESCE(enabled_trae_work, FALSE),
                        enabled_trae_cn = COALESCE(enabled_trae_cn, FALSE) OR COALESCE(enabled_trae_solo_cn, FALSE),
                        enabled_trae_work = FALSE,
                        enabled_trae_solo_cn = FALSE;
                    UPDATE skill_sync_targets SET tool_id = CASE tool_id
                        WHEN 'trae-work' THEN 'trae'
                        WHEN 'trae-solo-cn' THEN 'trae-cn'
                        ELSE tool_id END;",
                )
                .map_err(|e| AppError::Database(format!("Failed to migrate TRAE state: {}", e)))?;

            let targets = {
                let mut stmt = transaction
                    .prepare(
                        "SELECT id, skill_id, tool_id, target_path FROM skill_sync_targets
                         WHERE tool_id IN ('trae', 'trae-cn')
                         ORDER BY synced_at DESC, id DESC",
                    )
                    .map_err(|e| AppError::Database(e.to_string()))?;
                let rows = stmt
                    .query_map([], |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                        ))
                    })
                    .map_err(|e| AppError::Database(e.to_string()))?;
                rows.collect::<Result<Vec<_>, _>>()
                    .map_err(|e| AppError::Database(e.to_string()))?
            };
            let mut seen_targets = HashSet::new();
            for (id, skill_id, tool_id, target_path) in targets {
                let path_key = crate::agents::normalized_path_key(Path::new(&target_path));
                if !seen_targets.insert((skill_id, tool_id, path_key)) {
                    transaction
                        .execute("DELETE FROM skill_sync_targets WHERE id = ?1", [id])
                        .map_err(|e| AppError::Database(e.to_string()))?;
                }
            }
            if let Some(path) = migration_backup {
                transaction
                    .execute(
                        "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
                         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                        rusqlite::params![
                            "trae-unification-v1-backup",
                            path.to_string_lossy().as_ref()
                        ],
                    )
                    .map_err(|e| AppError::Database(e.to_string()))?;
            }
            transaction
                .execute(
                    "INSERT INTO app_settings (key, value) VALUES (?1, '1')",
                    [TRAE_UNIFICATION_VERSION],
                )
                .map_err(|e| AppError::Database(e.to_string()))?;
        }
        transaction
            .commit()
            .map_err(|e| AppError::Database(format!("Failed to commit TRAE migration: {}", e)))?;

        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, AppError> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT value FROM app_settings WHERE key = ?1")
            .map_err(|e| AppError::Database(format!("Failed to prepare setting query: {}", e)))?;
        let mut rows = stmt
            .query([key])
            .map_err(|e| AppError::Database(format!("Failed to query setting: {}", e)))?;

        if let Some(row) = rows
            .next()
            .map_err(|e| AppError::Database(format!("Failed to read setting row: {}", e)))?
        {
            let value: String = row
                .get(0)
                .map_err(|e| AppError::Database(format!("Failed to decode setting: {}", e)))?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), AppError> {
        let conn = self.conn.lock();
        conn.execute(
            "
            INSERT INTO app_settings (key, value, updated_at)
            VALUES (?1, ?2, strftime('%s', 'now') * 1000)
            ON CONFLICT(key) DO UPDATE SET
                value = excluded.value,
                updated_at = excluded.updated_at
            ",
            [key, value],
        )
        .map_err(|e| AppError::Database(format!("Failed to save setting: {}", e)))?;
        Ok(())
    }
}

fn has_trae_unification_marker(conn: &Connection) -> Result<bool, AppError> {
    let has_settings: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'app_settings')",
            [],
            |row| row.get(0),
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
    if !has_settings {
        return Ok(false);
    }
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM app_settings WHERE key = ?1)",
        [TRAE_UNIFICATION_VERSION],
        |row| row.get(0),
    )
    .map_err(|e| AppError::Database(e.to_string()))
}

fn backup_before_trae_unification(conn: &Connection) -> Result<Option<PathBuf>, AppError> {
    let Some(db_path) = conn
        .path()
        .filter(|path| !path.is_empty() && *path != ":memory:")
    else {
        return Ok(None);
    };
    if has_trae_unification_marker(conn)? {
        return Ok(None);
    }
    let has_mcp: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'mcp_servers')",
            [],
            |row| row.get(0),
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
    if !has_mcp {
        return Ok(None);
    }
    let backup_dir = Path::new(db_path)
        .parent()
        .ok_or_else(|| AppError::Database("Database path has no parent".into()))?
        .join("backups");
    std::fs::create_dir_all(&backup_dir).map_err(|e| {
        AppError::Database(format!(
            "Failed to create migration backup directory: {}",
            e
        ))
    })?;
    let backup_path = backup_dir.join(format!(
        "{}-{}.db",
        TRAE_UNIFICATION_VERSION,
        uuid::Uuid::new_v4()
    ));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(&backup_path)
        .map_err(|e| AppError::Database(format!("Failed to create migration backup: {}", e)))?;
    if let Err(error) = conn.execute("VACUUM INTO ?1", [backup_path.to_string_lossy().as_ref()]) {
        let _ = std::fs::remove_file(&backup_path);
        return Err(AppError::Database(format!(
            "Failed to back up database before TRAE migration: {}",
            error
        )));
    }
    Ok(Some(backup_path))
}

/// 数据库连接锁宏
#[macro_export]
macro_rules! lock_conn {
    ($conn:expr) => {
        $conn.lock()
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn private_database_tightens_existing_files_and_wal_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let sandbox = tempfile::tempdir().unwrap();
        let directory = sandbox.path().join("data");
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("database.db");
        Connection::open(&path)
            .unwrap()
            .execute_batch(
                "CREATE TABLE existing (value TEXT); INSERT INTO existing VALUES ('preserved');",
            )
            .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let db = Database::open_private(&path).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let conn = db.conn.lock();
        assert_eq!(
            conn.query_row("SELECT value FROM existing", [], |row| row
                .get::<_, String>(0))
                .unwrap(),
            "preserved"
        );
        conn.execute_batch("PRAGMA journal_mode=WAL; INSERT INTO existing VALUES ('new');")
            .unwrap();
        for suffix in ["-wal", "-shm"] {
            let permissions = std::fs::metadata(format!("{}{suffix}", path.display()))
                .unwrap()
                .permissions();
            assert_eq!(permissions.mode() & 0o777, 0o600);
        }
    }
    use crate::mcp::AppType;
    use parking_lot::Mutex;
    use rusqlite::Connection;
    use std::sync::Arc;

    fn in_memory_database() -> Database {
        Database::in_memory().expect("initialize in-memory database")
    }

    #[test]
    fn migrates_existing_mcp_table_with_shared_workbuddy_column() {
        let connection = Connection::open_in_memory().expect("open database");
        connection
            .execute_batch(
                "CREATE TABLE mcp_servers (
                    id TEXT PRIMARY KEY, name TEXT NOT NULL, server_config TEXT NOT NULL,
                    description TEXT, homepage TEXT, docs TEXT, tags TEXT DEFAULT '[]',
                    enabled_qwen_code BOOLEAN DEFAULT FALSE,
                    enabled_claude BOOLEAN DEFAULT FALSE, enabled_codex BOOLEAN DEFAULT FALSE,
                    enabled_gemini BOOLEAN DEFAULT FALSE, enabled_opencode BOOLEAN DEFAULT FALSE,
                    enabled_trae BOOLEAN DEFAULT FALSE, enabled_trae_cn BOOLEAN DEFAULT FALSE,
                    enabled_trae_work BOOLEAN DEFAULT FALSE, enabled_trae_solo_cn BOOLEAN DEFAULT FALSE,
                    enabled_qoder BOOLEAN DEFAULT FALSE, enabled_qodercli BOOLEAN DEFAULT FALSE,
                    enabled_codebuddy BOOLEAN DEFAULT FALSE, enabled_hermes BOOLEAN DEFAULT FALSE,
                    enabled_mimo_code BOOLEAN DEFAULT FALSE,
                    created_at INTEGER DEFAULT 0, updated_at INTEGER DEFAULT 0
                );
                INSERT INTO mcp_servers
                    (id, name, server_config, enabled_codex, enabled_trae_work, enabled_trae_solo_cn)
                VALUES ('existing', 'Existing', '{}', TRUE, TRUE, TRUE);",
            )
            .expect("create old schema");
        let db = Database {
            conn: Arc::new(Mutex::new(connection)),
        };

        db.init_schema().expect("migrate schema");

        let conn = db.conn.lock();
        let workbuddy_enabled: bool = conn
            .query_row(
                "SELECT enabled_workbuddy FROM mcp_servers WHERE id = 'existing'",
                [],
                |row| row.get(0),
            )
            .expect("read migrated column");
        let codex_enabled: bool = conn
            .query_row(
                "SELECT enabled_codex FROM mcp_servers WHERE id = 'existing'",
                [],
                |row| row.get(0),
            )
            .expect("read existing state");
        assert!(!workbuddy_enabled);
        assert!(codex_enabled);
        let additions: (bool, bool) = conn
            .query_row(
                "SELECT enabled_kimi_code, enabled_minimax_code FROM mcp_servers WHERE id = 'existing'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read new Agent switches after old schema migration");
        assert_eq!(additions, (false, false));
        let trae_bits: (bool, bool, bool, bool) = conn
            .query_row(
                "SELECT enabled_trae, enabled_trae_cn, enabled_trae_work, enabled_trae_solo_cn
                 FROM mcp_servers WHERE id = 'existing'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("read unified switches in old schema");
        assert_eq!(trae_bits, (true, true, false, false));
    }

    #[test]
    fn persists_workbuddy_aliases_as_one_shared_switch() {
        let db = in_memory_database();
        let mut apps = McpApps::default();
        apps.set_enabled_for(&AppType::WorkBuddyCn, true);
        let server = McpServer {
            id: "shared".to_string(),
            name: "Shared".to_string(),
            server: McpServerSpec::default(),
            apps,
            description: None,
            homepage: None,
            docs: None,
            tags: vec![],
        };

        db.save_mcp_server(&server).expect("save server");
        let saved = db
            .get_all_mcp_servers()
            .expect("load servers")
            .swap_remove("shared")
            .expect("shared server");
        assert!(saved.apps.workbuddy);
        assert!(!saved.apps.workbuddy_cn);
        assert!(saved.apps.is_enabled_for(&AppType::WorkBuddyCn));

        let mut disabled = saved;
        disabled.apps.set_enabled_for(&AppType::WorkBuddy, false);
        assert!(!disabled.apps.is_enabled_for(&AppType::WorkBuddyCn));
    }

    #[test]
    fn persists_new_agent_switches_and_preserves_existing_state_across_schema_reinitialization() {
        let db = in_memory_database();
        let mut apps = McpApps::default();
        let additions = [
            AppType::KimiCode,
            AppType::MinimaxCode,
            AppType::Cursor,
            AppType::Windsurf,
            AppType::Cline,
            AppType::Roo,
            AppType::Kilo,
            AppType::Continue,
            AppType::Goose,
            AppType::Kiro,
        ];
        for app in &additions {
            apps.set_enabled_for(app, true);
        }
        apps.set_enabled_for(&AppType::Codex, true);
        let server = McpServer {
            id: "new-agents".into(),
            name: "New agents".into(),
            server: McpServerSpec::default(),
            apps,
            description: None,
            homepage: None,
            docs: None,
            tags: vec![],
        };
        db.save_mcp_server(&server).unwrap();
        db.init_schema().unwrap();
        let mut saved = db
            .get_all_mcp_servers()
            .unwrap()
            .swap_remove("new-agents")
            .unwrap();
        for app in &additions {
            assert!(saved.apps.is_enabled_for(app));
        }
        assert!(saved.apps.codex);
        let serialized = serde_json::to_value(&saved.apps).unwrap();
        for app in &additions {
            assert_eq!(serialized[app.name()], true);
        }
        assert!(serialized.get("workbuddy-cn").is_none());
        saved.apps.set_enabled_for(&AppType::Cline, false);
        db.save_mcp_server(&saved).unwrap();
        let saved = db
            .get_all_mcp_servers()
            .unwrap()
            .swap_remove("new-agents")
            .unwrap();
        assert!(!saved.apps.cline);
        assert!(saved.apps.goose);
        assert!(saved.apps.codex);
    }

    fn mark_trae_migration_pending(db: &Database) {
        db.conn
            .lock()
            .execute(
                "DELETE FROM app_settings WHERE key = ?1",
                [TRAE_UNIFICATION_VERSION],
            )
            .expect("remove migration marker");
    }

    #[test]
    fn unifies_legacy_trae_switches_once_and_preserves_later_disable() {
        let db = in_memory_database();
        mark_trae_migration_pending(&db);
        db.conn
            .lock()
            .execute_batch(
                "INSERT INTO mcp_servers
                    (id, name, server_config, enabled_trae_work, enabled_trae_solo_cn, enabled_codex)
                 VALUES ('legacy', 'Legacy', '{}', TRUE, TRUE, TRUE);
                 INSERT INTO mcp_servers
                    (id, name, server_config, enabled_trae, enabled_trae_cn)
                 VALUES ('current', 'Current', '{}', TRUE, TRUE);",
            )
            .expect("insert legacy state");

        db.init_schema().expect("migrate legacy switches");
        let mut saved = db
            .get_all_mcp_servers()
            .expect("read servers")
            .swap_remove("legacy")
            .expect("legacy server");
        assert!(saved.apps.trae);
        assert!(saved.apps.trae_cn);
        assert!(saved.apps.codex);
        assert!(!saved.apps.trae_work);
        assert!(!saved.apps.trae_solo_cn);
        assert!(
            db.get_all_mcp_servers().expect("read current server")["current"]
                .apps
                .trae
        );

        saved.apps.set_enabled_for(&AppType::Trae, false);
        saved.apps.set_enabled_for(&AppType::TraeSoloCn, false);
        db.save_mcp_server(&saved).expect("save disabled state");
        db.init_schema().expect("repeat migration");
        let disabled = db.get_all_mcp_servers().expect("read disabled state");
        assert!(!disabled["legacy"].apps.is_enabled_for(&AppType::TraeWork));
        assert!(!disabled["legacy"].apps.is_enabled_for(&AppType::TraeCn));
        let legacy_bits: (bool, bool) = db
            .conn
            .lock()
            .query_row(
                "SELECT enabled_trae_work, enabled_trae_solo_cn FROM mcp_servers WHERE id = 'legacy'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read historical columns");
        assert_eq!(legacy_bits, (false, false));
        assert_eq!(
            db.get_setting(TRAE_UNIFICATION_VERSION)
                .expect("read marker"),
            Some("1".into())
        );
    }

    #[test]
    fn old_snapshot_apps_deserialize_and_serialize_as_two_trae_switches() {
        let mut apps: McpApps = serde_json::from_value(serde_json::json!({
            "trae": false,
            "trae-cn": false,
            "trae-work": true,
            "trae-solo-cn": true,
            "codex": true
        }))
        .expect("read old snapshot");
        assert!(apps.trae);
        assert!(apps.trae_cn);
        assert!(!apps.trae_work);
        assert!(!apps.trae_solo_cn);
        let json = serde_json::to_value(&apps).expect("serialize normalized switches");
        assert_eq!(json["trae"], true);
        assert_eq!(json["trae-cn"], true);
        assert!(json.get("trae-work").is_none());
        assert!(json.get("trae-solo-cn").is_none());
        apps.set_enabled_for(&AppType::TraeWork, false);
        apps.set_enabled_for(&AppType::TraeCn, false);
        assert!(!apps.is_enabled_for(&AppType::Trae));
        assert!(!apps.is_enabled_for(&AppType::TraeSoloCn));
        assert!(apps.codex);
    }

    fn mcp_server(id: &str) -> McpServer {
        McpServer {
            id: id.into(),
            name: id.into(),
            server: McpServerSpec::default(),
            apps: McpApps::default(),
            description: None,
            homepage: None,
            docs: None,
            tags: vec![],
        }
    }

    #[test]
    fn bulk_save_and_snapshot_restore_roll_back_on_one_failure() {
        let db = in_memory_database();
        db.save_mcp_server(&mcp_server("original"))
            .expect("save original state");
        db.conn
            .lock()
            .execute_batch(
                "CREATE TRIGGER reject_invalid_mcp BEFORE INSERT ON mcp_servers
                 WHEN NEW.id = 'invalid' BEGIN SELECT RAISE(ABORT, 'invalid MCP'); END;",
            )
            .expect("install failure trigger");
        let batch = [mcp_server("valid"), mcp_server("invalid")];
        assert!(db.save_mcp_servers(&batch).is_err());
        let servers = db.get_all_mcp_servers().expect("read after failed batch");
        assert_eq!(servers.len(), 1);
        assert!(servers.contains_key("original"));
        assert!(db.replace_mcp_servers(&batch).is_err());
        assert!(db
            .get_all_mcp_servers()
            .expect("read after failed restore")
            .contains_key("original"));
        db.replace_mcp_servers(&[mcp_server("replacement")])
            .expect("restore complete snapshot");
        let restored = db.get_all_mcp_servers().expect("read restored snapshot");
        assert_eq!(restored.len(), 1);
        assert!(restored.contains_key("replacement"));
    }

    #[test]
    fn skill_target_migration_deduplicates_same_entity_and_keeps_other_paths() {
        let db = in_memory_database();
        mark_trae_migration_pending(&db);
        db.conn
            .lock()
            .execute_batch(
                "INSERT INTO managed_skills (id, name, source_type, central_path)
                 VALUES ('skill', 'Skill', 'local', '/central/skill');
                 INSERT INTO skill_sync_targets (skill_id, tool_id, mode, status, target_path, synced_at)
                 VALUES ('skill', 'trae-work', 'symlink', 'synced', '/canonical/skill', 1),
                        ('skill', 'trae', 'copy', 'synced', '/canonical/skill', 2),
                        ('skill', 'trae-work', 'copy', 'synced', '/legacy/skill', 3),
                        ('skill', 'trae-solo-cn', 'copy', 'synced', '/cn/skill', 4);",
            )
            .expect("insert old sync targets");
        db.init_schema().expect("migrate skill aliases");
        let conn = db.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT tool_id, mode, target_path FROM skill_sync_targets ORDER BY target_path",
            )
            .expect("prepare targets");
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .expect("query targets")
            .collect::<Result<Vec<_>, _>>()
            .expect("read targets");
        assert_eq!(rows.len(), 3);
        assert!(rows.contains(&("trae".into(), "copy".into(), "/canonical/skill".into())));
        assert!(rows.contains(&("trae".into(), "copy".into(), "/legacy/skill".into())));
        assert!(rows.contains(&("trae-cn".into(), "copy".into(), "/cn/skill".into())));
    }

    #[test]
    fn file_database_keeps_private_backup_of_pre_migration_state() {
        let directory = tempfile::tempdir().expect("temporary database directory");
        let db = Database {
            conn: Arc::new(Mutex::new(
                Connection::open(directory.path().join("test.db")).expect("open test database"),
            )),
        };
        db.init_schema().expect("initialize database");
        mark_trae_migration_pending(&db);
        db.conn
            .lock()
            .execute_batch(
                "INSERT INTO mcp_servers (id, name, server_config, enabled_trae_work)
                 VALUES ('legacy', 'Legacy', '{}', TRUE);",
            )
            .expect("insert pre-migration state");
        db.init_schema().expect("back up and migrate");
        let backup_path = db
            .get_setting("trae-unification-v1-backup")
            .expect("read backup setting")
            .expect("backup path");
        let backup = Connection::open(&backup_path).expect("open backup");
        let old_bits: (bool, bool) = backup
            .query_row(
                "SELECT enabled_trae, enabled_trae_work FROM mcp_servers WHERE id = 'legacy'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read pre-migration state");
        assert_eq!(old_bits, (false, true));
        assert!(!has_trae_unification_marker(&backup).expect("read old marker"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = std::fs::metadata(&backup_path)
                .expect("backup metadata")
                .permissions();
            assert_eq!(permissions.mode() & 0o777, 0o600);
        }
        db.init_schema().expect("repeat initialization");
        assert_eq!(
            std::fs::read_dir(directory.path().join("backups"))
                .expect("read backups")
                .count(),
            1
        );
    }
}
