use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::utils::SuppressConsole;
use anyhow::{Context, Result};

/// Clone or update a git repository.
/// Returns the HEAD commit hash.
pub fn clone_or_pull(repo_url: &str, dest: &Path, branch: Option<&str>) -> Result<String> {
    // Ensure parent exists so `git clone` can create dest.
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create parent dir {:?}", parent))?;
    }

    if dest.exists() && dest.join(".git").exists() {
        // Pull updates
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
    cmd.arg(repo_url).arg(dest);

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
                .args(["checkout", "-B", b, &format!("origin/{}", b)])
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
    let output = Command::new("git")
        .suppress_console()
        .arg("-C")
        .arg(dest)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .with_context(|| format!("git rev-parse HEAD in {:?}", dest))?;

    if !output.status.success() {
        anyhow::bail!("git rev-parse failed");
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn run_command_with_timeout(cmd: &mut Command, _timeout: Duration) -> Result<std::process::Output> {
    // 使用 cmd.output() 代替 spawn + try_wait 轮询
    // output() 会同时读取 stdout/stderr 管道，避免缓冲区满导致死锁
    // 之前的 try_wait 轮询方式从不读取管道，git clone 输出量大时
    // 管道缓冲区被填满，导致 git 进程阻塞在 write() 上永远无法退出
    let output = cmd
        .output()
        .with_context(|| format!("spawn command: {:?}", cmd))?;
    Ok(output)
}
