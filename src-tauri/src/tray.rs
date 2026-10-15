use crate::app_state::AppState;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    App, AppHandle, Manager, RunEvent, Runtime, WindowEvent,
};

const MAIN_WINDOW: &str = "main";

pub fn setup<R: Runtime>(app: &App<R>) -> Result<(), Box<dyn std::error::Error>> {
    let open = MenuItem::with_id(app, "tray-open", "打开主窗口", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "tray-quit", "退出应用（停止代理）", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let icon = app.default_window_icon().ok_or("应用托盘图标缺失")?;

    TrayIconBuilder::with_id("ai-toolkit")
        .icon(icon.clone())
        .tooltip("AI Toolkit · 关闭窗口后继续在后台运行")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "tray-open" => restore_main_window(app),
            "tray-quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

pub fn restore_main_window<R: Runtime>(app: &AppHandle<R>) {
    let result = app
        .get_webview_window(MAIN_WINDOW)
        .ok_or(tauri::Error::WindowNotFound)
        .and_then(|window| {
            #[cfg(target_os = "macos")]
            app.show()?;
            window.show()?;
            window.unminimize()?;
            window.set_focus()
        });
    if let Err(error) = result {
        eprintln!("恢复主窗口失败: {error}");
    }
}

pub fn handle_run_event<R: Runtime>(app: &AppHandle<R>, event: &RunEvent) {
    match event {
        RunEvent::WindowEvent {
            label,
            event: WindowEvent::CloseRequested { api, .. },
            ..
        } if label == MAIN_WINDOW => {
            api.prevent_close();
            if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
                if let Err(error) = window.hide() {
                    eprintln!("隐藏主窗口失败: {error}");
                }
            }
        }
        // ExitRequested 可以被取消；只在确定退出时停止代理。
        RunEvent::Exit => {
            if let Some(state) = app.try_state::<AppState>() {
                tauri::async_runtime::block_on(state.credits.shutdown());
            }
        }
        #[cfg(target_os = "macos")]
        RunEvent::Reopen {
            has_visible_windows: false,
            ..
        } => restore_main_window(app),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app_state::InstalledToolsReport, credits::CreditManager, database::Database};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, RwLock,
    };
    use tauri::test::{mock_builder, mock_context, noop_assets};

    #[test]
    fn closing_main_window_preserves_proxy_until_actual_exit() {
        let credits = CreditManager::test();
        let state = AppState {
            db: Database::in_memory().unwrap(),
            credits: credits.clone(),
            mcp_gate: parking_lot::Mutex::new(()),
            installed_tools: RwLock::new(InstalledToolsReport::default()),
        };
        let app = mock_builder()
            .manage(state)
            .build(mock_context(noop_assets()))
            .unwrap();
        tauri::WebviewWindowBuilder::new(&app, MAIN_WINDOW, Default::default())
            .build()
            .unwrap();
        let status =
            tauri::async_runtime::block_on(credits.gateway.start(credits.clone(), 0)).unwrap();
        let port = status.port;
        let closes = Arc::new(AtomicUsize::new(0));
        let observed_closes = closes.clone();
        let running_credits = credits.clone();

        app.run_return(move |app, event| {
            handle_run_event(app, &event);
            match event {
                RunEvent::Ready => app
                    .get_webview_window(MAIN_WINDOW)
                    .unwrap()
                    .close()
                    .unwrap(),
                RunEvent::WindowEvent {
                    label,
                    event: WindowEvent::CloseRequested { .. },
                    ..
                } if label == MAIN_WINDOW => {
                    assert!(running_credits.gateway.status(port).running);
                    let response = tauri::async_runtime::block_on(async {
                        reqwest::Client::builder()
                            .no_proxy()
                            .timeout(std::time::Duration::from_secs(2))
                            .build()
                            .unwrap()
                            .get(format!("http://127.0.0.1:{port}/health"))
                            .send()
                            .await
                            .unwrap()
                    });
                    assert!(response.status().is_success());
                    restore_main_window(app);
                    let window = app.get_webview_window(MAIN_WINDOW).unwrap();
                    if observed_closes.fetch_add(1, Ordering::SeqCst) == 0 {
                        window.close().unwrap();
                    } else {
                        // MockRuntime 不支持 app.exit；destroy 触发实际的退出事件。
                        window.destroy().unwrap();
                    }
                }
                RunEvent::ExitRequested { .. } => {
                    assert!(running_credits.gateway.status(port).running);
                }
                RunEvent::Exit => assert!(!running_credits.gateway.status(port).running),
                _ => {}
            }
        });

        assert_eq!(closes.load(Ordering::SeqCst), 2);
        assert!(!credits.gateway.status(port).running);
        assert!(std::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).is_err());
    }

    #[test]
    fn secondary_window_close_is_not_prevented() {
        let app = mock_builder().build(mock_context(noop_assets())).unwrap();
        tauri::WebviewWindowBuilder::new(&app, "secondary", Default::default())
            .build()
            .unwrap();
        let closes = Arc::new(AtomicUsize::new(0));
        let observed_closes = closes.clone();
        let forced_destroys = Arc::new(AtomicUsize::new(0));
        let observed_destroys = forced_destroys.clone();

        app.run_return(move |app, event| {
            handle_run_event(app, &event);
            match event {
                RunEvent::Ready => app
                    .get_webview_window("secondary")
                    .unwrap()
                    .close()
                    .unwrap(),
                RunEvent::WindowEvent {
                    event: WindowEvent::CloseRequested { .. },
                    ..
                } => {
                    observed_closes.fetch_add(1, Ordering::SeqCst);
                    // 确保错误拦截时测试也能结束；正常关闭会先退出，不执行此消息。
                    app.get_webview_window("secondary")
                        .unwrap()
                        .destroy()
                        .unwrap();
                }
                RunEvent::WindowEvent {
                    event: WindowEvent::Destroyed,
                    ..
                } => {
                    observed_destroys.fetch_add(1, Ordering::SeqCst);
                }
                _ => {}
            }
        });

        assert_eq!(closes.load(Ordering::SeqCst), 1);
        assert_eq!(forced_destroys.load(Ordering::SeqCst), 0);
    }
}
