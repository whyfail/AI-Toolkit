use crate::app_state::AppState;
use crate::core::central_repo::resolve_central_repo_path;
use crate::core::featured_skills::{
    fetch_featured_skills as fetch_featured_skills_core, FeaturedSkill,
};
use crate::core::installer::{
    install_git_skill, install_git_skill_from_selection, scan_git_skill_candidates,
    GitSkillCandidate,
};
use crate::core::path_safety::{
    existing_skill_entry, publish_skill, remove_entry, skill_entry, skill_staging, source_within,
    validate_skill_name,
};
use crate::core::skills_search::{
    search_skills_online as search_skills_online_core, OnlineSkillResult,
};
use crate::database::{Database, SkillRecord};
use crate::skill_core::tool_adapters::{
    adapter_by_key, default_tool_adapters, get_all_tool_status, is_tool_installed,
    resolve_default_path, resolve_read_paths, scan_tool_dir, ToolStatus,
};
use crate::utils::SuppressConsole;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::State;

fn check_skill_identity(
    db: &Database,
    id: &str,
    name: &str,
) -> Result<Option<SkillRecord>, String> {
    validate_skill_name(name).map_err(|e| e.to_string())?;
    let record = db.get_skill_by_id(id).map_err(|e| e.to_string())?;
    match &record {
        Some(record) if record.name != name => return Err("技能 ID 与名称不一致".into()),
        None if id != format!("local-{name}") => return Err("技能 ID 不存在".into()),
        _ => {}
    }
    Ok(record)
}

fn managed_source(db: &Database, root: &Path, id: &str, name: &str) -> Result<PathBuf, String> {
    let record = check_skill_identity(db, id, name)?;
    let source = source_within(root, name).map_err(|e| e.to_string())?;
    if let Some(record) = record {
        if Path::new(&record.central_path)
            .canonicalize()
            .map_err(|e| e.to_string())?
            != source
        {
            return Err("技能记录指向受管目录之外或与当前目录不一致".into());
        }
    }
    Ok(source)
}

// Skills management commands
// Migrated from skills-hub-main

/// Safely remove a path, handling Windows junctions correctly.
/// On Windows, junctions are directory-like reparse points that `remove_file`
/// cannot delete (DeleteFile API fails on directories) and `remove_dir_all`
/// would recursively delete the *target* contents instead of just the link.
fn safe_remove(path: &Path) -> std::io::Result<()> {
    remove_entry(path).map_err(std::io::Error::other)
}

#[derive(Clone, Debug, Serialize)]
pub struct SyncTarget {
    pub tool: String,
    pub mode: String,
    pub status: String,
    pub target_path: String,
    pub synced_at: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SkillConflictItem {
    pub name: String,
    pub scope: String,
    pub message: String,
    pub paths: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ManagedSkill {
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
    pub status: String,
    pub targets: Vec<SyncTarget>,
}

#[derive(Clone, Debug, Serialize)]
pub struct OnboardingVariant {
    pub tool: String,
    pub name: String,
    pub path: String,
    pub fingerprint: Option<String>,
    pub is_link: bool,
    pub link_target: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct OnboardingGroup {
    pub name: String,
    pub variants: Vec<OnboardingVariant>,
    pub has_conflict: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct OnboardingPlan {
    pub total_tools_scanned: usize,
    pub total_skills_found: usize,
    pub groups: Vec<OnboardingGroup>,
}

#[tauri::command]
pub async fn get_managed_skills(state: State<'_, AppState>) -> Result<Vec<ManagedSkill>, String> {
    let all_tools = default_tool_adapters();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    // 从数据库读取技能元数据
    let db_skills = state.db.get_all_skills().map_err(|e| e.to_string())?;
    let db_skills_map: std::collections::HashMap<String, SkillRecord> =
        db_skills.into_iter().map(|s| (s.name.clone(), s)).collect();

    // 首先从 central repo 扫描所有技能
    let central_dir = resolve_central_repo_path().map_err(|e| e.to_string())?;
    let mut skills_in_repo: Vec<(String, PathBuf)> = Vec::new();

    if central_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&central_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        skills_in_repo.push((name.to_string(), path));
                    }
                }
            }
        }
    }

    // 按名称排序
    skills_in_repo.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

    // ========================================
    // 优化：预先检测已安装工具，避免重复检测
    // ========================================
    #[derive(Clone)]
    struct InstalledToolInfo {
        tool_id: String,
        skills_dir: PathBuf,
    }
    let installed_tools: Vec<InstalledToolInfo> = all_tools
        .iter()
        .filter_map(|tool| {
            let skills_dir = resolve_default_path(tool).ok()?;
            if is_tool_installed(tool) {
                Some(InstalledToolInfo {
                    tool_id: tool.id.as_key().to_string(),
                    skills_dir,
                })
            } else {
                None
            }
        })
        .collect();

    let mut result: Vec<ManagedSkill> = Vec::new();

    for (skill_name, central_path) in skills_in_repo {
        let valid_source = source_within(&central_dir, &skill_name).is_ok()
            && validate_skill_name(&skill_name).is_ok();
        // 检查这个技能在哪些工具中已同步（使用预检测的安装状态）
        let mut targets: Vec<SyncTarget> = Vec::new();

        for tool_info in &installed_tools {
            let skill_target_path = tool_info.skills_dir.join(&skill_name);

            if skill_target_path.exists() {
                let mode = if skill_target_path.is_symlink() {
                    "link".to_string()
                } else {
                    "copy".to_string()
                };
                targets.push(SyncTarget {
                    tool: tool_info.tool_id.clone(),
                    mode,
                    status: "synced".to_string(),
                    target_path: skill_target_path.to_string_lossy().to_string(),
                    synced_at: None,
                });
            }
        }

        // 从数据库获取技能元数据
        let (
            skill_id,
            source_type,
            source_ref,
            source_subpath,
            description,
            created_at,
            updated_at,
            last_sync_at,
        ) = if let Some(db_skill) = db_skills_map.get(&skill_name) {
            (
                db_skill.id.clone(),
                db_skill.source_type.clone(),
                db_skill.source_ref.clone(),
                db_skill.source_subpath.clone(),
                db_skill.description.clone(),
                db_skill.created_at,
                db_skill.updated_at,
                db_skill.last_sync_at,
            )
        } else {
            (
                format!("local-{}", skill_name),
                "local".to_string(),
                Some(central_path.to_string_lossy().to_string()),
                None,
                valid_source
                    .then(|| crate::core::installer::read_skill_description(&central_path))
                    .flatten(),
                now,
                now,
                None,
            )
        };

        result.push(ManagedSkill {
            id: skill_id,
            name: skill_name.clone(),
            description,
            source_type,
            source_ref,
            source_subpath,
            central_path: central_path.to_string_lossy().to_string(),
            created_at,
            updated_at,
            last_sync_at,
            status: if valid_source { "active" } else { "invalid" }.to_string(),
            targets,
        });
    }

    Ok(result)
}

#[tauri::command]
pub async fn get_tool_status() -> Result<Vec<ToolStatus>, String> {
    get_all_tool_status().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn open_skill_path(path: String) -> Result<(), String> {
    let path_buf = PathBuf::from(&path);
    if !path_buf.exists() {
        return Err(format!("路径不存在: {}", path));
    }

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut cmd = std::process::Command::new("open");
        cmd.arg(&path_buf);
        cmd
    };

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut cmd = std::process::Command::new("explorer");
        cmd.arg(&path_buf);
        cmd
    };

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let mut command = {
        let mut cmd = std::process::Command::new("xdg-open");
        cmd.arg(&path_buf);
        cmd
    };

    command
        .suppress_console()
        .spawn()
        .map_err(|e| format!("打开路径失败: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn detect_skill_name_conflicts(
    state: State<'_, AppState>,
) -> Result<Vec<SkillConflictItem>, String> {
    let managed = get_managed_skills(state).await?;
    let mut by_name: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for skill in managed {
        let entry = by_name.entry(skill.name.to_ascii_lowercase()).or_default();
        entry.push(skill.central_path.clone());
        for target in skill.targets {
            entry.push(target.target_path);
        }
    }

    Ok(by_name
        .into_iter()
        .filter_map(|(name, paths)| {
            let unique: std::collections::HashSet<_> = paths.iter().collect();
            if unique.len() > 1 {
                Some(SkillConflictItem {
                    name,
                    scope: "skill-name".to_string(),
                    message: "同名 Skill 分布在多个路径，安装/重命名时建议选择覆盖、重命名或跳过"
                        .to_string(),
                    paths,
                })
            } else {
                None
            }
        })
        .collect())
}

#[tauri::command]
pub async fn get_onboarding_plan() -> Result<OnboardingPlan, String> {
    let all_tools = default_tool_adapters();
    let mut groups_map: std::collections::HashMap<String, OnboardingGroup> =
        std::collections::HashMap::new();
    let mut total_skills = 0;
    let mut total_tools = 0;
    let mut seen_dirs = std::collections::HashSet::new();
    let mut seen_skills = std::collections::HashSet::new();

    for tool in &all_tools {
        let installed = is_tool_installed(tool);
        let read_paths = resolve_read_paths(tool).map_err(|e| e.to_string())?;
        if !installed {
            continue;
        }

        total_tools += 1;
        let tool_id = tool.id.as_key().to_string();
        for skills_dir in read_paths {
            if !seen_dirs.insert(crate::agents::normalized_path_key(&skills_dir)) {
                continue;
            }
            let skills = scan_tool_dir(tool, &skills_dir).map_err(|e| e.to_string())?;
            for skill in skills {
                if !seen_skills.insert(crate::agents::normalized_path_key(&skill.path)) {
                    continue;
                }
                total_skills += 1;
                let variant = OnboardingVariant {
                    tool: tool_id.clone(),
                    name: skill.name.clone(),
                    path: skill.path.to_string_lossy().to_string(),
                    fingerprint: None,
                    is_link: skill.is_link,
                    link_target: skill.link_target.map(|p| p.to_string_lossy().to_string()),
                };

                let entry =
                    groups_map
                        .entry(skill.name.clone())
                        .or_insert_with(|| OnboardingGroup {
                            name: skill.name.clone(),
                            variants: vec![],
                            has_conflict: false,
                        });
                entry.variants.push(variant);
            }
        }
    }

    for group in groups_map.values_mut() {
        if group.variants.len() > 1 {
            let paths: Vec<&String> = group.variants.iter().map(|v| &v.path).collect();
            let unique_paths: std::collections::HashSet<&String> = paths.iter().cloned().collect();
            group.has_conflict = unique_paths.len() > 1;
        }
    }

    Ok(OnboardingPlan {
        total_tools_scanned: total_tools,
        total_skills_found: total_skills,
        groups: groups_map.into_values().collect(),
    })
}

#[tauri::command]
pub async fn install_git(
    state: State<'_, AppState>,
    repo_url: String,
    name: Option<String>,
) -> Result<ManagedSkill, String> {
    eprintln!("[DEBUG] install_git called with url: {}", repo_url);
    let repo_url_clone = repo_url.clone();
    let name_clone = name.clone();

    let inner_result =
        tokio::task::spawn_blocking(move || install_git_skill(&repo_url_clone, name_clone, None))
            .await
            .map_err(|e| e.to_string())?;

    let result = inner_result.map_err(|e| {
        let msg = e.to_string();
        if msg.starts_with("MULTI_SKILLS|") {
            msg
        } else {
            msg
        }
    })?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    // 保存到数据库
    let skill_record = SkillRecord {
        id: result.skill_id.clone(),
        name: result.name.clone(),
        description: result.description.clone(),
        source_type: "git".to_string(),
        source_ref: Some(repo_url.clone()),
        source_subpath: result.source_subpath.clone(),
        central_path: result.central_path.to_string_lossy().to_string(),
        created_at: now,
        updated_at: now,
        last_sync_at: None,
    };
    state
        .db
        .save_skill(&skill_record)
        .map_err(|e| e.to_string())?;

    Ok(ManagedSkill {
        id: result.skill_id,
        name: result.name,
        description: result.description,
        source_type: "git".to_string(),
        source_ref: Some(repo_url),
        source_subpath: result.source_subpath,
        central_path: result.central_path.to_string_lossy().to_string(),
        created_at: now,
        updated_at: now,
        last_sync_at: None,
        status: "active".to_string(),
        targets: vec![],
    })
}

#[tauri::command]
pub async fn list_git_skills(repo_url: String) -> Result<Vec<GitSkillCandidate>, String> {
    eprintln!("[DEBUG] list_git_skills called with url: {}", repo_url);
    let repo_url_clone = repo_url.clone();

    tokio::task::spawn_blocking(move || {
        use crate::core::installer::parse_github_url;

        let parsed = parse_github_url(&repo_url_clone);
        let cached =
            crate::core::git_cache::cached_repo(&parsed.clone_url, parsed.branch.as_deref())
                .map_err(|e| e.to_string())?;

        let candidates = scan_git_skill_candidates(&cached.path);
        Ok(candidates)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn install_git_selection(
    state: State<'_, AppState>,
    repo_url: String,
    subpath: String,
    name: Option<String>,
) -> Result<ManagedSkill, String> {
    eprintln!(
        "[DEBUG] install_git_selection called with url: {}, subpath: {}",
        repo_url, subpath
    );
    let repo_url_clone = repo_url.clone();
    let subpath_clone = subpath.clone();
    let name_clone = name.clone();

    let result = tokio::task::spawn_blocking(move || {
        install_git_skill_from_selection(&repo_url_clone, &subpath_clone, name_clone)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    // 保存到数据库
    let skill_record = SkillRecord {
        id: result.skill_id.clone(),
        name: result.name.clone(),
        description: result.description.clone(),
        source_type: "git".to_string(),
        source_ref: Some(repo_url.clone()),
        source_subpath: result.source_subpath.clone(),
        central_path: result.central_path.to_string_lossy().to_string(),
        created_at: now,
        updated_at: now,
        last_sync_at: None,
    };
    state
        .db
        .save_skill(&skill_record)
        .map_err(|e| e.to_string())?;

    Ok(ManagedSkill {
        id: result.skill_id,
        name: result.name,
        description: result.description,
        source_type: "git".to_string(),
        source_ref: Some(repo_url),
        source_subpath: result.source_subpath,
        central_path: result.central_path.to_string_lossy().to_string(),
        created_at: now,
        updated_at: now,
        last_sync_at: None,
        status: "active".to_string(),
        targets: vec![],
    })
}

#[tauri::command]
pub async fn install_local_selection(
    state: State<'_, AppState>,
    base_path: String,
    subpath: String,
    name: Option<String>,
) -> Result<ManagedSkill, String> {
    eprintln!(
        "[DEBUG] install_local_selection called: base={}, subpath={}, name={:?}",
        base_path, subpath, name
    );

    let result: ManagedSkill =
        tokio::task::spawn_blocking(move || -> Result<ManagedSkill, String> {
            use crate::core::central_repo::{ensure_central_repo, resolve_central_repo_path};
            use crate::core::installer::copy_local_skill_dir_verified;

            let base = PathBuf::from(&base_path);
            let selected_dir = source_within(&base, &subpath).map_err(|e| e.to_string())?;

            let skill_name = name.unwrap_or_else(|| {
                selected_dir
                    .file_name()
                    .map(|v| v.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unnamed-skill".to_string())
            });

            let central_dir = resolve_central_repo_path().map_err(|e| e.to_string())?;
            ensure_central_repo(&central_dir).map_err(|e| e.to_string())?;

            let central_path = skill_entry(&central_dir, &skill_name).map_err(|e| e.to_string())?;
            if central_path.exists() {
                return Err(format!(
                    "Skill already exists in central repo: {:?}",
                    central_path
                ));
            }

            let staging = skill_staging(&central_dir).map_err(|e| e.to_string())?;
            let prepared = staging.path().join("skill");
            copy_local_skill_dir_verified(&selected_dir, &prepared).map_err(|e| e.to_string())?;
            let central_path = publish_skill(&prepared, &central_dir, &skill_name, false)
                .map_err(|e| e.to_string())?;

            // Parse description from the freshly-installed SKILL.md so it's
            // persisted in the DB (otherwise the UI sees an empty description
            // until the user opens the detail modal).
            let description = crate::core::installer::read_skill_description(&central_path);

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            Ok(ManagedSkill {
                id: format!("local-{}", skill_name),
                name: skill_name,
                description,
                source_type: "local".to_string(),
                source_ref: Some(selected_dir.to_string_lossy().to_string()),
                source_subpath: None,
                central_path: central_path.to_string_lossy().to_string(),
                created_at: now,
                updated_at: now,
                last_sync_at: None,
                status: "active".to_string(),
                targets: vec![],
            })
        })
        .await
        .map_err(|e| e.to_string())??;

    let skill_record = SkillRecord {
        id: result.id.clone(),
        name: result.name.clone(),
        description: result.description.clone(),
        source_type: result.source_type.clone(),
        source_ref: result.source_ref.clone(),
        source_subpath: result.source_subpath.clone(),
        central_path: result.central_path.clone(),
        created_at: result.created_at,
        updated_at: result.updated_at,
        last_sync_at: result.last_sync_at,
    };
    if let Err(error) = state.db.save_skill(&skill_record) {
        let _ = remove_entry(Path::new(&result.central_path));
        return Err(format!("保存本地技能记录失败: {}", error));
    }

    Ok(result)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn sync_skill_to_tool(
    state: State<'_, AppState>,
    skillId: String,
    skillName: String,
    tool: String,
    sourcePath: String,
) -> Result<SyncTarget, String> {
    eprintln!(
        "[DEBUG] sync_skill_to_tool called: skillId={}, skillName={}, tool={}, source={}",
        skillId, skillName, tool, sourcePath
    );

    let root = resolve_central_repo_path().map_err(|e| e.to_string())?;
    let source = managed_source(&state.db, &root, &skillId, &skillName)?;
    if Path::new(&sourcePath)
        .canonicalize()
        .map_err(|e| e.to_string())?
        != source
    {
        return Err("同步来源与受管技能记录不一致".into());
    }

    let result: SyncTarget = tokio::task::spawn_blocking(move || -> Result<SyncTarget, String> {
        use crate::core::sync_engine::sync_dir_for_tool_with_overwrite;

        let tool_adapter =
            adapter_by_key(&tool).ok_or_else(|| format!("Unknown tool: {}", tool))?;
        let tool = tool_adapter.id.as_key().to_string();
        if !is_tool_installed(&tool_adapter) {
            return Err(format!("未检测到已安装的 {}", tool_adapter.display_name));
        }

        let target_dir = crate::skill_core::tool_adapters::resolve_default_path(&tool_adapter)
            .map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;
        let target_path = skill_entry(&target_dir, &skillName).map_err(|e| e.to_string())?;

        let outcome = sync_dir_for_tool_with_overwrite(&tool, &source, &target_path, true)
            .map_err(|e| e.to_string())?;

        let mode = match outcome.mode_used {
            crate::core::sync_engine::SyncMode::Symlink => "link",
            crate::core::sync_engine::SyncMode::Junction => "junction",
            crate::core::sync_engine::SyncMode::Copy => "copy",
            crate::core::sync_engine::SyncMode::Auto => "copy",
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        Ok(SyncTarget {
            tool,
            mode: mode.to_string(),
            status: "synced".to_string(),
            target_path: outcome.target_path.to_string_lossy().to_string(),
            synced_at: Some(now),
        })
    })
    .await
    .map_err(|e| e.to_string())??;

    Ok(result)
}

#[tauri::command]
pub async fn delete_managed_skill(
    state: State<'_, AppState>,
    skill_id: String,
    skill_name: String,
) -> Result<(), String> {
    validate_skill_name(&skill_name).map_err(|e| e.to_string())?;
    let central_dir = resolve_central_repo_path().map_err(|e| e.to_string())?;
    check_skill_identity(&state.db, &skill_id, &skill_name)?;
    let central_skill_path = skill_entry(&central_dir, &skill_name).map_err(|e| e.to_string())?;
    // skill_id 格式: {tool_id}-{skill_name}
    // 我们需要找到这个技能在各个工具中的路径并删除

    // 首先获取所有技能，找到匹配的
    let all_tools = default_tool_adapters();
    let mut paths_to_delete: Vec<PathBuf> = Vec::new();
    let mut seen_paths = std::collections::HashSet::new();

    for tool in &all_tools {
        let skills_dir = resolve_default_path(tool).map_err(|e| e.to_string())?;
        let installed = is_tool_installed(tool);
        if !installed {
            continue;
        }

        let skills = scan_tool_dir(tool, &skills_dir).map_err(|e| e.to_string())?;

        for skill in skills {
            if skill.name == skill_name {
                let path_key = crate::agents::normalized_path_key(&skill.path);
                if seen_paths.insert(path_key) {
                    paths_to_delete.push(skill.path.clone());
                }
            }
        }
    }

    // 删除所有找到的路径
    let count = paths_to_delete.len();
    for path in paths_to_delete {
        if path.exists() || path.symlink_metadata().is_ok() {
            let result = remove_entry(&path);

            if let Err(e) = result {
                return Err(format!(
                    "Failed to remove skill '{}' at {}: {}",
                    skill_name,
                    path.display(),
                    e
                ));
            }
        }
    }

    remove_entry(&central_skill_path).map_err(|e| format!("删除中央技能失败: {e}"))?;

    if !skill_id.is_empty() {
        state.db.delete_skill(&skill_id).map_err(|e| {
            format!(
                "Skill files were removed, but failed to delete database record '{}': {}",
                skill_id, e
            )
        })?;
    }

    println!("技能 '{}' 已删除 (共 {} 个工具路径)", skill_name, count);
    Ok(())
}

#[tauri::command]
pub async fn unsync_skill_from_tool(skill_name: String, tool: String) -> Result<(), String> {
    validate_skill_name(&skill_name).map_err(|e| e.to_string())?;
    // 只从指定工具目录删除技能，不删除 central repo 中的源文件
    eprintln!(
        "[DEBUG] unsync_skill_from_tool called: skillName={}, tool={}",
        skill_name, tool
    );

    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let tool_adapter =
            adapter_by_key(&tool).ok_or_else(|| format!("Unknown tool: {}", tool))?;

        let installed = is_tool_installed(&tool_adapter);
        if !installed {
            return Err(format!("Tool {} is not installed", tool));
        }

        let skills_dir = resolve_default_path(&tool_adapter).map_err(|e| e.to_string())?;
        let skills = scan_tool_dir(&tool_adapter, &skills_dir).map_err(|e| e.to_string())?;

        let mut removed = false;

        for skill in skills {
            if skill.name == skill_name {
                let path = &skill.path;
                if path.exists() || path.symlink_metadata().is_ok() {
                    let central = skill_entry(
                        &resolve_central_repo_path().map_err(|e| e.to_string())?,
                        &skill_name,
                    )
                    .map_err(|e| e.to_string())?;
                    let result = remove_tool_skill_preserving_source(path, &central, skill.is_link);

                    if let Err(e) = result {
                        return Err(format!(
                            "Failed to remove skill '{}' from {} at {}: {}",
                            skill_name,
                            tool,
                            path.display(),
                            e
                        ));
                    }
                    println!("已从 {} 移除技能: {}", tool, skill_name);
                    removed = true;
                }
                break;
            }
        }

        if !removed {
            return Err(format!(
                "Skill '{}' was not found in {} skills directory",
                skill_name, tool
            ));
        }

        Ok(())
    })
    .await
    .map_err(|e| e.to_string())??;

    Ok(())
}

#[tauri::command]
pub async fn update_skill(state: State<'_, AppState>, skill_id: String) -> Result<(), String> {
    // 从数据库获取技能信息
    let skill_record = state
        .db
        .get_skill_by_id(&skill_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Skill not found: {}", skill_id))?;

    // 如果有 GitHub 地址，则重新拉取
    if let Some(source_ref) = &skill_record.source_ref {
        if skill_record.source_type == "git" {
            eprintln!("[DEBUG] Updating git skill from: {}", source_ref);
            let repo_url = source_ref.clone();
            let name = skill_record.name.clone();
            let root = resolve_central_repo_path().map_err(|e| e.to_string())?;
            managed_source(&state.db, &root, &skill_id, &name)?;
            let source_subpath = skill_record.source_subpath.clone();

            let skill_id_clone = skill_id.clone();
            let new_central_path =
                tokio::task::spawn_blocking(move || -> Result<String, String> {
                    let result =
                        install_git_skill(&repo_url, Some(name), source_subpath.as_deref())
                            .map_err(|e| format!("{e:#}"))?;
                    Ok(result.central_path.to_string_lossy().to_string())
                })
                .await
                .map_err(|e| e.to_string())??;

            // 更新数据库记录的时间戳
            state
                .db
                .update_skill_metadata(
                    &skill_id_clone,
                    &skill_record.name,
                    skill_record.source_ref.as_deref(),
                    &new_central_path,
                )
                .map_err(|e| e.to_string())?;
            state
                .db
                .update_skill_sync_time(&skill_id_clone)
                .map_err(|e| e.to_string())?;

            return Ok(());
        }
    }

    // 本地技能无需更新
    println!("Update skill requested: {} (no action needed)", skill_id);
    Ok(())
}

#[tauri::command]
pub async fn rename_skill(
    state: State<'_, AppState>,
    skill_id: String,
    new_name: String,
    new_source_ref: Option<String>,
) -> Result<(), String> {
    eprintln!(
        "[DEBUG] rename_skill called: skill_id={}, new_name={}, new_source_ref={:?}",
        skill_id, new_name, new_source_ref
    );

    let record = state
        .db
        .get_skill_by_id(&skill_id)
        .map_err(|e| e.to_string())?;
    let old_name = match &record {
        Some(record) => record.name.clone(),
        None => skill_id
            .strip_prefix("local-")
            .filter(|name| !name.is_empty())
            .ok_or_else(|| "技能 ID 不存在".to_string())?
            .to_string(),
    };
    let central_dir = resolve_central_repo_path().map_err(|e| e.to_string())?;
    let old_path = existing_skill_entry(&central_dir, &old_name).map_err(|e| e.to_string())?;
    let new_path = skill_entry(&central_dir, &new_name).map_err(|e| e.to_string())?;

    // 检查新路径是否已存在
    let name_changed = old_name != new_name;
    if new_path.exists() && name_changed {
        return Err(format!("目标路径已存在: {:?}", new_path));
    }

    // 在阻塞线程中执行文件系统操作（克隆需要的值）
    let old_name_for_blocking = old_name.clone();
    let new_name_for_blocking = new_name.clone();
    let old_path_clone = old_path.clone();
    let new_path_clone = new_path.clone();
    let central_dir_for_blocking = central_dir.clone();
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let old_path_for_blocking = old_path_clone;
        let new_path_for_blocking = new_path_clone;

        // 1. 重命名 central repo 中的文件夹
        if std::fs::symlink_metadata(&old_path_for_blocking).is_ok() {
            if old_name_for_blocking == new_name_for_blocking {
                // 名称没变，只更新 source_ref
            } else {
                std::fs::rename(&old_path_for_blocking, &new_path_for_blocking)
                    .map_err(|e| format!("重命名 central 技能文件夹失败: {}", e))?;
            }
        }

        // 2. 重命名所有已同步工具中的文件夹
        let managed_source =
            source_within(&central_dir_for_blocking, &new_name_for_blocking).is_ok();
        let all_tools = default_tool_adapters();
        for tool in &all_tools {
            let installed = is_tool_installed(tool);
            if !installed {
                continue;
            }

            let skills_dir = resolve_default_path(tool).map_err(|e| e.to_string())?;
            if !skills_dir.exists() {
                continue;
            }
            let old_tool_skill_path = skills_dir.join(&old_name_for_blocking);
            let new_tool_skill_path =
                skill_entry(&skills_dir, &new_name_for_blocking).map_err(|e| e.to_string())?;

            if std::fs::symlink_metadata(&old_tool_skill_path).is_ok() {
                let old_tool_skill_path = existing_skill_entry(&skills_dir, &old_name_for_blocking)
                    .map_err(|e| e.to_string())?;
                if old_name_for_blocking == new_name_for_blocking {
                    // 名称没变，不需要重命名工具目录
                } else {
                    // 先删除目标（如果存在）
                    if new_tool_skill_path.exists() {
                        remove_entry(&new_tool_skill_path)
                            .map_err(|e| format!("删除目标技能失败: {e}"))?;
                    }
                    let is_link = old_tool_skill_path.is_symlink();
                    #[cfg(windows)]
                    let is_link = is_link
                        || junction::exists(&old_tool_skill_path).map_err(|e| e.to_string())?;
                    // An existing outside/broken link may be renamed, never read or copied.
                    if is_link && managed_source {
                        remove_entry(&old_tool_skill_path).map_err(|e| e.to_string())?;
                        crate::core::sync_engine::sync_dir_for_tool_with_overwrite(
                            tool.id.as_key(),
                            &new_path_for_blocking,
                            &new_tool_skill_path,
                            true,
                        )
                        .map_err(|e| e.to_string())?;
                    } else {
                        std::fs::rename(&old_tool_skill_path, &new_tool_skill_path)
                            .map_err(|e| format!("重命名工具技能文件夹失败: {}", e))?;
                    }
                }
            }
        }

        Ok(())
    })
    .await
    .map_err(|e| e.to_string())??;

    let new_central_path = new_path.to_string_lossy().to_string();
    if record.is_some() {
        state
            .db
            .update_skill_metadata(
                &skill_id,
                &new_name,
                new_source_ref.as_deref(),
                &new_central_path,
            )
            .map_err(|e| e.to_string())?;
    } else {
        // Filesystem-discovered skills have a synthetic ID until their first edit.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs() as i64;
        let description = source_within(&central_dir, &new_name)
            .ok()
            .and_then(|source| crate::core::installer::read_skill_description(&source));
        state
            .db
            .save_skill(&SkillRecord {
                id: skill_id,
                name: new_name,
                description,
                source_type: "local".into(),
                source_ref: new_source_ref.or(Some(new_central_path.clone())),
                source_subpath: None,
                central_path: new_central_path,
                created_at: now,
                updated_at: now,
                last_sync_at: None,
            })
            .map_err(|e| e.to_string())?;
    }

    eprintln!("[DEBUG] rename_skill completed successfully");
    Ok(())
}

#[tauri::command]
pub async fn get_skill_readme(skill_name: String) -> Result<String, String> {
    let central_dir = resolve_central_repo_path().map_err(|e| e.to_string())?;
    validate_skill_name(&skill_name).map_err(|e| e.to_string())?;
    let skill_dir = source_within(&central_dir, &skill_name).map_err(|e| e.to_string())?;
    let skill_path = crate::core::installer::find_skill_md(&skill_dir)
        .ok_or_else(|| "SKILL.md 文件不存在或超出技能目录".to_string())?;
    let file = std::fs::File::open(skill_path).map_err(|e| e.to_string())?;
    use std::io::Read;
    let mut contents = String::new();
    file.take(4 * 1024 * 1024 + 1)
        .read_to_string(&mut contents)
        .map_err(|e| e.to_string())?;
    if contents.len() > 4 * 1024 * 1024 {
        return Err("技能文档超过 4 MiB".into());
    }
    Ok(contents)
}

#[derive(Debug, Clone, Serialize)]
pub struct OnlineSkillDto {
    pub name: String,
    pub installs: u64,
    pub source: String,
    pub source_url: String,
}

impl From<OnlineSkillResult> for OnlineSkillDto {
    fn from(r: OnlineSkillResult) -> Self {
        Self {
            name: r.name,
            installs: r.installs,
            source: r.source,
            source_url: r.source_url,
        }
    }
}

#[tauri::command]
pub async fn search_skills_online(
    query: String,
    limit: Option<u32>,
) -> Result<Vec<OnlineSkillDto>, String> {
    let limit = limit.unwrap_or(20) as usize;
    tauri::async_runtime::spawn_blocking(move || {
        let results = search_skills_online_core(&query, limit)?;
        Ok::<_, anyhow::Error>(results.into_iter().map(OnlineSkillDto::from).collect())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct FeaturedSkillDto {
    pub slug: String,
    pub name: String,
    pub installs: u64,
    pub source_url: String,
}

impl From<FeaturedSkill> for FeaturedSkillDto {
    fn from(s: FeaturedSkill) -> Self {
        Self {
            slug: s.slug,
            name: s.name,
            installs: s.installs,
            source_url: s.source_url,
        }
    }
}

#[tauri::command]
pub async fn get_featured_skills() -> Result<Vec<FeaturedSkillDto>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let skills = fetch_featured_skills_core()?;
        Ok::<_, anyhow::Error>(skills.into_iter().map(FeaturedSkillDto::from).collect())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}

#[derive(Clone, Debug, Serialize)]
pub struct LocalSkillValidation {
    pub valid: bool,
    pub reason: Option<String>,
}

#[tauri::command]
pub async fn validate_local_skill(path: String) -> Result<LocalSkillValidation, String> {
    tokio::task::spawn_blocking(move || -> Result<LocalSkillValidation, String> {
        let dir = PathBuf::from(&path);
        if !dir.exists() {
            return Ok(LocalSkillValidation {
                valid: false,
                reason: Some("路径不存在".to_string()),
            });
        }
        if !dir.is_dir() {
            return Ok(LocalSkillValidation {
                valid: false,
                reason: Some("路径不是文件夹".to_string()),
            });
        }

        // 检查是否包含 SKILL.md / skill.md
        let has_skill_md = dir.join("SKILL.md").exists() || dir.join("skill.md").exists();
        if has_skill_md {
            return Ok(LocalSkillValidation {
                valid: true,
                reason: None,
            });
        }

        // 检查是否包含其他常见 skill 标识文件
        let has_skill_json = dir.join("skill.json").exists();
        if has_skill_json {
            return Ok(LocalSkillValidation {
                valid: true,
                reason: None,
            });
        }

        Ok(LocalSkillValidation {
            valid: false,
            reason: Some("该文件夹不是有效的技能目录（缺少 SKILL.md 或 skill.json）".to_string()),
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

/// A shared directory is the source itself, so removing an Agent's sync must preserve it.
fn remove_tool_skill_preserving_source(
    path: &Path,
    source: &Path,
    is_link: bool,
) -> std::io::Result<()> {
    if !is_link
        && crate::agents::normalized_path_key(path) == crate::agents::normalized_path_key(source)
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "该技能位于共享中央目录，无法仅对一个 Agent 取消同步",
        ));
    }
    if is_link {
        safe_remove(path)
    } else {
        std::fs::remove_dir_all(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_sources_require_matching_identity_and_managed_root() {
        let db = Database::in_memory().unwrap();
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("中文 skill");
        std::fs::create_dir(&source).unwrap();
        assert_eq!(
            managed_source(&db, root.path(), "local-中文 skill", "中文 skill").unwrap(),
            source.canonicalize().unwrap()
        );
        assert!(managed_source(&db, root.path(), "other-id", "中文 skill").is_err());
        assert!(managed_source(&db, root.path(), "local-../outside", "../outside").is_err());
        let record = SkillRecord {
            id: "record".into(),
            name: "中文 skill".into(),
            description: None,
            source_type: "local".into(),
            source_ref: None,
            source_subpath: None,
            central_path: root.path().to_string_lossy().into_owned(),
            created_at: 0,
            updated_at: 0,
            last_sync_at: None,
        };
        db.save_skill(&record).unwrap();
        assert!(managed_source(&db, root.path(), "record", "中文 skill").is_err());
        assert!(check_skill_identity(&db, "record", "different").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn unsync_preserves_the_shared_source_and_only_removes_an_independent_link_or_copy() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("central/skills/example");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(source.join("SKILL.md"), "# Example").unwrap();
        let shared = directory.path().join("agent-skills");
        std::os::unix::fs::symlink(source.parent().unwrap(), &shared).unwrap();
        assert!(
            remove_tool_skill_preserving_source(&shared.join("example"), &source, false).is_err()
        );
        assert!(source.join("SKILL.md").is_file());
        let link = directory.path().join("linked-skill");
        std::os::unix::fs::symlink(&source, &link).unwrap();
        remove_tool_skill_preserving_source(&link, &source, true).unwrap();
        assert!(!link.exists());
        assert!(source.join("SKILL.md").is_file());
        let copy = directory.path().join("copied-skill");
        std::fs::create_dir(&copy).unwrap();
        std::fs::write(copy.join("SKILL.md"), "# Example").unwrap();
        remove_tool_skill_preserving_source(&copy, &source, false).unwrap();
        assert!(!copy.exists());
        assert!(source.join("SKILL.md").is_file());
    }
}
