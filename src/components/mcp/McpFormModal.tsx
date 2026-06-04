import { useState, useEffect, useCallback, useMemo } from "react";
import { X, Check, AlertCircle, ClipboardPaste, ChevronDown, ChevronUp, Play, Loader2 } from "lucide-react";
import { useUpsertMcpServer } from "@/hooks/useMcp";
import { mcpApi } from "@/lib/api";
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
  const defaultApps = useMemo(() => {
    const apps: Record<string, boolean> = {};
    installedAgents.forEach((a) => (apps[a.id] = true));
    return apps;
  }, [installedAgents]);

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
  }, [defaultApps, editingId, initialData, parseAndSetServer]);

  const handleTestConnection = async () => {
    if (!parsedServer) return;

    setIsTesting(true);
    setTestResult(null);
    try {
      const result = await mcpApi.testConnection({
        command: parsedServer.server.command || "",
        args: parsedServer.server.args || [],
        env: parsedServer.server.env || {},
      });
      setTestResult(result);
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
      <div className="glass-modal flex max-h-[90vh] w-full max-w-3xl flex-col overflow-hidden rounded-2xl sm:max-h-[85vh]">
        {/* 头部 */}
        <div className="flex flex-shrink-0 items-center justify-between border-b border-white/50 px-4 py-4 dark:border-white/10 sm:px-6 sm:py-5">
          <div className="min-w-0">
            <h2 className="text-base sm:text-lg font-semibold truncate">
              {editingId ? "编辑服务器" : "添加 MCP 服务器"}
            </h2>
            {!editingId && (
              <p className="mt-0.5 text-xs text-slate-500 dark:text-slate-400">
                粘贴 JSON 配置快速添加
              </p>
            )}
          </div>
          <button
            onClick={onClose}
            className="glass-icon-button flex-shrink-0"
          >
            <X size={18} />
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
              <label htmlFor="mcp-config-json" className="text-sm font-medium flex items-center gap-2">
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
                id="mcp-config-json"
                value={jsonInput}
                onChange={(e) => handleJsonChange(e.target.value)}
                placeholder={`请从 MCP 介绍页面复制配置 JSON (如 Claude Desktop/Settings.json)，粘贴到此处...\n\n支持格式:\n{ "mcpServers": { "server-id": { "command": "...", "args": [] } } }\n或\n{ "server-id": { "command": "...", "args": [] } }`}
                className={`glass-input w-full resize-y px-3 py-3 font-mono text-xs leading-relaxed sm:px-4 sm:text-sm ${
                  parseError
                    ? "border-red-500/50"
                    : parsedServer
                    ? "border-green-500/50"
                    : "border-white/60 dark:border-white/10"
                }`}
                rows={8}
              />
              {/* 状态提示 */}
              {parseError && (
                <div className="absolute bottom-3 right-3 flex max-w-[80%] items-center gap-1.5 truncate rounded bg-white/80 px-2 py-1 text-xs text-red-500 shadow-sm backdrop-blur-xl dark:bg-slate-950/80">
                  <AlertCircle size={12} className="flex-shrink-0" />
                  {parseError}
                </div>
              )}
              {parsedServer && (
                <div className="absolute bottom-3 right-3 flex items-center gap-1.5 rounded bg-white/80 px-2 py-1 text-xs text-green-500 shadow-sm backdrop-blur-xl dark:bg-slate-950/80">
                  <Check size={12} />
                  已解析: {parsedServer.name}
                </div>
              )}
            </div>

            {/* 示例代码 */}
            {showExample && (
              <div className="glass-code rounded-xl p-3">
                <p className="mb-2 text-xs text-slate-500 dark:text-slate-400">
                  {"// 示例:"}
                </p>
                <pre className="overflow-x-auto text-xs font-mono">
                  {EXAMPLE_JSON}
                </pre>
              </div>
            )}
          </div>

          {/* 解析结果预览 */}
          {parsedServer && (
            <div className="rounded-xl border border-blue-200/70 bg-blue-500/10 p-3 dark:border-sky-300/20 sm:p-4 space-y-3">
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
                        : "glass-secondary-button min-h-8 px-3 py-1.5 text-xs text-blue-700 dark:text-sky-300"
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
                  <span className="text-slate-500 dark:text-slate-400">ID</span>
                  <p className="font-mono text-xs mt-0.5 break-all">{parsedServer.id}</p>
                </div>
                <div>
                  <span className="text-slate-500 dark:text-slate-400">名称</span>
                  <p className="text-xs mt-0.5 truncate">{parsedServer.name}</p>
                </div>
              </div>
              <div className="glass-code rounded-xl p-3 text-xs font-mono">
                <div className="mb-1 flex justify-between text-slate-500 dark:text-slate-400">
                  <span>命令</span>
                </div>
                <div className="break-all">
                  {parsedServer.server.command || "N/A"}{" "}
                  {parsedServer.server.args?.join(" ")}
                </div>
              </div>
            </div>
          )}

          {/* 集成到工具 */}
          <div className="glass-card space-y-3 p-3 sm:p-5">
            <div className="flex items-center justify-between">
              <span className="text-sm font-medium">集成到工具</span>
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
                      className={`flex items-center gap-3 rounded-xl border px-3 py-2.5 text-left transition-all ${
                        enabled
                          ? "border-blue-200/70 bg-blue-500/10 dark:border-sky-300/20"
                          : "border-white/55 bg-white/50 hover:bg-white/75 dark:border-white/10 dark:bg-white/8 dark:hover:bg-white/12"
                      }`}
                    >
                      <div
                        className={`w-4 h-4 rounded flex items-center justify-center flex-shrink-0 transition-colors ${
                          enabled
                            ? agentColors[agent.id]
                            : "border border-white/50 bg-white/50 dark:border-white/10 dark:bg-white/8"
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
              <p className="text-sm text-slate-500 dark:text-slate-400">
                未检测到已安装的 AI 工具，请先安装相关工具。
              </p>
            )}
          </div>
        </form>

        {/* 按钮 */}
        <div className="flex flex-shrink-0 flex-wrap justify-end gap-2 border-t border-white/50 bg-white/25 px-4 py-3 dark:border-white/10 dark:bg-white/5 sm:gap-3 sm:px-6 sm:py-4">
          <button
            type="button"
            onClick={onClose}
            className="glass-secondary-button"
          >
            取消
          </button>
          <button
            onClick={handleSubmit}
            disabled={!parsedServer || isSubmitting || installedAgents.length === 0 || !!(parsedServer.server.command && !testResult?.success)}
            className="glass-primary-button"
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
