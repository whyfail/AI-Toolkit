use parking_lot::Mutex;
use security_framework::{
    item::{ItemClass, ItemSearchOptions, Limit},
    os::macos::keychain::SecKeychain,
    passwords::{self, PasswordOptions},
};

const SERVICE: &str = "com.ai-toolkit.credits";
const ITEM_NOT_FOUND: i32 = -25300;
const USER_CANCELED: i32 = -128;
const AUTH_FAILED: i32 = -25293;
static ACCESS_GATE: Mutex<()> = Mutex::new(());

/// Keychain interaction is process-wide. Serialize every native keychain call,
/// including explicit TRAE imports, and restore the previous state on all exits.
pub(crate) fn access<T>(
    allow_ui: bool,
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let _gate = ACCESS_GATE.lock();
    let _silent = if !allow_ui
        && SecKeychain::user_interaction_allowed().map_err(|_| "无法检查钥匙串授权状态")?
    {
        Some(SecKeychain::disable_user_interaction().map_err(|_| "无法禁止后台钥匙串授权弹窗")?)
    } else {
        None
    };
    operation()
}

pub(super) fn read(id: &str, allow_ui: bool) -> Result<Option<Vec<u8>>, String> {
    access(allow_ui, || {
        match passwords::generic_password(PasswordOptions::new_generic_password(SERVICE, id)) {
            Ok(raw) => Ok(Some(raw)),
            Err(error) if error.code() == ITEM_NOT_FOUND => Ok(None),
            Err(error) if allow_ui && matches!(error.code(), USER_CANCELED | AUTH_FAILED) => {
                Err("钥匙串授权未完成，请再次点击当前按钮重试，并在弹窗中允许访问".into())
            }
            Err(_) if allow_ui => Err("无法读取钥匙串凭据，请检查钥匙串状态后重试".into()),
            Err(_) => Err("无法静默读取钥匙串凭据，请点击“刷新”授权或重新登录账号".into()),
        }
    })
}

pub(super) fn write(id: &str, raw: &[u8], allow_ui: bool) -> Result<(), String> {
    access(allow_ui, || {
        // SecItemAdd/Update never fetch the old password as keyring::set_secret does.
        passwords::set_generic_password(SERVICE, id, raw)
            .map_err(|_| "无法保存到系统凭据库，请检查凭据库权限".into())
    })
}

pub(super) fn remove(id: &str) -> Result<(), String> {
    access(false, || {
        match passwords::delete_generic_password(SERVICE, id) {
            Ok(()) => Ok(()),
            Err(error) if error.code() == ITEM_NOT_FOUND => Ok(()),
            Err(_) => Err("无法删除系统凭据库中的凭据".into()),
        }
    })
}

pub(super) fn legacy_accounts(id: &str) -> Result<Vec<String>, String> {
    access(false, || {
        // Enumerate names, never password data, to find all legacy generations.
        let result = ItemSearchOptions::new()
            .class(ItemClass::generic_password())
            .service(SERVICE)
            .load_attributes(true)
            .limit(Limit::All)
            .search();
        match result {
            Ok(items) => items
                .into_iter()
                .map(|item| {
                    item.simplify_dict()
                        .and_then(|mut attributes| attributes.remove("acct"))
                        .ok_or_else(|| "无法识别旧版钥匙串凭据条目".to_string())
                })
                .filter_map(|item| match item {
                    Ok(account) if super::is_legacy_account(id, &account) => Some(Ok(account)),
                    Ok(_) => None,
                    Err(error) => Some(Err(error)),
                })
                .collect(),
            Err(error) if error.code() == ITEM_NOT_FOUND => Ok(Vec::new()),
            Err(_) => Err("无法查询旧版钥匙串凭据条目".into()),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use security_framework::os::macos::keychain::CreateOptions;
    use std::process::Command;
    static TEST_GATE: Mutex<()> = Mutex::new(());

    #[test]
    fn native_silent_access_disables_ui_and_restores_it_after_success_and_error() {
        let _test_gate = TEST_GATE.lock();
        let before = SecKeychain::user_interaction_allowed().unwrap();
        access(false, || {
            assert!(!SecKeychain::user_interaction_allowed().unwrap());
            Ok(())
        })
        .unwrap();
        assert_eq!(SecKeychain::user_interaction_allowed().unwrap(), before);
        let result: Result<(), String> = access(false, || {
            assert!(!SecKeychain::user_interaction_allowed().unwrap());
            Err("synthetic denial".into())
        });
        assert!(result.is_err());
        assert_eq!(SecKeychain::user_interaction_allowed().unwrap(), before);
        if before {
            let disabled = SecKeychain::disable_user_interaction().unwrap();
            access(false, || Ok(())).unwrap();
            assert!(!SecKeychain::user_interaction_allowed().unwrap());
            drop(disabled);
            assert!(SecKeychain::user_interaction_allowed().unwrap());
        }
    }

    #[test]
    fn locked_private_keychain_rejects_repeated_secitem_reads_without_ui() {
        let _test_gate = TEST_GATE.lock();
        // Use synthetic data in a disposable keychain, never the user's login keychain.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("silent-access.keychain");
        access(false, || {
            let keychain = CreateOptions::new()
                .password("synthetic-test-password")
                .create(&path)
                .map_err(|e| format!("create: {}", e.code()))?;
            let check = (|| -> Result<(), String> {
                keychain
                    .add_generic_password("ai-toolkit.synthetic-test", "account", b"test-secret")
                    .map_err(|e| format!("seed: {}", e.code()))?;
                let lock = Command::new("/usr/bin/security")
                    .arg("lock-keychain")
                    .arg(&path)
                    .output()
                    .map_err(|e| e.to_string())?;
                if !lock.status.success() {
                    return Err("lock-keychain failed".into());
                }
                for _ in 0..8 {
                    let result = ItemSearchOptions::new()
                        .keychains(std::slice::from_ref(&keychain))
                        .class(ItemClass::generic_password())
                        .service("ai-toolkit.synthetic-test")
                        .account("account")
                        .load_data(true)
                        .search();
                    match result {
                        Err(error) if error.code() == -25308 || error.code() == -25293 => (),
                        Err(error) => return Err(format!("unexpected status: {}", error.code())),
                        Ok(_) => return Err("locked keychain unexpectedly returned data".into()),
                    }
                }
                Ok(())
            })();
            let cleanup = Command::new("/usr/bin/security")
                .arg("delete-keychain")
                .arg(&path)
                .output()
                .map_err(|e| e.to_string());
            check?;
            if !cleanup?.status.success() {
                return Err("delete-keychain failed".into());
            }
            Ok(())
        })
        .unwrap();
    }
}
