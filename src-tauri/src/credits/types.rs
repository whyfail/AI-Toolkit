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
    /// Underlying model IDs explicitly advertised by the catalog, separate from routing keys.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub upstream_model_ids: Vec<String>,
    pub name: String,
    pub canonical_id: Option<String>,
    pub supports_tools: bool,
    pub context_window: Option<u64>,
    /// Selectable context tiers advertised by the channel. The proxy uses the largest.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub context_window_options: Vec<u64>,
    #[serde(default)]
    pub max_output_tokens: Option<u64>,
    #[serde(default)]
    pub supports_images: Option<bool>,
    #[serde(default)]
    pub supports_reasoning: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasoning_effort_options: Vec<String>,
    #[serde(default)]
    pub default_reasoning_effort: Option<String>,
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
    #[serde(default)]
    pub sync: SnapshotSync,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotSync {
    pub rewards_fetched_at: Option<i64>,
    pub models_fetched_at: Option<i64>,
    pub rewards_error: Option<String>,
    pub models_error: Option<String>,
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
    #[serde(default = "auto_refresh_default")]
    pub auto_refresh: bool,
}
fn auto_refresh_default() -> bool { true }
impl Default for ProxySettings {
    fn default() -> Self {
        Self {
            port: 17860,
            auto_claim: false,
            auto_refresh: true,
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
    pub refreshing_account_ids: Vec<String>,
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing_settings_enable_balance_refresh_without_enabling_claims() {
        let old: ProxySettings = serde_json::from_str(r#"{"port":17860,"autoClaim":false}"#).unwrap();
        assert!(old.auto_refresh); assert!(!old.auto_claim);
        let mut disabled = old; disabled.auto_refresh = false;
        let roundtrip: ProxySettings = serde_json::from_str(&serde_json::to_string(&disabled).unwrap()).unwrap();
        assert!(!roundtrip.auto_refresh);
    }

    #[test]
    fn old_model_snapshots_keep_new_capabilities_unknown() {
        let saved = serde_json::json!({
            "id":"glm-5.3-flash", "name":"GLM-5.3-Flash", "canonicalId":null,
            "supportsTools":true, "contextWindow":1000000,
            "verified":false, "verificationNote":null
        });
        let model: ModelInfo = serde_json::from_value(saved).unwrap();
        assert_eq!(model.context_window, Some(1000000));
        assert!(model.upstream_model_ids.is_empty());
        assert!(model.context_window_options.is_empty());
        assert_eq!(model.max_output_tokens, None);
        assert_eq!(model.supports_images, None);
        assert_eq!(model.supports_reasoning, None);
        assert!(model.reasoning_effort_options.is_empty());
        assert_eq!(model.default_reasoning_effort, None);
        let exported = serde_json::to_value(model).unwrap();
        assert!(exported["maxOutputTokens"].is_null());
        assert!(exported["supportsImages"].is_null());
        assert!(exported["supportsReasoning"].is_null());
        assert!(exported["defaultReasoningEffort"].is_null());
    }
}
