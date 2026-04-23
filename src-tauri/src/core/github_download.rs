//! Download a GitHub directory via the Contents API, bypassing git clone entirely.

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
    eprintln!("[DEBUG] Starting GitHub API download: {}/{}/{} ref={}", owner, repo, path, branch);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("build HTTP client")?;

    std::fs::create_dir_all(dest).with_context(|| format!("create directory {:?}", dest))?;

    let result = download_dir_recursive(&client, owner, repo, branch, path, dest, token);
    match &result {
        Ok(()) => eprintln!("[DEBUG] GitHub API download completed successfully"),
        Err(e) => eprintln!("[DEBUG] GitHub API download failed: {}", e),
    }
    result
}

fn download_dir_recursive(
    client: &reqwest::blocking::Client,
    owner: &str,
    repo: &str,
    branch: &str,
    path: &str,
    dest: &Path,
    token: Option<&str>,
) -> Result<()> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/contents/{}?ref={}",
        owner, repo, path, branch
    );

    let mut req = client
        .get(&url)
        .header("User-Agent", "ai-toolkit")
        .header("Accept", "application/vnd.github.v3+json");
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {}", t));
    }
    let resp = req
        .send()
        .with_context(|| format!("request GitHub contents: {}", url))?;
    let resp = check_github_response(resp)?;

    let items: Vec<GithubContent> = resp
        .json()
        .with_context(|| format!("parse GitHub contents response: {}", url))?;

    for item in items {
        let local_path = dest.join(&item.name);

        match item.content_type.as_str() {
            "file" => {
                if let Some(download_url) = &item.download_url {
                    if let Some(parent) = local_path.parent() {
                        std::fs::create_dir_all(parent)
                            .with_context(|| format!("create parent dir {:?}", parent))?;
                    }
                    let mut file_req = client
                        .get(download_url)
                        .header("User-Agent", "ai-toolkit");
                    if let Some(t) = token {
                        file_req = file_req.header("Authorization", format!("Bearer {}", t));
                    }
                    let file_resp = file_req
                        .send()
                        .with_context(|| format!("download file: {}", item.path))?;
                    let file_resp = check_github_response(file_resp)?;
                    let bytes = file_resp
                        .bytes()
                        .with_context(|| format!("read file bytes: {}", item.path))?;

                    std::fs::write(&local_path, &bytes)
                        .with_context(|| format!("write file {:?}", local_path))?;
                }
            }
            "dir" => {
                download_dir_recursive(client, owner, repo, branch, &item.path, &local_path, token)?;
            }
            _ => {
                // Skip symlinks, submodules, etc.
            }
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
    Err(anyhow::anyhow!(
        "GitHub API error {}",
        status
    ))
}
