//! Read the official CN client's device proof without starting its desktop UI.
use super::*;
use std::{path::PathBuf, process::Stdio};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

pub(super) fn helper_path() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    for root in crate::services::local_detection::mac_application_roots() {
        let path = root.join("Qoder CN.app/Contents/Resources/umid/runtime-info");
        if path.is_file() {
            return Some(path);
        }
    }
    // The CN CLI also installs this helper; never use the international cache.
    let directory = dirs::home_dir()?.join(".qoder-cn/.bin");
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
) -> Result<reqwest::header::HeaderMap, ProviderError> {
    let mut command = tokio::process::Command::new(path);
    command.arg("0"); // Official runtime environment 0 = CN production.
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
    fn only_valid_native_device_proof_becomes_headers() {
        let headers = parse_proof(&json!({"machineToken": "native-proof", "machineCode": "device-code", "machineType": "desktop"})).unwrap();
        assert_eq!(headers["cosy-machinetoken"], "native-proof");
        assert!(parse_proof(&json!({"machineToken": "fake\nheader", "machineCode": "code", "machineType": "desktop"})).is_err());
        assert!(parse_proof(&json!({"machineCode": "code", "machineType": "desktop"})).is_err());
    }
    #[tokio::test]
    async fn broken_native_helper_reports_failure_before_claiming() {
        assert!(
            proof(PathBuf::from("/nonexistent-qoder-helper"), "test-user")
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
        let result = proof(path.clone(), "test-user").await;
        std::fs::remove_file(path).unwrap();
        assert_eq!(result.unwrap()["cosy-machinetoken"], "native-proof");
    }
}
