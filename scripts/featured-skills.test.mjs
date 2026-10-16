import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { beforeEach, test } from "node:test";
import { runInNewContext } from "node:vm";
import * as React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import ts from "typescript";

const require = createRequire(import.meta.url);
let state;
const stubs = {
  react: { ...React, useState: (initial) => React.useState(initial === "git" ? state.tab : initial) },
  "@tanstack/react-query": { useQuery: (options) => { state.options = options; return state.query; } },
  "@tauri-apps/plugin-shell": {},
  "@tauri-apps/plugin-dialog": {},
  "motion/react": { motion: { span: ({ children, className }) => React.createElement("span", { className }, children) } },
  sonner: { toast: {} },
  "./GitPickModal": { default: () => null },
  "@/lib/tools": { APP_COLORS: {} },
  "@/lib/api": { skillsApi: { getFeaturedSkills: async () => [] } },
  "@/components/ui/Modal": { Modal: ({ open, children }) => open ? children : null },
  "@/components/ui/Pressable": { Pressable: ({ children, variant: _variant, ...props }) => {
    const label = React.Children.toArray(children).filter((child) => typeof child === "string").join("");
    state.buttons.push({ label, ...props });
    return React.createElement("button", props, children);
  } },
};
const source = readFileSync(new URL("../src/components/skills/modals/AddSkillModal.tsx", import.meta.url), "utf8");
const { outputText } = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.ReactJSX, target: ts.ScriptTarget.ES2020 },
});
const module = { exports: {} };
runInNewContext(outputText, { require: (id) => stubs[id] ?? require(id), module, exports: module.exports, Error });
const AddSkillModal = module.exports.default;

beforeEach(() => {
  state = { tab: "online", buttons: [], retries: 0, query: {
    data: [], error: null, isFetching: false,
    refetch: () => { state.retries += 1; return Promise.resolve(); },
  } };
});

function render(open = true) {
  return renderToStaticMarkup(React.createElement(AddSkillModal, {
    open, onClose() {}, tools: [], syncTargets: {}, onSyncTargetChange() {}, onSkillAdded() {},
  }));
}

test("热门列表显示具体错误，点击重试调用重新加载", () => {
  state.query.error = "热门技能列表请求失败（HTTP 404）";
  const html = render();
  assert.match(html, /role="alert"/);
  assert.match(html, /HTTP 404/);
  assert.doesNotMatch(html, /暂无热门技能/);
  state.buttons.find((button) => button.label === "重试加载").onClick();
  assert.equal(state.retries, 1);
  assert.equal(state.options.retry, false);
});

test("合法空列表与加载失败分别显示", () => {
  const html = render();
  assert.match(html, /暂无热门技能/);
  assert.doesNotMatch(html, /role="alert"|加载热门技能失败/);
});

test("加载中显示状态，成功后按真实安装量展示技能", () => {
  state.query.isFetching = true;
  assert.match(render(), /aria-label="正在加载热门技能"/);
  state.query.isFetching = false;
  state.query.data = [{ slug: "owner/repo/fixture", name: "fixture-skill", installs: 12345, source_url: "https://github.com/owner/repo" }];
  const html = render();
  assert.match(html, /fixture-skill/);
  assert.match(html, /12\.3K.*次安装/);
  assert.doesNotMatch(html, /暂无热门技能|role="alert"/);
});

test("仅打开弹窗的在线页时启用热门请求", () => {
  render();
  assert.equal(state.options.enabled, true);
  render(false);
  assert.equal(state.options.enabled, false);
  state.tab = "git";
  render();
  assert.equal(state.options.enabled, false);
});
