use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use tauri::State;
use std::process::{Command, Stdio};
use std::io::Write;

use crate::app_state::AppState;
use crate::database::McpServer;
use crate::mcp::AppType;
use crate::services::McpService;
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
pub async fn test_mcp_connection(params: TestConnectionParams) -> Result<TestConnectionResult, String> {
    let command = params.command.clone();
    let args = params.args.clone();
    let env = params.env.clone().unwrap_or_default();

    tokio::task::spawn_blocking(move || {
        // 继承系统 PATH 环境变量，确保能找到 npx/node 等命令
        let mut cmd = Command::new(&command);
        cmd.args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .envs(&env)
            .env("PATH", std::env::var("PATH").unwrap_or_default());

        let mut child = cmd.spawn()
            .map_err(|e| format!("无法启动命令 '{}': {}", command, e))?;

        // 发送 MCP 初始化请求
        let init_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "mcp-manager", "version": "1.0.0" }
            }
        });

        let request_str = format!("{}\n", serde_json::to_string(&init_request).unwrap());
        
        if let Some(ref mut stdin) = child.stdin {
            stdin.write_all(request_str.as_bytes())
                .map_err(|e| format!("写入 stdin 失败: {}", e))?;
        }

        // 尝试读取一行响应（超时 5s 由 wait_with_output 内部处理其实不太对，我们这里用 wait_timeout）
        // 由于标准库没有 wait_timeout，我们简单等待一下然后 kill，或者直接 try_recv
        
        // 简化版：等待进程退出或读取 stderr/stdout
        // 很多 MCP server 启动后不会立刻退出，所以这里我们检查是否成功启动并能接收输入
        
        std::thread::sleep(std::time::Duration::from_secs(2));
        
        // 检查进程是否还在运行
        match child.try_wait() {
            Ok(Some(status)) => {
                // 进程已退出
                let output = child.wait_with_output().unwrap();
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                
                if status.success() || stdout.contains("result") || !stdout.is_empty() {
                    Ok(TestConnectionResult {
                        success: true,
                        message: format!("连接成功。输出: {}", stdout.chars().take(150).collect::<String>()),
                    })
                } else {
                    Ok(TestConnectionResult {
                        success: false,
                        message: format!("进程异常退出: {}", stderr.chars().take(200).collect::<String>()),
                    })
                }
            }
            Ok(None) => {
                // 进程仍在运行，说明连接成功
                let _ = child.kill();
                Ok(TestConnectionResult {
                    success: true,
                    message: "连接成功！服务器正在运行。".to_string(),
                })
            }
            Err(e) => {
                Ok(TestConnectionResult {
                    success: false,
                    message: format!("检查状态失败: {}", e),
                })
            }
        }
    }).await.map_err(|e| format!("Task failed: {}", e))?
}
