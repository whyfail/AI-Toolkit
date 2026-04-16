import { useState, useEffect, useCallback } from "react";
import { X, Check, AlertCircle, ClipboardPaste, ChevronDown, ChevronUp, Play, Loader2 } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { useUpsertMcpServer } from "@/hooks/useMcp";
import type { McpServer, McpServerSpec } from "@/types";

interface AgentInfo {
  id: string;
  name: string;
}

interface McpFormModalProps {
  editingId?: string;
  initialData?: McpServer;
  installedAgents: AgentInfo[];
  onClose: () => void;
}

const agentColors: Record<string, string> = {
  "qwen-code": "bg-purple-500",
  claude: "bg-orange-500",
  codex: "bg-blue-500",
  gemini: "bg-green-500",
  opencode: "bg-cyan-500",
  trae: "bg-indigo-500",
  "trae-cn": "bg-violet-500",
  "trae-solo-cn": "bg-fuchsia-500",
  qoder: "bg-yellow-500",
  qodercli: "bg-amber-500",
  codebuddy: "bg-red-500",
};

const EXAMPLE_JSON = `{
  "mcpServers": {
    "example-server": {
      "command": "npx",
      "args": ["-y", "mcp-server-example"]
    }
  }
}`;

const McpFormModal: React.FC<McpFormModalProps> = ({
  editingId,
  initialData,
  installedAgents,
  onClose,
}) => {
  const upsertMutation = useUpsertMcpServer();

  // Build default apps state based on installed agents
  const defaultApps: Record<string, boolean> = {};
  installedAgents.forEach((a) => (defaultApps[a.id] = true));

  const [jsonInput, setJsonInput] = useState("");
  const [parseError, setParseError] = useState<string | null>(null);
  const [parsedServer, setParsedServer] = useState<{
    id: string;
    name: string;
    server: McpServerSpec;
  } | null>(null);
  const [selectedApps, setSelectedApps] = useState<Record<string, boolean>>(
    defaultApps
  );
  const [showExample, setShowExample] = useState(false);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isTesting, setIsTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ success: boolean; message: string } | null>(null);

  // Initialize for edit mode
  useEffect(() => {
    if (editingId && initialData) {
      const editJson = {
        mcpServers: {
          [initialData.id]: initialData.server,
        },
      };
      setJsonInput(JSON.stringify(editJson, null, 2));
      setSelectedApps(initialData.apps || defaultApps);
      parseAndSetServer(editJson);
    }
  }, [editingId, initialData]);

  const parseAndSetServer = useCallback((json: any) => {
    try {
      // Support both { "mcpServers": { ... } } and { "server-id": { ... } } formats
      const servers = json.mcpServers || json;
      const keys = Object.keys(servers);

      if (keys.length === 0) {
        setParseError("JSON 格式正确，但未找到 MCP 服务器配置");
        setParsedServer(null);
        return;
      }

      // If multiple servers, just take the first one for simplicity or show a note
      const serverId = keys[0];
      const serverConfig = servers[serverId];

      // Basic validation
      if (!serverConfig.command && !serverConfig.url && !serverConfig.httpUrl) {
        setParseError("配置缺少必要字段 (command 或 url/httpUrl)");
        setParsedServer(null);
        return;
      }

      setParseError(null);
      setParsedServer({
        id: serverId,
        name: serverConfig.name || serverId,
        server: serverConfig as McpServerSpec,
      });
    } catch (e: any) {
      setParseError(e.message);
      setParsedServer(null);
    }
  }, []);

  const handleTestConnection = async () => {
    if (!parsedServer) return;

    setIsTesting(true);
    setTestResult(null);
    try {
      const result = await invoke("test_mcp_connection", {
        params: {
          command: parsedServer.server.command || "",
          args: parsedServer.server.args || [],
          env: parsedServer.server.env || {},
        },
      });
      setTestResult(result as { success: boolean; message: string });
    } catch (e: any) {
      setTestResult({ success: false, message: String(e) });
    } finally {
      setIsTesting(false);
    }
  };

  const handleJsonChange = (value: string) => {
    setJsonInput(value);
    try {
      const parsed = JSON.parse(value);
      parseAndSetServer(parsed);
    } catch {
      setParseError("JSON 格式错误，请检查语法");
      setParsedServer(null);
    }
  };

  const toggleApp = (agentId: string) => {
    setSelectedApps((prev) => ({
      ...prev,
      [agentId]: !prev[agentId],
    }));
  };

  const toggleAllApps = () => {
    const allEnabled = installedAgents.every((a) => selectedApps[a.id]);
    const newState = !allEnabled;
    const newApps: Record<string, boolean> = {};
    installedAgents.forEach((a) => (newApps[a.id] = newState));
    setSelectedApps(newApps);
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!parsedServer) return;
    if (!Object.values(selectedApps).some(Boolean)) {
      alert("请至少选择一个目标工具");
      return;
    }

    setIsSubmitting(true);

    const server: McpServer = {
      id: parsedServer.id,
      name: parsedServer.name,
      server: parsedServer.server,
      apps: selectedApps as any,
      description: parsedServer.server.description,
      homepage: parsedServer.server.homepage,
      docs: parsedServer.server.docs,
      tags: parsedServer.server.tags || [],
    };

    try {
      await upsertMutation.mutateAsync(server);
      onClose();
    } catch (error) {
      console.error("Failed to save:", error);
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div className="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50 p-2 sm:p-4 animate-in fade-in duration-200">
      <div className="bg-[hsl(var(--card))] rounded-2xl w-full max-w-3xl max-h-[90vh] sm:max-h-[85vh] overflow-hidden shadow-2xl border border-[hsl(var(--border))] flex flex-col">
        {/* 头部 */}
        <div className="flex items-center justify-between px-4 sm:px-6 py-4 sm:py-5 border-b border-[hsl(var(--border))] flex-shrink-0">
          <div className="min-w-0">
            <h2 className="text-base sm:text-lg font-semibold truncate">
              {editingId ? "编辑服务器" : "添加 MCP 服务器"}
            </h2>
            {!editingId && (
              <p className="text-xs text-[hsl(var(--muted-foreground))] mt-0.5">
                粘贴 JSON 配置快速添加
              </p>
            )}
          </div>
          <button
            onClick={onClose}
            className="p-2 hover:bg-[hsl(var(--muted))] rounded-lg transition-colors flex-shrink-0"
          >
            <X size={18} className="text-[hsl(var(--muted-foreground))]" />
          </button>
        </div>

        {/* 表单内容 */}
        <form
          onSubmit={handleSubmit}
          className="flex-1 overflow-y-auto px-4 sm:px-6 py-4 sm:py-5 space-y-4 sm:space-y-5 min-h-0"
        >
          {/* JSON 输入区 */}
          <div className="space-y-2">
            <div className="flex items-center justify-between gap-2">
              <label className="text-sm font-medium flex items-center gap-2">
                <ClipboardPaste size={14} />
                MCP 配置 JSON
              </label>
              <button
                type="button"
                onClick={() => setShowExample(!showExample)}
                className="text-xs text-[hsl(var(--primary))] hover:underline flex items-center gap-1 flex-shrink-0"
              >
                {showExample ? "收起示例" : "查看示例"}
                {showExample ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
              </button>
            </div>
            <div className="relative">
              <textarea
                value={jsonInput}
                onChange={(e) => handleJsonChange(e.target.value)}
                placeholder={`请从 MCP 介绍页面复制配置 JSON (如 Claude Desktop/Settings.json)，粘贴到此处...\n\n支持格式:\n{ "mcpServers": { "server-id": { "command": "...", "args": [] } } }\n或\n{ "server-id": { "command": "...", "args": [] } }`}
                className={`w-full px-3 sm:px-4 py-3 bg-[hsl(var(--muted))] border rounded-lg text-xs sm:text-sm font-mono leading-relaxed focus:outline-none focus:ring-2 focus:ring-[hsl(var(--ring))] focus:border-transparent transition-all resize-y ${
                  parseError
                    ? "border-red-500/50"
                    : parsedServer
                    ? "border-green-500/50"
                    : "border-[hsl(var(--border))]"
                }`}
                rows={8}
              />
              {/* 状态提示 */}
              {parseError && (
                <div className="absolute bottom-3 right-3 flex items-center gap-1.5 text-xs text-red-500 bg-[hsl(var(--card))] px-2 py-1 rounded shadow-sm max-w-[80%] truncate">
                  <AlertCircle size={12} className="flex-shrink-0" />
                  {parseError}
                </div>
              )}
              {parsedServer && (
                <div className="absolute bottom-3 right-3 flex items-center gap-1.5 text-xs text-green-500 bg-[hsl(var(--card))] px-2 py-1 rounded shadow-sm">
                  <Check size={12} />
                  已解析: {parsedServer.name}
                </div>
              )}
            </div>

            {/* 示例代码 */}
            {showExample && (
              <div className="rounded-lg border border-[hsl(var(--border))] bg-[hsl(var(--muted))/30] p-3">
                <p className="text-xs text-[hsl(var(--muted-foreground))] mb-2">
                  // 示例:
                </p>
                <pre className="text-xs font-mono text-[hsl(var(--foreground))] overflow-x-auto">
                  {EXAMPLE_JSON}
                </pre>
              </div>
            )}
          </div>

          {/* 解析结果预览 */}
          {parsedServer && (
            <div className="rounded-xl border border-[hsl(var(--primary))/20 bg-[hsl(var(--primary))/5] p-3 sm:p-4 space-y-3">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <h3 className="text-sm font-medium text-[hsl(var(--primary))]">
                  配置解析成功
                </h3>
                {parsedServer.server.command && (
                  <button
                    type="button"
                    onClick={handleTestConnection}
                    disabled={isTesting}
                    className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-all ${
                      testResult?.success
                        ? "bg-green-500/20 text-green-500"
                        : testResult?.success === false
                        ? "bg-red-500/20 text-red-500"
                        : "bg-[hsl(var(--card))] text-[hsl(var(--primary))] hover:brightness-[0.95] active:brightness-[0.9] border border-[hsl(var(--border))]"
                    } disabled:opacity-50`}
                  >
                    {isTesting ? (
                      <Loader2 size={12} className="animate-spin" />
                    ) : testResult?.success ? (
                      <Check size={12} />
                    ) : testResult?.success === false ? (
                      <AlertCircle size={12} />
                    ) : (
                      <Play size={12} />
                    )}
                    {isTesting ? "测试中..." : testResult?.success ? "测试通过" : testResult?.success === false ? "测试失败" : "测试连接"}
                  </button>
                )}
              </div>

              {testResult && (
                <div className={`text-xs p-2 rounded border break-all ${testResult.success ? "bg-green-500/10 border-green-500/20 text-green-600" : "bg-red-500/10 border-red-500/20 text-red-600"}`}>
                  {testResult.message}
                </div>
              )}

              <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 sm:gap-4 text-sm">
                <div>
                  <span className="text-[hsl(var(--muted-foreground))]">ID</span>
                  <p className="font-mono text-xs mt-0.5 break-all">{parsedServer.id}</p>
                </div>
                <div>
                  <span className="text-[hsl(var(--muted-foreground))]">名称</span>
                  <p className="text-xs mt-0.5 truncate">{parsedServer.name}</p>
                </div>
              </div>
              <div className="text-xs font-mono bg-[hsl(var(--card))] rounded p-3 border border-[hsl(var(--border))]">
                <div className="flex justify-between text-[hsl(var(--muted-foreground))] mb-1">
                  <span>命令</span>
                </div>
                <div className="text-[hsl(var(--foreground))] break-all">
                  {parsedServer.server.command || "N/A"}{" "}
                  {parsedServer.server.args?.join(" ")}
                </div>
              </div>
            </div>
          )}

          {/* 集成到工具 */}
          <div className="rounded-xl border border-[hsl(var(--border))] bg-[hsl(var(--muted))/30] p-3 sm:p-5 space-y-3">
            <div className="flex items-center justify-between">
              <label className="text-sm font-medium">集成到工具</label>
              {installedAgents.length > 0 && (
                <button
                  type="button"
                  onClick={toggleAllApps}
                  className="text-xs text-[hsl(var(--primary))] hover:underline flex-shrink-0"
                >
                  {Object.values(selectedApps).every(Boolean) ? "取消全选" : "全选"}
                </button>
              )}
            </div>
            {installedAgents.length > 0 ? (
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                {installedAgents.map((agent) => {
                  const enabled = selectedApps[agent.id] ?? false;
                  return (
                    <button
                      key={agent.id}
                      type="button"
                      onClick={() => toggleApp(agent.id)}
                      className={`flex items-center gap-3 px-3 py-2.5 rounded-lg border transition-all text-left ${
                        enabled
                          ? "border-[hsl(var(--primary))] bg-[hsl(var(--primary))/5]"
                          : "border-[hsl(var(--border))] bg-[hsl(var(--card))] hover:border-[hsl(var(--ring))]"
                      }`}
                    >
                      <div
                        className={`w-4 h-4 rounded flex items-center justify-center flex-shrink-0 transition-colors ${
                          enabled
                            ? agentColors[agent.id]
                            : "bg-[hsl(var(--muted))] border border-[hsl(var(--border))]"
                        }`}
                      >
                        {enabled && <Check size={12} className="text-white" />}
                      </div>
                      <span className="text-sm">{agent.name}</span>
                    </button>
                  );
                })}
              </div>
            ) : (
              <p className="text-sm text-[hsl(var(--muted-foreground))]">
                未检测到已安装的 AI 工具，请先安装相关工具。
              </p>
            )}
          </div>
        </form>

        {/* 按钮 */}
        <div className="flex flex-wrap justify-end gap-2 sm:gap-3 px-4 sm:px-6 py-3 sm:py-4 border-t border-[hsl(var(--border))] bg-[hsl(var(--muted))/30] flex-shrink-0">
          <button
            type="button"
            onClick={onClose}
            className="px-4 sm:px-5 py-2 sm:py-2.5 bg-[hsl(var(--secondary))] hover:brightness-[0.95] active:brightness-[0.9] text-[hsl(var(--secondary-foreground))] rounded-lg text-sm font-medium transition-all border border-[hsl(var(--border))]"
          >
            取消
          </button>
          <button
            onClick={handleSubmit}
            disabled={!parsedServer || isSubmitting || installedAgents.length === 0 || !!(parsedServer.server.command && !testResult?.success)}
            className="px-4 sm:px-5 py-2 sm:py-2.5 bg-[hsl(var(--primary))] hover:brightness-[0.9] active:brightness-[0.85] text-white rounded-lg text-sm font-medium transition-all shadow-sm disabled:opacity-50 disabled:cursor-not-allowed"
            title={parsedServer?.server.command && !testResult?.success ? "请先测试连接成功后再保存" : ""}
          >
            {isSubmitting
              ? "保存中..."
              : editingId
              ? "保存更改"
              : "添加服务器"}
          </button>
        </div>
      </div>
    </div>
  );
};

export default McpFormModal;
