use serde::Serialize;
use std::path::PathBuf;
use tauri::State;
use crate::core::installer::{install_git_skill, install_git_skill_from_selection, scan_git_skill_candidates, GitSkillCandidate};
use crate::core::central_repo::resolve_central_repo_path;
use crate::core::featured_skills::{fetch_featured_skills as fetch_featured_skills_core, FeaturedSkill};
use crate::core::skills_search::{search_skills_online as search_skills_online_core, OnlineSkillResult};
use crate::skill_core::tool_adapters::{get_all_tool_status, default_tool_adapters, resolve_default_path, scan_tool_dir, is_tool_installed, ToolStatus, adapter_by_key};
use crate::database::SkillRecord;
use crate::app_state::AppState;

// Skills management commands
// Migrated from skills-hub-main

#[derive(Clone, Debug, Serialize)]
pub struct SyncTarget {
    pub tool: String,
    pub mode: String,
    pub status: String,
    pub target_path: String,
    pub synced_at: Option<i64>,
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
    let db_skills_map: std::collections::HashMap<String, SkillRecord> = db_skills
        .into_iter()
        .map(|s| (s.name.clone(), s))
        .collect();

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
            // 只要 binary 存在或 skills 目录存在，就认为工具已安装
            if is_tool_installed(tool) || skills_dir.exists() {
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
        let (skill_id, source_type, source_ref, source_subpath, created_at, updated_at, last_sync_at) =
            if let Some(db_skill) = db_skills_map.get(&skill_name) {
                (
                    db_skill.id.clone(),
                    db_skill.source_type.clone(),
                    db_skill.source_ref.clone(),
                    db_skill.source_subpath.clone(),
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
                    now,
                    now,
                    None,
                )
            };

        result.push(ManagedSkill {
            id: skill_id,
            name: skill_name.clone(),
            description: None,
            source_type,
            source_ref,
            source_subpath,
            central_path: central_path.to_string_lossy().to_string(),
            created_at,
            updated_at,
            last_sync_at,
            status: "active".to_string(),
            targets,
        });
    }

    Ok(result)
}

#[tauri::command]
pub async fn get_tool_status() -> Result<Vec<ToolStatus>, String> {
    get_all_tool_status()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_onboarding_plan() -> Result<OnboardingPlan, String> {
    let all_tools = default_tool_adapters();
    let mut groups_map: std::collections::HashMap<String, OnboardingGroup> = std::collections::HashMap::new();
    let mut total_skills = 0;
    let mut total_tools = 0;

    for tool in &all_tools {
        let installed = is_tool_installed(tool);
        if !installed {
            continue;
        }

        total_tools += 1;
        let skills_dir = resolve_default_path(tool).map_err(|e| e.to_string())?;
        let skills = scan_tool_dir(tool, &skills_dir).map_err(|e| e.to_string())?;
        total_skills += skills.len();

        let tool_id = tool.id.as_key().to_string();

        for skill in skills {
            let variant = OnboardingVariant {
                tool: tool_id.clone(),
                name: skill.name.clone(),
                path: skill.path.to_string_lossy().to_string(),
                fingerprint: None,
                is_link: skill.is_link,
                link_target: skill.link_target.map(|p| p.to_string_lossy().to_string()),
            };

            let entry = groups_map.entry(skill.name.clone()).or_insert_with(|| OnboardingGroup {
                name: skill.name.clone(),
                variants: vec![],
                has_conflict: false,
            });
            entry.variants.push(variant);
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

    let inner_result = tokio::task::spawn_blocking(move || {
        install_git_skill(&repo_url_clone, name_clone, None)
    })
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
        description: None,
        source_type: "git".to_string(),
        source_ref: Some(repo_url.clone()),
        source_subpath: result.source_subpath.clone(),
        central_path: result.central_path.to_string_lossy().to_string(),
        created_at: now,
        updated_at: now,
        last_sync_at: None,
    };
    state.db.save_skill(&skill_record).map_err(|e| e.to_string())?;

    Ok(ManagedSkill {
        id: result.skill_id,
        name: result.name,
        description: None,
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
        let (repo_dir, _) = clone_to_cache_for_list(&parsed.clone_url, parsed.branch.as_deref())
            .map_err(|e| e.to_string())?;

        let candidates = scan_git_skill_candidates(&repo_dir);
        Ok(candidates)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn clone_to_cache_for_list(clone_url: &str, branch: Option<&str>) -> Result<(PathBuf, String), String> {
    use crate::core::git_fetcher::clone_or_pull;

    let cache_root = std::env::temp_dir().join("ai-tool-manager-git-cache");
    std::fs::create_dir_all(&cache_root)
        .map_err(|e| e.to_string())?;

    let repo_key = {
        use sha2::Digest;
        let mut hasher = sha2::Sha256::new();
        hasher.update(clone_url.as_bytes());
        hasher.update(b"\n");
        if let Some(b) = branch {
            hasher.update(b.as_bytes());
        }
        hex::encode(hasher.finalize())
    };

    let repo_dir = cache_root.join(repo_key);
    let revision = clone_or_pull(clone_url, &repo_dir, branch)
        .map_err(|e| e.to_string())?;

    Ok((repo_dir, revision))
}

#[tauri::command]
pub async fn install_git_selection(
    state: State<'_, AppState>,
    repo_url: String,
    subpath: String,
    name: Option<String>,
) -> Result<ManagedSkill, String> {
    eprintln!("[DEBUG] install_git_selection called with url: {}, subpath: {}", repo_url, subpath);
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
        description: None,
        source_type: "git".to_string(),
        source_ref: Some(repo_url.clone()),
        source_subpath: result.source_subpath.clone(),
        central_path: result.central_path.to_string_lossy().to_string(),
        created_at: now,
        updated_at: now,
        last_sync_at: None,
    };
    state.db.save_skill(&skill_record).map_err(|e| e.to_string())?;

    Ok(ManagedSkill {
        id: result.skill_id,
        name: result.name,
        description: None,
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
    base_path: String,
    subpath: String,
    name: Option<String>,
) -> Result<ManagedSkill, String> {
    eprintln!("[DEBUG] install_local_selection called: base={}, subpath={}, name={:?}", base_path, subpath, name);

    let result: ManagedSkill = tokio::task::spawn_blocking(move || -> Result<ManagedSkill, String> {
        use crate::core::sync_engine::copy_dir_recursive;
        use crate::core::central_repo::{ensure_central_repo, resolve_central_repo_path};

        let base = PathBuf::from(&base_path);
        let selected_dir = if subpath.is_empty() || subpath == "." {
            base.clone()
        } else {
            base.join(&subpath)
        };

        if !selected_dir.exists() {
            return Err(format!("Source path does not exist: {:?}", selected_dir));
        }

        let skill_name = name.unwrap_or_else(|| {
            selected_dir
                .file_name()
                .map(|v| v.to_string_lossy().to_string())
                .unwrap_or_else(|| "unnamed-skill".to_string())
        });

        let central_dir = resolve_central_repo_path().map_err(|e| e.to_string())?;
        ensure_central_repo(&central_dir).map_err(|e| e.to_string())?;

        let central_path = central_dir.join(&skill_name);
        if central_path.exists() {
            return Err(format!("Skill already exists in central repo: {:?}", central_path));
        }

        copy_dir_recursive(&selected_dir, &central_path)
            .map_err(|e| e.to_string())?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        Ok(ManagedSkill {
            id: format!("local-{}", skill_name),
            name: skill_name,
            description: None,
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

    Ok(result)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn sync_skill_to_tool(
    skillId: String,
    skillName: String,
    tool: String,
    sourcePath: String,
) -> Result<SyncTarget, String> {
    eprintln!("[DEBUG] sync_skill_to_tool called: skillId={}, skillName={}, tool={}, source={}", skillId, skillName, tool, sourcePath);

    let result: SyncTarget = tokio::task::spawn_blocking(move || -> Result<SyncTarget, String> {
        use crate::core::sync_engine::sync_dir_for_tool_with_overwrite;

        let tool_adapter = adapter_by_key(&tool)
            .ok_or_else(|| format!("Unknown tool: {}", tool))?;

        let source = PathBuf::from(&sourcePath);
        let target_dir = crate::skill_core::tool_adapters::resolve_default_path(&tool_adapter)
            .map_err(|e| e.to_string())?;
        let target_path = target_dir.join(&skillName);

        let outcome = sync_dir_for_tool_with_overwrite(
            &tool,
            &source,
            &target_path,
            true,
        ).map_err(|e| e.to_string())?;

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
pub async fn import_existing_skill(
    source_path: String,
    name: String,
) -> Result<ManagedSkill, String> {
    eprintln!("[DEBUG] import_existing_skill called: source={}, name={}", source_path, name);

    let result: ManagedSkill = tokio::task::spawn_blocking(move || -> Result<ManagedSkill, String> {
        use crate::core::sync_engine::copy_dir_recursive;
        use crate::core::central_repo::{ensure_central_repo, resolve_central_repo_path};

        let source = PathBuf::from(&source_path);
        if !source.exists() {
            return Err(format!("Source path does not exist: {}", source_path));
        }

        let central_dir = resolve_central_repo_path().map_err(|e| e.to_string())?;
        ensure_central_repo(&central_dir).map_err(|e| e.to_string())?;

        let central_path = central_dir.join(&name);
        if central_path.exists() {
            return Err(format!("Skill already exists in central repo: {:?}", central_path));
        }

        copy_dir_recursive(&source, &central_path)
            .map_err(|e| e.to_string())?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        Ok(ManagedSkill {
            id: format!("local-{}", name),
            name: name,
            description: None,
            source_type: "local".to_string(),
            source_ref: Some(source_path),
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

    Ok(result)
}

#[tauri::command]
pub async fn delete_managed_skill(_skill_id: String, skill_name: String) -> Result<(), String> {
    // skill_id 格式: {tool_id}-{skill_name}
    // 我们需要找到这个技能在各个工具中的路径并删除

    // 首先获取所有技能，找到匹配的
    let all_tools = default_tool_adapters();
    let mut paths_to_delete: Vec<(PathBuf, bool)> = Vec::new(); // (path, is_link)

    for tool in &all_tools {
        let installed = is_tool_installed(tool);
        if !installed {
            continue;
        }

        let skills_dir = resolve_default_path(tool).map_err(|e| e.to_string())?;
        let skills = scan_tool_dir(tool, &skills_dir).map_err(|e| e.to_string())?;

        for skill in skills {
            if skill.name == skill_name {
                paths_to_delete.push((skill.path.clone(), skill.is_link));
            }
        }
    }

    // 删除所有找到的路径
    let count = paths_to_delete.len();
    for (path, is_link) in paths_to_delete {
        if path.exists() {
            if is_link {
                if let Err(e) = std::fs::remove_file(&path) {
                    eprintln!("Warning: failed to remove symlink {}: {}", path.display(), e);
                }
            } else {
                if let Err(e) = std::fs::remove_dir_all(&path) {
                    eprintln!("Warning: failed to remove directory {}: {}", path.display(), e);
                }
            }
        }
    }

    // 删除 central repo 中的原始技能文件夹
    let central_dir = resolve_central_repo_path().map_err(|e| e.to_string())?;
    let central_skill_path = central_dir.join(&skill_name);
    if central_skill_path.exists() {
        if let Err(e) = std::fs::remove_dir_all(&central_skill_path) {
            eprintln!("Warning: failed to remove central skill directory {}: {}", central_skill_path.display(), e);
        } else {
            println!("已删除 central repo 中的技能: {:?}", central_skill_path);
        }
    }

    println!("技能 '{}' 已删除 (共 {} 个工具路径)", skill_name, count);
    Ok(())
}

#[tauri::command]
pub async fn unsync_skill_from_tool(
    skill_name: String,
    tool: String,
) -> Result<(), String> {
    // 只从指定工具目录删除技能，不删除 central repo 中的源文件
    eprintln!("[DEBUG] unsync_skill_from_tool called: skillName={}, tool={}", skill_name, tool);

    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let tool_adapter = adapter_by_key(&tool)
            .ok_or_else(|| format!("Unknown tool: {}", tool))?;

        let installed = is_tool_installed(&tool_adapter);
        if !installed {
            return Err(format!("Tool {} is not installed", tool));
        }

        let skills_dir = resolve_default_path(&tool_adapter).map_err(|e| e.to_string())?;
        let skills = scan_tool_dir(&tool_adapter, &skills_dir).map_err(|e| e.to_string())?;

        for skill in skills {
            if skill.name == skill_name {
                let path = &skill.path;
                if path.exists() {
                    if skill.is_link {
                        if let Err(e) = std::fs::remove_file(path) {
                            eprintln!("Warning: failed to remove symlink {}: {}", path.display(), e);
                        }
                    } else {
                        if let Err(e) = std::fs::remove_dir_all(path) {
                            eprintln!("Warning: failed to remove directory {}: {}", path.display(), e);
                        }
                    }
                    println!("已从 {} 移除技能: {}", tool, skill_name);
                }
                break;
            }
        }

        Ok(())
    })
    .await
    .map_err(|e| e.to_string())??;

    Ok(())
}

#[tauri::command]
pub async fn update_skill(
    state: State<'_, AppState>,
    skill_id: String,
) -> Result<(), String> {
    // 从数据库获取技能信息
    let skill_record = state.db.get_skill_by_id(&skill_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Skill not found: {}", skill_id))?;

    // 如果有 GitHub 地址，则重新拉取
    if let Some(source_ref) = &skill_record.source_ref {
        if source_ref.starts_with("http://") || source_ref.starts_with("https://") {
            eprintln!("[DEBUG] Updating git skill from: {}", source_ref);
            let repo_url = source_ref.clone();
            let name = skill_record.name.clone();
            let central_path = skill_record.central_path.clone();
            let source_subpath = skill_record.source_subpath.clone();

            let skill_id_clone = skill_id.clone();
            tokio::task::spawn_blocking(move || -> Result<(), String> {
                // 先删除旧的技能目录
                let central_path_buf = PathBuf::from(&central_path);
                if central_path_buf.exists() {
                    eprintln!("[DEBUG] Removing existing skill directory: {:?}", central_path_buf);
                    std::fs::remove_dir_all(&central_path_buf)
                        .map_err(|e| format!("Failed to remove old skill: {}", e))?;
                }
                // 重新安装（可能因 APFS 延迟而报 "already exists"，此时重试一次）
                let name_for_retry = name.clone();
                if let Err(e) = install_git_skill(&repo_url, Some(name), source_subpath.as_deref()) {
                    let err_msg = format!("{:?}", e);
                    if err_msg.contains("already exists") {
                        eprintln!("[DEBUG] Race condition detected, retrying after delay...");
                        std::thread::sleep(std::time::Duration::from_millis(200));
                        install_git_skill(&repo_url, Some(name_for_retry), source_subpath.as_deref())
                            .map_err(|e| format!("{:?}", e))?;
                    } else {
                        return Err(err_msg);
                    }
                }
                Ok(())
            })
            .await
            .map_err(|e| e.to_string())??;

            // 更新数据库记录的时间戳
            state.db.update_skill_sync_time(&skill_id_clone)
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
    eprintln!("[DEBUG] rename_skill called: skill_id={}, new_name={}, new_source_ref={:?}", skill_id, new_name, new_source_ref);

    // 获取原有技能信息
    let skill_record = state.db.get_skill_by_id(&skill_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Skill not found: {}", skill_id))?;

    let old_name = skill_record.name.clone();
    let _old_central_path = skill_record.central_path.clone();

    // 解析 central repo 路径
    let central_dir = resolve_central_repo_path().map_err(|e| e.to_string())?;
    let old_path = central_dir.join(&old_name);
    let new_path = central_dir.join(&new_name);

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
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let old_path_for_blocking = old_path_clone;
        let new_path_for_blocking = new_path_clone;

        // 1. 重命名 central repo 中的文件夹
        if old_path_for_blocking.exists() {
            if old_name_for_blocking == new_name_for_blocking {
                // 名称没变，只更新 source_ref
            } else {
                std::fs::rename(&old_path_for_blocking, &new_path_for_blocking)
                    .map_err(|e| format!("重命名 central 技能文件夹失败: {}", e))?;
            }
        }

        // 2. 重命名所有已同步工具中的文件夹
        let all_tools = default_tool_adapters();
        for tool in &all_tools {
            let installed = is_tool_installed(tool);
            if !installed {
                continue;
            }

            let skills_dir = resolve_default_path(tool).map_err(|e| e.to_string())?;
            let old_tool_skill_path = skills_dir.join(&old_name_for_blocking);
            let new_tool_skill_path = skills_dir.join(&new_name_for_blocking);

            if old_tool_skill_path.exists() {
                if old_name_for_blocking == new_name_for_blocking {
                    // 名称没变，不需要重命名工具目录
                } else {
                    // 先删除目标（如果存在）
                    if new_tool_skill_path.exists() {
                        if new_tool_skill_path.is_symlink() {
                            std::fs::remove_file(&new_tool_skill_path)
                                .map_err(|e| format!("删除目标符号链接失败: {}", e))?;
                        } else {
                            std::fs::remove_dir_all(&new_tool_skill_path)
                                .map_err(|e| format!("删除目标目录失败: {}", e))?;
                        }
                    }
                    std::fs::rename(&old_tool_skill_path, &new_tool_skill_path)
                        .map_err(|e| format!("重命名工具技能文件夹失败: {}", e))?;
                }
            }
        }

        Ok(())
    })
    .await
    .map_err(|e| e.to_string())??;

    // 3. 更新数据库
    let new_central_path = new_path.to_string_lossy().to_string();
    state.db.update_skill_metadata(
        &skill_id,
        &new_name,
        new_source_ref.as_deref(),
        &new_central_path,
    ).map_err(|e| e.to_string())?;

    eprintln!("[DEBUG] rename_skill completed successfully");
    Ok(())
}

#[tauri::command]
pub async fn get_skill_readme(skill_name: String) -> Result<String, String> {
    let central_dir = resolve_central_repo_path().map_err(|e| e.to_string())?;
    let skill_path = central_dir.join(&skill_name).join("SKILL.md");

    if !skill_path.exists() {
        return Err("SKILL.md 文件不存在".to_string());
    }

    std::fs::read_to_string(&skill_path)
        .map_err(|e| format!("读取文件失败: {}", e))
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
    pub summary: String,
    pub downloads: u64,
    pub stars: u64,
    pub source_url: String,
}

impl From<FeaturedSkill> for FeaturedSkillDto {
    fn from(s: FeaturedSkill) -> Self {
        Self {
            slug: s.slug,
            name: s.name,
            summary: s.summary,
            downloads: s.downloads,
            stars: s.stars,
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
    .map_err(|e| e.to_string())
}
