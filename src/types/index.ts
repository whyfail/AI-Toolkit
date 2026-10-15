import { SUPPORTED_TOOLS } from "@/lib/tools";

// MCP 服务器连接参数
export interface McpServerSpec {
  type?: "stdio" | "http" | "sse";
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  cwd?: string;
  url?: string;
  headers?: Record<string, string>;
  [key: string]: any;
}

// 应用启用状态
export type McpApps = Record<string, boolean>;

// MCP 服务器条目
export interface McpServer {
  id: string;
  name: string;
  server: McpServerSpec;
  apps: McpApps;
  description?: string;
  homepage?: string;
  docs?: string;
  tags?: string[];
}

// MCP 服务器映射
export type McpServersMap = Record<string, McpServer>;

// 应用 ID 类型
export type AppId = keyof McpApps;

// 应用配置信息
export interface AppConfigInfo {
  id: string;
  name: string;
  configPath: string;
  mcpCount: number;
}

// 产品列表与 Skills 使用相同的 Agent 定义。
export const SUPPORTED_APPS = SUPPORTED_TOOLS.map(tool => ({
  id: tool.id,
  name: tool.displayName,
  icon: tool.id,
}));
