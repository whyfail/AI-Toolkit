export const AI_TOOLKIT_REPO_URL = "https://github.com/whyfail/ai-toolkit";

export type ChangelogRelease = {
  /** 版本号，去掉 `v` 前缀，例如 `2.1.4` */
  version: string;
  /** 发布日期，例如 `2026-10-22` */
  date: string | null;
  /** 版本标题下的 Markdown 正文 */
  body: string;
};

const releaseHeadingPattern =
  /^v?(\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?)(?:\s*\(([^)]+)\))?\s*$/;

function parseReleaseHeading(heading: string): Omit<ChangelogRelease, "body"> | null {
  const match = releaseHeadingPattern.exec(heading.trim());
  if (!match) return null;
  return { version: match[1], date: match[2]?.trim() || null };
}

/** 版本之间用 `---` 分隔，卡片正文里不需要这条横线。 */
function stripTrailingDivider(body: string): string {
  return body.replace(/(?:^|\n)[ \t]*(?:-{3,}|\*{3,}|_{3,})[ \t]*$/, "").trimEnd();
}

/**
 * 把 CHANGELOG.md 切分为版本列表，保持源顺序（新版本在前）。
 * 非版本标题（例如 `## Notes`）和首个版本之前的引导文字会被忽略。
 */
export function parseChangelog(raw: string): ChangelogRelease[] {
  const text = raw.replace(/^\uFEFF/, "").replace(/\r\n?/g, "\n");
  const headings = [...text.matchAll(/^##[ \t]+(.+)$/gm)];
  const releases: ChangelogRelease[] = [];

  headings.forEach((heading, index) => {
    const parsed = parseReleaseHeading(heading[1]);
    if (!parsed) return;

    const start = (heading.index ?? 0) + heading[0].length;
    const end = headings[index + 1]?.index ?? text.length;
    releases.push({ ...parsed, body: stripTrailingDivider(text.slice(start, end).trim()) });
  });

  return releases;
}

/**
 * 更新日志里的链接只应打开系统浏览器：
 * 绝对 http(s) 地址原样返回，仓内相对 Markdown 路径改写为 GitHub blob 地址，
 * 其余（页内锚点、mailto 等）返回 null，由调用方降级为纯文本。
 */
export function resolveChangelogHref(href: string): string | null {
  const trimmed = href.trim();
  if (/^https?:\/\//i.test(trimmed)) return trimmed;

  const relativeDoc = /^(?:\.\/)?((?:[A-Za-z0-9._-]+\/)*[A-Za-z0-9._-]+\.md(?:#[A-Za-z0-9._-]+)?)$/.exec(trimmed);
  if (!relativeDoc) return null;

  // `..` / `.` 段会被 GitHub 规范化到错误路径，直接拒绝。
  const segments = relativeDoc[1].split("/");
  if (segments.some((segment) => segment === "." || segment === "..")) return null;

  return `${AI_TOOLKIT_REPO_URL}/blob/main/${relativeDoc[1]}`;
}
