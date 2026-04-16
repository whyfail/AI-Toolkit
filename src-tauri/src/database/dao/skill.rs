use crate::database::Database;
use crate::error::AppError;
use rusqlite::params;

pub struct SkillRecord {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub source_type: String,
    pub source_ref: Option<String>,
    pub source_subpath: Option<String>,
    pub central_path: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_sync_at: Option<i64>,
}

impl Database {
    /// 保存技能到数据库
    pub fn save_skill(&self, skill: &SkillRecord) -> Result<(), AppError> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO managed_skills (
                id, name, description, source_type, source_ref, source_subpath, central_path,
                created_at, updated_at, last_sync_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                skill.id,
                skill.name,
                skill.description,
                skill.source_type,
                skill.source_ref,
                skill.source_subpath,
                skill.central_path,
                skill.created_at,
                skill.updated_at,
                skill.last_sync_at,
            ],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    /// 获取所有技能
    pub fn get_all_skills(&self) -> Result<Vec<SkillRecord>, AppError> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, description, source_type, source_ref, source_subpath, central_path,
                        created_at, updated_at, last_sync_at
                 FROM managed_skills
                 ORDER BY name ASC",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;

        let skill_iter = stmt
            .query_map([], |row| {
                Ok(SkillRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    source_type: row.get(3)?,
                    source_ref: row.get(4)?,
                    source_subpath: row.get(5)?,
                    central_path: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                    last_sync_at: row.get(9)?,
                })
            })
            .map_err(|e| AppError::Database(e.to_string()))?;

        let mut skills = Vec::new();
        for skill in skill_iter {
            skills.push(skill.map_err(|e| AppError::Database(e.to_string()))?);
        }
        Ok(skills)
    }

    /// 根据 ID 获取技能
    pub fn get_skill_by_id(&self, id: &str) -> Result<Option<SkillRecord>, AppError> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, description, source_type, source_ref, source_subpath, central_path,
                        created_at, updated_at, last_sync_at
                 FROM managed_skills
                 WHERE id = ?1",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;

        let mut rows = stmt
            .query(params![id])
            .map_err(|e| AppError::Database(e.to_string()))?;

        if let Some(row) = rows.next().map_err(|e| AppError::Database(e.to_string()))? {
            Ok(Some(SkillRecord {
                id: row.get(0).map_err(|e| AppError::Database(e.to_string()))?,
                name: row.get(1).map_err(|e| AppError::Database(e.to_string()))?,
                description: row.get(2).map_err(|e| AppError::Database(e.to_string()))?,
                source_type: row.get(3).map_err(|e| AppError::Database(e.to_string()))?,
                source_ref: row.get(4).map_err(|e| AppError::Database(e.to_string()))?,
                source_subpath: row.get(5).map_err(|e| AppError::Database(e.to_string()))?,
                central_path: row.get(6).map_err(|e| AppError::Database(e.to_string()))?,
                created_at: row.get(7).map_err(|e| AppError::Database(e.to_string()))?,
                updated_at: row.get(8).map_err(|e| AppError::Database(e.to_string()))?,
                last_sync_at: row.get(9).map_err(|e| AppError::Database(e.to_string()))?,
            }))
        } else {
            Ok(None)
        }
    }

    /// 删除技能
    pub fn delete_skill(&self, id: &str) -> Result<(), AppError> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM managed_skills WHERE id = ?1", params![id])
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    /// 更新技能的最后同步时间
    pub fn update_skill_sync_time(&self, id: &str) -> Result<(), AppError> {
        let conn = self.conn.lock();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        conn.execute(
            "UPDATE managed_skills SET last_sync_at = ?1, updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }
}
