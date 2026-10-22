import { useMemo } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import { open as openUrl } from "@tauri-apps/plugin-shell";
import { History } from "lucide-react";
import { toast } from "sonner";
import changelogSource from "../../../CHANGELOG.md?raw";
import { AI_TOOLKIT_REPO_URL, parseChangelog, resolveChangelogHref } from "@/lib/changelog";
import { useAppVersion } from "@/hooks/useAppVersion";
import { Pressable } from "@/components/ui/Pressable";

const markdownComponents: Components = {
  a: ({ href, children, title }) => {
    const target = href ? resolveChangelogHref(href) : null;
    if (!target) return <span>{children}</span>;
    return (
      <a
        href={target}
        title={title}
        rel="noreferrer noopener"
        onClick={(event) => {
          event.preventDefault();
          void openUrl(target).catch(() => toast.error("无法打开文档链接"));
        }}
      >
        {children}
      </a>
    );
  },
};

// 变更日志的标题层级比面板标题低一两级，只能收紧排版，不能抢视觉焦点。
const markdownClassName = [
  "text-slate-600 dark:text-slate-300",
  "[&_h3]:mt-4 [&_h3]:mb-1.5 [&_h3]:text-sm [&_h3]:font-semibold [&_h3]:tracking-tight [&_h3]:text-slate-900 dark:[&_h3]:text-white",
  "[&_h3:first-child]:mt-0",
  "[&_h4]:mt-3 [&_h4]:mb-1 [&_h4]:text-[13px] [&_h4]:font-semibold [&_h4]:text-slate-900 dark:[&_h4]:text-white",
  "[&_p]:my-2 [&_p]:text-sm [&_p]:leading-relaxed",
  "[&_ul]:my-2 [&_ul]:list-disc [&_ul]:pl-5",
  "[&_ol]:my-2 [&_ol]:list-decimal [&_ol]:pl-5",
  "[&_li]:my-1 [&_li]:text-sm [&_li]:leading-relaxed",
  "[&_strong]:font-semibold [&_strong]:text-slate-900 dark:[&_strong]:text-white",
  "[&_a]:font-medium [&_a]:text-[hsl(var(--primary))] [&_a]:underline [&_a]:underline-offset-2",
  "[&_code]:rounded-md [&_code]:bg-slate-900/5 [&_code]:px-1.5 [&_code]:py-0.5 [&_code]:font-mono [&_code]:text-[12px] dark:[&_code]:bg-white/10",
].join(" ");

function ChangelogPanel() {
  const appVersion = useAppVersion().replace(/^v/i, "");
  const releases = useMemo(() => parseChangelog(changelogSource), []);

  const openRepository = () => {
    void openUrl(`${AI_TOOLKIT_REPO_URL}/blob/main/CHANGELOG.md`).catch(() =>
      toast.error("无法打开文档链接"),
    );
  };

  return (
    <div className="glass-app flex h-full flex-col overflow-hidden">
      <div className="glass-header flex-shrink-0">
        <div className="glass-kicker">
          <History size={13} />
          Changelog
        </div>
        <h2 className="mt-3 text-display">更新日志</h2>
        <p className="mt-2 text-caption text-slate-500 dark:text-slate-400 sm:text-body">
          当前版本 v{appVersion} · 共 {releases.length} 个版本
        </p>
      </div>

      <div className="glass-content">
        {releases.length === 0 ? (
          <section className="glass-card max-w-2xl p-6">
            <h3 className="text-base font-medium">暂无更新日志</h3>
            <p className="mt-2 text-sm leading-relaxed text-slate-500 dark:text-slate-400">
              可以前往 GitHub 查看发布记录。
            </p>
            <Pressable className="glass-secondary-button mt-4" onClick={openRepository}>
              GitHub 更新日志
            </Pressable>
          </section>
        ) : (
          <div className="max-w-3xl space-y-4">
            {releases.map((release, index) => {
              const isCurrent = release.version === appVersion;
              const anchorId = `release-${release.version}-${index}`;
              return (
                <article
                  key={anchorId}
                  className="glass-card p-5 sm:p-6"
                  aria-labelledby={anchorId}
                >
                  <header className="flex flex-wrap items-baseline gap-2">
                    <h3
                      id={anchorId}
                      className="font-mono text-base font-semibold tracking-tight text-slate-950 dark:text-white"
                    >
                      v{release.version}
                    </h3>
                    {isCurrent && (
                      <span className="rounded-full bg-emerald-500/10 px-2 py-0.5 text-[11px] font-medium text-emerald-700 dark:text-emerald-300">
                        当前版本
                      </span>
                    )}
                    {release.date && (
                      <time
                        className="text-micro text-slate-400 dark:text-slate-500"
                        dateTime={release.date}
                      >
                        {release.date}
                      </time>
                    )}
                  </header>
                  <div className={`mt-2 ${markdownClassName}`}>
                    <ReactMarkdown
                      remarkPlugins={[remarkGfm]}
                      components={markdownComponents}
                    >
                      {release.body}
                    </ReactMarkdown>
                  </div>
                </article>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}

export default ChangelogPanel;
