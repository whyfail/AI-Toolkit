use crate::{
    app_state::AppState,
    credits::{
        providers::{OAuthPoll, OAuthStart},
        types::*,
    },
};
use serde_json::{json, Value};
use tauri::{Manager, State};

#[tauri::command]
pub async fn credits_import_local(
    state: State<'_, AppState>,
    platform: Platform,
    region: Option<CreditRegion>,
) -> Result<Account, String> {
    let region = region.unwrap_or_default();
    let credentials = match platform {
        Platform::MinimaxCode => crate::credits::providers::minimax::local_credentials_for_region(region)?
            .ok_or_else(|| format!("本机未找到 MiniMax Code {}登录信息，请先登录对应区域的 MiniMax Code 或 mcode CLI", if region == CreditRegion::Cn { "国内版" } else { "国际版" }))?,
        Platform::Traework => match crate::credits::providers::local_credentials_for_region(region) {
            Ok(Some(credentials)) => credentials,
            Ok(None) => {
                return Err(format!("本机未找到 TRAE {}桌面或 CLI 登录信息，请先登录对应区域的 TRAE", if region == CreditRegion::Cn { "国内版" } else { "国际版" }));
            }
            Err(message) => {
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
    account
}

#[tauri::command]
pub fn credits_dashboard(state: State<'_, AppState>) -> Result<Dashboard, String> {
    state.credits.dashboard()
}

#[tauri::command]
pub fn credits_set_view_active(state: State<'_, AppState>, app: tauri::AppHandle, active: bool) -> Result<(), String> {
    let active = active && app.get_webview_window("main").is_some_and(|window|
        window.is_visible().unwrap_or(false) && window.is_focused().unwrap_or(false)
            && !window.is_minimized().unwrap_or(true));
    state.credits.set_credits_view_active(active)
}

#[tauri::command]
pub fn credits_network_restored(state: State<'_, AppState>) -> Result<(), String> {
    state.credits.credits_network_restored()
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
    region: Option<CreditRegion>,
) -> Result<OAuthStart, String> {
    state
        .credits
        .provider
        .start_login(platform, region.unwrap_or_default())
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
