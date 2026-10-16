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
fn windows_acl(path: &Path, restrict: bool) -> Result<()> {
    use std::{os::windows::ffi::OsStrExt, ptr};
    use windows_sys::Win32::{
        Foundation::{CloseHandle, LocalFree},
        Security::{
            Authorization::{
                ConvertStringSecurityDescriptorToSecurityDescriptorW, GetNamedSecurityInfoW,
                SDDL_REVISION_1, SE_FILE_OBJECT,
            },
            EqualSid, GetTokenInformation, SetFileSecurityW, TokenUser, DACL_SECURITY_INFORMATION,
            OWNER_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, TOKEN_QUERY,
            TOKEN_USER,
        },
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };
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
        let result = (|| -> Result<()> {
            let mut token = ptr::null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            let mut needed = 0;
            GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &mut needed);
            let mut buffer = vec![0usize; (needed as usize).div_ceil(std::mem::size_of::<usize>())];
            let ok = GetTokenInformation(
                token,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                needed,
                &mut needed,
            );
            CloseHandle(token);
            if ok == 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
            if EqualSid(owner, user.User.Sid) == 0 {
                bail!("私有数据路径不属于当前用户");
            }
            Ok(())
        })();
        LocalFree(descriptor);
        result?;
        if restrict {
            // Protected DACL: only the verified owner and SYSTEM may access the path.
            let sddl: Vec<u16> = "D:P(A;OICI;FA;;;OW)(A;OICI;FA;;;SY)"
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
