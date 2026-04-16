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
    #[serde(rename = "trae-solo-cn")]
    TraeSoloCn,
    #[serde(rename = "qoder")]
    Qoder,
    #[serde(rename = "qodercli")]
    Qodercli,
    #[serde(rename = "codebuddy")]
    CodeBuddy,
}

/// 安装方式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InstallMethod {
    /// Homebrew 安装: brew install <package>
    Brew { package: String },
    /// NPM 安装: npm install -g <package>
    Npm { package: String },
    /// Curl 脚本安装: curl -fsSL <url> | bash
    Curl { url: String },
    /// 自定义命令
    Custom { command: String },
    /// 仅下载 (无安装命令，如 IDE)
    Download { url: String },
}

impl InstallMethod {
    /// 获取显示名称
    pub fn display_name(&self) -> &str {
        match self {
            InstallMethod::Brew { .. } => "Homebrew",
            InstallMethod::Npm { .. } => "npm",
            InstallMethod::Curl { .. } => "curl 脚本",
            InstallMethod::Custom { .. } => "自定义",
            InstallMethod::Download { .. } => "下载安装",
        }
    }

    /// 获取显示命令
    pub fn display_command(&self) -> String {
        match self {
            InstallMethod::Brew { package } => format!("brew install {}", package),
            InstallMethod::Npm { package } => format!("npm install -g {}", package),
            InstallMethod::Curl { url } => format!("curl -fsSL {} | bash", url),
            InstallMethod::Custom { command } => command.clone(),
            InstallMethod::Download { url } => format!("下载自 {}", url),
        }
    }

    /// 是否需要确认 (curl 脚本需要用户确认)
    pub fn needs_confirm(&self) -> bool {
        matches!(self, InstallMethod::Curl { .. })
    }
}

/// 工具安装信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallInfo {
    /// 显示名称
    pub name: String,
    /// 安装方式列表
    pub methods: Vec<InstallMethod>,
    /// 更新命令 (为空表示不支持)
    pub update_cmd: String,
    /// 版本检测命令 (为空表示不支持)
    pub version_cmd: String,
    /// 官方主页
    pub homepage: String,
}

impl AppType {
    pub fn all() -> Vec<Self> {
        vec![
            Self::QwenCode,
            Self::Claude,
            Self::Codex,
            Self::Gemini,
            Self::OpenCode,
            Self::Trae,
            Self::TraeCn,
            Self::TraeSoloCn,
            Self::Qoder,
            Self::Qodercli,
            Self::CodeBuddy,
        ]
    }

    pub fn name(&self) -> &str {
        match self {
            Self::QwenCode => "qwen-code",
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
            Self::OpenCode => "opencode",
            Self::Trae => "trae",
            Self::TraeCn => "trae-cn",
            Self::TraeSoloCn => "trae-solo-cn",
            Self::Qoder => "qoder",
            Self::Qodercli => "qodercli",
            Self::CodeBuddy => "codebuddy",
        }
    }

    pub fn get_install_info(&self) -> Option<InstallInfo> {
        match self {
            Self::QwenCode => Some(InstallInfo {
                name: "Qwen Code".into(),
                methods: vec![
                    InstallMethod::Brew { package: "qwen-code".into() },
                    InstallMethod::Npm { package: "@qwen-code/qwen-code".into() },
                    InstallMethod::Curl {
                        url: "https://qwen-code-assets.oss-cn-hangzhou.aliyuncs.com/installation/install-qwen.sh".into(),
                    },
                ],
                update_cmd: "npm install -g @qwen-code/qwen-code@latest".into(),
                version_cmd: "qwen --version".into(),
                homepage: "https://qwencode.ai".into(),
            }),
            Self::Claude => Some(InstallInfo {
                name: "Claude Code".into(),
                methods: vec![
                    InstallMethod::Curl {
                        url: "https://claude.ai/install.sh".into(),
                    },
                    InstallMethod::Brew { package: "claude-code".into() },
                    InstallMethod::Npm { package: "@anthropic-ai/claude-code".into() },
                ],
                update_cmd: "npm install -g @anthropic-ai/claude-code@latest".into(),
                version_cmd: "claude --version".into(),
                homepage: "https://claude.ai/code".into(),
            }),
            Self::Codex => Some(InstallInfo {
                name: "Codex".into(),
                methods: vec![InstallMethod::Npm {
                    package: "@openai/codex".into(),
                }],
                update_cmd: "npm install -g @openai/codex@latest".into(),
                version_cmd: "codex --version".into(),
                homepage: "https://openai.com/codex".into(),
            }),
            Self::Gemini => Some(InstallInfo {
                name: "Gemini CLI".into(),
                methods: vec![
                    InstallMethod::Npm {
                        package: "@google/gemini-cli".into(),
                    },
                    InstallMethod::Brew { package: "gemini-cli".into() },
                ],
                update_cmd: "npm install -g @google/gemini-cli@latest".into(),
                version_cmd: "gemini --version".into(),
                homepage: "https://github.com/google-gemini/gemini-cli".into(),
            }),
            Self::OpenCode => Some(InstallInfo {
                name: "OpenCode".into(),
                methods: vec![
                    InstallMethod::Curl {
                        url: "https://opencode.ai/install".into(),
                    },
                    InstallMethod::Npm {
                        package: "opencode-ai".into(),
                    },
                    InstallMethod::Brew {
                        package: "anomalyco/tap/opencode".into(),
                    },
                ],
                update_cmd: "opencode upgrade".into(),
                version_cmd: "opencode --version".into(),
                homepage: "https://opencode.ai".into(),
            }),
            Self::Trae => Some(InstallInfo {
                name: "Trae".into(),
                methods: vec![InstallMethod::Download {
                    url: "https://trae.ai".into(),
                }],
                update_cmd: String::new(),
                version_cmd: String::new(),
                homepage: "https://trae.ai".into(),
            }),
            Self::TraeCn => Some(InstallInfo {
                name: "Trae CN".into(),
                methods: vec![InstallMethod::Download {
                    url: "https://www.trae.cn".into(),
                }],
                update_cmd: String::new(),
                version_cmd: String::new(),
                homepage: "https://www.trae.cn".into(),
            }),
            Self::TraeSoloCn => Some(InstallInfo {
                name: "TRAE SOLO CN".into(),
                methods: vec![InstallMethod::Download {
                    url: "https://www.trae.cn".into(),
                }],
                update_cmd: String::new(),
                version_cmd: String::new(),
                homepage: "https://www.trae.cn".into(),
            }),
            Self::Qoder => Some(InstallInfo {
                name: "Qoder".into(),
                methods: vec![InstallMethod::Download {
                    url: "https://qoder.com".into(),
                }],
                update_cmd: String::new(),
                version_cmd: String::new(),
                homepage: "https://qoder.com".into(),
            }),
            Self::Qodercli => Some(InstallInfo {
                name: "Qoder CLI".into(),
                methods: vec![
                    InstallMethod::Curl {
                        url: "https://qoder.com/install".into(),
                    },
                    InstallMethod::Brew {
                        package: "qoderai/qoder/qodercli".into(),
                    },
                    InstallMethod::Npm {
                        package: "@qoder-ai/qodercli".into(),
                    },
                ],
                update_cmd: "qodercli update".into(),
                version_cmd: "qodercli --version".into(),
                homepage: "https://qoder.com".into(),
            }),
            Self::CodeBuddy => Some(InstallInfo {
                name: "CodeBuddy".into(),
                methods: vec![
                    InstallMethod::Curl {
                        url: "https://copilot.tencent.com/cli/install.sh".into(),
                    },
                    InstallMethod::Npm {
                        package: "@tencent-ai/codebuddy-code".into(),
                    },
                ],
                update_cmd: "codebuddy update".into(),
                version_cmd: "codebuddy --version".into(),
                homepage: "https://codebuddy.ai".into(),
            }),
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
            "trae-solo-cn" => Ok(Self::TraeSoloCn),
            "qoder" => Ok(Self::Qoder),
            "qodercli" => Ok(Self::Qodercli),
            "codebuddy" => Ok(Self::CodeBuddy),
            _ => Err(format!("Unknown app type: {}", s)),
        }
    }
}
