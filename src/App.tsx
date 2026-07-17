import { useState, useEffect } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Toaster, toast } from "sonner";
import UnifiedMcpPanel from "@/components/mcp/UnifiedMcpPanel";
import UpdateModal from "@/components/mcp/UpdateModal";
import SkillsPanel from "@/components/skills/SkillsPanel";
import ToolManagerPanel from "@/components/tool-manager/ToolManagerPanel";
import EnhancementsPanel from "@/components/enhancements/EnhancementsPanel";
import { Pressable } from "@/components/ui/Pressable";
import {
  Database,
  Settings,
  Info,
  ArrowUpCircle,
  Check,
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
import { useTheme, type ThemeMode } from "@/hooks/useTheme";
import { useReducedMotion } from "@/hooks/useReducedMotion";
import { appApi, updateApi } from "@/lib/api";
import type { SkillsInstallPreferences } from "@/lib/api";
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
  const theme = useTheme();
  const reduced = useReducedMotion();

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
      <aside className="glass-sidebar z-10 flex w-[240px] flex-col border-y-0 border-l-0">
        {/* Logo */}
        <div className="px-6 pt-6 pb-5">
          <div className="flex items-center gap-3">
            <img
              src={appLogo}
              alt="AI Toolkit"
              className="h-11 w-11 rounded-2xl"
            />
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
        <nav className="relative flex-1 px-3 py-2 space-y-1">
          {navItems.map((item) => {
            const active = activeTab === item.id;
            return (
              <Pressable
                key={item.id}
                onClick={() => setActiveTab(item.id)}
                aria-current={active ? "page" : undefined}
                className={`relative z-10 flex min-h-11 w-full items-center gap-3 rounded-xl px-3 py-2.5 text-sm font-medium transition-colors duration-200 ease-out ${
                  active
                    ? "text-white"
                    : "text-slate-500 hover:text-slate-950 dark:text-slate-400 dark:hover:text-white"
                }`}
              >
                {active && (
                  <motion.span
                    layoutId="sidebar-pill"
                    className="absolute inset-0 -z-10 rounded-xl bg-gradient-to-r from-[#0A84FF] to-[#5AC8FA] shadow-[0_8px_24px_rgba(10,132,255,0.28)]"
                    transition={{ type: "spring", bounce: 0.15, duration: 0.45 }}
                  />
                )}
                <item.icon size={18} />
                <span>{item.label}</span>
              </Pressable>
            );
          })}
        </nav>

        {/* 版本 */}
        <div className="border-t border-white/40 px-6 py-4 text-center dark:border-white/10">
          <p className="text-[11px] font-medium tracking-wide text-slate-400 dark:text-slate-500">
            v{appVersion}
          </p>
        </div>
      </aside>

      {/* 主内容区 */}
      <main className="flex-1 overflow-hidden">
        <AnimatePresence mode="wait">
          <motion.div
            key={activeTab}
            initial={reduced ? { opacity: 0 } : { opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={reduced ? { opacity: 0 } : { opacity: 0, y: -6 }}
            transition={{ type: "spring", bounce: 0, duration: reduced ? 0.18 : 0.25 }}
            className="h-full"
          >
            {activeTab === "tools" && <ToolManagerPanel />}
            {activeTab === "skills" && <SkillsPanel />}
            {activeTab === "mcp" && <UnifiedMcpPanel />}
            {activeTab === "enhancements" && <EnhancementsPanel />}
            {activeTab === "settings" && <SettingsTab theme={theme} />}
            {activeTab === "about" && <AboutTab />}
          </motion.div>
        </AnimatePresence>
      </main>

      {/* Toast 通知 */}
      <Toaster
        position="top-right"
        richColors
        closeButton
        toastOptions={{
          duration: 3500,
          classNames: {
            toast: "!rounded-modal !border !border-white/30 !bg-white/70 !backdrop-blur-xl !shadow-2 dark:!border-white/10 dark:!bg-slate-950/70",
          },
        }}
      />

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
const SettingsTab: React.FC<{ theme: ReturnType<typeof useTheme> }> = ({ theme }) => {
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
  const [skillsInstallPreferences, setSkillsInstallPreferences] = useState<SkillsInstallPreferences | null>(null);
  const [savingSkillsLocation, setSavingSkillsLocation] = useState(false);
  const appVersion = useAppVersion();
  const isWindows = navigator.userAgent.includes("Windows");
  const isMac = navigator.userAgent.includes("Mac");
  const dbPath = isWindows ? "%USERPROFILE%\\.ai-toolkit\\ai-toolkit.db" : "~/.ai-toolkit/ai-toolkit.db";
  const selectedSkillsPath = skillsInstallPreferences?.options.find(
    (option) => option.id === skillsInstallPreferences.selected
  )?.path;

  const [copied, setCopied] = useState(false);

  const copyShareUrl = async () => {
    try {
      await copyText(OFFICIAL_WEBSITE_URL);
      toast.success("官网地址已复制");
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch (err) {
      console.error("复制官网地址失败:", err);
      toast.error("复制失败，请稍后重试");
    }
  };

  useEffect(() => {
    let cancelled = false;

    const loadSettings = async () => {
      try {
        const [configs, skillsPreferences] = await Promise.all([
          appApi.getAppConfigs(),
          appApi.getSkillsInstallPreferences(),
        ]);
        if (!cancelled) {
          setApps(configs);
          setSkillsInstallPreferences(skillsPreferences);
        }
      } catch (err) {
        console.error("获取设置失败:", err);
        if (!cancelled) {
          toast.error(`获取设置失败: ${err}`);
        }
      }
    };

    loadSettings();

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

  const handleSkillsLocationChange = async (locationId: string) => {
    if (!skillsInstallPreferences) return;

    const previous = skillsInstallPreferences.selected;
    setSkillsInstallPreferences({
      ...skillsInstallPreferences,
      selected: locationId,
    });
    setSavingSkillsLocation(true);
    try {
      await appApi.setSkillsInstallLocation(locationId);
      toast.success("Skills 安装位置已更新");
    } catch (err) {
      console.error("保存 Skills 安装位置失败:", err);
      setSkillsInstallPreferences({
        ...skillsInstallPreferences,
        selected: previous,
      });
      toast.error(`保存 Skills 安装位置失败: ${err}`);
    } finally {
      setSavingSkillsLocation(false);
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
  const themeOptions: Array<{ id: ThemeMode; label: string }> = [
    { id: "system", label: "跟随系统" },
    { id: "light", label: "浅色" },
    { id: "dark", label: "深色" },
  ];

  return (
    <div className="glass-app flex h-full flex-col overflow-hidden">
      {/* 头部 */}
      <div className="glass-header">
        <div className="glass-kicker">
          <Settings size={13} />
          Preferences
        </div>
        <h2 className="mt-3 text-display">设置</h2>
        <p className="mt-2 text-caption text-slate-500 dark:text-slate-400">
          管理应用配置和数据存储
        </p>
      </div>

      {/* 内容 */}
      <div className="glass-content">
        <div className="max-w-2xl space-y-6">
          {/* 显示 */}
          <section className={settingSections}>
            <h3 className="text-base font-medium mb-4">显示</h3>
            <div className="relative inline-flex w-full rounded-xl border border-white/60 bg-white/45 p-1 shadow-inner shadow-slate-200/40 backdrop-blur-xl dark:border-white/10 dark:bg-white/8 dark:shadow-black/10">
              {themeOptions.map((option) => {
                const active = theme.mode === option.id;
                return (
                  <Pressable
                    key={option.id}
                    type="button"
                    onClick={() => theme.setMode(option.id)}
                    className={`relative z-10 flex-1 rounded-lg px-3 py-2 text-sm font-medium transition-colors duration-200 ease-out ${
                      active
                        ? "text-white"
                        : "text-slate-500 hover:text-slate-950 dark:text-slate-400 dark:hover:text-white"
                    }`}
                  >
                    {active && (
                      <motion.span
                        layoutId="settings-theme-pill"
                        className="absolute inset-0 -z-10 rounded-lg bg-gradient-to-r from-[#0A84FF] to-[#5AC8FA] shadow-[0_4px_12px_rgba(10,132,255,0.22)]"
                        transition={{ type: "spring", bounce: 0.15, duration: 0.4 }}
                      />
                    )}
                    {option.label}
                  </Pressable>
                );
              })}
            </div>
            <p className="mt-3 text-caption text-slate-500 dark:text-slate-400">
              {theme.mode === "system"
                ? `跟随系统外观（当前解析为${theme.resolved === "dark" ? "深色" : "浅色"}）`
                : `当前为${theme.mode === "dark" ? "深色" : "浅色"}模式`}
            </p>
          </section>

          {/* 检查更新 */}
          <section className={settingSections}>
            <h3 className="text-base font-medium mb-4">软件更新</h3>
            <div className="flex items-center gap-4">
              <Pressable
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
              </Pressable>
              {isLatest && (
                <span className="flex items-center gap-1.5 text-sm text-emerald-600 dark:text-emerald-400">
                  <CheckCircle size={14} />
                  已是最新版本
                </span>
              )}
            </div>
            <p className="mt-3 text-caption text-slate-500 dark:text-slate-400">
              当前版本 v{appVersion} · 更新源：GitHub Releases
            </p>
          </section>

          {/* 分享 */}
          <section className={settingSections}>
            <h3 className="text-base font-medium mb-4">分享应用</h3>
            <div className="flex items-center gap-3">
              <Pressable
                onClick={copyShareUrl}
                aria-label={copied ? "已复制" : "复制官网地址"}
                className="glass-primary-button"
              >
                <AnimatePresence mode="wait" initial={false}>
                  {copied ? (
                    <motion.span
                      key="copied"
                      className="flex items-center gap-2"
                      initial={{ opacity: 0, y: 4 }}
                      animate={{ opacity: 1, y: 0 }}
                      exit={{ opacity: 0, y: -4 }}
                      transition={{ type: "spring", bounce: 0, duration: 0.18 }}
                    >
                      <Check size={16} />
                      已复制
                    </motion.span>
                  ) : (
                    <motion.span
                      key="copy"
                      className="flex items-center gap-2"
                      initial={{ opacity: 0, y: 4 }}
                      animate={{ opacity: 1, y: 0 }}
                      exit={{ opacity: 0, y: -4 }}
                      transition={{ type: "spring", bounce: 0, duration: 0.18 }}
                    >
                      <Share2 size={16} />
                      复制官网地址
                    </motion.span>
                  )}
                </AnimatePresence>
              </Pressable>
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
                  Skills 安装位置
                </p>
                {skillsInstallPreferences && (
                  <div
                    className={`relative mt-2 inline-flex w-full rounded-xl border border-white/60 bg-white/45 p-1 shadow-inner shadow-slate-200/40 backdrop-blur-xl dark:border-white/10 dark:bg-white/8 dark:shadow-black/10 ${
                      savingSkillsLocation ? "pointer-events-none opacity-60" : ""
                    }`}
                  >
                    {skillsInstallPreferences.options.map((option) => {
                      const active = skillsInstallPreferences.selected === option.id;
                      return (
                        <Pressable
                          key={option.id}
                          type="button"
                          onClick={() => handleSkillsLocationChange(option.id)}
                          className={`relative z-10 flex-1 rounded-lg px-3 py-2 text-sm font-medium transition-colors duration-200 ease-out ${
                            active
                              ? "text-white"
                              : "text-slate-500 hover:text-slate-950 dark:text-slate-400 dark:hover:text-white"
                          }`}
                        >
                          {active && (
                            <motion.span
                              layoutId="skills-location-pill"
                              className="absolute inset-0 -z-10 rounded-lg bg-gradient-to-r from-[#0A84FF] to-[#5AC8FA] shadow-[0_4px_12px_rgba(10,132,255,0.22)]"
                              transition={{ type: "spring", bounce: 0.15, duration: 0.4 }}
                            />
                          )}
                          {option.label}
                        </Pressable>
                      );
                    })}
                  </div>
                )}
                <code className={codeBlock}>
                  {selectedSkillsPath || "加载中..."}
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
        <h2 className="mt-3 text-display">关于</h2>
        <p className="mt-2 text-caption text-slate-500 dark:text-slate-400">
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
                <p className="mt-0.5 text-caption text-slate-500 dark:text-slate-400">
                  v{appVersion}· MCP 和 Skills 管理工具
                </p>
              </div>
              <Pressable
                onClick={() =>
                  open(GITHUB_REPO_URL)
                }
                className="glass-secondary-button min-h-8 px-3 py-1.5 text-xs"
              >
                <Github size={12} />
                GitHub
                <ExternalLink size={10} />
              </Pressable>
            </div>
            <p className="text-sm leading-relaxed text-slate-500 dark:text-slate-400">
              一款基于 Tauri 2 构建的跨平台桌面应用，专注于管理 AI 编程工具的 MCP 服务器配置和 Skills 技能同步。兼容 Qwen Code、Claude Code、Codex、Gemini CLI、OpenCode、TRAE IDE、TRAE IDE CN、TRAE Work、TRAE Work CN、Qoder、CodeBuddy、Hermes Agent、Mimo Code 等主流工具。
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
                <Pressable
                  onClick={() =>
                    open(`${GITHUB_REPO_URL}/issues`)
                  }
                  className="text-[hsl(var(--primary))] hover:underline inline-flex items-center gap-0.5 !bg-transparent !shadow-none !min-h-0 !p-0"
                >
                  GitHub Issues
                  <ExternalLink size={10} />
                </Pressable>{" "}
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
