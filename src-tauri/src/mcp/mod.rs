use serde::{Deserialize, Serialize};

/// 支持的应用类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AppType {
    #[serde(rename = "qwen-code")]
    QwenCode,
    #[serde(rename = "claude")]
    Claude,
    #[serde(rename = "codex")]
    Codex,
    #[serde(rename = "gemini")]
    Gemini,
    #[serde(rename = "opencode")]
    OpenCode,
    #[serde(rename = "trae")]
    Trae,
    #[serde(rename = "trae-cn")]
    TraeCn,
    #[serde(rename = "trae-work")]
    TraeWork,
    #[serde(rename = "trae-solo-cn")]
    TraeSoloCn,
    #[serde(rename = "qoder")]
    Qoder,
    #[serde(rename = "qodercli")]
    Qodercli,
    #[serde(rename = "codebuddy")]
    CodeBuddy,
    #[serde(rename = "hermes")]
    Hermes,
    #[serde(rename = "mimo-code")]
    MimoCode,
    #[serde(rename = "kimi-code")]
    KimiCode,
    #[serde(rename = "minimax-code")]
    MinimaxCode,
    #[serde(rename = "workbuddy")]
    WorkBuddy,
    #[serde(rename = "workbuddy-cn")]
    WorkBuddyCn,
    #[serde(rename = "zcode")]
    Zcode,
    #[serde(rename = "cursor")]
    Cursor,
    #[serde(rename = "windsurf")]
    Windsurf,
    #[serde(rename = "cline")]
    Cline,
    #[serde(rename = "roo")]
    Roo,
    #[serde(rename = "kilo")]
    Kilo,
    #[serde(rename = "continue")]
    Continue,
    #[serde(rename = "goose")]
    Goose,
    #[serde(rename = "kiro")]
    Kiro,
}

impl AppType {
    /// Legacy product IDs remain readable, while all new operations use the merged product.
    pub fn canonical(&self) -> Self {
        match self {
            Self::TraeWork => Self::Trae,
            Self::TraeSoloCn => Self::TraeCn,
            Self::WorkBuddyCn => Self::WorkBuddy,
            _ => self.clone(),
        }
    }

    pub fn is_trae(&self) -> bool {
        matches!(self.canonical(), Self::Trae | Self::TraeCn)
    }

    pub fn all() -> Vec<Self> {
        vec![
            Self::QwenCode,
            Self::Claude,
            Self::Codex,
            Self::Gemini,
            Self::OpenCode,
            Self::Trae,
            Self::TraeCn,
            Self::Qoder,
            Self::Qodercli,
            Self::CodeBuddy,
            Self::Hermes,
            Self::MimoCode,
            Self::KimiCode,
            Self::MinimaxCode,
            Self::WorkBuddy,
            Self::Zcode,
            Self::Cursor,
            Self::Windsurf,
            Self::Cline,
            Self::Roo,
            Self::Kilo,
            Self::Continue,
            Self::Goose,
            Self::Kiro,
        ]
    }

    pub fn name(&self) -> &str {
        match self {
            Self::QwenCode => "qwen-code",
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
            Self::OpenCode => "opencode",
            Self::Trae | Self::TraeWork => "trae",
            Self::TraeCn | Self::TraeSoloCn => "trae-cn",
            Self::Qoder => "qoder",
            Self::Qodercli => "qodercli",
            Self::CodeBuddy => "codebuddy",
            Self::Hermes => "hermes",
            Self::MimoCode => "mimo-code",
            Self::KimiCode => "kimi-code",
            Self::MinimaxCode => "minimax-code",
            Self::WorkBuddy | Self::WorkBuddyCn => "workbuddy",
            Self::Zcode => "zcode",
            Self::Cursor => "cursor",
            Self::Windsurf => "windsurf",
            Self::Cline => "cline",
            Self::Roo => "roo",
            Self::Kilo => "kilo",
            Self::Continue => "continue",
            Self::Goose => "goose",
            Self::Kiro => "kiro",
        }
    }
}

impl std::str::FromStr for AppType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "qwen-code" => Ok(Self::QwenCode),
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            "gemini" => Ok(Self::Gemini),
            "opencode" => Ok(Self::OpenCode),
            "trae" => Ok(Self::Trae),
            "trae-cn" => Ok(Self::TraeCn),
            "trae-work" => Ok(Self::Trae),
            "trae-solo-cn" => Ok(Self::TraeCn),
            "qoder" => Ok(Self::Qoder),
            "qodercli" => Ok(Self::Qodercli),
            "codebuddy" => Ok(Self::CodeBuddy),
            "hermes" => Ok(Self::Hermes),
            "mimo-code" => Ok(Self::MimoCode),
            "kimi-code" => Ok(Self::KimiCode),
            "minimax-code" => Ok(Self::MinimaxCode),
            "workbuddy" => Ok(Self::WorkBuddy),
            "workbuddy-cn" => Ok(Self::WorkBuddy),
            "zcode" => Ok(Self::Zcode),
            "cursor" => Ok(Self::Cursor),
            "windsurf" => Ok(Self::Windsurf),
            "cline" => Ok(Self::Cline),
            "roo" => Ok(Self::Roo),
            "kilo" => Ok(Self::Kilo),
            "continue" => Ok(Self::Continue),
            "goose" => Ok(Self::Goose),
            "kiro" => Ok(Self::Kiro),
            _ => Err(format!("Unknown app type: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_trae_ids_deserialize_and_normalize_without_duplicating_products() {
        let legacy: AppType = serde_json::from_str("\"trae-work\"").unwrap();
        let legacy_cn: AppType = serde_json::from_str("\"trae-solo-cn\"").unwrap();
        assert_eq!(legacy.canonical(), AppType::Trae);
        assert_eq!(legacy_cn.canonical(), AppType::TraeCn);
        assert_eq!("trae-work".parse::<AppType>().unwrap(), AppType::Trae);
        assert_eq!("trae-solo-cn".parse::<AppType>().unwrap(), AppType::TraeCn);
        assert_eq!(AppType::all().len(), 24);
        assert!(!AppType::all().contains(&AppType::TraeWork));
        assert!(!AppType::all().contains(&AppType::TraeSoloCn));
        assert_eq!(AppType::WorkBuddyCn.canonical(), AppType::WorkBuddy);
        assert!(!AppType::all().contains(&AppType::WorkBuddyCn));
    }
}
