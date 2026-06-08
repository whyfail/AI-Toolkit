import { useState, useEffect } from "react";
import { Toaster, toast } from "sonner";
import UnifiedMcpPanel from "@/components/mcp/UnifiedMcpPanel";
import UpdateModal from "@/components/mcp/UpdateModal";
import SkillsPanel from "@/components/skills/SkillsPanel";
import ToolManagerPanel from "@/components/tool-manager/ToolManagerPanel";
import EnhancementsPanel from "@/components/enhancements/EnhancementsPanel";
import {
  Database,
  Settings,
  Info,
  ArrowUpCircle,
  CheckCircle,
  Loader2,
  Github,
  ExternalLink,
  Package,
  Sparkles,
  Share2,
  Wrench,
} from "lucide-react";
import { open } from "@tauri-apps/plugin-shell";
import { useAppVersion } from "@/hooks/useAppVersion";
import { appApi, updateApi } from "@/lib/api";
import type { AppConfigInfo, LaunchPreferences } from "@/types";
import appLogo from "../src-tauri/icons/128x128.png";

type Tab = "mcp" | "skills" | "tools" | "enhancements" | "settings" | "about";
const GITHUB_REPO_URL = "https://github.com/whyfail/ai-toolkit";
const OFFICIAL_WEBSITE_URL = "https://whyfail.github.io/ai-toolkit-website/";
let startupUpdateCheckStarted = false;

const copyText = async (text: string) => {
  try {
    await navigator.clipboard?.writeText(text);
    return;
  } catch {
    // Fall through to the textarea fallback below.
  }

  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  document.body.appendChild(textarea);
  textarea.focus();
  textarea.select();
  document.execCommand("copy");
  document.body.removeChild(textarea);
};

function App() {
  const [activeTab, setActiveTab] = useState<Tab>("tools");
  const [startupUpdateInfo, setStartupUpdateInfo] = useState<{
    version: string;
    body: string;
  } | null>(null);
  const [showStartupUpdateModal, setShowStartupUpdateModal] = useState(false);
  const [startupInstalling, setStartupInstalling] = useState(false);
  const appVersion = useAppVersion();

  // 固定浅色主题。
  useEffect(() => {
    document.documentElement.classList.remove("dark");
  }, []);

  // 首次打开应用时自动检查新版本，有更新时交给用户决定是否安装。
  useEffect(() => {
    if (startupUpdateCheckStarted) return;
    startupUpdateCheckStarted = true;

    let cancelled = false;

    const checkStartupUpdate = async () => {
      try {
        const result = await updateApi.checkUpdate();

        if (!cancelled && result.available) {
          setStartupUpdateInfo({
            version: result.version,
            body: result.body || "",
          });
          setShowStartupUpdateModal(true);
        }
      } catch (err) {
        console.error("启动时检查更新失败:", err);
      }
    };

    checkStartupUpdate();

    return () => {
      cancelled = true;
    };
  }, []);

  const installStartupUpdate = async () => {
    setStartupInstalling(true);
    try {
      await updateApi.installUpdate();
      toast.success("更新下载完成，正在重启应用...");
    } catch (err) {
      console.error("安装更新失败:", err);
      toast.error(`安装更新失败: ${err}`);
    } finally {
      setStartupInstalling(false);
    }
  };

  const navItems = [
    { id: "tools" as Tab, label: "工具管理", icon: Package },
    { id: "skills" as Tab, label: "Skills 管理", icon: Sparkles },
    { id: "mcp" as Tab, label: "MCP 服务器", icon: Database },
    { id: "enhancements" as Tab, label: "增强中心", icon: Wrench },
    { id: "settings" as Tab, label: "设置", icon: Settings },
    { id: "about" as Tab, label: "关于", icon: Info },
  ];

  return (
    <div className="glass-app flex h-full">
      {/* 侧边栏 */}
      <aside className="glass-sidebar z-10 flex w-[260px] flex-col border-y-0 border-l-0">
        {/* Logo */}
        <div className="px-6 pt-6 pb-5">
          <div className="flex items-center gap-3">
            <div className="flex h-11 w-11 items-center justify-center rounded-2xl border border-white/70 bg-white/70 shadow-lg shadow-blue-500/15 backdrop-blur-xl">
              <img
                src={appLogo}
                alt="AI Toolkit"
                className="h-8 w-8 rounded-xl"
              />
            </div>
            <div>
              <h1 className="text-base font-semibold tracking-tight text-slate-950 dark:text-white">
                AI Toolkit
              </h1>
              <p className="mt-0.5 text-xs text-slate-500 dark:text-slate-400">
                AI 编程工具管理
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
              className={`flex min-h-11 w-full items-center gap-3 rounded-xl px-3 py-2.5 text-sm font-semibold transition-all duration-200 ${
                activeTab === item.id
                  ? "bg-gradient-to-r from-blue-600 to-sky-500 text-white shadow-lg shadow-blue-500/20"
                  : "text-slate-500 hover:bg-white/60 hover:text-slate-950 dark:text-slate-400 dark:hover:bg-white/10 dark:hover:text-white"
              }`}
            >
              <item.icon size={18} />
              <span>{item.label}</span>
            </button>
          ))}
        </nav>

        {/* 版本 */}
        <div className="border-t border-white/50 px-6 py-4 text-center">
          <p className="text-xs font-medium text-slate-400 dark:text-slate-500">v{appVersion}</p>
        </div>
      </aside>

      {/* 主内容区 */}
      <main className="flex-1 overflow-hidden">
        {activeTab === "tools" && <ToolManagerPanel />}
        {activeTab === "skills" && <SkillsPanel />}
        {activeTab === "mcp" && <UnifiedMcpPanel />}
        {activeTab === "enhancements" && <EnhancementsPanel />}
        {activeTab === "settings" && <SettingsTab />}
        {activeTab === "about" && <AboutTab />}
      </main>

      {/* Toast 通知 */}
      <Toaster position="top-right" richColors closeButton />

      <UpdateModal
        open={showStartupUpdateModal}
        onClose={() => setShowStartupUpdateModal(false)}
        version={startupUpdateInfo?.version || ""}
        body={startupUpdateInfo?.body || ""}
        onInstall={installStartupUpdate}
        installing={startupInstalling}
      />
    </div>
  );
}

// 设置标签页
const SettingsTab: React.FC = () => {
  const [checking, setChecking] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<{
    version: string;
    body: string;
  } | null>(null);
  const [isLatest, setIsLatest] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [showModal, setShowModal] = useState(false);
  const [apps, setApps] = useState<AppConfigInfo[]>([]);
  const [launchPreferences, setLaunchPreferences] = useState<LaunchPreferences | null>(null);
  const [savingTerminal, setSavingTerminal] = useState(false);
  const appVersion = useAppVersion();
  const isWindows = navigator.userAgent.includes("Windows");
  const isMac = navigator.userAgent.includes("Mac");
  const dbPath = isWindows ? "%USERPROFILE%\\.ai-toolkit\\ai-toolkit.db" : "~/.ai-toolkit/ai-toolkit.db";
  const skillsPath = isWindows ? "%USERPROFILE%\\.ai-toolkit\\skills\\" : "~/.ai-toolkit/skills/";

  const copyShareUrl = async () => {
    try {
      await copyText(OFFICIAL_WEBSITE_URL);
      toast.success("官网地址已复制");
    } catch (err) {
      console.error("复制官网地址失败:", err);
      toast.error("复制失败，请稍后重试");
    }
  };

  useEffect(() => {
    let cancelled = false;

    const loadAppConfigs = async () => {
      try {
        const configs = await appApi.getAppConfigs();
        if (!cancelled) {
          setApps(configs);
        }
      } catch (err) {
        console.error("获取应用配置失败:", err);
        if (!cancelled) {
          toast.error(`获取应用配置失败: ${err}`);
        }
      }
    };

    loadAppConfigs();

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!isMac && !isWindows) return;

    let cancelled = false;
    const loadLaunchPreferences = async () => {
      try {
        const preferences = await appApi.getLaunchPreferences();
        if (!cancelled) {
          setLaunchPreferences(preferences);
        }
      } catch (err) {
        console.error("获取启动偏好失败:", err);
        if (!cancelled) {
          toast.error(`获取启动偏好失败: ${err}`);
        }
      }
    };

    loadLaunchPreferences();

    return () => {
      cancelled = true;
    };
  }, [isMac, isWindows]);

  const handleTerminalChange = async (terminalId: string) => {
    if (!launchPreferences) return;

    const previous = launchPreferences.defaultTerminal;
    setLaunchPreferences({
      ...launchPreferences,
      defaultTerminal: terminalId,
    });
    setSavingTerminal(true);
    try {
      await appApi.setDefaultTerminal(terminalId);
      toast.success("默认启动终端已更新");
    } catch (err) {
      console.error("保存默认终端失败:", err);
      setLaunchPreferences({
        ...launchPreferences,
        defaultTerminal: previous,
      });
      toast.error(`保存默认终端失败: ${err}`);
    } finally {
      setSavingTerminal(false);
    }
  };

  const checkUpdate = async () => {
    setChecking(true);
    setUpdateInfo(null);
    setIsLatest(false);
    try {
      const result = await updateApi.checkUpdate();
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
      toast.error(`检查更新失败: ${err}`);
    } finally {
      setChecking(false);
    }
  };

  const installUpdate = async () => {
    setInstalling(true);
    try {
      await updateApi.installUpdate();
      toast.success("更新下载完成，正在重启应用...");
    } catch (err) {
      console.error("安装更新失败:", err);
      toast.error(`安装更新失败: ${err}`);
    } finally {
      setInstalling(false);
    }
  };

  const settingSections = "glass-card p-6";
  const codeBlock = "glass-code block mt-1 rounded-xl px-3 py-2 text-sm font-mono";

  return (
    <div className="glass-app flex h-full flex-col overflow-hidden">
      {/* 头部 */}
      <div className="glass-header">
        <div className="glass-kicker">
          <Settings size={13} />
          Preferences
        </div>
        <h2 className="mt-3 text-3xl font-semibold tracking-tight">设置</h2>
        <p className="mt-2 text-sm text-slate-500 dark:text-slate-400">
          管理应用配置和数据存储
        </p>
      </div>

      {/* 内容 */}
      <div className="glass-content">
        <div className="max-w-2xl space-y-6">
          {/* 检查更新 */}
          <section className={settingSections}>
            <h3 className="text-base font-medium mb-4">软件更新</h3>
            <div className="flex items-center gap-4">
              <button
                onClick={checkUpdate}
                disabled={checking}
                className="glass-primary-button"
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
            <p className="mt-3 text-xs text-slate-500 dark:text-slate-400">
              当前版本 v{appVersion} · 更新源：GitHub Releases
            </p>
          </section>

          {/* 分享 */}
          <section className={settingSections}>
            <h3 className="text-base font-medium mb-4">分享应用</h3>
            <div className="flex items-center gap-3">
              <button
                onClick={copyShareUrl}
                className="glass-primary-button"
              >
                <Share2 size={16} />
                复制官网地址
              </button>
              <code className="glass-code min-w-0 flex-1 truncate rounded-xl px-3 py-2 text-xs font-mono text-slate-500 dark:text-slate-400">
                {OFFICIAL_WEBSITE_URL}
              </code>
            </div>
          </section>

          {/* 数据库 */}
          <section className={settingSections}>
            <h3 className="text-base font-medium mb-4">数据存储</h3>
            <div className="space-y-3">
              <div>
                <p className="text-sm font-medium text-slate-500 dark:text-slate-400">
                  数据库路径
                </p>
                <code className={codeBlock}>
                  {dbPath}
                </code>
              </div>
              <div>
                <p className="text-sm font-medium text-slate-500 dark:text-slate-400">
                  Skills 列表路径
                </p>
                <code className={codeBlock}>
                  {skillsPath}
                </code>
              </div>
            </div>
          </section>

          {(isMac || isWindows) && launchPreferences && launchPreferences.availableTerminals.length > 0 && (
            <section className={settingSections}>
              <h3 className="text-base font-medium mb-4">默认启动终端</h3>
              <div className="space-y-3">
                <select
                  value={launchPreferences.defaultTerminal}
                  onChange={(e) => handleTerminalChange(e.target.value)}
                  disabled={savingTerminal}
                  className="glass-select w-full px-3 py-2.5 text-sm disabled:opacity-60"
                >
                  {launchPreferences.availableTerminals.map((terminal) => (
                    <option
                      key={terminal.id}
                      value={terminal.id}
                      disabled={!terminal.available}
                    >
                      {terminal.label}{terminal.available ? "" : "（未安装）"}
                    </option>
                  ))}
                </select>
                <p className="text-xs text-slate-500 dark:text-slate-400">
                  {isMac
                    ? "启动 CLI 工具时优先使用这个终端。目前支持 Terminal、iTerm、Warp 和 Ghostty。"
                    : "启动 CLI 工具时优先使用这个终端。目前支持 Windows Terminal、PowerShell 和 Command Prompt。"}
                </p>
              </div>
            </section>
          )}

          {/* 支持的应用 */}
          <section className={settingSections}>
            <h3 className="text-base font-medium mb-4">支持的应用</h3>
            <div className="space-y-2">
              {apps.map((app) => (
                <div
                  key={app.id}
                  className="flex items-center rounded-xl px-3 py-2.5 transition-colors hover:bg-white/60 dark:hover:bg-white/10"
                >
                  <span className="text-sm font-medium">{app.name}</span>
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
  const appVersion = useAppVersion();

  const features = [
    "MCP 服务器统一管理，支持一键启用/禁用",
    "Skills 技能同步到多个 AI 编程工具",
    "自动扫描并导入现有工具配置",
    "跨平台支持（macOS、Windows、Linux）",
    "本地 SQLite 数据库存储，开箱即用",
  ];

  return (
    <div className="glass-app flex h-full flex-col overflow-hidden">
      {/* 头部 */}
      <div className="glass-header">
        <div className="glass-kicker">
          <Info size={13} />
          About
        </div>
        <h2 className="mt-3 text-3xl font-semibold tracking-tight">关于</h2>
        <p className="mt-2 text-sm text-slate-500 dark:text-slate-400">
          了解 AI Toolkit 的更多信息
        </p>
      </div>

      {/* 内容 */}
      <div className="glass-content">
        <div className="max-w-2xl space-y-6">
          {/* 项目信息 */}
          <section className="glass-card p-6">
            <div className="flex items-start justify-between mb-4">
              <div>
                <h3 className="text-base font-medium">AI Toolkit</h3>
                <p className="mt-0.5 text-xs text-slate-500 dark:text-slate-400">
                  v{appVersion}· MCP 和 Skills 管理工具
                </p>
              </div>
              <button
                onClick={() =>
                  open(GITHUB_REPO_URL)
                }
                className="glass-secondary-button min-h-8 px-3 py-1.5 text-xs"
              >
                <Github size={12} />
                GitHub
                <ExternalLink size={10} />
              </button>
            </div>
            <p className="text-sm leading-relaxed text-slate-500 dark:text-slate-400">
              一款基于 Tauri 2 构建的跨平台桌面应用，专注于管理 AI 编程工具的 MCP 服务器配置和 Skills 技能同步。兼容 Qwen Code、Claude Code、Codex、Gemini CLI、OpenCode、Trae、Trae CN、Qoder、CodeBuddy 等主流工具。
            </p>
          </section>

          {/* 核心特性 */}
          <section className="glass-card p-6">
            <h3 className="text-base font-medium mb-4">核心特性</h3>
            <ul className="space-y-2.5">
              {features.map((feature) => (
                <li key={feature} className="flex items-start gap-2.5 text-sm">
                  <div className="mt-1.5 h-1.5 w-1.5 flex-shrink-0 rounded-full bg-blue-500" />
                  <span className="text-slate-700 dark:text-slate-200">
                    {feature}
                  </span>
                </li>
              ))}
            </ul>
          </section>

          {/* 技术栈 */}
          <section className="glass-card p-6">
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
                        className="glass-pill"
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
                    className="glass-pill"
                    >
                      {tech}
                    </span>
                  ))}
                </div>
              </div>
            </div>
          </section>

          {/* 支持与反馈 */}
          <section className="glass-card p-6">
            <h3 className="text-base font-medium mb-3">支持与反馈</h3>
            <div className="space-y-2 text-sm text-slate-500 dark:text-slate-400">
              <p>
                如有问题或建议，欢迎在{" "}
                <button
                  onClick={() =>
                    open(`${GITHUB_REPO_URL}/issues`)
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
