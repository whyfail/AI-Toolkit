use std::fs::{self, OpenOptions};
use std::path::Path;

use anyhow::{bail, Context, Result};

fn check_owned(path: &Path, directory: bool) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink()
        || if directory {
            !metadata.is_dir()
        } else {
            !metadata.is_file()
        }
    {
        bail!("私有数据路径类型不安全: {:?}", path);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.uid() != unsafe { libc::geteuid() } || (!directory && metadata.nlink() != 1) {
            bail!("私有数据路径不属于当前用户或存在硬链接: {:?}", path);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            bail!("私有数据路径不能是重解析点: {:?}", path);
        }
        windows_acl(path, false)?;
    }
    Ok(())
}

pub fn ensure_private_dir(path: &Path) -> Result<()> {
    if fs::symlink_metadata(path).is_err() {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(path)
            .with_context(|| format!("创建私有目录 {:?}", path))?;
    }
    check_owned(path, true)?;
    restrict(path, true)
}

pub fn secure_file(path: &Path) -> Result<()> {
    check_owned(path, false)?;
    restrict(path, false)
}

pub fn prepare_private_file(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_private_dir(parent)?;
    }
    if fs::symlink_metadata(path).is_ok() {
        return secure_file(path);
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    options.open(path).context("创建私有文件失败")?;
    secure_file(path)
}

fn restrict(path: &Path, _directory: bool) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            path,
            fs::Permissions::from_mode(if _directory { 0o700 } else { 0o600 }),
        )?;
    }
    #[cfg(windows)]
    windows_acl(path, true)?;
    Ok(())
}

#[cfg(windows)]
fn windows_token_information(
    class: windows_sys::Win32::Security::TOKEN_INFORMATION_CLASS,
) -> Result<Vec<usize>> {
    use std::{os::windows::io::FromRawHandle, ptr};
    use windows_sys::Win32::{
        Security::{GetTokenInformation, TOKEN_QUERY},
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };
    unsafe {
        let mut token = ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let _token = std::os::windows::io::OwnedHandle::from_raw_handle(token);
        let mut needed = 0;
        GetTokenInformation(token, class, ptr::null_mut(), 0, &mut needed);
        let mut buffer = vec![0usize; (needed as usize).div_ceil(std::mem::size_of::<usize>())];
        if GetTokenInformation(
            token,
            class,
            buffer.as_mut_ptr().cast(),
            needed,
            &mut needed,
        ) == 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(buffer)
    }
}

#[cfg(windows)]
fn windows_sid_string(sid: windows_sys::Win32::Security::PSID) -> Result<String> {
    use windows_sys::Win32::{
        Foundation::LocalFree, Security::Authorization::ConvertSidToStringSidW,
    };
    unsafe {
        let mut value = std::ptr::null_mut();
        if ConvertSidToStringSidW(sid, &mut value) == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let mut length = 0;
        while *value.add(length) != 0 {
            length += 1;
        }
        let result = String::from_utf16_lossy(std::slice::from_raw_parts(value, length));
        LocalFree(value.cast());
        Ok(result)
    }
}

#[cfg(windows)]
fn windows_acl(path: &Path, restrict: bool) -> Result<()> {
    use std::{os::windows::ffi::OsStrExt, ptr};
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::{
            Authorization::{
                ConvertStringSecurityDescriptorToSecurityDescriptorW, GetNamedSecurityInfoW,
                SDDL_REVISION_1, SE_FILE_OBJECT,
            },
            EqualSid, SetFileSecurityW, TokenOwner, TokenUser, DACL_SECURITY_INFORMATION,
            OWNER_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, TOKEN_OWNER,
            TOKEN_USER,
        },
    };
    let user_buffer = windows_token_information(TokenUser)?;
    let owner_buffer = windows_token_information(TokenOwner)?;
    let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        let mut owner = ptr::null_mut();
        let mut descriptor = ptr::null_mut();
        let code = GetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            &mut owner,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            &mut descriptor,
        );
        if code != 0 {
            return Err(std::io::Error::from_raw_os_error(code as i32).into());
        }
        let result = (|| -> Result<String> {
            let user = &*user_buffer.as_ptr().cast::<TOKEN_USER>();
            let default_owner = &*owner_buffer.as_ptr().cast::<TOKEN_OWNER>();
            // Elevated processes may create objects owned by their default owner group.
            if EqualSid(owner, user.User.Sid) == 0 && EqualSid(owner, default_owner.Owner) == 0 {
                bail!("私有数据路径不属于当前用户");
            }
            windows_sid_string(user.User.Sid)
        })();
        LocalFree(descriptor);
        let user_sid = result?;
        if restrict {
            // Use the user SID explicitly; OW could grant the entire default owner group.
            let sddl: Vec<u16> = format!("D:P(A;OICI;FA;;;{user_sid})(A;OICI;FA;;;SY)")
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let mut descriptor = ptr::null_mut();
            if ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                ptr::null_mut(),
            ) == 0
            {
                return Err(std::io::Error::last_os_error().into());
            }
            let ok = SetFileSecurityW(
                name.as_ptr(),
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                descriptor,
            );
            let error = std::io::Error::last_os_error();
            LocalFree(descriptor);
            if ok == 0 {
                return Err(error.into());
            }
        }
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, PermissionsExt};

    #[test]
    fn tightens_existing_files_and_refuses_links_without_changing_the_target() {
        let sandbox = tempfile::tempdir().unwrap();
        let directory = sandbox.path().join("private");
        ensure_private_dir(&directory).unwrap();
        let file = directory.join("database");
        fs::write(&file, "original").unwrap();
        secure_file(&file).unwrap();
        assert_eq!(
            fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let link = sandbox.path().join("link");
        symlink(&file, &link).unwrap();
        assert!(prepare_private_file(&link).is_err());
        assert_eq!(fs::read_to_string(file).unwrap(), "original");
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;
    use std::{os::windows::ffi::OsStrExt, ptr};
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::{
            Authorization::{
                ConvertSecurityDescriptorToStringSecurityDescriptorW,
                ConvertStringSecurityDescriptorToSecurityDescriptorW, GetNamedSecurityInfoW,
                SDDL_REVISION_1, SE_FILE_OBJECT,
            },
            SetFileSecurityW, TokenUser, DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION,
            TOKEN_USER,
        },
    };

    fn security_string(path: &Path, information: u32) -> String {
        let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        unsafe {
            let mut descriptor = ptr::null_mut();
            assert_eq!(
                GetNamedSecurityInfoW(
                    name.as_ptr(),
                    SE_FILE_OBJECT,
                    information,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    &mut descriptor,
                ),
                0
            );
            let mut value = ptr::null_mut();
            let mut length = 0;
            let ok = ConvertSecurityDescriptorToStringSecurityDescriptorW(
                descriptor,
                SDDL_REVISION_1,
                information,
                &mut value,
                &mut length,
            );
            LocalFree(descriptor);
            assert_ne!(ok, 0);
            let result =
                String::from_utf16_lossy(std::slice::from_raw_parts(value, length as usize - 1));
            LocalFree(value.cast());
            result
        }
    }

    fn current_user_sid() -> String {
        let buffer = windows_token_information(TokenUser).unwrap();
        let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
        windows_sid_string(user.User.Sid).unwrap()
    }

    #[test]
    fn default_owned_paths_get_protected_user_and_system_access() {
        let sandbox = tempfile::tempdir().unwrap();
        let directory = sandbox.path().join("private");
        fs::create_dir(&directory).unwrap();
        let original_owner = security_string(&directory, OWNER_SECURITY_INFORMATION);
        ensure_private_dir(&directory).unwrap();
        ensure_private_dir(&directory).unwrap();
        assert_eq!(
            security_string(&directory, OWNER_SECURITY_INFORMATION),
            original_owner
        );
        let expected = format!("D:P(A;OICI;FA;;;{})(A;OICI;FA;;;SY)", current_user_sid());
        assert_eq!(
            security_string(&directory, DACL_SECURITY_INFORMATION),
            expected
        );

        let nested = directory.join("nested");
        ensure_private_dir(&nested).unwrap();
        let file = nested.join("database");
        prepare_private_file(&file).unwrap();
        fs::write(&file, "private data").unwrap();
        secure_file(&file).unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "private data");
        assert_eq!(security_string(&file, DACL_SECURITY_INFORMATION), expected);
    }

    #[test]
    fn explicit_user_owned_paths_are_also_accepted() {
        let sandbox = tempfile::tempdir().unwrap();
        let file = sandbox.path().join("database");
        fs::write(&file, "original").unwrap();
        let name: Vec<u16> = file.as_os_str().encode_wide().chain(Some(0)).collect();
        let sddl: Vec<u16> = format!("O:{}", current_user_sid())
            .encode_utf16()
            .chain(Some(0))
            .collect();
        unsafe {
            let mut descriptor = ptr::null_mut();
            assert_ne!(
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    sddl.as_ptr(),
                    SDDL_REVISION_1,
                    &mut descriptor,
                    ptr::null_mut(),
                ),
                0
            );
            let ok = SetFileSecurityW(name.as_ptr(), OWNER_SECURITY_INFORMATION, descriptor);
            LocalFree(descriptor);
            assert_ne!(ok, 0);
        }
        secure_file(&file).unwrap();
        fs::write(&file, "updated").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "updated");
    }
}
