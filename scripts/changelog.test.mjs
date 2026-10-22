import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { test } from "node:test";
import { runInNewContext } from "node:vm";
import * as React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import ts from "typescript";

const require = createRequire(import.meta.url);

function load(path, stubs = {}) {
  const source = readFileSync(new URL(path, import.meta.url), "utf8");
  const { outputText } = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.ReactJSX, target: ts.ScriptTarget.ES2020 },
  });
  const module = { exports: {} };
  runInNewContext(outputText, {
    require: (id) => stubs[id] ?? require(id),
    module,
    exports: module.exports,
  });
  return module.exports;
}

const { AI_TOOLKIT_REPO_URL, parseChangelog, resolveChangelogHref } = load(
  "../src/lib/changelog.ts",
);

const changelog = readFileSync(new URL("../CHANGELOG.md", import.meta.url), "utf8");
const packageVersion = JSON.parse(
  readFileSync(new URL("../package.json", import.meta.url), "utf8"),
).version;

test("内置 CHANGELOG 解析出版本、日期与正文", () => {
  const releases = parseChangelog(changelog);

  assert.ok(releases.length >= 30, `expected >= 30 releases, got ${releases.length}`);
  assert.equal(releases[0].version, packageVersion);
  assert.equal(new Set(releases.map((release) => release.version)).size, releases.length);

  for (const release of releases) {
    assert.match(release.version, /^\d+\.\d+\.\d+/);
    assert.match(release.date ?? "", /^\d{4}-\d{2}-\d{2}$/);
    assert.ok(release.body.length > 0, `${release.version} 正文为空`);
    assert.ok(!release.body.startsWith("## "), `${release.version} 正文包含下一个版本标题`);
  }
});

test("忽略前置说明与无法识别的标题，缺失日期为 null", () => {
  const raw = [
    "\uFEFF# Changelog",
    "",
    "发布说明。",
    "",
    "## v1.2.0",
    "",
    "### 新功能",
    "",
    "- 甲",
    "",
    "## Notes",
    "",
    "- 忽略",
    "",
    "## v1.1.0 (2024-01-02)",
    "",
    "- 乙",
    "",
  ].join("\r\n");

  const releases = parseChangelog(raw);

  // 跨 vm 上下文创建的数组原型不同，用字符串比较避免 deepStrictEqual 的原型检查。
  assert.equal(releases.map((release) => release.version).join(","), "1.2.0,1.1.0");
  assert.equal(releases[0].date, null);
  assert.match(releases[0].body, /### 新功能/);
  assert.equal(releases[1].date, "2024-01-02");
  assert.equal(releases[1].body, "- 乙");
});

test("仓内文档链接改写为 GitHub，其他协议不跳转", () => {
  assert.equal(
    resolveChangelogHref("docs/security-hardening.md"),
    `${AI_TOOLKIT_REPO_URL}/blob/main/docs/security-hardening.md`,
  );
  assert.equal(
    resolveChangelogHref("./docs/credits-proxy.md#usage"),
    `${AI_TOOLKIT_REPO_URL}/blob/main/docs/credits-proxy.md#usage`,
  );
  assert.equal(
    resolveChangelogHref("https://example.com/releases"),
    "https://example.com/releases",
  );
  assert.equal(resolveChangelogHref("#releases"), null);
  assert.equal(resolveChangelogHref("mailto:maintainer@example.com"), null);
  assert.equal(resolveChangelogHref("javascript:alert(1)"), null);
  assert.equal(resolveChangelogHref("data:text/html,<b>x</b>"), null);
  assert.equal(resolveChangelogHref("//example.com/x.md"), null);
  assert.equal(resolveChangelogHref("../docs/x.md"), null);
  assert.equal(resolveChangelogHref("docs/../x.md"), null);
});

test("版本末尾的分隔线不留在卡片正文里", () => {
  const releases = parseChangelog(
    "## v1.0.0 (2024-01-01)\n\n- 甲\n\n---\n\n## v0.9.0 (2023-12-01)\n\n- 乙\n",
  );

  assert.equal(releases[0].body, "- 甲");
  assert.equal(releases[1].body, "- 乙");
});

test("面板按版本渲染卡片，并只给当前运行版本加徽标", () => {
  const stubs = {
    "react-markdown": {
      __esModule: true,
      default: ({ children }) => React.createElement("div", { "data-markdown": "true" }, children),
    },
    "remark-gfm": { __esModule: true, default: () => {} },
    "@tauri-apps/plugin-shell": { open: async () => {} },
    "lucide-react": { History: () => React.createElement("span") },
    sonner: { toast: { error: () => {} } },
    "../../../CHANGELOG.md?raw": { __esModule: true, default: changelog },
    "@/lib/changelog": { AI_TOOLKIT_REPO_URL, parseChangelog, resolveChangelogHref },
    "@/hooks/useAppVersion": { useAppVersion: () => packageVersion },
    "@/components/ui/Pressable": {
      Pressable: ({ children, ...props }) => React.createElement("button", props, children),
    },
  };

  const { default: ChangelogPanel } = load("../src/components/changelog/ChangelogPanel.tsx", stubs);
  const html = renderToStaticMarkup(React.createElement(ChangelogPanel));
  const releases = parseChangelog(changelog);

  assert.equal((html.match(/id="release-/g) ?? []).length, releases.length);
  assert.match(html, new RegExp(`共 ${releases.length} 个版本`));
  assert.match(html, new RegExp(`v${releases[0].version}`));
  assert.match(html, new RegExp(`v${releases[releases.length - 1].version}`));
  // 头部说明里也有「当前版本」字样，徽标本身以 </span> 收尾，单独计数。
  assert.equal((html.match(/当前版本<\/span>/g) ?? []).length, 1);
  assert.match(html, new RegExp(`当前版本 v${packageVersion}`));
  // 徽标必须紧跟在当前版本的标题后，而不是落在别的版本卡片上。
  assert.match(html, new RegExp(`v${packageVersion}</h3><span[^>]*>当前版本</span>`));
  assert.match(html, new RegExp(releases[0].date));
  assert.match(html, /<time /);
});
