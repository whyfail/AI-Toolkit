use crate::credits::types::Credentials;
use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use base64::Engine;
use serde_json::{json, Value};
use sha2::{Digest, Sha512};
use std::path::{Path, PathBuf};

const AUTH_KEY: &str = "iCubeAuthInfo://icube.cloudide";
const DEVICE_PREFIX: &str = "iCubeAuthInfo://icube-dc:";

// Public byteCrypto format constants from TRAE CN 3.4.2's packaged
// out/main.js, vs/base/common/byteCrypto.js (line 1192). These are the XORs
// of the bundled ule/dle and cle/lle tables, not user credentials or keys.
const AES_HEADER: [u8; 6] = [116, 99, 5, 16, 0, 0];
const PRIVATE_HEADER: [u8; 6] = [18, 57, 32, 32, 2, 3];
const AES_FORMAT: [u8; 64] = [
    77, 212, 194, 230, 184, 49, 98, 9, 14, 82, 179, 199, 166, 115, 59, 164, 28, 178, 70, 43, 130,
    154, 181, 138, 25, 107, 57, 219, 87, 23, 117, 36, 244, 155, 175, 127, 8, 232, 214, 141, 38,
    167, 46, 55, 193, 169, 90, 47, 31, 5, 165, 24, 146, 174, 242, 148, 151, 50, 182, 42, 56, 170,
    221, 88,
];
const PRIVATE_FORMAT: [u8; 64] = [
    73, 12, 194, 18, 146, 176, 93, 12, 192, 108, 203, 233, 31, 185, 46, 33, 181, 167, 197, 56, 129,
    29, 200, 94, 39, 169, 110, 40, 16, 78, 28, 246, 156, 113, 102, 239, 171, 35, 40, 7, 57, 13,
    167, 138, 139, 145, 228, 156, 68, 180, 154, 67, 254, 185, 65, 161, 228, 111, 38, 86, 230, 19,
    228, 123,
];

fn decode_blob(encoded: &str) -> Result<Vec<u8>, String> {
    let mut bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| "TRAE CN 桌面凭据不是有效的 Base64".to_string())?;
    if bytes.len() < 54 || (bytes.len() - 38) % 16 != 0 {
        return Err("TRAE CN 桌面凭据长度无效".into());
    }
    let format = if bytes.starts_with(&AES_HEADER) {
        &AES_FORMAT
    } else if bytes.starts_with(&PRIVATE_HEADER) {
        &PRIVATE_FORMAT
    } else {
        return Err("TRAE CN 桌面凭据加密格式不支持".into());
    };
    // SHA512(SHA512(seed32) || format64), then AES-128-CBC/PKCS7.
    let seed_hash = Sha512::digest(&bytes[6..38]);
    let mut derivation = Sha512::new();
    derivation.update(seed_hash);
    derivation.update(format);
    let derived = derivation.finalize();
    let plaintext =
        cbc::Decryptor::<aes::Aes128>::new_from_slices(&derived[..16], &derived[16..32])
            .map_err(|_| "TRAE CN 桌面凭据密钥格式无效".to_string())?
            .decrypt_padded_mut::<Pkcs7>(&mut bytes[38..])
            .map_err(|_| "TRAE CN 桌面凭据解密失败".to_string())?;
    if plaintext.len() < 64 {
        return Err("TRAE CN 桌面凭据校验信息缺失".into());
    }
    let (checksum, body) = plaintext.split_at(64);
    if checksum != Sha512::digest(body).as_slice() {
        return Err("TRAE CN 桌面凭据完整性校验失败".into());
    }
    Ok(body.to_vec())
}

fn decode_json(encoded: &str) -> Result<Value, String> {
    serde_json::from_slice(&decode_blob(encoded)?)
        .map_err(|_| "TRAE CN 桌面凭据内容不是有效 JSON".into())
}

fn string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)?
        .as_str()
        .filter(|text| !text.trim().is_empty())
        .map(str::to_owned)
}

fn expiration(value: &Value, key: &str) -> Result<Option<i64>, String> {
    let Some(raw) = value.get(key).filter(|raw| !raw.is_null()) else {
        return Ok(None);
    };
    let date = raw
        .as_str()
        .ok_or_else(|| "TRAE CN 桌面凭据有效期格式无效".to_string())?;
    chrono::DateTime::parse_from_rfc3339(date)
        .map(|date| Some(date.timestamp_millis()))
        .map_err(|_| "TRAE CN 桌面凭据有效期格式无效".to_string())
}

fn personal_api_base(host: &str) -> Result<&'static str, String> {
    let url = reqwest::Url::parse(host).map_err(|_| "TRAE CN 桌面凭据账号区域无效".to_string())?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
    {
        return Err("TRAE CN 桌面凭据账号区域无效".into());
    }
    match url.host_str() {
        Some("api.trae.cn" | "www.trae.cn" | "trae.cn") => Ok("https://api.trae.cn"),
        Some("api.trae.com.cn" | "www.trae.com.cn" | "trae.com.cn") => {
            Ok("https://api.trae.com.cn")
        }
        _ => Err("TRAE CN 桌面凭据不属于支持的国内官方账号区域".into()),
    }
}

fn credentials_from_storage(
    storage: &Value,
    metadata: Option<&Value>,
) -> Result<Option<Credentials>, String> {
    let Some(encoded) = storage.get(AUTH_KEY) else {
        return Ok(None);
    };
    let encoded = encoded
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "TRAE CN 桌面登录凭据为空或格式无效".to_string())?;
    let info = decode_json(encoded)?;
    credentials_from_info(&info, storage, metadata).map(Some)
}

fn credentials_from_info(
    info: &Value,
    storage: &Value,
    metadata: Option<&Value>,
) -> Result<Credentials, String> {
    let account = info
        .get("account")
        .filter(|account| account.is_object())
        .ok_or_else(|| "TRAE CN 桌面凭据缺少账号信息".to_string())?;
    if string(account, "scope").as_deref() != Some("marscode") {
        return Err("TRAE CN 当前桌面账号不是受支持的个人账号".into());
    }
    let access_token =
        string(&info, "token").ok_or_else(|| "TRAE CN 桌面凭据缺少访问令牌".to_string())?;
    let user_id =
        string(&info, "userId").ok_or_else(|| "TRAE CN 桌面凭据缺少账号身份".to_string())?;
    let host = string(&info, "host").ok_or_else(|| "TRAE CN 桌面凭据缺少账号区域".to_string())?;
    let expires_at = expiration(&info, "expiredAt")?;
    let mut extra = json!({
        "credentialSource": "desktop", "scope": "marscode", "apiBase": personal_api_base(&host)?,
        "account": account, "label": string(account, "username").unwrap_or_else(|| format!("TRAE CN {user_id}")),
        "platformCode": "IDE_PC"
    });
    if let Some(region) = info.get("userRegion") {
        extra["userRegion"] = region.clone();
    }
    if let Some(expiry) = expiration(&info, "refreshExpiredAt")? {
        extra["refreshExpiresAt"] = json!(expiry);
    }
    if let Some(released) = expiration(&info, "tokenReleaseAt")? {
        extra["tokenReleaseAt"] = json!(released);
    }
    if let Some(metadata) = metadata {
        // package.version is the VS Code engine, not TRAE's IDEVersion.
        if let Some(version) = string(metadata, "appVersion") {
            extra["ideVersion"] = json!(version);
        }
        if let Some(quality) = string(metadata, "quality") {
            if let Some(client) = metadata
                .pointer("/iCubeApp/authConfig/TRAE")
                .and_then(|clients| string(clients, &quality))
            {
                extra["clientId"] = json!(client);
            }
        }
    }
    // QZ/KTe in packaged main.js read this existing machine identifier. Never
    // derive a replacement from a hostname or invent an OAuth device identity.
    if let Some(machine) = string(storage, "telemetry.machineId") {
        extra["machineId"] = json!(machine);
    }
    let mut device_id = None;
    let devices: Vec<_> = storage
        .as_object()
        .into_iter()
        .flat_map(|storage| storage.iter())
        .filter_map(|(key, value)| {
            key.strip_prefix(DEVICE_PREFIX)
                .filter(|id| !id.is_empty())
                .map(|id| (id, value))
        })
        .collect();
    // userStorage.getDeviceKeyPair resolves `${DEVICE_PREFIX}${DeviceID}`.
    // Without a server-provided binding, multiple stored devices are ambiguous.
    if devices.len() == 1 {
        if let Some(encoded) = devices[0].1.as_str() {
            if let Ok(pair) = decode_json(encoded) {
                if let (Some(private), Some(public)) = (
                    string(&pair, "privateKeyPEM"),
                    string(&pair, "publicKeyPEM"),
                ) {
                    device_id = Some(devices[0].0.to_string());
                    extra["devicePrivateKey"] = json!(private);
                    extra["devicePublicKey"] = json!(public);
                }
            }
        }
    }
    Ok(Credentials {
        access_token,
        refresh_token: string(&info, "refreshToken"),
        user_id: Some(user_id),
        device_id,
        expires_at,
        extra,
    })
}

fn cn_storage_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "无法获取 TRAE CN 用户目录".to_string())?;
    #[cfg(target_os = "macos")]
    let root = home.join("Library/Application Support/Trae CN");
    #[cfg(target_os = "windows")]
    let root = std::env::var("APPDATA")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join("AppData/Roaming"))
        .join("Trae CN");
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let root = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"))
        .join("Trae CN");
    Ok(root.join("User/globalStorage/storage.json"))
}

fn cn_product_metadata() -> Option<Value> {
    #[cfg(target_os = "macos")]
    let path = PathBuf::from("/Applications/Trae CN.app/Contents/Resources/app/product.json");
    #[cfg(target_os = "windows")]
    let path =
        crate::services::local_detection::find_app_executable_windows(&crate::mcp::AppType::TraeCn)?
            .parent()?
            .join("resources/app/product.json");
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    return None;
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn read_credentials(path: &Path, metadata: Option<&Value>) -> Result<Option<Credentials>, String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("无法读取 TRAE CN 桌面登录凭据文件".into()),
    };
    let storage: Value = serde_json::from_slice(&bytes)
        .map_err(|_| "TRAE CN 桌面登录凭据文件格式无效".to_string())?;
    credentials_from_storage(&storage, metadata)
}

pub(super) fn local_credentials() -> Result<Option<Credentials>, String> {
    read_credentials(&cn_storage_path()?, cn_product_metadata().as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Generated by TRAE CN 3.4.2's original packaged XHe/KHe WebCrypto encoder,
    // evaluated in an isolated Node VM with entirely synthetic session data.
    // PRIVATE_FIXTURE uses the same packaged tle/zHe with its AES_PRIVATE format.
    const AUTH_FIXTURE: &str = "dGMFEAAAFpCmOk1uo2baCNrE7fJNCtvaDYvgdNQ6i+RGjSrV2nHLdB7fL6m38Yfz4OkDAzB1Gd4F8mo6nLWCsohHKI50ao7iqIcItdn0wWHtvctOwe3XyUZw75J5jguRCaIzTk9Kye6bJsGybNPzKIWC+gyV4SqxEbTK1VaARW3CKuJcHY0XXbHw/OiORQ82VkLVFf76IR+UXNUXm8wsQBXtUHJAIVURVd5zoMhgrp7b+0p8K+zITOONNQdpTlacMnFPo0OOsuxMthSktiQVr6OZc08j6ZKrCcHPkIwDLDv3LjZd/q3Zn0U+vvE8DpAZalIQ1s65DqZbYX38+yjk6uHy5kgnBe+mkC6m2MgEOcPhmEmCshP7OANzWbvXTCbo02xrGEryXugO7DKfFNnrYPz0ODLGRtaF40t8wnn00Hcp6M3IYgHu9NClwLofCOti2XPt/mrQpJke5yH5FMc2dT3URaG7iNNJaxw4jvXVtEdWJNh4bvqF+h8qOtYoEo2Zma43XNqptbe8GIm9rlGahd3Uj59j/h02366ctYcbbD/GXvIWJkzeW6lfZsUQ3eQy1+tkKWpR1oQTsTpMFdUjpfLAU17VEiALMwb5tg/XMpv68gYfUO5HcAqaTEit+cvCxWa0+3X0OX9eqhCunF4hNzUJnkir6Q==";
    const DEVICE_FIXTURE: &str = "dGMFEAAAlM9whmwqg0pE+m92kKj6gdDRvwcokrbH9tK5nY6iEcs7s4om9OM88k+bTYN1Ntj4tI+9zcK+z9SRGSz0xudvqxJj1v2Vgv2wKQQ0HLW7+jgbasCV8wT8o0OCrplnoiVbgNKI3I2ClkX3pbB+FUYQxlqHO8hFiy2MCAYn1Hzma3koDW6WGpHG8VICLV8mhd8jG5FrhwFjzkrkYQzhIjvvGPe89kBkNKd7k4RGLg917G11NzG7AzF040ZypZnUHfr+Lx3lFLdWN4KgJ1znHJ4wtQ==";
    const PRIVATE_FIXTURE: &str = "EjkgIAIDz3Qrd6LJRm2aLXeo1tQqpo/zf/4VImRy6FWPQhzaqo78HJ0OXwaJwyrBd+YzDu6hhp+RxwL0mB7ShVgDzaTsvv5j02OjScEUCpZn7JK/RVqS5e5ZUmOK8cd3bzd/V7Xv3nXyS8PvG0Ainw7xpRrNecZs/zi1jMIbYfYBV7vdhRyNMd9AOWj1JackiM/j0UCizOX2vY9z8yiJ5XZaCqESbdhjnJNBH0j5kH3zaHVNHTbvJhF3cIPO69vpRh+fuibHAMjGXjzAFAAhQc8k+IHl8y+Ogai5FYF1hVn6e1qpFuvntjYlFpGT+8drZaXaOIKdM7iDT7MslyjqHpROkWgAmhOebTQjT6YxUtH6YzMNNbNg2D7sXMOFHgmu7U2xR3954HvUlC7JcHLLpsADOsoDMoyDHZsNghmCBTZOlWxB7eH0PzxYCDID4EOclMgoz2+Ea+L0iLdtxUwGZ1nc1EqGSOX+DWpBApBoy4QJCCblKgp7vTsvpXwmNyVlXB0BoGn+PvwJIfqvTrg+mj9GnmEfu/KJrb0rKjjDsEAlNpSegjVP/WX+9mLVZ8k8M+rIwU8Ud1OB3pTecZoM9xLaOpDsT8tBAijzLsWu2iEosfGLUsv9skhYmxw6GccVf2vyRf38Pe/aSkCR5yvoTGHHa1x9pQ==";

    fn metadata() -> Value {
        json!({"appVersion":"3.4.2", "version":"1.107.1", "quality":"stable",
            "iCubeApp":{"authConfig":{"TRAE":{"stable":"public-test-client"}}}})
    }

    #[test]
    fn desktop_credentials_interoperate_with_packaged_encoder_and_bound_device_storage() {
        let storage = json!({
            (AUTH_KEY): AUTH_FIXTURE,
            "telemetry.machineId":"fixture-existing-machine",
            "iCubeAuthInfo://icube-dc:fixture-existing-device": DEVICE_FIXTURE
        });
        let credentials = credentials_from_storage(&storage, Some(&metadata()))
            .unwrap()
            .unwrap();
        assert_eq!(credentials.access_token, "fixture-access-token");
        assert_eq!(
            credentials.refresh_token.as_deref(),
            Some("fixture-refresh-token")
        );
        assert_eq!(credentials.user_id.as_deref(), Some("fixture-user"));
        assert_eq!(
            credentials.device_id.as_deref(),
            Some("fixture-existing-device")
        );
        assert_eq!(credentials.expires_at, Some(1_791_979_200_000));
        assert_eq!(credentials.extra["refreshExpiresAt"], 1_794_657_600_000i64);
        assert_eq!(credentials.extra["tokenReleaseAt"], 1_791_975_600_000i64);
        assert_eq!(credentials.extra["credentialSource"], "desktop");
        assert_eq!(credentials.extra["scope"], "marscode");
        assert_eq!(credentials.extra["ideVersion"], "3.4.2");
        assert_eq!(credentials.extra["clientId"], "public-test-client");
        assert_eq!(credentials.extra["label"], "Fixture User");
        assert_eq!(credentials.extra["machineId"], "fixture-existing-machine");
        assert_eq!(
            credentials.extra["devicePrivateKey"],
            "fixture-private-key-not-a-real-key"
        );
        assert_eq!(
            credentials.extra["devicePublicKey"],
            "fixture-public-key-not-a-real-key"
        );
        assert_eq!(
            decode_json(PRIVATE_FIXTURE).unwrap(),
            decode_json(AUTH_FIXTURE).unwrap()
        );
    }

    #[test]
    fn corrupt_prefix_length_padding_or_checksum_never_yields_credentials() {
        assert!(decode_blob("").is_err());
        assert!(decode_blob("not base64").is_err());
        assert!(decode_blob(&base64::engine::general_purpose::STANDARD.encode([0u8; 53])).is_err());
        let original = base64::engine::general_purpose::STANDARD
            .decode(AUTH_FIXTURE)
            .unwrap();
        let mut bad_prefix = original.clone();
        bad_prefix[0] ^= 1;
        assert!(
            decode_blob(&base64::engine::general_purpose::STANDARD.encode(bad_prefix))
                .unwrap_err()
                .contains("格式不支持")
        );
        let mut bad_checksum = original.clone();
        bad_checksum[38] ^= 1;
        assert!(
            decode_blob(&base64::engine::general_purpose::STANDARD.encode(bad_checksum))
                .unwrap_err()
                .contains("完整性")
        );
        let mut bad_padding = original;
        *bad_padding.last_mut().unwrap() ^= 1;
        assert!(
            decode_blob(&base64::engine::general_purpose::STANDARD.encode(bad_padding)).is_err()
        );
    }

    #[test]
    fn invalid_fields_expiry_and_unsupported_account_are_rejected_without_secret_errors() {
        let original = decode_json(AUTH_FIXTURE).unwrap();
        for field in ["token", "userId", "host", "account"] {
            let mut info = original.clone();
            info.as_object_mut().unwrap().remove(field);
            let error = credentials_from_info(&info, &json!({}), None)
                .err()
                .unwrap();
            assert!(!error.contains("fixture-access-token"));
        }
        let mut info = original.clone();
        info["expiredAt"] = json!("invalid-date");
        assert!(credentials_from_info(&info, &json!({}), None).is_err());
        info = original;
        info["account"]["scope"] = json!("saas");
        assert!(credentials_from_info(&info, &json!({}), None).is_err());
        assert!(credentials_from_storage(&json!({}), None)
            .unwrap()
            .is_none());
        assert!(credentials_from_storage(&json!({(AUTH_KEY): ""}), None).is_err());
    }

    #[test]
    fn missing_or_ambiguous_device_keys_do_not_create_an_identity() {
        let mut storage = json!({(AUTH_KEY):AUTH_FIXTURE});
        let credentials = credentials_from_storage(&storage, None).unwrap().unwrap();
        assert!(credentials.device_id.is_none());
        assert!(credentials.extra.get("machineId").is_none());
        assert!(credentials.extra.get("clientId").is_none());
        assert!(credentials.extra.get("ideVersion").is_none());
        storage["iCubeAuthInfo://icube-dc:first"] = json!(DEVICE_FIXTURE);
        storage["iCubeAuthInfo://icube-dc:second"] = json!(DEVICE_FIXTURE);
        let credentials = credentials_from_storage(&storage, None).unwrap().unwrap();
        assert!(credentials.device_id.is_none());
        assert!(credentials.extra.get("devicePrivateKey").is_none());
        storage
            .as_object_mut()
            .unwrap()
            .remove("iCubeAuthInfo://icube-dc:second");
        storage["iCubeAuthInfo://icube-dc:first"] = json!("corrupt encoded pair");
        assert!(credentials_from_storage(&storage, None)
            .unwrap()
            .unwrap()
            .device_id
            .is_none());
    }

    #[test]
    fn official_cn_hosts_are_mapped_without_accepting_arbitrary_urls() {
        assert_eq!(
            personal_api_base("https://www.trae.cn").unwrap(),
            "https://api.trae.cn"
        );
        assert_eq!(
            personal_api_base("https://api.trae.com.cn").unwrap(),
            "https://api.trae.com.cn"
        );
        for host in [
            "http://api.trae.cn",
            "https://api.trae.cn.evil.test",
            "https://evil.trae.cn",
            "https://api.trae.cn:8443",
            "https://user@api.trae.cn",
            "https://api.trae.ai",
            "https://api.trae.cn/path",
        ] {
            assert!(personal_api_base(host).is_err());
        }
    }

    #[test]
    fn credential_file_reader_only_treats_absent_files_or_absent_login_keys_as_missing() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("storage.json");
        assert!(read_credentials(&path, None).unwrap().is_none());
        std::fs::write(&path, "{}").unwrap();
        assert!(read_credentials(&path, None).unwrap().is_none());
        std::fs::write(&path, "invalid-json").unwrap();
        assert!(read_credentials(&path, None).is_err());
        std::fs::write(&path, json!({(AUTH_KEY):AUTH_FIXTURE}).to_string()).unwrap();
        assert!(read_credentials(&path, None).unwrap().is_some());
    }
}
