use anyhow::Result;
use reqwest::blocking::Client;
use serde::Deserialize;

const FEATURED_SKILLS_URL: &str =
    "https://raw.githubusercontent.com/qufei1993/skills-hub/main/featured-skills.json";

#[derive(Debug, Deserialize)]
struct FeaturedSkillsData {
    skills: Vec<FeaturedSkillRaw>,
}

#[derive(Debug, Deserialize)]
struct FeaturedSkillRaw {
    slug: String,
    name: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    stars: u64,
    #[serde(default)]
    source_url: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FeaturedSkill {
    pub slug: String,
    pub name: String,
    pub summary: String,
    pub downloads: u64,
    pub stars: u64,
    pub source_url: String,
}

pub fn fetch_featured_skills() -> Result<Vec<FeaturedSkill>> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;

    let body = client
        .get(FEATURED_SKILLS_URL)
        .header("User-Agent", "ai-toolkit")
        .send()?
        .error_for_status()?
        .text()?;

    let data: FeaturedSkillsData = serde_json::from_str(&body)?;
    Ok(data
        .skills
        .into_iter()
        .filter(|s| !s.source_url.is_empty())
        .map(|s| FeaturedSkill {
            slug: s.slug,
            name: s.name,
            summary: s.summary,
            downloads: s.downloads,
            stars: s.stars,
            source_url: s.source_url,
        })
        .collect())
}
