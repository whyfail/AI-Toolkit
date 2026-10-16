use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::utils::SuppressConsole;
use anyhow::{Context, Result};

/// Clone or update a git repository.
/// Returns the HEAD commit hash.
pub fn clone_or_pull(repo_url: &str, dest: &Path, branch: Option<&str>) -> Result<String> {
    validate_git_source(repo_url, branch)?;
    // Ensure parent exists so `git clone` can create dest.
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create parent dir {:?}", parent))?;
    }

    if dest.exists() && dest.join(".git").exists() {
        validate_cached_origin(repo_url, dest)?;
        fetch_and_checkout(dest, branch)
    } else {
        // Clone
        git_clone(repo_url, dest, branch)
    }
}

fn git_clone(repo_url: &str, dest: &Path, branch: Option<&str>) -> Result<String> {
    eprintln!("[DEBUG] Starting git clone: {} -> {:?}", repo_url, dest);
    let mut cmd = Command::new("git");
    cmd.suppress_console()
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("clone")
        // We deliberately omit `--filter=blob:none` (partial clone). With that
        // filter the working tree contains promisor placeholders and a plain
        // `std::fs::copy` can produce 0-byte target files when the lazy fetch
        // fails silently (offline / rate-limited / no credentials). Shallow
        // clones (`--depth 1`) are still small but guarantee blob material is
        // present before we copy it into the central repo.
        .args(["--depth", "1", "--no-tags"])
        .env("GIT_TERMINAL_PROMPT", "0");

    // `echo` is a Unix builtin; on Windows it doesn't exist as a standalone
    // executable, so GIT_ASKPASS=echo would fail.  Rely solely on
    // GIT_TERMINAL_PROMPT=0 to suppress interactive prompts.
    #[cfg(not(windows))]
    cmd.env("GIT_ASKPASS", "echo");

    if let Some(b) = branch {
        cmd.arg("--branch").arg(b).arg("--single-branch");
    }
    cmd.arg("--").arg(repo_url).arg(dest);

    let output = run_command_with_timeout(&mut cmd, Duration::from_secs(300))
        .with_context(|| format!("git clone {} into {:?}", repo_url, dest))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("[ERROR] git clone failed: {}", stderr);
        anyhow::bail!("git clone failed: {}", stderr);
    }

    eprintln!("[DEBUG] git clone succeeded, getting HEAD");
    // Get HEAD revision
    get_head_revision(dest)
}

fn fetch_and_checkout(dest: &Path, branch: Option<&str>) -> Result<String> {
    eprintln!("[DEBUG] Starting git fetch in {:?}", dest);
    // Fetch updates
    let output = run_command_with_timeout(
        Command::new("git")
            .suppress_console()
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .arg("-C")
            .arg(dest)
            .args(["fetch", "--prune", "origin"])
            .env("GIT_TERMINAL_PROMPT", "0"),
        Duration::from_secs(180),
    )
    .with_context(|| format!("git fetch in {:?}", dest))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("[ERROR] git fetch failed: {}", stderr);
        anyhow::bail!("git fetch failed: {}", stderr);
    }

    // Checkout branch if specified
    if let Some(b) = branch {
        let output = run_command_with_timeout(
            &mut Command::new("git")
                .suppress_console()
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .arg("-C")
                .arg(dest)
                .args(["checkout", "-B", b, &format!("origin/{}", b), "--"])
                .env("GIT_TERMINAL_PROMPT", "0"),
            Duration::from_secs(60),
        )
        .with_context(|| format!("git checkout {} in {:?}", b, dest))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("[ERROR] git checkout failed: {}", stderr);
            anyhow::bail!("git checkout branch failed: {}", stderr);
        }
    } else {
        // Reset to FETCH_HEAD
        let output = run_command_with_timeout(
            &mut Command::new("git")
                .suppress_console()
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .arg("-C")
                .arg(dest)
                .args(["reset", "--hard", "FETCH_HEAD"])
                .env("GIT_TERMINAL_PROMPT", "0"),
            Duration::from_secs(60),
        )
        .with_context(|| format!("git reset --hard in {:?}", dest))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("[ERROR] git reset failed: {}", stderr);
            anyhow::bail!("git reset --hard failed: {}", stderr);
        }
    }

    get_head_revision(dest)
}

fn get_head_revision(dest: &Path) -> Result<String> {
    let output = run_command_with_timeout(
        Command::new("git")
            .suppress_console()
            .arg("-C")
            .arg(dest)
            .arg("rev-parse")
            .arg("HEAD"),
        Duration::from_secs(15),
    )
    .with_context(|| format!("git rev-parse HEAD in {:?}", dest))?;

    if !output.status.success() {
        anyhow::bail!("git rev-parse failed");
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(crate) fn validate_git_source(repo_url: &str, branch: Option<&str>) -> Result<()> {
    if repo_url.is_empty()
        || repo_url.starts_with('-')
        || repo_url.chars().any(char::is_control)
        || repo_url.contains("::")
    {
        anyhow::bail!("不支持的 Git 来源");
    }
    if !Path::new(repo_url).exists() {
        let allowed_url = reqwest::Url::parse(repo_url).ok().is_some_and(|url| {
            matches!(url.scheme(), "https" | "http" | "ssh" | "git" | "file")
                && (url.scheme() == "file" || url.host_str().is_some())
        });
        let scp = repo_url.split_once(':').is_some_and(|(host, path)| {
            !host.is_empty()
                && !path.is_empty()
                && !host.starts_with('-')
                && host
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || ".-_@".contains(c))
        });
        if !allowed_url && !scp {
            anyhow::bail!("不支持的 Git URL 或本地路径");
        }
    }
    if let Some(branch) = branch {
        if branch.is_empty()
            || branch.starts_with(['-', '/'])
            || branch.ends_with(['/', '.'])
            || branch.contains("..")
            || branch.contains("@{")
            || branch.contains("//")
            || branch
                .split('/')
                .any(|part| part.is_empty() || part.starts_with('.') || part.ends_with(".lock"))
            || branch
                .chars()
                .any(|c| c.is_whitespace() || c.is_control() || "~^:?*[\\".contains(c))
        {
            anyhow::bail!("无效的 Git 分支名称");
        }
    }
    Ok(())
}

pub(crate) fn validate_cached_origin(repo_url: &str, dest: &Path) -> Result<()> {
    super::private_fs::ensure_private_dir(dest)?;
    super::private_fs::ensure_private_dir(&dest.join(".git"))?;
    super::private_fs::secure_file(&dest.join(".git/config"))?;
    let output = run_command_with_timeout(
        Command::new("git")
            .suppress_console()
            .arg("-C")
            .arg(dest)
            .args(["config", "--get", "remote.origin.url"]),
        Duration::from_secs(10),
    )?;
    let origin = String::from_utf8(output.stdout)?;
    if !output.status.success()
        || origin.trim().trim_end_matches('/') != repo_url.trim_end_matches('/')
    {
        anyhow::bail!("Git 缓存来源与请求不一致");
    }
    Ok(())
}

pub(crate) fn run_command_with_timeout(
    cmd: &mut Command,
    timeout: Duration,
) -> Result<std::process::Output> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd.spawn().context("启动 Git 子进程失败")?;
    #[cfg(windows)]
    let job = ProcessJob::attach(&mut child)?;
    #[cfg(unix)]
    let pid = child.id();
    // Drain both pipes concurrently, retaining a bounded diagnostic prefix.
    fn drain(mut pipe: impl Read) -> std::io::Result<Vec<u8>> {
        let mut result = Vec::new();
        let mut buffer = [0u8; 16384];
        loop {
            let n = pipe.read(&mut buffer)?;
            if n == 0 {
                return Ok(result);
            }
            let keep = n.min((2 * 1024 * 1024usize).saturating_sub(result.len()));
            result.extend_from_slice(&buffer[..keep]);
        }
    }
    let stdout = child.stdout.take().context("缺少 stdout 管道")?;
    let stderr = child.stderr.take().context("缺少 stderr 管道")?;
    let out = std::thread::spawn(move || drain(stdout));
    let err = std::thread::spawn(move || drain(stderr));
    let deadline = Instant::now() + timeout;
    let status = (|| -> std::io::Result<_> {
        loop {
            if let Some(status) = child.try_wait()? {
                return Ok(Some(status));
            }
            if Instant::now() >= deadline {
                return Ok(None);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    })();
    // A command and its helpers share a fresh process group on Unix.
    #[cfg(unix)]
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL);
    }
    #[cfg(windows)]
    drop(job);
    if !matches!(status, Ok(Some(_))) {
        let _ = child.kill();
    }
    let _ = child.wait();
    let stdout = out
        .join()
        .map_err(|_| anyhow::anyhow!("stdout 读取线程失败"))??;
    let stderr = err
        .join()
        .map_err(|_| anyhow::anyhow!("stderr 读取线程失败"))??;
    let status = status?
        .ok_or_else(|| anyhow::anyhow!("Git 操作超时（{} 秒），子进程已终止", timeout.as_secs()))?;
    Ok(std::process::Output {
        status,
        stdout,
        stderr,
    })
}

#[cfg(windows)]
struct ProcessJob(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl ProcessJob {
    fn attach(child: &mut std::process::Child) -> Result<Self> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::*;
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                let error = std::io::Error::last_os_error();
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
            let job = Self(handle);
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&limits) as u32,
            ) == 0
                || AssignProcessToJobObject(handle, child.as_raw_handle()) == 0
            {
                let error = std::io::Error::last_os_error();
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
            Ok(job)
        }
    }
}

#[cfg(windows)]
impl Drop for ProcessJob {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_sources_reject_helpers_and_option_inputs_but_allow_ssh_and_slash_branches() {
        for url in ["-oops", "ext::invalid", "https://example.test/repo\narg"] {
            assert!(validate_git_source(url, None).is_err());
        }
        for url in [
            "https://example.test/repo.git",
            "ssh://git@example.test/repo",
            "git@example.test:repo.git",
        ] {
            validate_git_source(url, Some("feature/security-fix")).unwrap();
        }
        assert!(validate_git_source("https://example.test/repo", Some("--bad")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn timeout_drains_pipes_and_terminates_descendants() {
        let sandbox = tempfile::tempdir().unwrap();
        let marker = sandbox.path().join("late-marker");
        let mut command = Command::new("sh");
        command
            .args([
                "-c",
                "(sleep 0.4; touch \"$1\") & while :; do printf '%8192s' x >&2; done",
                "test",
            ])
            .arg(&marker);
        let started = Instant::now();
        assert!(run_command_with_timeout(&mut command, Duration::from_millis(100)).is_err());
        assert!(started.elapsed() < Duration::from_secs(2));
        std::thread::sleep(Duration::from_millis(500));
        assert!(!marker.exists());
    }
}
