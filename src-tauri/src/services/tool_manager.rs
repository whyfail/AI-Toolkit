use crate::mcp::{AppType, InstallMethod};
use crate::utils::SuppressConsole;
use std::io::Write;
use std::process::Stdio;

#[derive(Debug, Clone, PartialEq)]
pub enum InstallMethodType {
    Brew,
    Npm,
    Curl,
    Winget,
    Scoop,
    Custom,
}

impl InstallMethodType {
    pub fn display_name(&self) -> &'static str {
        match self {
            InstallMethodType::Brew => "Homebrew",
            InstallMethodType::Npm => "npm",
            InstallMethodType::Curl => "curl 脚本",
            InstallMethodType::Winget => "Winget",
            InstallMethodType::Scoop => "Scoop",
            InstallMethodType::Custom => "自定义",
        }
    }
}

pub struct ToolManagerService;

fn get_binary_name(app: &AppType) -> String {
    match app {
        AppType::Trae => "trae".into(),
        AppType::TraeCn => "trae".into(),
        AppType::TraeSoloCn => "trae".into(),
        AppType::QwenCode => "qwen".into(),
        AppType::Claude => "claude".into(),
        _ => app.name().to_lowercase(),
    }
}

fn is_app_installed_mac(app: &AppType) -> bool {
    let app_name = match app {
        AppType::Trae => "Trae.app",
        AppType::TraeCn => "Trae CN.app",
        AppType::TraeSoloCn => "TRAE SOLO CN.app",
        AppType::Qoder => "Qoder.app",
        _ => return false,
    };
    std::path::Path::new(&format!("/Applications/{}", app_name)).exists()
}

pub fn is_app_installed_windows(app: &AppType) -> bool {
    if !cfg!(windows) {
        return false;
    }
    let candidates: &[&str] = match app {
        AppType::Trae => &["Trae.exe"],
        AppType::TraeCn => &["Trae.exe", "Trae CN.exe"],
        AppType::TraeSoloCn => &["Trae.exe", "TRAE SOLO CN.exe"],
        AppType::Qoder => &["Qoder.exe"],
        _ => return false,
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
        "Trae",
        "Trae CN",
        "TRAE SOLO CN",
        "Qoder",
        "Programs\\Trae",
        "Programs\\Trae CN",
        "Programs\\TRAE SOLO CN",
        "Programs\\Qoder",
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
                if dir.join(exe).exists() {
                    return true;
                }
            }
        }
    }
    false
}

fn ensure_npm_path_in_shell_config() -> Result<(), String> {
    if cfg!(windows) {
        ensure_npm_path_windows()
    } else {
        ensure_npm_path_unix()
    }
}

fn ensure_npm_path_unix() -> Result<(), String> {
    let output = std::process::Command::new("sh")
        .suppress_console()
        .args(["-c", "npm config get prefix"])
        .output()
        .map_err(|e| format!("获取 npm prefix 失败: {}", e))?;

    if !output.status.success() {
        return Ok(());
    }

    let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if prefix.is_empty() {
        return Ok(());
    }

    // Skip if npm is managed by a version manager (nvm, nvmd, fnm, volta, etc.)
    // These tools already handle PATH setup via their own init scripts
    let version_manager_paths = [
        "/.nvm/versions/node",
        "/.nvmd/versions/",
        "/.fnm/versions/node",
        "/.volta/",
    ];
    if version_manager_paths.iter().any(|p| prefix.contains(p)) {
        return Ok(());
    }

    let bin_path = format!("{}/bin", prefix);

    let zshrc_path =
        std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".zshrc");
    let content = std::fs::read_to_string(&zshrc_path).unwrap_or_default();

    if !content.contains(&bin_path) {
        let export_line = format!("\nexport PATH=\"{}:$PATH\"\n", bin_path);
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&zshrc_path)
            .map_err(|e| format!("打开 .zshrc 失败: {}", e))?;
        file.write_all(export_line.as_bytes())
            .map_err(|e| format!("写入 .zshrc 失败: {}", e))?;
    }

    Ok(())
}

fn ensure_npm_path_windows() -> Result<(), String> {
    let output = std::process::Command::new("cmd")
        .suppress_console()
        .args(["/C", "npm config get prefix"])
        .output()
        .map_err(|e| format!("获取 npm prefix 失败: {}", e))?;

    if !output.status.success() {
        return Ok(());
    }

    let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if prefix.is_empty() {
        return Ok(());
    }
    let prefix = prefix.replace('/', "\\");

    let user_profile = std::env::var("USERPROFILE").unwrap_or_default();
    let powershell_profile = std::path::PathBuf::from(&user_profile)
        .join("Documents")
        .join("PowerShell")
        .join("Microsoft.PowerShell_profile.ps1");
    let cmd_admin_profile = std::path::PathBuf::from(&user_profile)
        .join("Documents")
        .join("WindowsPowerShell")
        .join("Microsoft.PowerShell_profile.ps1");

    let profile_path = if powershell_profile.exists() {
        powershell_profile
    } else if cmd_admin_profile.exists() {
        cmd_admin_profile
    } else {
        return Ok(());
    };

    let content = std::fs::read_to_string(&profile_path).unwrap_or_default();

    if !content.contains(&prefix) {
        let export_line = format!("\n$env:PATH = \"{};$env:PATH\"\n", prefix);
        std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&profile_path)
            .map_err(|e| format!("打开 PowerShell profile 失败: {}", e))?
            .write_all(export_line.as_bytes())
            .map_err(|e| format!("写入 PowerShell profile 失败: {}", e))?;
    }

    Ok(())
}

#[cfg(windows)]
pub fn which_binary(binary: &str) -> Option<String> {
    let output = std::process::Command::new("where")
        .suppress_console()
        .arg(binary)
        .output()
        .ok()?;
    if output.status.success() {
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .map(|s| s.to_string())
    } else {
        None
    }
}

#[cfg(not(windows))]
fn is_nvmd_shim(path: &str, binary: &str) -> bool {
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => return false,
    };
    let nvmd_bin_dir = format!("{}/.nvmd/bin", home);
    let nvmd_main = format!("{}/nvmd", nvmd_bin_dir);

    // Check if path is inside ~/.nvmd/bin/
    if !path.starts_with(&nvmd_bin_dir) {
        return false;
    }

    // Case 1: symlink pointing to nvmd
    if let Ok(meta) = std::fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() {
            if let Ok(target) = std::fs::read_link(path) {
                if target.file_name().map_or(false, |f| f == "nvmd") {
                    return true;
                }
            }
        }
    }

    // Case 2: hard copy with same size as nvmd binary
    if let (Ok(shim_meta), Ok(nvmd_meta)) =
        (std::fs::metadata(path), std::fs::metadata(&nvmd_main))
    {
        if shim_meta.len() == nvmd_meta.len() && shim_meta.len() > 1024 {
            // Verify no real binary exists in any nvmd version directory
            let nvmd_dir = format!("{}/.nvmd", home);
            let versions_dir = format!("{}/versions", nvmd_dir);
            if let Ok(entries) = std::fs::read_dir(&versions_dir) {
                for entry in entries.flatten() {
                    let real_bin = entry.path().join("bin").join(binary);
                    if real_bin.exists() {
                        return false; // Real binary exists, not a dangling shim
                    }
                }
            }
            return true;
        }
    }

    false
}

#[cfg(not(windows))]
pub fn which_binary(binary: &str) -> Option<String> {
    // First try system which
    let output = std::process::Command::new("sh")
        .suppress_console()
        .args(["-c", &format!("which {}", binary)])
        .output()
        .ok()?;
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            if is_nvmd_shim(&path, binary) {
                return None;
            }
            return Some(path);
        }
    }

    // If system which failed, check common installation paths directly
    // This is needed because GUI apps don't inherit user's shell PATH
    let home = std::env::var("HOME").ok()?;
    let common_paths = [
        // homebrew
        format!("{}/.brew/bin/{}", home, binary),
        format!("/opt/homebrew/bin/{}", binary),
        format!("/usr/local/bin/{}", binary),
        // fnm
        format!("{}/.fnm/versions/node-default/bin/{}", home, binary),
        // volta
        format!("{}/.volta/bin/{}", home, binary),
        // npm global (via npm config get prefix)
    ];

    for path in &common_paths {
        if std::path::Path::new(path).exists() {
            return Some(path.clone());
        }
    }

    // nvmd (nvm-desktop): read default version, then check that version's bin dir
    let nvmd_dir = format!("{}/.nvmd", home);
    let nvmd_default_file = format!("{}/default", nvmd_dir);
    if let Ok(default_ver) = std::fs::read_to_string(&nvmd_default_file) {
        let default_ver = default_ver.trim();
        if !default_ver.is_empty() {
            let bin_path = format!("{}/versions/{}/bin/{}", nvmd_dir, default_ver, binary);
            if std::path::Path::new(&bin_path).exists() {
                return Some(bin_path);
            }
        }
    }
    // nvmd fallback: iterate all version directories (real binaries take priority)
    let nvmd_versions_dir = format!("{}/versions", nvmd_dir);
    if let Ok(entries) = std::fs::read_dir(&nvmd_versions_dir) {
        for entry in entries.flatten() {
            let bin_path = entry.path().join("bin").join(binary);
            if bin_path.exists() {
                return Some(bin_path.to_string_lossy().to_string());
            }
        }
    }
    // nvmd fallback: also check ~/.nvmd/bin/ (shim directory)
    // This is checked last because shims persist even after package uninstall
    let nvmd_shim = format!("{}/bin/{}", nvmd_dir, binary);
    if std::path::Path::new(&nvmd_shim).exists() && !is_nvmd_shim(&nvmd_shim, binary) {
        return Some(nvmd_shim);
    }

    // nvm: iterate version directories under ~/.nvm/versions/node/
    let nvm_node_dir = format!("{}/.nvm/versions/node", home);
    if let Ok(entries) = std::fs::read_dir(&nvm_node_dir) {
        for entry in entries.flatten() {
            let bin_path = entry.path().join("bin").join(binary);
            if bin_path.exists() {
                return Some(bin_path.to_string_lossy().to_string());
            }
        }
    }

    // Try to find npm global bin path
    if let Some(npm_prefix) = get_npm_global_prefix() {
        let npm_path = format!("{}/bin/{}", npm_prefix, binary);
        if std::path::Path::new(&npm_path).exists() {
            return Some(npm_path);
        }
    }

    None
}

#[cfg(not(windows))]
fn get_npm_global_prefix() -> Option<String> {
    let output = std::process::Command::new("sh")
        .suppress_console()
        .args(["-c", "npm config get prefix 2>/dev/null"])
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

#[cfg(windows)]
fn get_npm_global_prefix() -> Option<String> {
    let output = std::process::Command::new("cmd")
        .suppress_console()
        .args(["/C", "npm config get prefix"])
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

#[cfg(windows)]
fn npm_list_global(package: &str) -> Option<bool> {
    let output = std::process::Command::new("cmd")
        .suppress_console()
        .args(["/C", &format!("npm list -g {} 2>nul", package)])
        .output()
        .ok()?;
    Some(output.status.success() && String::from_utf8_lossy(&output.stdout).contains(package))
}

#[cfg(not(windows))]
fn npm_list_global(package: &str) -> Option<bool> {
    // Try using npm from PATH first
    let output = std::process::Command::new("sh")
        .suppress_console()
        .args(["-c", &format!("npm list -g {} 2>/dev/null", package)])
        .output()
        .ok()?;

    if output.status.success() && String::from_utf8_lossy(&output.stdout).contains(package) {
        return Some(true);
    }

    // If npm not in PATH, try to find it via which_binary
    let npm_path = which_binary("npm")?;
    let npm_prefix = std::path::Path::new(&npm_path)
        .parent()? // bin/npm -> bin/
        .parent()? // bin/ -> prefix
        .to_path_buf();

    // Check if the package directory exists in npm global lib
    let global_lib = npm_prefix.join("lib").join("node_modules");
    if !global_lib.exists() {
        return Some(false);
    }

    // Check if the package directory exists
    if let Ok(mut entries) = std::fs::read_dir(&global_lib) {
        while let Some(Ok(entry)) = entries.next() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            // npm packages can be @scope/name or just name
            if name_str == package || name_str.starts_with(&format!("{}/", package)) {
                return Some(true);
            }
            // Also check if it's a bin symlink in the package
            let bin_dir = entry.path().join("bin");
            if bin_dir.exists() {
                return Some(true);
            }
        }
    }

    // Also check @scope/package format for scoped packages
    if package.starts_with('@') {
        let parts: Vec<&str> = package.split('/').collect();
        if parts.len() == 2 {
            let scope_dir = global_lib.join(parts[0]);
            if scope_dir.exists() {
                if scope_dir.join(parts[1]).exists() {
                    return Some(true);
                }
            }
        }
    }

    Some(false)
}

impl ToolManagerService {
    pub async fn is_installed(app: &AppType) -> bool {
        if cfg!(windows) {
            if is_app_installed_windows(app) {
                return true;
            }
        } else {
            if is_app_installed_mac(app) {
                return true;
            }
        }

        let binary_name = get_binary_name(app);
        which_binary(&binary_name).is_some()
    }

    pub async fn detect_install_method(app: &AppType) -> Option<InstallMethodType> {
        if matches!(app, AppType::Trae | AppType::TraeCn | AppType::TraeSoloCn) {
            return None;
        }

        let binary_name = get_binary_name(app);
        let binary_path = which_binary(&binary_name)?;
        let binary_path_str = binary_path.as_str();

        #[cfg(not(windows))]
        {
            let home = std::env::var("HOME").ok();

            // Check if binary is in Homebrew directories
            let homebrew_bin_paths = ["/opt/homebrew/bin", "/usr/local/bin"];
            for path in homebrew_bin_paths {
                if binary_path_str.starts_with(path) {
                    // Verify it's actually Homebrew by checking if brew exists and the binary is linked
                    if which_binary("brew").is_some() {
                        // Additional check: see if the binary is managed by Homebrew
                        let output = std::process::Command::new("sh")
                            .suppress_console()
                            .args(["-c", &format!("brew list {} 2>/dev/null", binary_name)])
                            .output();
                        if let Ok(out) = output {
                            if out.status.success() {
                                return Some(InstallMethodType::Brew);
                            }
                        }
                    }
                    // If brew check fails but path suggests Homebrew, still return Brew
                    return Some(InstallMethodType::Brew);
                }
            }

            // Check Homebrew Cellar (actual installation path)
            if binary_path_str.contains("/Cellar/") || binary_path_str.contains("/homebrew/") {
                return Some(InstallMethodType::Brew);
            }

            // Check if binary is in nvm/nvmd/fnm/volta/npm global directories
            let npm_prefix = get_npm_global_prefix();
            let npm_prefix_str = npm_prefix.as_deref().unwrap_or("");
            if !npm_prefix_str.is_empty() {
                if binary_path_str.contains(&format!("{}/versions/node", npm_prefix_str))
                    || binary_path_str.contains(&format!("{}/node", npm_prefix_str))
                    || binary_path_str.contains(&format!("{}/bin", npm_prefix_str))
                {
                    return Some(InstallMethodType::Npm);
                }
            }

            // Check common nvm/nvmd/fnm/volta paths
            if let Some(ref h) = home {
                let nvm_paths = [
                    format!("{}/.nvm/versions/node", h),
                    format!("{}/.fnm/versions/node", h),
                    format!("{}/.volta", h),
                    format!("{}/.nvmd/versions", h),
                ];
                for nvm_path in &nvm_paths {
                    if binary_path_str.contains(nvm_path) {
                        return Some(InstallMethodType::Npm);
                    }
                }
            }

            // Check for cargo (Rust packages)
            if let Some(ref h) = home {
                let cargo_bin = format!("{}/.cargo/bin", h);
                if binary_path_str.starts_with(&cargo_bin) {
                    return Some(InstallMethodType::Curl); // Treat cargo as curl/custom
                }
            }

            // Check for user-local binaries
            if let Some(ref h) = home {
                let local_bin = format!("{}/.local/bin", h);
                if binary_path_str.starts_with(&local_bin) {
                    return Some(InstallMethodType::Curl);
                }
            }
        }

        #[cfg(windows)]
        {
            // Check Winget locations
            let winget_paths = [
                std::env::var("LOCALAPPDATA").ok(),
                std::env::var("ProgramFiles").ok(),
                std::env::var("UserProfile").ok(),
            ];
            for base in winget_paths.into_iter().flatten() {
                let _winget_dir = std::path::PathBuf::from(&base);
                for path in ["Microsoft\\WindowsApps", "winget", "Programs"] {
                    if binary_path_str.contains(path) {
                        return Some(InstallMethodType::Winget);
                    }
                }
            }

            // Check Scoop
            if let Some(ref h) = std::env::var("UserProfile").ok() {
                let scoop_dir = format!("{}\\scoop\\shims", h);
                if binary_path_str.starts_with(&scoop_dir) {
                    return Some(InstallMethodType::Scoop);
                }
            }

            // Check npm on Windows
            if let Some(npm_prefix) = get_npm_global_prefix() {
                let npm_prefix_path = std::path::Path::new(&npm_prefix);
                if binary_path_str.starts_with(npm_prefix_path.to_str().unwrap_or("")) {
                    return Some(InstallMethodType::Npm);
                }
            }
        }

        // Fallback: check if npm global list contains the package
        let install_info = app.get_install_info()?;
        if let Some(method) = install_info
            .methods
            .iter()
            .find(|m| matches!(m, InstallMethod::Npm { .. }))
        {
            if let InstallMethod::Npm { package } = method {
                if npm_list_global(package).unwrap_or(false) {
                    return Some(InstallMethodType::Npm);
                }
            }
        }

        // Default fallback for Unix
        #[cfg(not(windows))]
        if which_binary(&binary_name).is_some() {
            return Some(InstallMethodType::Curl);
        }

        #[cfg(windows)]
        if which_binary(&binary_name).is_some() {
            return Some(InstallMethodType::Winget);
        }

        None
    }

    pub async fn get_version(app: &AppType) -> Option<String> {
        let install_info = app.get_install_info()?;
        if install_info.version_cmd.is_empty() {
            return None;
        }

        // 尝试用 which_binary 获取完整路径，解决 GUI 进程 PATH 不完整时找不到命令的问题
        let binary_name = get_binary_name(app);
        let resolved_version_cmd = if let Some(full_path) = which_binary(&binary_name) {
            // 用完整路径替换 version_cmd 中的二进制名（只替换第一个匹配）
            if let Some(pos) = install_info.version_cmd.find(&binary_name) {
                let mut cmd = install_info.version_cmd.clone();
                cmd.replace_range(pos..pos + binary_name.len(), &full_path);
                cmd
            } else {
                install_info.version_cmd.clone()
            }
        } else {
            install_info.version_cmd.clone()
        };

        #[cfg(windows)]
        {
            let output = std::process::Command::new("cmd")
                .suppress_console()
                .args(["/C", &resolved_version_cmd])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .ok()?;
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !version.is_empty() {
                    return Some(version);
                }
            }
        }

        #[cfg(not(windows))]
        {
            let output = tokio::process::Command::new("sh")
                .suppress_console()
                .args(["-c", &resolved_version_cmd])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
                .ok()?;
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !version.is_empty() {
                    return Some(version);
                }
            }
        }

        None
    }

    pub async fn get_latest_version(app: &AppType) -> Option<String> {
        let install_info = app.get_install_info()?;

        for method in &install_info.methods {
            if let InstallMethod::Npm { package } = method {
                #[cfg(windows)]
                {
                    let output = std::process::Command::new("cmd")
                        .suppress_console()
                        .args(["/C", &format!("npm view {} version", package)])
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .output()
                        .ok()?;
                    if output.status.success() {
                        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        if !version.is_empty() {
                            return Some(version);
                        }
                    }
                }
                #[cfg(not(windows))]
                {
                    let output = tokio::process::Command::new("sh")
                        .suppress_console()
                        .args(["-c", &format!("npm view {} version", package)])
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .output()
                        .await
                        .ok()?;
                    if output.status.success() {
                        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        if !version.is_empty() {
                            return Some(version);
                        }
                    }
                }
            }

            #[cfg(not(windows))]
            if let InstallMethod::Brew { package } = method {
                let output = tokio::process::Command::new("sh")
                    .suppress_console()
                    .args([
                        "-c",
                        &format!("brew info {} 2>/dev/null | head -1", package),
                    ])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .await
                    .ok()?;
                if output.status.success() {
                    let output_str = String::from_utf8_lossy(&output.stdout);
                    if let Some(version) = output_str.split_whitespace().nth(1) {
                        let version = version.trim_start_matches('[').trim_end_matches(',');
                        if !version.is_empty()
                            && version
                                .chars()
                                .next()
                                .map(|c| c.is_ascii_digit())
                                .unwrap_or(false)
                        {
                            return Some(version.to_string());
                        }
                    }
                }
            }
        }

        None
    }

    pub async fn install(_app: &AppType, method: &InstallMethod) -> Result<(), String> {
        match method {
            #[cfg(not(windows))]
            InstallMethod::Brew { package } => {
                let mut cmd = tokio::process::Command::new("brew");
                cmd.suppress_console().arg("install").arg(package);
                Self::execute_command(&mut cmd).await
            }
            #[cfg(windows)]
            InstallMethod::Brew { package } => {
                let mut cmd = std::process::Command::new("brew");
                cmd.suppress_console().arg("install").arg(package);
                Self::execute_command_windows(&mut cmd).await
            }
            InstallMethod::Npm { package } => {
                #[cfg(windows)]
                {
                    let mut cmd = std::process::Command::new("cmd");
                    cmd.suppress_console()
                        .args(["/C", "npm", "install", "-g", package]);
                    Self::execute_command_windows(&mut cmd).await?;
                }
                #[cfg(not(windows))]
                {
                    let mut cmd = tokio::process::Command::new("npm");
                    cmd.suppress_console().arg("install").arg("-g").arg(package);
                    Self::execute_command(&mut cmd).await?;
                }
                ensure_npm_path_in_shell_config()?;
                Ok(())
            }
            #[cfg(not(windows))]
            InstallMethod::Curl { url } => {
                let script = format!("curl -fsSL {} | bash", url);
                let mut cmd = tokio::process::Command::new("sh");
                cmd.suppress_console().arg("-c").arg(&script);
                Self::execute_command(&mut cmd).await
            }
            #[cfg(windows)]
            InstallMethod::Curl { .. } => {
                Err("Windows 不支持 curl 安装方式，请使用其他安装方法".into())
            }
            InstallMethod::Custom { command } => {
                #[cfg(windows)]
                {
                    let mut cmd = std::process::Command::new("cmd");
                    cmd.suppress_console().args(["/C", command]);
                    Self::execute_command_windows(&mut cmd).await
                }
                #[cfg(not(windows))]
                {
                    let mut parts = command.split_whitespace();
                    let program = parts.next().ok_or("Empty command")?;
                    let args: Vec<&str> = parts.collect();
                    let mut cmd = tokio::process::Command::new(program);
                    cmd.suppress_console().args(&args);
                    Self::execute_command(&mut cmd).await
                }
            }
            InstallMethod::Download { url } => Err(format!("请手动下载安装: {}", url)),
        }
    }

    pub async fn update(app: &AppType) -> Result<(), String> {
        let install_info = app.get_install_info().ok_or("Unknown app type")?;

        let detected_method = Self::detect_install_method(app).await;

        match detected_method {
            #[cfg(not(windows))]
            Some(InstallMethodType::Brew) => {
                let package = install_info
                    .methods
                    .iter()
                    .find_map(|m| {
                        if let InstallMethod::Brew { package } = m {
                            Some(package.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| app.name().to_lowercase());
                let mut cmd = tokio::process::Command::new("brew");
                cmd.suppress_console().arg("upgrade").arg(&package);
                Self::execute_command(&mut cmd).await
            }
            #[cfg(windows)]
            Some(InstallMethodType::Winget) => {
                let package = install_info
                    .methods
                    .iter()
                    .find_map(|m| match m {
                        InstallMethod::Brew { package } => Some(package.clone()),
                        InstallMethod::Npm { package } => Some(package.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| app.name().to_lowercase());
                let mut cmd = std::process::Command::new("winget");
                cmd.suppress_console()
                    .args(["upgrade", "--id", &package, "-e"]);
                Self::execute_command_windows(&mut cmd).await
            }
            #[cfg(windows)]
            Some(InstallMethodType::Scoop) => {
                let package = install_info
                    .methods
                    .iter()
                    .find_map(|m| match m {
                        InstallMethod::Brew { package } => Some(package.clone()),
                        InstallMethod::Npm { package } => Some(package.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| app.name().to_lowercase());
                let mut cmd = std::process::Command::new("scoop");
                cmd.suppress_console().args(["update", &package]);
                Self::execute_command_windows(&mut cmd).await
            }
            Some(InstallMethodType::Npm) => {
                let package = install_info
                    .methods
                    .iter()
                    .find_map(|m| {
                        if let InstallMethod::Npm { package } = m {
                            Some(package.clone())
                        } else {
                            None
                        }
                    })
                    .ok_or("未找到 npm 包信息")?;
                #[cfg(windows)]
                {
                    let mut cmd = std::process::Command::new("cmd");
                    cmd.suppress_console().args([
                        "/C",
                        "npm",
                        "install",
                        "-g",
                        &format!("{}@latest", package),
                    ]);
                    Self::execute_command_windows(&mut cmd).await
                }
                #[cfg(not(windows))]
                {
                    let mut cmd = tokio::process::Command::new("npm");
                    cmd.suppress_console()
                        .arg("install")
                        .arg("-g")
                        .arg(format!("{}@latest", package));
                    Self::execute_command(&mut cmd).await
                }
            }
            #[cfg(not(windows))]
            Some(InstallMethodType::Curl) => {
                if let Some(url) = install_info.methods.iter().find_map(|m| {
                    if let InstallMethod::Curl { url } = m {
                        Some(url.clone())
                    } else {
                        None
                    }
                }) {
                    let script = format!("curl -fsSL {} | bash", url);
                    let mut cmd = tokio::process::Command::new("sh");
                    cmd.suppress_console().arg("-c").arg(&script);
                    Self::execute_command(&mut cmd).await
                } else if !install_info.update_cmd.is_empty() {
                    let mut parts = install_info.update_cmd.split_whitespace();
                    let program = parts.next().ok_or("Empty update command")?;
                    let args: Vec<&str> = parts.collect();
                    let mut cmd = tokio::process::Command::new(program);
                    cmd.suppress_console().args(&args);
                    Self::execute_command(&mut cmd).await
                } else {
                    Err("此工具不支持自动更新，请手动下载新版本".into())
                }
            }
            #[cfg(windows)]
            Some(InstallMethodType::Curl) => Err("此工具不支持自动更新，请手动下载新版本".into()),
            Some(InstallMethodType::Custom) => {
                if install_info.update_cmd.is_empty() {
                    return Err("此工具不支持自动更新，请手动下载新版本".into());
                }
                #[cfg(windows)]
                {
                    let mut cmd = std::process::Command::new("cmd");
                    cmd.suppress_console()
                        .args(["/C", &install_info.update_cmd]);
                    Self::execute_command_windows(&mut cmd).await
                }
                #[cfg(not(windows))]
                {
                    let mut parts = install_info.update_cmd.split_whitespace();
                    let program = parts.next().ok_or("Empty update command")?;
                    let args: Vec<&str> = parts.collect();
                    let mut cmd = tokio::process::Command::new(program);
                    cmd.suppress_console().args(&args);
                    Self::execute_command(&mut cmd).await
                }
            }
            #[cfg(windows)]
            Some(InstallMethodType::Brew) => unreachable!(),
            #[cfg(not(windows))]
            Some(InstallMethodType::Winget) | Some(InstallMethodType::Scoop) => {
                // Winget and Scoop are Windows-only
                Err("Winget/Scoop 仅在 Windows 上可用".into())
            }
            None => {
                if !install_info.update_cmd.is_empty() {
                    #[cfg(windows)]
                    {
                        let mut cmd = std::process::Command::new("cmd");
                        cmd.suppress_console()
                            .args(["/C", &install_info.update_cmd]);
                        Self::execute_command_windows(&mut cmd).await
                    }
                    #[cfg(not(windows))]
                    {
                        let mut parts = install_info.update_cmd.split_whitespace();
                        let program = parts.next().ok_or("Empty update command")?;
                        let args: Vec<&str> = parts.collect();
                        let mut cmd = tokio::process::Command::new(program);
                        cmd.suppress_console().args(&args);
                        Self::execute_command(&mut cmd).await
                    }
                } else {
                    Err("未检测到安装方式，请尝试重新安装".into())
                }
            }
        }
    }

    pub async fn uninstall(app: &AppType) -> Result<(), String> {
        let install_info = app.get_install_info().ok_or("未知工具类型")?;
        let detected_method = Self::detect_install_method(app).await;

        match detected_method {
            #[cfg(not(windows))]
            Some(InstallMethodType::Brew) => {
                let package = install_info
                    .methods
                    .iter()
                    .find_map(|m| {
                        if let InstallMethod::Brew { package } = m {
                            Some(package.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| app.name().to_lowercase());
                let mut cmd = tokio::process::Command::new("brew");
                cmd.suppress_console().arg("uninstall").arg(&package);
                Self::execute_command(&mut cmd).await
            }
            Some(InstallMethodType::Npm) => {
                let package = install_info
                    .methods
                    .iter()
                    .find_map(|m| {
                        if let InstallMethod::Npm { package } = m {
                            Some(package.clone())
                        } else {
                            None
                        }
                    })
                    .ok_or("未找到 npm 包信息")?;
                #[cfg(windows)]
                {
                    let mut cmd = std::process::Command::new("cmd");
                    cmd.suppress_console()
                        .args(["/C", "npm", "uninstall", "-g", &package]);
                    Self::execute_command_windows(&mut cmd).await
                }
                #[cfg(not(windows))]
                {
                    let mut cmd = tokio::process::Command::new("npm");
                    cmd.suppress_console().arg("uninstall").arg("-g").arg(&package);
                    Self::execute_command(&mut cmd).await
                }
            }
            #[cfg(windows)]
            Some(InstallMethodType::Winget) => {
                let package = install_info
                    .methods
                    .iter()
                    .find_map(|m| match m {
                        InstallMethod::Brew { package } => Some(package.clone()),
                        InstallMethod::Npm { package } => Some(package.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| app.name().to_lowercase());
                let mut cmd = std::process::Command::new("winget");
                cmd.suppress_console()
                    .args(["uninstall", "--id", &package, "-e"]);
                Self::execute_command_windows(&mut cmd).await
            }
            #[cfg(windows)]
            Some(InstallMethodType::Scoop) => {
                let package = install_info
                    .methods
                    .iter()
                    .find_map(|m| match m {
                        InstallMethod::Brew { package } => Some(package.clone()),
                        InstallMethod::Npm { package } => Some(package.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| app.name().to_lowercase());
                let mut cmd = std::process::Command::new("scoop");
                cmd.suppress_console().args(["uninstall", &package]);
                Self::execute_command_windows(&mut cmd).await
            }
            _ => Err("此工具不支持自动卸载，请手动删除".into()),
        }
    }

    #[cfg(not(windows))]
    async fn execute_command(cmd: &mut tokio::process::Command) -> Result<(), String> {
        let output = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| format!("执行命令失败: {}", e))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stderr.is_empty() {
                Err(format!("命令执行失败: {}", stdout))
            } else {
                Err(format!("执行错误: {}", stderr))
            }
        }
    }

    #[cfg(windows)]
    async fn execute_command_windows(cmd: &mut std::process::Command) -> Result<(), String> {
        cmd.suppress_console();
        let output = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| format!("执行命令失败: {}", e))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stderr.is_empty() {
                Err(format!("命令执行失败: {}", stdout))
            } else {
                Err(format!("执行错误: {}", stderr))
            }
        }
    }
}

pub async fn build_tool_info(app: &AppType) -> Option<crate::commands::tool_manager::ToolInfo> {
    let install_info = app.get_install_info()?;
    let installed = ToolManagerService::is_installed(app).await;

    let methods: Vec<crate::commands::tool_manager::ToolMethodInfo> = install_info
        .methods
        .iter()
        .enumerate()
        .map(|(index, method)| {
            let method_type = match method {
                InstallMethod::Brew { .. } => "brew",
                InstallMethod::Npm { .. } => "npm",
                InstallMethod::Curl { .. } => "curl",
                InstallMethod::Custom { .. } => "custom",
                InstallMethod::Download { .. } => "download",
            };
            crate::commands::tool_manager::ToolMethodInfo {
                index,
                method_type: method_type.to_string(),
                name: method.display_name().to_string(),
                package: match method {
                    InstallMethod::Brew { package } => Some(package.clone()),
                    InstallMethod::Npm { package } => Some(package.clone()),
                    _ => None,
                },
                url: match method {
                    InstallMethod::Curl { url } => Some(url.clone()),
                    InstallMethod::Download { url } => Some(url.clone()),
                    _ => None,
                },
                command: method.display_command(),
                needs_confirm: method.needs_confirm(),
            }
        })
        .collect();

    let detected_method = if installed {
        ToolManagerService::detect_install_method(app)
            .await
            .map(|m| m.display_name().to_string())
    } else {
        None
    };

    Some(crate::commands::tool_manager::ToolInfo {
        app_type: app.name().to_string(),
        name: install_info.name,
        installed,
        version: None,
        latest_version: None,
        detected_method,
        methods,
        homepage: install_info.homepage,
    })
}
