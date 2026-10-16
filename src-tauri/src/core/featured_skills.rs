use anyhow::{ensure, Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::io::Read;

const FEATURED_SKILLS_URL: &str = "https://www.skills.sh/api/skills/all-time/0";

#[derive(Debug, Deserialize)]
struct FeaturedSkillsData {
    skills: Vec<FeaturedSkillRaw>,
}

#[derive(Debug, Deserialize)]
struct FeaturedSkillRaw {
    source: String,
    #[serde(rename = "skillId")]
    skill_id: String,
    name: String,
    installs: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FeaturedSkill {
    pub slug: String,
    pub name: String,
    pub installs: u64,
    pub source_url: String,
}

pub fn fetch_featured_skills() -> Result<Vec<FeaturedSkill>> {
    fetch_featured_skills_from_url(FEATURED_SKILLS_URL)
}

fn fetch_featured_skills_from_url(url: &str) -> Result<Vec<FeaturedSkill>> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;

    let response = client
        .get(url)
        .header("User-Agent", "ai-toolkit")
        .send()
        .context("请求热门技能列表失败")?;
    ensure!(
        response.status().is_success(),
        "热门技能列表请求失败（HTTP {}）",
        response.status().as_u16()
    );
    let mut body = Vec::new();
    response
        .take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut body)
        .context("读取热门技能列表失败")?;
    ensure!(body.len() <= 2 * 1024 * 1024, "热门技能列表超过 2 MiB");

    let data: FeaturedSkillsData =
        serde_json::from_slice(&body).context("热门技能列表数据格式无效")?;
    data.skills
        .into_iter()
        .take(20)
        .map(|skill| {
            let parts: Vec<_> = skill.source.split('/').collect();
            ensure!(
                parts.len() == 2
                    && parts.iter().all(|part| {
                        !part.is_empty()
                            && *part != "."
                            && *part != ".."
                            && part
                                .bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                    }),
                "热门技能来源仓库格式无效"
            );
            ensure!(
                !skill.name.trim().is_empty() && !skill.skill_id.trim().is_empty(),
                "热门技能缺少名称或标识"
            );
            Ok(FeaturedSkill {
                slug: format!("{}/{}", skill.source, skill.skill_id),
                name: skill.name,
                installs: skill.installs,
                source_url: format!("https://github.com/{}", skill.source),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    fn fetch_fixture(status: &str, body: String) -> Result<Vec<FeaturedSkill>> {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let status = status.to_owned();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .unwrap();
            let mut reader = BufReader::new(&stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert!(line.starts_with("GET /api/skills/all-time/0 "));
            loop {
                line.clear();
                if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                    break;
                }
            }
            drop(reader);
            write!(stream,"HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        });
        let result =
            fetch_featured_skills_from_url(&format!("http://{address}/api/skills/all-time/0"));
        server.join().unwrap();
        result
    }

    #[test]
    fn leaderboard_preserves_install_counts_and_distinct_sources_and_limits_results() {
        let rows: Vec<_> = (0..25).map(|index| json!({"source":format!("owner-{index}/skills"),"skillId":"fixture","name":"fixture","installs":1000-index,"weeklyInstalls":[],"isOfficial":false})).collect();
        let skills = fetch_fixture(
            "200 OK",
            json!({"skills":rows,"hasMore":true,"page":0}).to_string(),
        )
        .unwrap();
        assert_eq!(skills.len(), 20);
        assert_eq!(skills[0].installs, 1000);
        assert_eq!(skills[19].installs, 981);
        assert_eq!(skills[0].name, "fixture");
        assert_eq!(skills[0].source_url, "https://github.com/owner-0/skills");
        assert_ne!(skills[0].slug, skills[1].slug);
    }

    #[test]
    fn empty_leaderboard_is_distinct_from_a_failed_request() {
        assert!(fetch_fixture("200 OK", r#"{"skills":[]}"#.into())
            .unwrap()
            .is_empty());
    }

    #[test]
    fn http_and_schema_errors_are_not_hidden_or_returned_as_empty_success() {
        let error = fetch_fixture("404 Not Found", "not found".into()).unwrap_err();
        assert!(format!("{error:#}").contains("HTTP 404"));
        for body in [
            r#"{"items":[]}"#,
            r#"{"skills":[{"source":"owner/repo","skillId":"fixture","name":"fixture","installs":"bad"}]}"#,
        ] {
            let error = fetch_fixture("200 OK", body.into()).unwrap_err();
            assert!(format!("{error:#}").contains("数据格式无效"));
        }
    }

    #[test]
    fn malformed_repository_sources_are_rejected() {
        for source in [
            "owner/repo/extra",
            "owner/..",
            "/repo",
            "https://example.test/repo",
            "owner/repo?redirect=evil",
        ] {
            let body = json!({"skills":[{"source":source,"skillId":"fixture","name":"fixture","installs":1}]}).to_string();
            assert!(fetch_fixture("200 OK", body)
                .unwrap_err()
                .to_string()
                .contains("来源仓库格式无效"));
        }
    }

    #[test]
    fn oversized_leaderboard_is_rejected() {
        let mut body = r#"{"skills":[]}"#.to_owned();
        body.extend(std::iter::repeat(' ').take(2 * 1024 * 1024 + 1 - body.len()));
        assert!(fetch_fixture("200 OK", body)
            .unwrap_err()
            .to_string()
            .contains("超过 2 MiB"));
    }
}
