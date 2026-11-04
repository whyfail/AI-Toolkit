//! Read the matching official client's device proof without starting its desktop UI.
use super::*;
use std::{path::PathBuf, process::Stdio};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

fn cli_directory(region: CreditRegion) -> &'static str {
    match region { CreditRegion::Cn => ".qoder-cn/.bin", CreditRegion::Intl => ".qoder/.bin" }
}
fn runtime_environment(region: CreditRegion) -> &'static str {
    match region { CreditRegion::Cn => "0", CreditRegion::Intl => "4" }
}
pub(super) fn helper_path(region: CreditRegion) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    for root in crate::services::local_detection::mac_application_roots() {
        let app = match region { CreditRegion::Cn => "Qoder CN.app", CreditRegion::Intl => "Qoder.app" };
        let path = root.join(app).join("Contents/Resources/umid/runtime-info");
        if path.is_file() {
            return Some(path);
        }
    }
    // A helper and its cache belong to one account region. Never fall back
    // to the other region when the corresponding client is not installed.
    let directory = dirs::home_dir()?.join(cli_directory(region));
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        os => os,
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        "x86_64" => "x64",
        arch => arch,
    };
    let prefix = format!("runtime-info-{os}-{arch}-");
    let mut candidates: Vec<_> = std::fs::read_dir(directory)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with(&prefix))
        })
        .collect();
    candidates.sort_by_key(|path| path.metadata().and_then(|meta| meta.modified()).ok());
    candidates.pop()
}

pub(super) fn parse_proof(value: &Value) -> Result<reqwest::header::HeaderMap, ProviderError> {
    let mut headers = reqwest::header::HeaderMap::new();
    for (key, header) in [
        ("machineToken", "cosy-machinetoken"),
        ("machineCode", "cosy-machinecode"),
        ("machineType", "cosy-machinetype"),
    ] {
        let value = value[key]
            .as_str()
            .map(str::trim)
            .filter(|value| {
                !value.is_empty()
                    && value.len() <= 4096
                    && value.bytes().all(|c| (0x20..=0x7e).contains(&c))
            })
            .ok_or_else(|| ProviderError::new(502, "Qoder 本机设备凭据格式无效"))?;
        headers.insert(
            header,
            reqwest::header::HeaderValue::from_str(value)
                .map_err(|_| ProviderError::new(502, "Qoder 本机设备凭据格式无效"))?,
        );
    }
    Ok(headers)
}

pub(super) async fn proof(
    path: PathBuf,
    account: &str,
    region: CreditRegion,
) -> Result<reqwest::header::HeaderMap, ProviderError> {
    let mut command = tokio::process::Command::new(path);
    // Official qodercli 1.1.81 PAt(isCn): ONLINE=0, SINGAPORE=4.
    command.arg(runtime_environment(region));
    if cfg!(any(target_os = "macos", target_os = "windows")) {
        command.arg("--account-stdin");
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(target_os = "windows")]
    command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let mut child = command
        .spawn()
        .map_err(|_| ProviderError::new(502, "无法读取 Qoder 本机设备凭据"))?;
    let result = tokio::time::timeout(Duration::from_secs(25), async {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| ProviderError::new(502, "无法读取 Qoder 本机设备凭据"))?;
        if cfg!(any(target_os = "macos", target_os = "windows")) {
            stdin
                .write_all(format!("{}\n", json!({"account": account})).as_bytes())
                .await
                .map_err(|_| ProviderError::new(502, "无法读取 Qoder 本机设备凭据"))?;
        }
        drop(stdin);
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ProviderError::new(502, "无法读取 Qoder 本机设备凭据"))?;
        let mut line = Vec::new();
        BufReader::new(stdout.take(16_384))
            .read_until(b'\n', &mut line)
            .await
            .map_err(|_| ProviderError::new(502, "无法读取 Qoder 本机设备凭据"))?;
        let value: Value = serde_json::from_slice(&line)
            .map_err(|_| ProviderError::new(502, "Qoder 本机设备凭据格式无效"))?;
        parse_proof(&value)
    })
    .await
    .map_err(|_| ProviderError::new(504, "读取 Qoder 本机设备凭据超时"));
    // The helper may remain alive after emitting the first line.
    let _ = child.kill().await;
    result?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_helper_caches_and_environments_are_region_scoped() {
        assert_eq!(cli_directory(CreditRegion::Cn), ".qoder-cn/.bin");
        assert_eq!(cli_directory(CreditRegion::Intl), ".qoder/.bin");
        assert_eq!(runtime_environment(CreditRegion::Cn), "0");
        assert_eq!(runtime_environment(CreditRegion::Intl), "4");
    }
    #[test]
    fn only_valid_native_device_proof_becomes_headers() {
        let headers = parse_proof(&json!({"machineToken": "native-proof", "machineCode": "device-code", "machineType": "desktop"})).unwrap();
        assert_eq!(headers["cosy-machinetoken"], "native-proof");
        assert!(parse_proof(&json!({"machineToken": "fake\nheader", "machineCode": "code", "machineType": "desktop"})).is_err());
        assert!(parse_proof(&json!({"machineCode": "code", "machineType": "desktop"})).is_err());
    }
    #[tokio::test]
    async fn broken_native_helper_reports_failure_before_claiming() {
        assert!(
            proof(PathBuf::from("/nonexistent-qoder-helper"), "test-user", CreditRegion::Cn)
                .await
                .is_err()
        );
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn native_proof_is_read_without_starting_the_desktop() {
        use std::os::unix::fs::PermissionsExt;
        let path = std::env::temp_dir().join(format!("qoder-proof-test-{}", uuid::Uuid::new_v4()));
        std::fs::write(&path, "#!/bin/sh\nIFS= read -r account\nprintf '%s\\n' '{\"machineToken\":\"native-proof\",\"machineCode\":\"device-code\",\"machineType\":\"desktop\"}'\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let result = proof(path.clone(), "test-user", CreditRegion::Cn).await;
        std::fs::remove_file(path).unwrap();
        assert_eq!(result.unwrap()["cosy-machinetoken"], "native-proof");
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn international_proof_passes_singapore_environment_to_official_helper() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("runtime-info");
        std::fs::write(&path, "#!/bin/sh\n[ \"$1\" = 4 ] || exit 1\nIFS= read -r account\nprintf '%s\\n' '{\"machineToken\":\"native-proof\",\"machineCode\":\"device-code\",\"machineType\":\"desktop\"}'\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(proof(path, "synthetic-user", CreditRegion::Intl).await.unwrap()["cosy-machinetoken"], "native-proof");
    }
}
