use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Workbuddy,
    Traework,
    Qoder,
}
impl Platform {
    pub fn key(self) -> &'static str {
        match self {
            Self::Workbuddy => "workbuddy",
            Self::Traework => "traework",
            Self::Qoder => "qoder",
        }
    }
}

// Secrets are persisted only in the operating system credential store.
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Credentials {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub device_id: Option<String>,
    #[serde(default)]
    pub expires_at: Option<i64>,
    #[serde(default)]
    pub extra: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditPack {
    pub id: String,
    pub name: String,
    pub remaining: Option<f64>,
    pub expires_at: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    /// The platform's own model key when `id` is a display-derived slug
    /// (e.g. Qoder key `gfmodel` behind the public id `glm-5.3-flash`).
    #[serde(default)]
    pub upstream_id: Option<String>,
    pub name: String,
    pub canonical_id: Option<String>,
    pub supports_tools: bool,
    pub context_window: Option<u64>,
    pub verified: bool,
    pub verification_note: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reward {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub status: String,
    pub amount: Option<f64>,
    pub next_available_at: Option<i64>,
    /// Optional activity details link. Credit claims use the provider protocol
    /// and the account's live campaign ID, never this URL.
    #[serde(default)]
    pub action_url: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSnapshot {
    pub balance: Option<f64>,
    pub unit: String,
    pub quota: Option<String>,
    pub packs: Vec<CreditPack>,
    pub rewards: Vec<Reward>,
    pub models: Vec<ModelInfo>,
    pub fetched_at: i64,
    pub stale: bool,
    pub diagnostic: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub platform: Platform,
    pub user_id: String,
    pub label: String,
    pub enabled: bool,
    pub status: String,
    pub last_error: Option<String>,
    pub snapshot: Option<ProviderSnapshot>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RewardOutcome {
    pub account_id: String,
    pub reward_id: String,
    pub name: String,
    pub status: String,
    pub amount: Option<f64>,
    pub kind: String,
    pub message: String,
    pub created_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxySettings {
    pub port: u16,
    pub auto_claim: bool,
}
impl Default for ProxySettings {
    fn default() -> Self {
        Self {
            port: 17860,
            auto_claim: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayStatus {
    pub running: bool,
    pub port: u16,
    pub base_url: String,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestSummary {
    pub id: String,
    pub account_id: String,
    pub platform: Platform,
    pub model: String,
    pub elapsed_ms: u64,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub status: String,
    pub created_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    pub accounts: Vec<Account>,
    pub settings: ProxySettings,
    pub gateway: GatewayStatus,
    pub rewards: Vec<RewardOutcome>,
    pub requests: Vec<RequestSummary>,
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
