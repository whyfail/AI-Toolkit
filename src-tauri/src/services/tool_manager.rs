use crate::mcp::{AppType, InstallMethod};
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

fn is_app_installed_windows(app: &AppType) -> bool {
    if !cfg!(windows) {
        return false;
    }
    let app_name = match app {
        AppType::Trae => "Trae.exe",
        AppType::TraeCn => "Trae.exe",
        AppType::TraeSoloCn => "Trae.exe",
        _ => return false,
    };
    let paths = [
        std::env::var("ProgramFiles").ok(),
        std::env::var("LOCALAPPDATA").ok(),
        std::env::var("USERPROFILE").ok(),
    ];
    for base in paths.into_iter().flatten() {
        let path = std::path::PathBuf::from(base);
        if path.join(app_name).exists() {
            return true;
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
        .args(["-c", "npm config get prefix"])
        .output()
        .map_err(|e| format!("获取 npm prefix 失败: {}", e))?;

    if !output.status.success() {
        return Ok(());
    }

    let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let bin_path = format!("{}/bin", prefix);

    let zshrc_path = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".zshrc");
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
        .args(["/C", "npm config get prefix"])
        .output()
        .map_err(|e| format!("获取 npm prefix 失败: {}", e))?;

    if !output.status.success() {
        return Ok(());
    }

    let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let bin_path = format!("{}\\\\npm", prefix.replace('/', "\\"));

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

    if !content.contains(&bin_path) && !content.contains(&prefix) {
        let export_line = format!("\n$env:PATH = \"{};$env:PATH\"\n", bin_path);
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
fn which_binary(binary: &str) -> Option<String> {
    let output = std::process::Command::new("where")
        .arg(binary)
        .output()
        .ok()?;
    if output.status.success() {
        String::from_utf8_lossy(&output.stdout).lines().next().map(|s| s.to_string())
    } else {
        None
    }
}

#[cfg(not(windows))]
fn which_binary(binary: &str) -> Option<String> {
    let output = std::process::Command::new("sh")
        .args(["-c", &format!("which {}", binary)])
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
        .args(["/C", &format!("npm list -g {} 2>nul", package)])
        .output()
        .ok()?;
    Some(output.status.success() && String::from_utf8_lossy(&output.stdout).contains(package))
}

#[cfg(not(windows))]
fn npm_list_global(package: &str) -> Option<bool> {
    let output = std::process::Command::new("sh")
        .args(["-c", &format!("npm list -g {} 2>/dev/null", package)])
        .output()
        .ok()?;
    Some(output.status.success() && String::from_utf8_lossy(&output.stdout).contains(package))
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

        let install_info = app.get_install_info()?;

        if let Some(method) = install_info.methods.iter().find(|m| matches!(m, InstallMethod::Npm { .. })) {
            if let InstallMethod::Npm { package } = method {
                if npm_list_global(package).unwrap_or(false) {
                    return Some(InstallMethodType::Npm);
                }
            }
        }

        #[cfg(windows)]
        {
            if let Some(path) = which_binary("winget") {
                if !path.is_empty() {
                    let binary_name = app.name();
                    if which_binary(&binary_name).is_some() {
                        return Some(InstallMethodType::Winget);
                    }
                }
            }
            if let Some(path) = which_binary("scoop") {
                if !path.is_empty() {
                    let binary_name = app.name();
                    if which_binary(&binary_name).is_some() {
                        return Some(InstallMethodType::Scoop);
                    }
                }
            }
        }

        #[cfg(not(windows))]
        {
            if let Some(path) = which_binary("brew") {
                if !path.is_empty() {
                    let binary_name = app.name();
                    if which_binary(&binary_name).is_some() {
                        return Some(InstallMethodType::Brew);
                    }
                }
            }
        }

        let binary_name = app.name();
        if which_binary(&binary_name).is_some() {
            #[cfg(windows)]
            return Some(InstallMethodType::Winget);
            #[cfg(not(windows))]
            return Some(InstallMethodType::Curl);
        }

        None
    }

    pub async fn get_version(app: &AppType) -> Option<String> {
        let install_info = app.get_install_info()?;
        if install_info.version_cmd.is_empty() {
            return None;
        }

        #[cfg(windows)]
        {
            let output = std::process::Command::new("cmd")
                .args(["/C", &install_info.version_cmd])
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

        #[cfg(not(windows))]
        {
            let output = tokio::process::Command::new("sh")
                .args(["-c", &install_info.version_cmd])
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
                        .args(["/C", &format!("npm view {} version", package)])
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
                #[cfg(not(windows))]
                {
                    let output = tokio::process::Command::new("sh")
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

            if let InstallMethod::Brew { package } = method {
                #[cfg(not(windows))]
                {
                    let output = tokio::process::Command::new("sh")
                        .args(["-c", &format!("brew info {} 2>/dev/null | head -1", package)])
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .output()
                        .await
                        .ok()?;
                    if output.status.success() {
                        let output_str = String::from_utf8_lossy(&output.stdout);
                        if let Some(version) = output_str.split_whitespace().nth(1) {
                            let version = version.trim_start_matches('[').trim_end_matches(',');
                            if !version.is_empty() && version.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                                return Some(version.to_string());
                            }
                        }
                    }
                }
            }
        }

        None
    }

    pub async fn install(app: &AppType, method: &InstallMethod) -> Result<(), String> {
        match method {
            #[cfg(not(windows))]
            InstallMethod::Brew { package } => {
                let mut cmd = tokio::process::Command::new("brew");
                cmd.arg("install").arg(package);
                Self::execute_command(&mut cmd).await
            }
            #[cfg(windows)]
            InstallMethod::Brew { package } => {
                let mut cmd = std::process::Command::new("brew");
                cmd.arg("install").arg(package);
                Self::execute_command_windows(&mut cmd).await
            }
            InstallMethod::Npm { package } => {
                #[cfg(windows)]
                {
                    let mut cmd = std::process::Command::new("cmd");
                    cmd.args(["/C", "npm", "install", "-g", package]);
                    Self::execute_command_windows(&mut cmd).await?;
                }
                #[cfg(not(windows))]
                {
                    let mut cmd = tokio::process::Command::new("npm");
                    cmd.arg("install").arg("-g").arg(package);
                    Self::execute_command(&mut cmd).await?;
                }
                ensure_npm_path_in_shell_config()?;
                Ok(())
            }
            #[cfg(not(windows))]
            InstallMethod::Curl { url } => {
                let script = format!("curl -fsSL {} | bash", url);
                let mut cmd = tokio::process::Command::new("sh");
                cmd.arg("-c").arg(&script);
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
                    cmd.args(["/C", command]);
                    Self::execute_command_windows(&mut cmd).await
                }
                #[cfg(not(windows))]
                {
                    let mut parts = command.split_whitespace();
                    let program = parts.next().ok_or("Empty command")?;
                    let args: Vec<&str> = parts.collect();
                    let mut cmd = tokio::process::Command::new(program);
                    cmd.args(&args);
                    Self::execute_command(&mut cmd).await
                }
            }
            InstallMethod::Download { url } => {
                Err(format!("请手动下载安装: {}", url))
            }
        }
    }

    pub async fn update(app: &AppType) -> Result<(), String> {
        let install_info = app.get_install_info().ok_or("Unknown app type")?;

        let detected_method = Self::detect_install_method(app).await;

        match detected_method {
            #[cfg(not(windows))]
            Some(InstallMethodType::Brew) => {
                let package = install_info.methods.iter()
                    .find_map(|m| {
                        if let InstallMethod::Brew { package } = m {
                            Some(package.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| app.name().to_lowercase());
                let mut cmd = tokio::process::Command::new("brew");
                cmd.arg("upgrade").arg(&package);
                Self::execute_command(&mut cmd).await
            }
            #[cfg(windows)]
            Some(InstallMethodType::Winget) => {
                let package = install_info.methods.iter()
                    .find_map(|m| {
                        match m {
                            InstallMethod::Brew { package } => Some(package.clone()),
                            InstallMethod::Npm { package } => Some(package.clone()),
                            _ => None,
                        }
                    })
                    .unwrap_or_else(|| app.name().to_lowercase());
                let mut cmd = std::process::Command::new("winget");
                cmd.args(["upgrade", "--id", &package, "-e"]);
                Self::execute_command_windows(&mut cmd).await
            }
            #[cfg(windows)]
            Some(InstallMethodType::Scoop) => {
                let package = install_info.methods.iter()
                    .find_map(|m| {
                        match m {
                            InstallMethod::Brew { package } => Some(package.clone()),
                            InstallMethod::Npm { package } => Some(package.clone()),
                            _ => None,
                        }
                    })
                    .unwrap_or_else(|| app.name().to_lowercase());
                let mut cmd = std::process::Command::new("scoop");
                cmd.args(["update", &package]);
                Self::execute_command_windows(&mut cmd).await
            }
            Some(InstallMethodType::Npm) => {
                let package = install_info.methods.iter()
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
                    cmd.args(["/C", "npm", "install", "-g", &format!("{}@latest", package)]);
                    Self::execute_command_windows(&mut cmd).await
                }
                #[cfg(not(windows))]
                {
                    let mut cmd = tokio::process::Command::new("npm");
                    cmd.arg("install").arg("-g").arg(format!("{}@latest", package));
                    Self::execute_command(&mut cmd).await
                }
            }
            #[cfg(not(windows))]
            Some(InstallMethodType::Curl) => {
                if let Some(url) = install_info.methods.iter()
                    .find_map(|m| {
                        if let InstallMethod::Curl { url } = m {
                            Some(url.clone())
                        } else {
                            None
                        }
                    }) {
                    let script = format!("curl -fsSL {} | bash", url);
                    let mut cmd = tokio::process::Command::new("sh");
                    cmd.arg("-c").arg(&script);
                    Self::execute_command(&mut cmd).await
                } else if !install_info.update_cmd.is_empty() {
                    let mut parts = install_info.update_cmd.split_whitespace();
                    let program = parts.next().ok_or("Empty update command")?;
                    let args: Vec<&str> = parts.collect();
                    let mut cmd = tokio::process::Command::new(program);
                    cmd.args(&args);
                    Self::execute_command(&mut cmd).await
                } else {
                    Err("此工具不支持自动更新，请手动下载新版本".into())
                }
            }
            #[cfg(windows)]
            Some(InstallMethodType::Curl) => {
                Err("此工具不支持自动更新，请手动下载新版本".into())
            }
            Some(InstallMethodType::Custom) => {
                if install_info.update_cmd.is_empty() {
                    return Err("此工具不支持自动更新，请手动下载新版本".into());
                }
                #[cfg(windows)]
                {
                    let mut cmd = std::process::Command::new("cmd");
                    cmd.args(["/C", &install_info.update_cmd]);
                    Self::execute_command_windows(&mut cmd).await
                }
                #[cfg(not(windows))]
                {
                    let mut parts = install_info.update_cmd.split_whitespace();
                    let program = parts.next().ok_or("Empty update command")?;
                    let args: Vec<&str> = parts.collect();
                    let mut cmd = tokio::process::Command::new(program);
                    cmd.args(&args);
                    Self::execute_command(&mut cmd).await
                }
            }
            Some(InstallMethodType::Winget) | Some(InstallMethodType::Scoop) => {
                // These should not be detected on non-Windows, but just in case
                Err("Windows-specific installation method detected".into())
            }
            None => {
                if !install_info.update_cmd.is_empty() {
                    #[cfg(windows)]
                    {
                        let mut cmd = std::process::Command::new("cmd");
                        cmd.args(["/C", &install_info.update_cmd]);
                        Self::execute_command_windows(&mut cmd).await
                    }
                    #[cfg(not(windows))]
                    {
                        let mut parts = install_info.update_cmd.split_whitespace();
                        let program = parts.next().ok_or("Empty update command")?;
                        let args: Vec<&str> = parts.collect();
                        let mut cmd = tokio::process::Command::new(program);
                        cmd.args(&args);
                        Self::execute_command(&mut cmd).await
                    }
                } else {
                    Err("未检测到安装方式，请尝试重新安装".into())
                }
            }
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

    Some(crate::commands::tool_manager::ToolInfo {
        app_type: app.name().to_string(),
        name: install_info.name,
        installed,
        version: None,
        latest_version: None,
        detected_method: None,
        methods,
        homepage: install_info.homepage,
    })
}