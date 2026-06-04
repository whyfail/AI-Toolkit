use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;
use tauri::State;

use crate::app_state::AppState;
use crate::database::McpServer;
use crate::mcp::AppType;
use crate::services::McpService;
use crate::utils::SuppressConsole;
use std::str::FromStr;

/// 获取所有 MCP 服务器
#[tauri::command]
pub async fn get_mcp_servers(
    state: State<'_, AppState>,
) -> Result<IndexMap<String, McpServer>, String> {
    McpService::get_all_servers(&state).map_err(|e| e.to_string())
}

/// 添加或更新 MCP 服务器
#[tauri::command]
pub async fn upsert_mcp_server(
    state: State<'_, AppState>,
    server: McpServer,
) -> Result<(), String> {
    McpService::upsert_server(&state, server).map_err(|e| e.to_string())
}

/// 删除 MCP 服务器
#[tauri::command]
pub async fn delete_mcp_server(state: State<'_, AppState>, id: String) -> Result<(), String> {
    McpService::delete_server(&state, &id).map_err(|e| e.to_string())
}

/// 切换 MCP 服务器在指定应用的启用状态
#[tauri::command]
pub async fn toggle_mcp_app(
    state: State<'_, AppState>,
    server_id: String,
    app: String,
    enabled: bool,
) -> Result<(), String> {
    let app_ty = AppType::from_str(&app).map_err(|e| e.to_string())?;
    McpService::toggle_app(&state, &server_id, app_ty, enabled).map_err(|e| e.to_string())
}

/// 从所有应用导入 MCP 服务器
#[tauri::command]
pub async fn import_mcp_from_apps(state: State<'_, AppState>) -> Result<usize, String> {
    McpService::import_from_apps(&state).map_err(|e| e.to_string())
}

/// 测试 MCP 服务器连接
#[derive(Serialize)]
pub struct TestConnectionResult {
    pub success: bool,
    pub message: String,
}

#[derive(Deserialize)]
pub struct TestConnectionParams {
    pub command: String,
    pub args: Vec<String>,
    pub env: Option<std::collections::HashMap<String, String>>,
}

#[tauri::command]
pub async fn test_mcp_connection(
    params: TestConnectionParams,
) -> Result<TestConnectionResult, String> {
    let command = params.command.clone();
    let args = params.args.clone();
    let env = params.env.clone().unwrap_or_default();

    if command.trim().is_empty() {
        return Ok(TestConnectionResult {
            success: false,
            message: "当前连接测试仅支持 stdio MCP，请提供 command。".to_string(),
        });
    }

    tokio::task::spawn_blocking(move || {
        // 继承系统 PATH 环境变量，确保能找到 npx/node 等命令
        let mut cmd = Command::new(&command);
        cmd.suppress_console()
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .envs(&env)
            .env("PATH", std::env::var("PATH").unwrap_or_default());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("无法启动命令 '{}': {}", command, e))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "无法读取 MCP 服务器 stdout".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "无法读取 MCP 服务器 stderr".to_string())?;

        let (stdout_tx, stdout_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut line = String::new();
            let mut reader = BufReader::new(stdout);
            let result = reader.read_line(&mut line).map(|_| line);
            let _ = stdout_tx.send(result);
        });

        let (stderr_tx, stderr_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut buffer = String::new();
            let _ = reader.read_line(&mut buffer);
            let _ = stderr_tx.send(buffer);
        });

        // 发送 MCP 初始化请求
        let init_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "ai-toolkit", "version": "1.5.4" }
            }
        });

        let request_str = format!(
            "{}\n",
            serde_json::to_string(&init_request).map_err(|e| e.to_string())?
        );

        if let Some(ref mut stdin) = child.stdin {
            stdin
                .write_all(request_str.as_bytes())
                .map_err(|e| format!("写入 stdin 失败: {}", e))?;
            stdin
                .flush()
                .map_err(|e| format!("刷新 stdin 失败: {}", e))?;
        }

        match stdout_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(line)) => {
                let is_initialize_result = serde_json::from_str::<serde_json::Value>(&line)
                    .ok()
                    .and_then(|value| value.get("result").cloned())
                    .is_some();
                let _ = child.kill();
                let _ = child.wait();

                if is_initialize_result {
                    Ok(TestConnectionResult {
                        success: true,
                        message: "连接成功！服务器返回了 MCP 初始化响应。".to_string(),
                    })
                } else {
                    Ok(TestConnectionResult {
                        success: false,
                        message: format!(
                            "服务器响应不是有效的 MCP 初始化结果: {}",
                            line.chars().take(200).collect::<String>()
                        ),
                    })
                }
            }
            Ok(Err(e)) => {
                let _ = child.kill();
                let _ = child.wait();
                Ok(TestConnectionResult {
                    success: false,
                    message: format!("读取 stdout 失败: {}", e),
                })
            }
            Err(mpsc::RecvTimeoutError::Timeout) => match child.try_wait() {
                Ok(Some(status)) => {
                    let stderr = stderr_rx.try_recv().unwrap_or_default();
                    let _ = child.wait();
                    Ok(TestConnectionResult {
                        success: false,
                        message: format!(
                            "进程已退出但未返回 MCP 初始化响应: {}{}",
                            status,
                            if stderr.is_empty() {
                                String::new()
                            } else {
                                format!("，错误: {}", stderr.chars().take(200).collect::<String>())
                            }
                        ),
                    })
                }
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    Ok(TestConnectionResult {
                        success: false,
                        message: "连接超时：服务器启动了，但 5 秒内没有返回 MCP 初始化响应。"
                            .to_string(),
                    })
                }
                Err(e) => Ok(TestConnectionResult {
                    success: false,
                    message: format!("检查状态失败: {}", e),
                }),
            },
            Err(mpsc::RecvTimeoutError::Disconnected) => match child.try_wait() {
                Ok(Some(status)) => {
                    let stderr = stderr_rx.try_recv().unwrap_or_default();
                    let _ = child.wait();
                    Ok(TestConnectionResult {
                        success: false,
                        message: format!(
                            "进程异常退出: {}{}",
                            status,
                            if stderr.is_empty() {
                                String::new()
                            } else {
                                format!("，错误: {}", stderr.chars().take(200).collect::<String>())
                            }
                        ),
                    })
                }
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    Ok(TestConnectionResult {
                        success: false,
                        message: "stdout 读取已结束，但服务器没有返回初始化响应。".to_string(),
                    })
                }
                Err(e) => Ok(TestConnectionResult {
                    success: false,
                    message: format!("检查状态失败: {}", e),
                }),
            },
        }
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?
}
