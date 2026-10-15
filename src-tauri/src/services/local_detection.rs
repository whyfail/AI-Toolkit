use crate::mcp::AppType;
use std::path::{Path, PathBuf};

pub fn mac_application_roots() -> Vec<PathBuf> {
    let mut roots = vec![PathBuf::from("/Applications")];
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join("Applications"));
    }
    roots
}

/// Scan only local executables. Never run an Agent or a package manager during detection.
pub fn which_binary(binary: &str) -> Option<String> {
    let home = dirs::home_dir()?;
    let mut directories: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();
    for relative in [
        ".local/bin",
        ".cargo/bin",
        ".bun/bin",
        ".npm-global/bin",
        ".yarn/bin",
        ".mimocode/bin",
        ".minimax-code/bin",
        ".minimax-code",
        ".minimax/bin",
        ".minimax/shims",
        ".hermes/venv/bin",
        "Library/pnpm",
        ".pnpm-global/bin",
        ".volta/bin",
        ".fnm/versions/node-default/bin",
        ".brew/bin",
    ] {
        directories.push(home.join(relative));
    }
    directories.extend([
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ]);
    for (variable, relative) in [("APPDATA", "npm"), ("LOCALAPPDATA", "pnpm")] {
        if let Some(base) = std::env::var_os(variable) {
            directories.push(PathBuf::from(base).join(relative));
        }
    }
    directories.push(home.join("scoop/shims"));
    let nvmd = home.join(".nvmd");
    let selected = std::env::var("NVMD_NODE_VERSION")
        .ok()
        .or_else(|| std::fs::read_to_string(nvmd.join("default")).ok());
    if let Some(version) = selected.filter(|version| !version.trim().is_empty()) {
        directories.push(nvmd.join("versions").join(version.trim()).join("bin"));
    }
    for versions in [nvmd.join("versions"), home.join(".nvm/versions/node")] {
        if let Ok(entries) = std::fs::read_dir(versions) {
            let mut paths: Vec<_> = entries
                .flatten()
                .map(|entry| entry.path().join("bin"))
                .collect();
            paths.sort();
            paths.reverse();
            directories.extend(paths);
        }
    }
    let mut seen = std::collections::HashSet::new();
    directories
        .into_iter()
        .filter(|dir| seen.insert(dir.clone()))
        .find_map(|dir| {
            let extensions: &[&str] = if cfg!(windows) {
                &[".exe", ".cmd", ".bat", ""]
            } else {
                &[""]
            };
            extensions.iter().find_map(|extension| {
                let path = dir.join(format!("{binary}{extension}"));
                if !is_executable(&path) || is_nvmd_shim(&path, &nvmd) {
                    return None;
                }
                Some(path.to_string_lossy().into_owned())
            })
        })
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    true
}

fn is_nvmd_shim(path: &Path, nvmd: &Path) -> bool {
    if path.parent() != Some(nvmd.join("bin").as_path()) {
        return false;
    }
    if std::fs::read_link(path)
        .ok()
        .and_then(|target| target.file_name().map(|name| name == "nvmd"))
        .unwrap_or(false)
    {
        return true;
    }
    match (path.metadata(), nvmd.join("bin/nvmd").metadata()) {
        (Ok(shim), Ok(manager)) => shim.len() > 1024 && shim.len() == manager.len(),
        _ => false,
    }
}

pub fn agent_binary_names(app: &AppType) -> &'static [&'static str] {
    match app.canonical() {
        AppType::QwenCode => &["qwen"],
        AppType::Claude => &["claude"],
        AppType::Codex => &["codex"],
        AppType::Gemini => &["gemini"],
        AppType::OpenCode => &["opencode"],
        AppType::Qodercli => &["qodercli"],
        AppType::CodeBuddy => &["codebuddy"],
        AppType::Hermes => &["hermes"],
        AppType::MimoCode => &["mimo"],
        AppType::KimiCode => &["kimi"],
        AppType::MinimaxCode => &["mcode"],
        AppType::Zcode => &["zcode"],
        AppType::Cursor => &["cursor-agent", "cursor"],
        AppType::Windsurf => &["windsurf", "devin"],
        AppType::Cline => &["cline"],
        AppType::Kilo => &["kilo"],
        AppType::Continue => &["cn"],
        AppType::Goose => &["goose"],
        AppType::Kiro => &["kiro-cli", "kiro"],
        // TRAE's common CLI cannot distinguish regional installations.
        _ => &[],
    }
}

fn extension_ids(app: &AppType) -> &'static [&'static str] {
    match app {
        AppType::Cline => &["saoudrizwan.claude-dev"],
        AppType::Roo => &["rooveterinaryinc.roo-cline", "roo-cline.roo-cline"],
        AppType::Kilo => &["kilocode.kilo-code", "kilo-code.kilo-code"],
        AppType::Continue => &["continue.continue"],
        AppType::Codex => &["openai.chatgpt"],
        _ => &[],
    }
}

fn extension_is_installed(root: &Path, ids: &[&str]) -> bool {
    let Ok(content) = std::fs::read_to_string(root.join("extensions.json")) else {
        return false;
    };
    let Ok(entries) = serde_json::from_str::<Vec<serde_json::Value>>(&content) else {
        return false;
    };
    let obsolete: serde_json::Value = std::fs::read_to_string(root.join(".obsolete"))
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default();
    entries.iter().any(|entry| {
        let id = entry
            .pointer("/identifier/id")
            .and_then(|id| id.as_str())
            .unwrap_or_default();
        if !ids.iter().any(|expected| id.eq_ignore_ascii_case(expected)) {
            return false;
        }
        let Some(location) = entry
            .get("relativeLocation")
            .and_then(|location| location.as_str())
        else {
            return false;
        };
        if !Path::new(location)
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
        {
            return false;
        }
        let directory = root.join(location);
        directory.starts_with(root)
            && obsolete.get(location).and_then(|value| value.as_bool()) != Some(true)
            && directory.join("package.json").is_file()
    })
}

pub fn is_agent_installed(app: &AppType) -> bool {
    if agent_binary_names(app)
        .iter()
        .any(|binary| which_binary(binary).is_some())
    {
        return true;
    }
    if dirs::home_dir().is_some() {
        let ids = extension_ids(app);
        if !ids.is_empty()
            && [
                "Code",
                "Code - Insiders",
                "VSCodium",
                "Cursor",
                "Windsurf",
                "Devin",
                "Trae",
                "Trae CN",
                "Kiro",
            ]
            .iter()
            .any(|editor| extension_installed_in_editor(editor, ids))
        {
            return true;
        }
    }
    #[cfg(target_os = "macos")]
    if is_app_installed_mac(app) {
        return true;
    }
    #[cfg(target_os = "windows")]
    if is_app_installed_windows(app) {
        return true;
    }
    false
}

pub(crate) fn extension_installed_in_editor(editor: &str, ids: &[&str]) -> bool {
    let Some(home) = dirs::home_dir() else {
        return false;
    };
    let (directory, binary, bundle, windows_directory, executable) = match editor {
        "Code" => (
            ".vscode",
            "code",
            "Visual Studio Code.app",
            "Microsoft VS Code",
            "Code.exe",
        ),
        "Code - Insiders" => (
            ".vscode-insiders",
            "code-insiders",
            "Visual Studio Code - Insiders.app",
            "Microsoft VS Code Insiders",
            "Code - Insiders.exe",
        ),
        "VSCodium" => (
            ".vscodium",
            "codium",
            "VSCodium.app",
            "VSCodium",
            "VSCodium.exe",
        ),
        "Cursor" => (".cursor", "cursor", "Cursor.app", "Cursor", "Cursor.exe"),
        "Windsurf" => (
            ".windsurf",
            "windsurf",
            "Windsurf.app",
            "Windsurf",
            "Windsurf.exe",
        ),
        "Devin" => (".windsurf", "devin", "Devin.app", "Devin", "Devin.exe"),
        "Trae" => (".trae", "trae", "Trae.app", "Trae", "Trae.exe"),
        "Trae CN" => (
            ".trae-cn",
            "trae-cn",
            "Trae CN.app",
            "Trae CN",
            "Trae CN.exe",
        ),
        "Kiro" => (".kiro", "kiro", "Kiro.app", "Kiro", "Kiro.exe"),
        _ => return false,
    };
    let mut host_installed = which_binary(binary).is_some();
    if cfg!(target_os = "macos") {
        host_installed |= mac_application_roots()
            .iter()
            .any(|root| root.join(bundle).is_dir());
    }
    if cfg!(windows) {
        host_installed |= ["LOCALAPPDATA", "ProgramFiles", "ProgramFiles(x86)"]
            .iter()
            .filter_map(|key| std::env::var_os(key).map(PathBuf::from))
            .any(|base| {
                [
                    base.join(windows_directory),
                    base.join("Programs").join(windows_directory),
                ]
                .iter()
                .any(|directory| directory.join(executable).is_file())
            });
    }
    host_installed && extension_is_installed(&home.join(directory).join("extensions"), ids)
}

pub fn mac_app_names(app: &AppType) -> &'static [&'static str] {
    match app {
        AppType::Claude => &["Claude Code.app"],
        AppType::Codex => &["Codex.app"],
        AppType::OpenCode => &["OpenCode.app"],
        AppType::KimiCode => &["Kimi Code.app"],
        AppType::MinimaxCode => &["MiniMax Code.app"],
        AppType::Trae | AppType::TraeWork => &["Trae.app", "TRAE Work.app"],
        AppType::TraeCn | AppType::TraeSoloCn => {
            &["Trae CN.app", "TRAE Work CN.app", "TRAE SOLO CN.app"]
        }
        AppType::Qoder => &["Qoder CN.app", "Qoder.app"],
        AppType::WorkBuddy | AppType::WorkBuddyCn => &["WorkBuddy.app"],
        AppType::Zcode => &["ZCode.app"],
        AppType::Cursor => &["Cursor.app"],
        AppType::Windsurf => &["Windsurf.app", "Devin.app"],
        AppType::Kilo => &["Kilo Code.app", "Kilo.app"],
        AppType::Goose => &["Goose.app", "goose.app"],
        AppType::Kiro => &["Kiro.app"],
        _ => &[],
    }
}

pub fn is_app_installed_mac(app: &AppType) -> bool {
    if app.is_trae() {
        return mac_application_roots()
            .iter()
            .any(|root| trae_mac_installation_path(app, root, true).is_some());
    }
    mac_application_roots().iter().any(|root| {
        mac_app_names(app)
            .iter()
            .any(|name| root.join(name).is_dir())
    })
}

fn trae_mac_installation_path(
    app: &AppType,
    applications: &Path,
    include_legacy: bool,
) -> Option<PathBuf> {
    if !app.is_trae() {
        return None;
    }
    mac_app_names(app)
        .iter()
        .take(if include_legacy { usize::MAX } else { 1 })
        .map(|name| applications.join(name))
        .find(|path| path.exists())
}

pub fn is_app_installed_windows(app: &AppType) -> bool {
    if !cfg!(windows) {
        return false;
    }
    find_app_executable_windows(app).is_some()
}

fn trae_windows_candidates(app: &AppType, bases: &[PathBuf], include_legacy: bool) -> Vec<PathBuf> {
    let installations: &[(&str, &[&str])] = match app.canonical() {
        AppType::Trae => &[
            ("Trae", &["Trae.exe"]),
            ("TRAE Work", &["Trae.exe", "TRAE Work.exe"]),
        ],
        AppType::TraeCn => &[
            ("Trae CN", &["Trae.exe", "Trae CN.exe"]),
            ("TRAE Work CN", &["Trae.exe", "TRAE Work CN.exe"]),
            ("TRAE SOLO CN", &["Trae.exe", "TRAE SOLO CN.exe"]),
        ],
        _ => return vec![],
    };
    installations
        .iter()
        .take(if include_legacy {
            installations.len()
        } else {
            1
        })
        .flat_map(|(directory, executable_names)| {
            bases.iter().flat_map(move |base| {
                [base.join("Programs").join(directory), base.join(directory)]
                    .into_iter()
                    .flat_map(move |directory| {
                        executable_names
                            .iter()
                            .map(move |name| directory.join(name))
                    })
            })
        })
        .collect()
}

fn trae_windows_candidates_from_env(app: &AppType, include_legacy: bool) -> Vec<PathBuf> {
    let bases: Vec<PathBuf> = [
        "LOCALAPPDATA",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "APPDATA",
        "USERPROFILE",
    ]
    .into_iter()
    .filter_map(|key| std::env::var(key).ok().map(PathBuf::from))
    .collect();
    trae_windows_candidates(app, &bases, include_legacy)
}

/// The shared `trae` CLI cannot prove which regional desktop product is installed.
pub fn has_canonical_trae_installation(app: &AppType) -> bool {
    if !app.is_trae() {
        return false;
    }
    #[cfg(target_os = "macos")]
    {
        return mac_application_roots()
            .iter()
            .any(|root| trae_mac_installation_path(app, root, false).is_some());
    }
    #[cfg(target_os = "windows")]
    {
        return trae_windows_candidates_from_env(app, false)
            .iter()
            .any(|path| path.exists());
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    false
}

pub fn find_app_executable_windows(app: &AppType) -> Option<std::path::PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    if app.is_trae() {
        return trae_windows_candidates_from_env(app, true)
            .into_iter()
            .find(|path| path.exists());
    }

    let candidates: &[&str] = match app {
        AppType::Claude => &["Claude.exe", "Claude Code.exe", "ClaudeCode.exe"],
        AppType::Codex => &["Codex.exe", "OpenAI Codex.exe", "OpenAICodex.exe"],
        AppType::OpenCode => &["OpenCode.exe", "opencode.exe"],
        AppType::KimiCode => &["Kimi Code.exe"],
        AppType::MinimaxCode => &["MiniMax Code.exe"],
        AppType::Qoder => &["Qoder.exe"],
        AppType::Zcode => &["ZCode.exe", "zcode.exe"],
        AppType::WorkBuddy | AppType::WorkBuddyCn => &["WorkBuddy.exe"],
        AppType::Cursor => &["Cursor.exe"],
        AppType::Windsurf => &["Windsurf.exe", "Devin.exe"],
        AppType::Kilo => &["Kilo Code.exe", "Kilo.exe"],
        AppType::Goose => &["Goose.exe", "goose.exe"],
        AppType::Kiro => &["Kiro.exe"],
        _ => return None,
    };

    let base_paths = [
        std::env::var("ProgramFiles").ok(),
        std::env::var("ProgramFiles(x86)").ok(),
        std::env::var("LOCALAPPDATA").ok(),
        std::env::var("APPDATA").ok(),
        std::env::var("USERPROFILE").ok(),
    ];

    let common_subdirs = [
        "",
        "Programs",
        "Claude",
        "Claude Code",
        "ClaudeCode",
        "AnthropicClaude",
        "Codex",
        "OpenAI Codex",
        "OpenAICodex",
        "Trae",
        "Trae CN",
        "TRAE Work",
        "TRAE Work CN",
        "TRAE SOLO CN",
        "OpenCode",
        "opencode",
        "Kimi Code",
        "MiniMax Code",
        "Qoder",
        "ZCode",
        "zcode",
        "Programs\\Claude",
        "Programs\\Claude Code",
        "Programs\\ClaudeCode",
        "Programs\\AnthropicClaude",
        "Programs\\Codex",
        "Programs\\OpenAI Codex",
        "Programs\\OpenAICodex",
        "Programs\\Trae",
        "Programs\\Trae CN",
        "Programs\\TRAE Work",
        "Programs\\TRAE Work CN",
        "Programs\\TRAE SOLO CN",
        "Programs\\OpenCode",
        "Programs\\opencode",
        "Programs\\Kimi Code",
        "Programs\\MiniMax Code",
        "Programs\\Qoder",
        "Programs\\ZCode",
        "WorkBuddy",
        "Programs\\WorkBuddy",
        "Cursor",
        "Programs\\Cursor",
        "Windsurf",
        "Programs\\Windsurf",
        "Devin",
        "Programs\\Devin",
        "Kilo Code",
        "Programs\\Kilo Code",
        "Goose",
        "Programs\\Goose",
        "Kiro",
        "Programs\\Kiro",
        "Microsoft\\WindowsApps",
    ];

    for base in base_paths.into_iter().flatten() {
        let base = std::path::PathBuf::from(base);
        for subdir in &common_subdirs {
            let dir = if subdir.is_empty() {
                base.clone()
            } else {
                base.join(subdir)
            };
            for exe in candidates {
                let path = dir.join(exe);
                if path.is_file() {
                    return Some(path);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_discovery_ignores_leftovers_removed_packages_and_obsolete_versions() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let location = "saoudrizwan.claude-dev-1.0.0";
        std::fs::create_dir_all(root.join(location)).unwrap();
        std::fs::write(root.join(location).join("package.json"), "{}").unwrap();
        assert!(!extension_is_installed(root, &["saoudrizwan.claude-dev"]));
        std::fs::write(
            root.join("extensions.json"),
            serde_json::json!([{
                "identifier": {"id": "SAOUDRIZWAN.claude-dev"}, "relativeLocation": location
            }])
            .to_string(),
        )
        .unwrap();
        assert!(extension_is_installed(root, &["saoudrizwan.claude-dev"]));
        assert!(!extension_is_installed(root, &["continue.continue"]));
        std::fs::write(
            root.join(".obsolete"),
            serde_json::json!({location: true}).to_string(),
        )
        .unwrap();
        assert!(!extension_is_installed(root, &["saoudrizwan.claude-dev"]));
        std::fs::remove_file(root.join(".obsolete")).unwrap();
        std::fs::remove_file(root.join(location).join("package.json")).unwrap();
        assert!(!extension_is_installed(root, &["saoudrizwan.claude-dev"]));
    }

    #[cfg(unix)]
    #[test]
    fn executable_detection_rejects_broken_links_and_non_executable_files_without_running_them() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("agent");
        let marker = directory.path().join("ran");
        std::fs::write(&path, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
        assert!(!is_executable(&path));
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(is_executable(&path));
        symlink(&path, directory.path().join("alias")).unwrap();
        assert!(is_executable(&directory.path().join("alias")));
        std::fs::remove_file(&path).unwrap();
        assert!(!is_executable(&directory.path().join("alias")));
        assert!(!marker.exists());
    }

    #[test]
    fn desktop_only_codex_and_workbuddy_use_one_product_without_region_metadata() {
        assert_eq!(mac_app_names(&AppType::Codex), &["Codex.app"]);
        assert_eq!(
            mac_app_names(&AppType::WorkBuddy),
            mac_app_names(&AppType::WorkBuddyCn)
        );
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("TRAE SOLO CN.app")).unwrap();
        assert!(trae_mac_installation_path(&AppType::TraeCn, directory.path(), true).is_some());
        assert!(trae_mac_installation_path(&AppType::Trae, directory.path(), true).is_none());
        assert!(trae_mac_installation_path(&AppType::TraeCn, directory.path(), false).is_none());
    }
}
