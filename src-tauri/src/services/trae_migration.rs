use crate::agents::{get_agent_config_paths, get_canonical_mcp_config_path};
use crate::app_state::AppState;
use crate::database::McpServer;
use crate::error::AppError;
use crate::mcp::AppType;
use crate::services::{mcp_service, sync};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationEntry {
    key: String,
    app_id: String,
    source_path: String,
    target_path: String,
    server_id: String,
    command: Option<String>,
    url: Option<String>,
    conflict: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationPreview {
    revision: String,
    entries: Vec<MigrationEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationSelection {
    pub key: String,
    pub rename_to: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationOutcome {
    imported: usize,
    backup_id: String,
    backup_path: String,
}

struct Candidate {
    key: String,
    app: AppType,
    source: PathBuf,
    server: McpServer,
}

#[derive(Serialize, Deserialize)]
struct FileBackup {
    path: PathBuf,
    content: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct MigrationBackup {
    servers: Vec<McpServer>,
    files: Vec<FileBackup>,
    after_revision: String,
}

struct PreparedMigration {
    current: IndexMap<String, McpServer>,
    candidates: Vec<Candidate>,
    preview: MigrationPreview,
    files: Vec<FileBackup>,
    targets: Vec<(AppType, PathBuf)>,
}

impl PreparedMigration {
    fn target(&self, app: &AppType) -> Result<&PathBuf, AppError> {
        self.targets
            .iter()
            .find(|(target_app, _)| target_app == app)
            .map(|(_, path)| path)
            .ok_or_else(|| AppError::Parse("未找到迁移目标".into()))
    }
}

fn fingerprint(
    servers: &IndexMap<String, McpServer>,
    files: &[FileBackup],
) -> Result<String, AppError> {
    let value = serde_json::json!({ "servers": servers, "files": files });
    Ok(hex::encode(Sha256::digest(value.to_string().as_bytes())))
}

fn read_file(path: PathBuf) -> Result<FileBackup, AppError> {
    let content = match fs::read_to_string(&path) {
        Ok(content) => Some(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    Ok(FileBackup { path, content })
}

fn prepare(state: &AppState) -> Result<PreparedMigration, AppError> {
    let current = state.db.get_all_mcp_servers()?;
    let mut base = current.clone();
    let mut files = Vec::new();
    let mut candidates = Vec::new();
    let mut targets = Vec::new();
    for app in [AppType::Trae, AppType::TraeCn] {
        let target = get_canonical_mcp_config_path(&app);
        targets.push((app.clone(), target.clone()));
        for path in get_agent_config_paths(&app) {
            let file = read_file(path.clone())?;
            let parsed = file
                .content
                .as_deref()
                .map(|content| mcp_service::parse_json_servers(content, &app))
                .transpose()?;
            files.push(file);
            if path == target {
                for server in parsed.unwrap_or_default() {
                    base.entry(server.id.clone()).or_insert(server);
                }
                continue;
            }
            for server in parsed.unwrap_or_default() {
                let key = hex::encode(Sha256::digest(
                    serde_json::json!({
                        "app": app.name(), "path": path, "server": server
                    })
                    .to_string()
                    .as_bytes(),
                ));
                candidates.push(Candidate {
                    key,
                    app: app.clone(),
                    source: path.clone(),
                    server,
                });
            }
        }
    }
    let revision = fingerprint(&current, &files)?;
    let mut seen = base.clone();
    let mut entries = Vec::new();
    for candidate in &candidates {
        let conflict = seen
            .get(&candidate.server.id)
            .is_some_and(|existing| existing.server != candidate.server.server);
        if !conflict {
            seen.entry(candidate.server.id.clone())
                .or_insert_with(|| candidate.server.clone());
        }
        entries.push(MigrationEntry {
            key: candidate.key.clone(),
            app_id: candidate.app.name().into(),
            source_path: candidate.source.to_string_lossy().into_owned(),
            target_path: get_canonical_mcp_config_path(&candidate.app)
                .to_string_lossy()
                .into_owned(),
            server_id: candidate.server.id.clone(),
            command: candidate.server.server.command.clone(),
            url: candidate.server.server.url.clone(),
            conflict,
        });
    }
    Ok(PreparedMigration {
        current,
        candidates,
        files,
        targets,
        preview: MigrationPreview { revision, entries },
    })
}

pub fn preview(state: &AppState) -> Result<MigrationPreview, AppError> {
    let _guard = state.mcp_gate.lock();
    Ok(prepare(state)?.preview)
}

fn resolve_selections(
    prepared: &PreparedMigration,
    selections: &[MigrationSelection],
) -> Result<(IndexMap<String, McpServer>, Vec<AppType>, usize), AppError> {
    let mut merged = prepared.current.clone();
    let mut selected_keys = HashSet::new();
    let mut apps = Vec::new();
    let mut ids = HashSet::new();
    for selection in selections {
        let candidate = prepared
            .candidates
            .iter()
            .find(|candidate| candidate.key == selection.key)
            .ok_or_else(|| AppError::Parse("迁移选项已变化，请重新预览".into()))?;
        if !apps.contains(&candidate.app) {
            apps.push(candidate.app.clone());
        }
    }
    for app in &apps {
        let path = prepared.target(app)?;
        if let Some(content) = prepared
            .files
            .iter()
            .find(|file| &file.path == path)
            .and_then(|file| file.content.as_deref())
        {
            let target_servers = mcp_service::parse_json_servers(content, app)?;
            merged = mcp_service::merge_imported_servers(
                merged,
                target_servers
                    .into_iter()
                    .map(|server| (app.clone(), server))
                    .collect(),
            )?;
        }
    }
    for selection in selections {
        if !selected_keys.insert(&selection.key) {
            return Err(AppError::Parse("重复的迁移选项".into()));
        }
        let candidate = prepared
            .candidates
            .iter()
            .find(|candidate| candidate.key == selection.key)
            .ok_or_else(|| AppError::Parse("迁移选项已变化，请重新预览".into()))?;
        let mut server = candidate.server.clone();
        if let Some(name) = selection.rename_to.as_deref() {
            let name = name.trim();
            if name.is_empty() || name.len() > 120 || name.chars().any(char::is_control) {
                return Err(AppError::Parse(
                    "请输入有效的新服务器名称（最多 120 字节）".into(),
                ));
            }
            server.id = name.into();
            server.name = name.into();
        }
        merged = mcp_service::merge_imported_servers(
            merged,
            vec![(candidate.app.clone(), server.clone())],
        )?;
        ids.insert(server.id);
    }
    Ok((merged, apps, ids.len()))
}

fn backup_directory(id: &str) -> Result<PathBuf, AppError> {
    uuid::Uuid::parse_str(id).map_err(|_| AppError::Parse("无效的迁移备份 ID".into()))?;
    Ok(dirs::home_dir()
        .ok_or_else(|| AppError::Parse("无法获取用户目录".into()))?
        .join(".ai-toolkit/backups")
        .join(format!("trae-migration-{id}")))
}

fn write_backup(path: &PathBuf, backup: &MigrationBackup) -> Result<(), AppError> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    let content = serde_json::to_string_pretty(backup)
        .map_err(|error| AppError::Serialization(error.to_string()))?;
    sync::atomic_write(&path.join("backup.json"), &content)
}

fn restore_files(files: &[FileBackup]) -> Result<(), AppError> {
    for file in files {
        if let Some(content) = &file.content {
            sync::atomic_write(&file.path, content)?;
        } else {
            match fs::remove_file(&file.path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
    Ok(())
}

pub fn apply(
    state: &AppState,
    revision: &str,
    selections: &[MigrationSelection],
) -> Result<MigrationOutcome, AppError> {
    let _guard = state.mcp_gate.lock();
    let prepared = prepare(state)?;
    let backup_id = uuid::Uuid::new_v4().to_string();
    let directory = backup_directory(&backup_id)?;
    apply_prepared(
        &state.db, &prepared, revision, selections, &backup_id, &directory,
    )
}

fn apply_prepared(
    db: &crate::database::Database,
    prepared: &PreparedMigration,
    revision: &str,
    selections: &[MigrationSelection],
    backup_id: &str,
    directory: &PathBuf,
) -> Result<MigrationOutcome, AppError> {
    if prepared.preview.revision != revision {
        return Err(AppError::Parse(
            "配置在预览后发生了变化，请重新预览再迁移".into(),
        ));
    }
    if selections.is_empty() {
        return Err(AppError::Parse("请至少选择一个配置".into()));
    }
    let (merged, apps, imported) = resolve_selections(&prepared, selections)?;
    let verified_files = prepared
        .files
        .iter()
        .map(|file| read_file(file.path.clone()))
        .collect::<Result<Vec<_>, _>>()?;
    if fingerprint(&prepared.current, &verified_files)? != revision {
        return Err(AppError::Parse(
            "配置在预览后发生了变化，请重新预览再迁移".into(),
        ));
    }
    let files = apps
        .iter()
        .map(|app| {
            let target = prepared.target(app)?;
            prepared
                .files
                .iter()
                .find(|file| &file.path == target)
                .map(|file| FileBackup {
                    path: file.path.clone(),
                    content: file.content.clone(),
                })
                .ok_or_else(|| AppError::Parse("未找到目标配置快照".into()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut backup = MigrationBackup {
        servers: prepared.current.values().cloned().collect(),
        files,
        after_revision: String::new(),
    };
    write_backup(&directory, &backup)?;
    let result = (|| {
        for app in &apps {
            let servers: Vec<_> = merged
                .values()
                .filter(|server| server.apps.is_enabled_for(app))
                .cloned()
                .collect();
            sync::sync_json_config(prepared.target(app)?, app, &servers)?;
        }
        let after_files = backup
            .files
            .iter()
            .map(|file| read_file(file.path.clone()))
            .collect::<Result<Vec<_>, _>>()?;
        backup.after_revision = fingerprint(&merged, &after_files)?;
        write_backup(&directory, &backup)?;
        db.save_mcp_servers(&merged.values().cloned().collect::<Vec<_>>())
    })();
    if let Err(error) = result {
        if let Err(restore_error) = restore_files(&backup.files) {
            return Err(AppError::Parse(format!(
                "迁移失败：{error}；自动恢复失败：{restore_error}；备份位于 {}",
                directory.display()
            )));
        }
        return Err(error);
    }
    Ok(MigrationOutcome {
        imported,
        backup_id: backup_id.into(),
        backup_path: directory.to_string_lossy().into_owned(),
    })
}

pub fn restore(state: &AppState, backup_id: &str) -> Result<(), AppError> {
    let _guard = state.mcp_gate.lock();
    let directory = backup_directory(backup_id)?;
    let backup: MigrationBackup =
        serde_json::from_str(&fs::read_to_string(directory.join("backup.json"))?)
            .map_err(|error| AppError::Parse(error.to_string()))?;
    restore_backup(&state.db, &backup)
}

fn restore_backup(
    db: &crate::database::Database,
    backup: &MigrationBackup,
) -> Result<(), AppError> {
    let current = db.get_all_mcp_servers()?;
    let after_files = backup
        .files
        .iter()
        .map(|file| read_file(file.path.clone()))
        .collect::<Result<Vec<_>, _>>()?;
    if fingerprint(&current, &after_files)? != backup.after_revision {
        return Err(AppError::Parse(
            "迁移后配置已被修改，无法自动撤销，以免覆盖后续修改。备份仍保留。".into(),
        ));
    }
    if let Err(error) =
        restore_files(&backup.files).and_then(|_| db.replace_mcp_servers(&backup.servers))
    {
        restore_files(&after_files)?;
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{McpApps, McpServerSpec};

    fn server(id: &str, command: &str) -> McpServer {
        McpServer {
            id: id.into(),
            name: id.into(),
            server: McpServerSpec {
                command: Some(command.into()),
                ..Default::default()
            },
            apps: McpApps::default(),
            description: None,
            homepage: None,
            docs: None,
            tags: vec![],
        }
    }

    fn prepared(current: McpServer, sources: Vec<McpServer>) -> PreparedMigration {
        let current = IndexMap::from([(current.id.clone(), current)]);
        PreparedMigration {
            current,
            candidates: sources
                .into_iter()
                .enumerate()
                .map(|(index, server)| Candidate {
                    key: index.to_string(),
                    app: AppType::TraeCn,
                    source: PathBuf::from("old-work/mcp.json"),
                    server,
                })
                .collect(),
            preview: MigrationPreview {
                revision: String::new(),
                entries: vec![],
            },
            files: vec![],
            targets: vec![
                (AppType::Trae, get_canonical_mcp_config_path(&AppType::Trae)),
                (
                    AppType::TraeCn,
                    get_canonical_mcp_config_path(&AppType::TraeCn),
                ),
            ],
        }
    }

    #[test]
    fn conflicting_legacy_config_requires_a_new_name() {
        let prepared = prepared(server("demo", "current"), vec![server("demo", "old")]);
        assert!(resolve_selections(
            &prepared,
            &[MigrationSelection {
                key: "0".into(),
                rename_to: None,
            }]
        )
        .is_err());
        let (merged, apps, count) = resolve_selections(
            &prepared,
            &[MigrationSelection {
                key: "0".into(),
                rename_to: Some("demo-work".into()),
            }],
        )
        .unwrap();
        assert_eq!(merged["demo"].server.command.as_deref(), Some("current"));
        assert_eq!(merged["demo-work"].server.command.as_deref(), Some("old"));
        assert!(merged["demo-work"].apps.trae_cn);
        assert_eq!(apps, vec![AppType::TraeCn]);
        assert_eq!(count, 1);
    }

    #[test]
    fn equal_sources_deduplicate_and_preserve_other_tools() {
        let mut existing = server("demo", "npx");
        existing.apps.codex = true;
        let prepared = prepared(existing, vec![server("demo", "npx"), server("demo", "npx")]);
        let selections: Vec<_> = (0..2)
            .map(|index| MigrationSelection {
                key: index.to_string(),
                rename_to: None,
            })
            .collect();
        let (merged, apps, count) = resolve_selections(&prepared, &selections).unwrap();
        assert_eq!(merged.len(), 1);
        assert!(merged["demo"].apps.codex && merged["demo"].apps.trae_cn);
        assert!(!merged["demo"].apps.trae_solo_cn);
        assert_eq!(apps.len(), 1);
        assert_eq!(count, 1);
    }

    #[test]
    fn migration_does_not_import_or_enable_an_unselected_region() {
        let mut existing = server("demo", "npx");
        existing.apps.trae_cn = false;
        let mut prepared = prepared(existing, vec![server("intl-only", "node")]);
        prepared.candidates[0].app = AppType::Trae;
        prepared.files.push(FileBackup {
            path: get_canonical_mcp_config_path(&AppType::TraeCn),
            content: Some(
                r#"{"mcpServers":{"cn-only":{"command":"node"},"demo":{"command":"npx"}}}"#.into(),
            ),
        });
        let (merged, apps, _) = resolve_selections(
            &prepared,
            &[MigrationSelection {
                key: "0".into(),
                rename_to: None,
            }],
        )
        .unwrap();
        assert!(!merged.contains_key("cn-only"));
        assert!(!merged["demo"].apps.trae_cn);
        assert!(merged["intl-only"].apps.trae);
        assert_eq!(apps, vec![AppType::Trae]);
    }

    #[test]
    fn changing_config_or_files_invalidates_revision() {
        let mut servers = IndexMap::from([("demo".into(), server("demo", "npx"))]);
        let files = vec![FileBackup {
            path: PathBuf::from("mcp.json"),
            content: Some("before".into()),
        }];
        let revision = fingerprint(&servers, &files).unwrap();
        servers["demo"].apps.trae_cn = true;
        assert_ne!(fingerprint(&servers, &files).unwrap(), revision);
        servers["demo"].apps.trae_cn = false;
        let changed_files = vec![FileBackup {
            path: PathBuf::from("mcp.json"),
            content: Some("after".into()),
        }];
        assert_ne!(fingerprint(&servers, &changed_files).unwrap(), revision);
    }

    #[test]
    fn rollback_restores_existing_file_and_removes_created_file() {
        let directory = tempfile::tempdir().unwrap();
        let existing = directory.path().join("existing.json");
        let created = directory.path().join("created.json");
        fs::write(&existing, "before").unwrap();
        let backups = vec![
            read_file(existing.clone()).unwrap(),
            read_file(created.clone()).unwrap(),
        ];
        fs::write(&existing, "after").unwrap();
        fs::write(&created, "after").unwrap();
        restore_files(&backups).unwrap();
        assert_eq!(fs::read_to_string(existing).unwrap(), "before");
        assert!(!created.exists());
    }

    #[test]
    fn duplicate_or_unknown_selection_does_not_mutate_the_plan() {
        let prepared = prepared(server("demo", "npx"), vec![server("demo", "npx")]);
        let selections = vec![
            MigrationSelection {
                key: "0".into(),
                rename_to: None,
            },
            MigrationSelection {
                key: "0".into(),
                rename_to: None,
            },
        ];
        assert!(resolve_selections(&prepared, &selections).is_err());
        assert!(resolve_selections(
            &prepared,
            &[MigrationSelection {
                key: "unknown".into(),
                rename_to: None
            }]
        )
        .is_err());
        assert!(!prepared.current["demo"].apps.trae_cn);
    }

    fn integration_plan(
        db: &crate::database::Database,
        root: &std::path::Path,
    ) -> PreparedMigration {
        let mut existing = server("demo", "npx");
        existing.apps.codex = true;
        db.save_mcp_server(&existing).unwrap();
        let target = root.join("new/mcp.json");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(
            &target,
            r#"{"otherSetting":true,"mcpServers":{"demo":{"command":"npx"}}}"#,
        )
        .unwrap();
        let mut plan = prepared(existing, vec![server("legacy", "uvx")]);
        plan.targets = vec![(AppType::TraeCn, target.clone())];
        plan.files = vec![read_file(target).unwrap()];
        plan.preview.revision = fingerprint(&plan.current, &plan.files).unwrap();
        plan
    }

    #[test]
    fn migration_and_restore_cover_database_and_config_together() {
        let db = crate::database::Database::in_memory().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let plan = integration_plan(&db, directory.path());
        let original = plan.files[0].content.clone().unwrap();
        let backup_dir = directory.path().join("backup");
        let outcome = apply_prepared(
            &db,
            &plan,
            &plan.preview.revision,
            &[MigrationSelection {
                key: "0".into(),
                rename_to: None,
            }],
            "test",
            &backup_dir,
        )
        .unwrap();
        assert_eq!(outcome.imported, 1);
        let after: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&plan.files[0].path).unwrap()).unwrap();
        assert_eq!(after["otherSetting"], true);
        assert_eq!(after["mcpServers"]["legacy"]["command"], "uvx");
        assert!(db.get_all_mcp_servers().unwrap()["legacy"].apps.trae_cn);
        let backup: MigrationBackup =
            serde_json::from_str(&fs::read_to_string(backup_dir.join("backup.json")).unwrap())
                .unwrap();
        restore_backup(&db, &backup).unwrap();
        assert_eq!(fs::read_to_string(&plan.files[0].path).unwrap(), original);
        let restored = db.get_all_mcp_servers().unwrap();
        assert_eq!(restored.len(), 1);
        assert!(restored["demo"].apps.codex);
        assert!(!restored["demo"].apps.trae_cn);
    }

    #[test]
    fn failed_database_commit_restores_the_file() {
        let db = crate::database::Database::in_memory().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let plan = integration_plan(&db, directory.path());
        db.conn.lock().execute_batch("CREATE TRIGGER reject_migration BEFORE INSERT ON mcp_servers BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
        assert!(apply_prepared(
            &db,
            &plan,
            &plan.preview.revision,
            &[MigrationSelection {
                key: "0".into(),
                rename_to: None
            }],
            "test",
            &directory.path().join("backup")
        )
        .is_err());
        assert_eq!(
            fs::read_to_string(&plan.files[0].path).unwrap(),
            plan.files[0].content.as_deref().unwrap()
        );
        let unchanged = db.get_all_mcp_servers().unwrap();
        assert_eq!(unchanged.len(), 1);
        assert!(unchanged["demo"].apps.codex);
    }

    #[test]
    fn changed_target_rejects_apply_before_any_write() {
        let db = crate::database::Database::in_memory().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let plan = integration_plan(&db, directory.path());
        fs::write(&plan.files[0].path, r#"{"mcpServers":{},"edited":true}"#).unwrap();
        let backup_dir = directory.path().join("backup");
        assert!(apply_prepared(
            &db,
            &plan,
            &plan.preview.revision,
            &[MigrationSelection {
                key: "0".into(),
                rename_to: None
            }],
            "test",
            &backup_dir
        )
        .is_err());
        assert!(!backup_dir.exists());
        assert_eq!(db.get_all_mcp_servers().unwrap().len(), 1);
        assert!(fs::read_to_string(&plan.files[0].path)
            .unwrap()
            .contains("edited"));
    }

    #[test]
    fn restore_preserves_edits_made_after_migration() {
        let db = crate::database::Database::in_memory().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let plan = integration_plan(&db, directory.path());
        let backup_dir = directory.path().join("backup");
        apply_prepared(
            &db,
            &plan,
            &plan.preview.revision,
            &[MigrationSelection {
                key: "0".into(),
                rename_to: None,
            }],
            "test",
            &backup_dir,
        )
        .unwrap();
        let backup: MigrationBackup =
            serde_json::from_str(&fs::read_to_string(backup_dir.join("backup.json")).unwrap())
                .unwrap();
        let mut edited = db.get_all_mcp_servers().unwrap()["legacy"].clone();
        edited.name = "Changed after migration".into();
        db.save_mcp_server(&edited).unwrap();
        let after = fs::read_to_string(&plan.files[0].path).unwrap();
        assert!(restore_backup(&db, &backup).is_err());
        assert_eq!(
            db.get_all_mcp_servers().unwrap()["legacy"].name,
            "Changed after migration"
        );
        assert_eq!(fs::read_to_string(&plan.files[0].path).unwrap(), after);
    }
}
