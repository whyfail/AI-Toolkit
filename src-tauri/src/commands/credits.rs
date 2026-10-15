use crate::{
    app_state::AppState,
    credits::{
        providers::{OAuthPoll, OAuthStart},
        types::*,
    },
};
use serde_json::{json, Value};
use tauri::State;

#[tauri::command]
pub async fn credits_import_local(
    state: State<'_, AppState>,
    platform: Platform,
) -> Result<Account, String> {
    let credentials = match platform {
        Platform::Traework => match crate::credits::providers::local_credentials() {
            Ok(Some(credentials)) => credentials,
            Ok(None) => {
                eprintln!("credits: 本机导入未找到 TRAE CN 登录信息");
                return Err("本机未找到 TRAE CN 桌面或 CLI 登录信息，请先登录 TRAE CN".into());
            }
            Err(message) => {
                eprintln!("credits: 本机导入读取凭据失败 {message}");
                return Err(message);
            }
        },
        _ => return Err("该平台暂不支持本机导入，请使用官方登录".into()),
    };
    state.credits.retry_secret_access()?;
    let account = state
        .credits
        .add_account(platform, String::new(), credentials)
        .await;
    eprintln!(
        "credits: 本机导入 {}",
        account
            .as_ref()
            .map(|a| format!("成功 {}", a.user_id))
            .as_deref()
            .unwrap_or("失败")
    );
    account
}

#[tauri::command]
pub fn credits_dashboard(state: State<'_, AppState>) -> Result<Dashboard, String> {
    state.credits.dashboard()
}

#[tauri::command]
pub async fn credits_set_account_enabled(
    state: State<'_, AppState>,
    account_id: String,
    enabled: bool,
) -> Result<(), String> {
    state.credits.set_enabled(&account_id, enabled).await
}
#[tauri::command]
pub async fn credits_delete_account(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<(), String> {
    state.credits.retry_secret_access()?;
    state.credits.delete_account(&account_id).await
}
#[tauri::command]
pub async fn credits_refresh(
    state: State<'_, AppState>,
    account_id: Option<String>,
) -> Result<Vec<Account>, String> {
    state.credits.retry_secret_access()?;
    state.credits.refresh(account_id.as_deref()).await
}
#[tauri::command]
pub async fn credits_claim(
    state: State<'_, AppState>,
    account_id: Option<String>,
) -> Result<Vec<RewardOutcome>, String> {
    state.credits.retry_secret_access()?;
    state.credits.claim(account_id.as_deref()).await
}
#[tauri::command]
pub async fn credits_save_settings(
    state: State<'_, AppState>,
    settings: ProxySettings,
) -> Result<ProxySettings, String> {
    state.credits.save_settings(settings).await
}
#[tauri::command]
pub async fn credits_start_proxy(state: State<'_, AppState>) -> Result<GatewayStatus, String> {
    let settings = state.credits.store.settings()?;
    state.credits.start_proxy_for_user(settings.port).await
}
#[tauri::command]
pub async fn credits_stop_proxy(state: State<'_, AppState>) -> Result<GatewayStatus, String> {
    state.credits.gateway.stop().await
}
#[tauri::command]
pub async fn credits_proxy_key(state: State<'_, AppState>) -> Result<String, String> {
    state.credits.retry_gateway_key_access()?;
    state.credits.gateway_key()
}
#[tauri::command]
pub async fn credits_probe_proxy(state: State<'_, AppState>) -> Result<String, String> {
    state.credits.retry_gateway_key_access()?;
    state.credits.probe_proxy().await
}
#[tauri::command]
pub async fn credits_oauth_start(
    state: State<'_, AppState>,
    platform: Platform,
) -> Result<OAuthStart, String> {
    state
        .credits
        .provider
        .start_login(platform)
        .await
        .map_err(|e| e.message)
}
#[tauri::command]
pub async fn credits_oauth_poll(
    state: State<'_, AppState>,
    flow_id: String,
) -> Result<Value, String> {
    match state
        .credits
        .provider
        .poll_login(&flow_id)
        .await
        .map_err(|e| e.message)?
    {
        OAuthPoll::Pending => Ok(json!({"status":"pending"})),
        OAuthPoll::Failed { message } => Ok(json!({"status":"failed","message":message})),
        OAuthPoll::Completed {
            platform,
            user_id: _,
            label,
            credentials,
        } => {
            state.credits.retry_secret_access()?;
            let account = state
                .credits
                .add_account(platform, label, credentials)
                .await?;
            Ok(json!({"status":"completed","account":account}))
        }
    }
}
