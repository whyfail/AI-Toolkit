//! Download a GitHub directory via the Contents API, bypassing git clone entirely.

use super::path_safety::{validate_skill_name, validate_subpath};
use super::resource_limits::ResourceBudget;
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct GithubContent {
    name: String,
    #[serde(rename = "type")]
    content_type: String,
    download_url: Option<String>,
    path: String,
}

/// Check if a GitHub URL with subpath can use the fast API download path.
/// Returns Some((owner, repo, branch, subpath)) if applicable.
pub fn parse_github_api_params(
    clone_url: &str,
    branch: Option<&str>,
    subpath: Option<&str>,
) -> Option<(String, String, String, String)> {
    let subpath = subpath?;
    if subpath.is_empty() {
        return None;
    }

    let url = clone_url.trim_end_matches('/').trim_end_matches(".git");
    let prefix = "https://github.com/";
    if !url.starts_with(prefix) {
        return None;
    }

    let rest = &url[prefix.len()..];
    let parts: Vec<&str> = rest.split('/').collect();
    if parts.len() < 2 {
        return None;
    }

    Some((
        parts[0].to_string(),
        parts[1].to_string(),
        branch.unwrap_or("main").to_string(),
        subpath.to_string(),
    ))
}

/// Download a directory from a GitHub repo using the Contents API.
pub fn download_github_directory(
    owner: &str,
    repo: &str,
    branch: &str,
    path: &str,
    dest: &Path,
    token: Option<&str>,
) -> Result<()> {
    eprintln!(
        "[DEBUG] Starting GitHub API download: {}/{}/{} ref={}",
        owner, repo, path, branch
    );
    validate_skill_name(owner)?;
    validate_skill_name(repo)?;
    validate_subpath(path)?;
    let mut budget = ResourceBudget::new()?;
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .build()
        .context("build HTTP client")?;

    std::fs::create_dir_all(dest).with_context(|| format!("create directory {:?}", dest))?;

    let result = download_dir_recursive(
        &client,
        owner,
        repo,
        branch,
        path,
        dest,
        token,
        &mut budget,
        0,
    );
    match &result {
        Ok(()) => eprintln!("[DEBUG] GitHub API download completed successfully"),
        Err(e) => eprintln!("[DEBUG] GitHub API download failed: {}", e),
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn download_dir_recursive(
    client: &reqwest::blocking::Client,
    owner: &str,
    repo: &str,
    branch: &str,
    path: &str,
    dest: &Path,
    token: Option<&str>,
    budget: &mut ResourceBudget,
    depth: usize,
) -> Result<()> {
    budget.enter(depth)?;
    let mut url = reqwest::Url::parse("https://api.github.com")?;
    url.path_segments_mut()
        .map_err(|_| anyhow::anyhow!("GitHub URL 无效"))?
        .extend(["repos", owner, repo, "contents"])
        .extend(
            path.split('/')
                .filter(|part| !part.is_empty() && *part != "."),
        );
    url.query_pairs_mut().append_pair("ref", branch);
    let mut req = client
        .get(url)
        .header("User-Agent", "ai-toolkit")
        .header("Accept", "application/vnd.github.v3+json");
    if let Some(t) = token {
        req = req.bearer_auth(t);
    }
    let response = check_github_response(req.send().context("request GitHub contents")?)?;
    // The API listing has its own bound before JSON allocation.
    let mut body = Vec::new();
    response.take(8 * 1024 * 1024 + 1).read_to_end(&mut body)?;
    if body.len() > 8 * 1024 * 1024 {
        anyhow::bail!("GitHub 目录清单超过限制");
    }
    let items: Vec<GithubContent> = serde_json::from_slice(&body)?;
    for item in items {
        validate_skill_name(&item.name)?;
        validate_subpath(&item.path)?;
        budget.check()?;
        let local_path = dest.join(&item.name);
        match item.content_type.as_str() {
            "file" => {
                budget.enter(depth + 1)?;
                if let Some(download_url) = &item.download_url {
                    let url = reqwest::Url::parse(download_url)?;
                    if url.scheme() != "https"
                        || url.host_str() != Some("raw.githubusercontent.com")
                        || !url.username().is_empty()
                        || url.password().is_some()
                        || url.port().is_some()
                    {
                        anyhow::bail!("GitHub 文件下载地址超出允许范围");
                    }
                    std::fs::create_dir_all(dest)?;
                    let mut request = client.get(url).header("User-Agent", "ai-toolkit");
                    if let Some(t) = token {
                        request = request.bearer_auth(t);
                    }
                    let mut response =
                        check_github_response(request.send().context("download GitHub file")?)?;
                    let mut output = std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&local_path)?;
                    let mut chunk = [0u8; 64 * 1024];
                    loop {
                        let count = response.read(&mut chunk)?;
                        if count == 0 {
                            break;
                        }
                        budget.bytes(count as u64)?;
                        output.write_all(&chunk[..count])?;
                    }
                }
            }
            "dir" => download_dir_recursive(
                client,
                owner,
                repo,
                branch,
                &item.path,
                &local_path,
                token,
                budget,
                depth + 1,
            )?,
            _ => {} // Never create links or submodules from API entries.
        }
    }
    Ok(())
}

fn check_github_response(resp: reqwest::blocking::Response) -> Result<reqwest::blocking::Response> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    if status.as_u16() == 403 {
        let reset_hint = resp
            .headers()
            .get("x-ratelimit-reset")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<i64>().ok())
            .map(|ts| {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                let wait_mins = ((ts - now).max(0) + 59) / 60;
                format!("RATE_LIMITED|{}", wait_mins)
            })
            .unwrap_or_else(|| "403 Forbidden".to_string());
        anyhow::bail!("{}", reset_hint);
    }
    Err(anyhow::anyhow!("GitHub API error {}", status))
}
