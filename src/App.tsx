import { useState, useEffect } from "react";
import UnifiedMcpPanel from "@/components/mcp/UnifiedMcpPanel";
import UpdateModal from "@/components/mcp/UpdateModal";
import {
  Database,
  Settings,
  Info,
  Moon,
  Sun,
  Monitor,
  ArrowUpCircle,
  CheckCircle,
  Loader2,
  Github,
  ExternalLink,
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-shell";

type Tab = "mcp" | "settings" | "about";
type Theme = "light" | "dark" | "system";

function App() {
  const [activeTab, setActiveTab] = useState<Tab>("mcp");
  const [theme, setTheme] = useState<Theme>("system");

  // 应用主题
  useEffect(() => {
    const root = document.documentElement;
    
    if (theme === "system") {
      const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
      root.classList.toggle("dark", prefersDark);
    } else {
      root.classList.toggle("dark", theme === "dark");
    }
  }, [theme]);

  // 监听系统主题变化
  useEffect(() => {
    if (theme !== "system") return;
    
    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = () => {
      document.documentElement.classList.toggle("dark", mediaQuery.matches);
    };
    
    mediaQuery.addEventListener("change", handler);
    return () => mediaQuery.removeEventListener("change", handler);
  }, [theme]);

  const navItems = [
    { id: "mcp" as Tab, label: "MCP 服务器", icon: Database },
    { id: "settings" as Tab, label: "设置", icon: Settings },
    { id: "about" as Tab, label: "关于", icon: Info },
  ];

  return (
    <div className="flex h-full bg-[hsl(var(--background))] text-[hsl(var(--foreground))]">
      {/* 侧边栏 */}
      <aside className="w-[260px] border-r border-[hsl(var(--border))] bg-[hsl(var(--card))] flex flex-col">
        {/* Logo */}
        <div className="px-6 pt-6 pb-5">
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-xl bg-[hsl(var(--primary))] flex items-center justify-center shadow-lg shadow-[hsl(var(--primary)/0.2)]">
              <Database size={18} className="text-white" />
            </div>
            <div>
              <h1 className="text-base font-semibold tracking-tight">
                MCP Manager
              </h1>
              <p className="text-xs text-[hsl(var(--muted-foreground))] mt-0.5">
                通用配置管理
              </p>
            </div>
          </div>
        </div>

        {/* 导航 */}
        <nav className="flex-1 px-3 py-2 space-y-1">
          {navItems.map((item) => (
            <button
              key={item.id}
              onClick={() => setActiveTab(item.id)}
              className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium transition-all duration-150 ${
                activeTab === item.id
                  ? "bg-[hsl(var(--primary))] text-white shadow-md shadow-[hsl(var(--primary)/0.15)]"
                  : "text-[hsl(var(--muted-foreground))] hover:bg-[hsl(var(--muted))] hover:text-[hsl(var(--foreground))]"
              }`}
            >
              <item.icon size={18} />
              <span>{item.label}</span>
            </button>
          ))}
        </nav>

        {/* 主题切换 */}
        <div className="px-3 py-4 border-t border-[hsl(var(--border))]">
          <div className="flex items-center gap-1 bg-[hsl(var(--muted))] rounded-lg p-1">
            {(["light", "dark", "system"] as Theme[]).map((t) => (
              <button
                key={t}
                onClick={() => setTheme(t)}
                className={`flex-1 flex items-center justify-center gap-1.5 px-2 py-1.5 rounded-md text-xs font-medium transition-all ${
                  theme === t
                    ? "bg-[hsl(var(--card))] text-[hsl(var(--foreground))] shadow-sm"
                    : "text-[hsl(var(--muted-foreground))] hover:text-[hsl(var(--foreground))]"
                }`}
              >
                {t === "light" && <Sun size={12} />}
                {t === "dark" && <Moon size={12} />}
                {t === "system" && <Monitor size={12} />}
                <span className="hidden sm:inline">
                  {t === "light"
                    ? "浅色"
                    : t === "dark"
                    ? "深色"
                    : "系统"}
                </span>
              </button>
            ))}
          </div>
        </div>

        {/* 版本 */}
        <div className="px-6 py-4 text-center">
          <p className="text-xs text-[hsl(var(--muted-foreground))]">v1.0.2</p>
        </div>
      </aside>

      {/* 主内容区 */}
      <main className="flex-1 overflow-hidden">
        {activeTab === "mcp" && <UnifiedMcpPanel />}
        {activeTab === "settings" && <SettingsTab />}
        {activeTab === "about" && <AboutTab />}
      </main>
    </div>
  );
}

// 设置标签页
const SettingsTab: React.FC = () => {
  const apps = [
    { name: "Qwen Code", path: "~/.qwen/settings.json" },
    { name: "Claude Code", path: "~/.claude.json" },
    { name: "Codex", path: "~/.codex/config.toml" },
    { name: "Gemini CLI", path: "~/.gemini/settings.json" },
    { name: "OpenCode", path: "~/.config/opencode/opencode.json" },
    { name: "OpenClaw", path: "~/.openclaw/openclaw.json" },
    { name: "Trae", path: "~/Library/Application Support/Trae/User/mcp.json" },
    { name: "Trae CN", path: "~/Library/Application Support/Trae CN/User/mcp.json" },
    { name: "Qoder", path: "~/.qoder/settings.json" },
    { name: "CodeBuddy", path: "~/.codebuddy/mcp.json" },
  ];

  const [checking, setChecking] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<{
    version: string;
    body: string;
  } | null>(null);
  const [isLatest, setIsLatest] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [showModal, setShowModal] = useState(false);

  const checkUpdate = async () => {
    setChecking(true);
    setUpdateInfo(null);
    setIsLatest(false);
    try {
      const result = await invoke<{
        available: boolean;
        version: string;
        body: string | null;
      }>("check_update");
      if (result.available) {
        setUpdateInfo({
          version: result.version,
          body: result.body || "",
        });
        setShowModal(true);
      } else {
        setIsLatest(true);
        setTimeout(() => setIsLatest(false), 3000);
      }
    } catch (err) {
      console.error("检查更新失败:", err);
    } finally {
      setChecking(false);
    }
  };

  const installUpdate = async () => {
    setInstalling(true);
    try {
      await invoke("install_update");
    } catch (err) {
      console.error("安装更新失败:", err);
    } finally {
      setInstalling(false);
    }
  };

  return (
    <div className="flex flex-col h-full overflow-hidden">
      {/* 头部 */}
      <div className="px-8 pt-8 pb-6 border-b border-[hsl(var(--border))]">
        <h2 className="text-2xl font-semibold tracking-tight">设置</h2>
        <p className="text-sm text-[hsl(var(--muted-foreground))] mt-1">
          管理应用配置和数据存储
        </p>
      </div>

      {/* 内容 */}
      <div className="flex-1 overflow-y-auto px-8 py-6">
        <div className="max-w-2xl space-y-6">
          {/* 检查更新 */}
          <section className="rounded-xl border border-[hsl(var(--border))] bg-[hsl(var(--card))] p-6">
            <h3 className="text-base font-medium mb-4">软件更新</h3>
            <div className="flex items-center gap-4">
              <button
                onClick={checkUpdate}
                disabled={checking}
                className="flex items-center gap-2 px-4 py-2.5 bg-[hsl(var(--primary))] text-white rounded-lg hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed transition-all text-sm font-medium"
              >
                {checking ? (
                  <Loader2 size={16} className="animate-spin" />
                ) : (
                  <ArrowUpCircle size={16} />
                )}
                {checking ? "检查中..." : "检查更新"}
              </button>
              {isLatest && (
                <span className="flex items-center gap-1.5 text-sm text-emerald-600 dark:text-emerald-400">
                  <CheckCircle size={14} />
                  已是最新版本
                </span>
              )}
            </div>
            <p className="text-xs text-[hsl(var(--muted-foreground))] mt-3">
              当前版本 v1.0.2 · 更新源：GitHub Releases
            </p>
          </section>

          {/* 数据库 */}
          <section className="rounded-xl border border-[hsl(var(--border))] bg-[hsl(var(--card))] p-6">
            <h3 className="text-base font-medium mb-4">数据存储</h3>
            <div className="space-y-3">
              <div>
                <p className="text-sm font-medium text-[hsl(var(--muted-foreground))]">
                  数据库路径
                </p>
                <code className="block mt-1 px-3 py-2 bg-[hsl(var(--muted))] rounded-lg text-sm font-mono">
                  ~/.mcp-manager/mcp-manager.db
                </code>
              </div>
            </div>
          </section>

          {/* 支持的应用 */}
          <section className="rounded-xl border border-[hsl(var(--border))] bg-[hsl(var(--card))] p-6">
            <h3 className="text-base font-medium mb-4">支持的应用</h3>
            <div className="space-y-2">
              {apps.map((app) => (
                <div
                  key={app.name}
                  className="flex items-center justify-between py-2.5 px-3 rounded-lg hover:bg-[hsl(var(--muted))] transition-colors"
                >
                  <span className="text-sm font-medium">{app.name}</span>
                  <code className="text-xs text-[hsl(var(--muted-foreground))] bg-[hsl(var(--muted))] px-2 py-1 rounded">
                    {app.path}
                  </code>
                </div>
              ))}
            </div>
          </section>
        </div>
      </div>

      {/* 更新弹窗 */}
      <UpdateModal
        open={showModal}
        onClose={() => setShowModal(false)}
        version={updateInfo?.version || ""}
        body={updateInfo?.body || ""}
        onInstall={installUpdate}
        installing={installing}
      />
    </div>
  );
};

// 关于标签页
const AboutTab: React.FC = () => {
  const features = [
    "统一管理多个 AI CLI 工具的 MCP 服务器",
    "一键启用/禁用特定应用的服务器",
    "从现有配置自动导入",
    "现代化可视化界面",
    "SQLite 数据库持久化存储",
    "支持 10 种 AI 开发工具",
    "在线自动更新功能",
  ];

  return (
    <div className="flex flex-col h-full overflow-hidden">
      {/* 头部 */}
      <div className="px-8 pt-8 pb-6 border-b border-[hsl(var(--border))]">
        <h2 className="text-2xl font-semibold tracking-tight">关于</h2>
        <p className="text-sm text-[hsl(var(--muted-foreground))] mt-1">
          了解 MCP Manager 的更多信息
        </p>
      </div>

      {/* 内容 */}
      <div className="flex-1 overflow-y-auto px-8 py-6">
        <div className="max-w-2xl space-y-6">
          {/* 项目信息 */}
          <section className="rounded-xl border border-[hsl(var(--border))] bg-[hsl(var(--card))] p-6">
            <div className="flex items-start justify-between mb-4">
              <div>
                <h3 className="text-base font-medium">MCP Manager</h3>
                <p className="text-xs text-[hsl(var(--muted-foreground))] mt-0.5">
                  v1.0.2 · 通用 MCP 配置管理工具
                </p>
              </div>
              <button
                onClick={() =>
                  open("https://github.com/whyfail/mcp-manager")
                }
                className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-[hsl(var(--border))] text-xs font-medium hover:bg-[hsl(var(--muted))] transition-colors"
              >
                <Github size={12} />
                GitHub
                <ExternalLink size={10} className="text-[hsl(var(--muted-foreground))]" />
              </button>
            </div>
            <p className="text-sm text-[hsl(var(--muted-foreground))] leading-relaxed">
              基于 Tauri 2 构建的跨平台桌面应用，用于统一管理多个 AI 编程工具的
              MCP (Model Context Protocol) 服务器配置。支持 Qwen Code、Claude
              Code、Codex、Gemini CLI、OpenCode、OpenClaw、Trae、Trae CN、Qoder
              和 CodeBuddy。
            </p>
          </section>

          {/* 核心特性 */}
          <section className="rounded-xl border border-[hsl(var(--border))] bg-[hsl(var(--card))] p-6">
            <h3 className="text-base font-medium mb-4">核心特性</h3>
            <ul className="space-y-2.5">
              {features.map((feature, i) => (
                <li key={i} className="flex items-start gap-2.5 text-sm">
                  <div className="w-1.5 h-1.5 rounded-full bg-[hsl(var(--primary))] mt-1.5 flex-shrink-0" />
                  <span className="text-[hsl(var(--foreground))]">
                    {feature}
                  </span>
                </li>
              ))}
            </ul>
          </section>

          {/* 技术栈 */}
          <section className="rounded-xl border border-[hsl(var(--border))] bg-[hsl(var(--card))] p-6">
            <h3 className="text-base font-medium mb-4">技术栈</h3>
            <div className="grid grid-cols-2 gap-4">
              <div>
                <p className="text-xs font-medium text-[hsl(var(--muted-foreground))] uppercase tracking-wider mb-2">
                  前端
                </p>
                <div className="flex flex-wrap gap-2">
                  {["React", "TypeScript", "TailwindCSS", "TanStack Query"].map(
                    (tech) => (
                      <span
                        key={tech}
                        className="px-2.5 py-1 bg-[hsl(var(--muted))] rounded-md text-xs font-medium"
                      >
                        {tech}
                      </span>
                    )
                  )}
                </div>
              </div>
              <div>
                <p className="text-xs font-medium text-[hsl(var(--muted-foreground))] uppercase tracking-wider mb-2">
                  后端
                </p>
                <div className="flex flex-wrap gap-2">
                  {["Tauri 2", "Rust", "SQLite"].map((tech) => (
                    <span
                      key={tech}
                      className="px-2.5 py-1 bg-[hsl(var(--muted))] rounded-md text-xs font-medium"
                    >
                      {tech}
                    </span>
                  ))}
                </div>
              </div>
            </div>
          </section>

          {/* 支持与反馈 */}
          <section className="rounded-xl border border-[hsl(var(--border))] bg-[hsl(var(--card))] p-6">
            <h3 className="text-base font-medium mb-3">支持与反馈</h3>
            <div className="space-y-2 text-sm text-[hsl(var(--muted-foreground))]">
              <p>
                如有问题或建议，欢迎在{" "}
                <button
                  onClick={() =>
                    open("https://github.com/whyfail/mcp-manager/issues")
                  }
                  className="text-[hsl(var(--primary))] hover:underline inline-flex items-center gap-0.5"
                >
                  GitHub Issues
                  <ExternalLink size={10} />
                </button>{" "}
                提交反馈。
              </p>
            </div>
          </section>
        </div>
      </div>
    </div>
  );
};

export default App;
