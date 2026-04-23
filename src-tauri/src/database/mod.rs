mod dao;

use parking_lot::Mutex;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Arc;

pub use dao::mcp::*;
pub use dao::skill::*;

use crate::error::AppError;

/// 数据库连接包装器（线程安全）
pub struct Database {
    pub conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn new() -> Result<Self, AppError> {
        let db_path = Self::get_db_path()?;

        // 确保目录存在
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AppError::Database(format!("Failed to create database directory: {}", e))
            })?;
        }

        let conn = Connection::open(&db_path)
            .map_err(|e| AppError::Database(format!("Failed to open database: {}", e)))?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        // 初始化表
        db.init_schema()?;

        Ok(db)
    }

    fn get_db_path() -> Result<PathBuf, AppError> {
        let config_dir = dirs::home_dir()
            .ok_or_else(|| AppError::Database("Could not find home directory".to_string()))?
            .join(".ai-toolkit");
        Ok(config_dir.join("ai-toolkit.db"))
    }

    fn init_schema(&self) -> Result<(), AppError> {
        let conn = self.conn.lock();

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
                enabled_trae_solo_cn BOOLEAN DEFAULT FALSE,
                enabled_qoder BOOLEAN DEFAULT FALSE,
                enabled_codebuddy BOOLEAN DEFAULT FALSE,
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
            "ALTER TABLE mcp_servers ADD COLUMN enabled_trae_solo_cn BOOLEAN DEFAULT FALSE",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE mcp_servers ADD COLUMN enabled_qoder BOOLEAN DEFAULT FALSE",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE mcp_servers ADD COLUMN enabled_codebuddy BOOLEAN DEFAULT FALSE",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE managed_skills ADD COLUMN source_subpath TEXT",
            [],
        );

        Ok(())
    }
}

/// 数据库连接锁宏
#[macro_export]
macro_rules! lock_conn {
    ($conn:expr) => {
        $conn.lock()
    };
}
