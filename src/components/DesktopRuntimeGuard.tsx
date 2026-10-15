import type { ReactNode } from "react";
import { isTauri } from "@tauri-apps/api/core";

export function DesktopRuntimeGuard({ children }: { children: ReactNode }) {
  if (isTauri()) {
    return <>{children}</>;
  }

  return (
    <main className="glass-app flex h-full items-center justify-center overflow-y-auto p-6">
      <section className="glass-card w-full max-w-lg p-6 sm:p-8" aria-labelledby="desktop-runtime-title">
        <span className="glass-kicker">AI Toolkit</span>
        <h1 id="desktop-runtime-title" className="mt-4 text-2xl font-semibold">
          请在桌面应用中打开
        </h1>
        <p className="mt-3 text-sm leading-6 text-slate-500 dark:text-slate-400">
          当前页面未连接桌面后端，无法读取本地 Skills、扫描 Agent 或管理 MCP 配置。
          请打开已安装的 AI Toolkit 桌面应用。
        </p>
        <p className="mt-6 text-sm font-medium">本地开发时，在项目目录运行：</p>
        <pre className="glass-code mt-3 overflow-x-auto rounded-xl px-4 py-3 text-sm">
          <code>pnpm tauri:dev</code>
        </pre>
        <p className="mt-3 text-xs leading-5 text-slate-500 dark:text-slate-400">
          该命令会启动桌面窗口。pnpm dev 仅启动网页服务，浏览器中无法使用本地管理功能。
        </p>
      </section>
    </main>
  );
}
