import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { test } from "node:test";
import { runInNewContext } from "node:vm";
import * as React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import ts from "typescript";

const require = createRequire(import.meta.url);
function load(path, stubs = {}, extra = "", globals = {}) {
  const source = readFileSync(new URL(path, import.meta.url), "utf8");
  const { outputText } = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.ReactJSX, target: ts.ScriptTarget.ES2020 },
  });
  const module = { exports: {} };
  runInNewContext(outputText + extra, { require: (id) => stubs[id] ?? require(id), module, exports: module.exports, ...globals });
  return module.exports;
}

const { ModelInfoContent, modelInfoPosition } = load("../src/components/credits/ModelInfoPopover.tsx");
const account = { id: "fixture", platform: "qoder", snapshot: { fetchedAt: 1792390604612, stale: false } };
const model = { id: "glm-5.3-flash", name: "GLM-5.3-Flash", contextWindow: 1000000, maxOutputTokens: 128000, supportsTools: true, supportsImages: true, supportsReasoning: false };
const render = (overrides = {}, owner = account) => renderToStaticMarkup(React.createElement(ModelInfoContent, { account: owner, model: { ...model, ...overrides }, platformName: "Qoder" })).replace(/<[^>]+>/g, "");

test("模型信息分别显示通道上下文、输出上限与输入能力", () => {
  const text = render();
  assert.match(text, /Qoder · 通道参数/);
  assert.match(text, /qoder\/glm-5.3-flash/);
  assert.match(text, /上下文窗口（默认最大）1M1,000,000 Token/);
  assert.match(text, /最大输出128K128,000 Token/);
  assert.match(text, /文本支持图片支持视频代理未开放PDF代理未开放/);
  assert.match(text, /工具调用支持思考输出不支持/);
});

test("旧快照缺失的能力显示未知，不按模型名称或上下文补出输出上限", () => {
  const text = render({ maxOutputTokens: undefined, supportsImages: undefined, supportsReasoning: null });
  assert.match(text, /最大输出未知通道未提供/);
  assert.match(text, /图片未知/);
  assert.match(text, /思考输出未知/);
  assert.doesNotMatch(text, /最大输出1M|32,000|32K|图片不支持/);
});

test("模型卡片分别展示调用 ID 与目录提供的完整底层模型 ID", () => {
  const text = render({ id: "deepseek-flash", upstreamId: "dfmodel", upstreamModelIds: ["deepseek-v4-1-flash-260910", "deepseek-v4-1-flash-261017"] });
  assert.match(text, /qoder\/deepseek-flash调用 IDdfmodel/);
  assert.match(text, /底层模型 ID目录声明deepseek-v4-1-flash-260910deepseek-v4-1-flash-261017/);
  assert.doesNotMatch(text, /通道未公开具体版本 ID/);
});

test("未公开底层版本时明确标注，不把调用 ID 或别名充当版本号", () => {
  for (const upstreamModelIds of [undefined, []]) {
    const text = render({ upstreamId: "gfmodel", upstreamModelIds });
    assert.match(text, /调用 IDgfmodel底层模型 ID通道未公开具体版本 ID/);
    assert.doesNotMatch(text, /目录声明|260910|261017/);
  }
  assert.match(render(), /调用 IDglm-5.3-flash/);
});

test("明确不支持与未知分开，缓存数据保持可辨认", () => {
  const text = render({ supportsImages: false }, { ...account, snapshot: { fetchedAt: 0, stale: true } });
  assert.match(text, /图片不支持/);
  assert.match(text, /缓存数据/);
  assert.match(text, /更新于 尚未同步/);
  for (const invalid of [0, -1, 128.5, Infinity, NaN]) {
    const invalidText = render({ contextWindow: invalid, maxOutputTokens: invalid });
    assert.match(invalidText, /上下文窗口（默认最大）未知通道未提供最大输出未知通道未提供/);
  }
});

test("浮层可左右翻转，窄窗口与四角位置均不超出可视区域", () => {
  assert.equal(modelInfoPosition({ left: 40, right: 260, top: 40, bottom: 100 }, { width: 320, height: 420 }, { width: 1400, height: 900 }).left, 270);
  assert.equal(modelInfoPosition({ left: 850, right: 1100, top: 40, bottom: 100 }, { width: 320, height: 420 }, { width: 1200, height: 900 }).left, 520);
  for (const width of [280, 640, 1024, 1600]) {
    for (const height of [260, 600, 900]) {
      for (const [left, top] of [[0, 0], [width - 100, 0], [0, height - 60], [width - 100, height - 60], [width / 2, height / 2]]) {
        const position = modelInfoPosition({ left, right: left + 100, top, bottom: top + 60 }, { width: 320, height: 420 }, { width, height });
        assert.ok(position.left >= 12 && position.top >= 12);
        assert.ok(position.left + Math.min(320, width - 24) <= width - 12);
        assert.ok(position.top + Math.min(420, position.maxHeight) <= height - 12);
      }
    }
  }
});

test("同账户同 ID 的目录条目按目录优先级去重", () => {
  const { accountModels } = load("../src/components/credits/CreditsPanel.tsx", {
    "@tauri-apps/plugin-shell": {}, "@/components/ui/Modal": {}, "@/components/ui/AgentIcon": {},
    "@/components/ui/Pressable": {}, "./ModelInfoPopover": {}, "@/hooks/useCredits": {},
    "@/lib/api/credits": {}, sonner: {},
  }, "\nmodule.exports.accountModels = accountModels;");
  const routed = { ...model, contextWindow: 200000 };
  const duplicate = { ...model, contextWindow: null };
  const entries = accountModels({ ...account, snapshot: { ...account.snapshot, models: [routed, duplicate] } });
  assert.equal(entries.length, 1);
  assert.equal(entries[0], routed);
});

test("每个模型的悬浮、聚焦与点击均关联详情，复制按钮保持独立", () => {
  let row;
  let trigger;
  let copy;
  const copied = [];
  const previewed = [];
  const toggled = [];
  const runtime = require("react/jsx-runtime");
  const record = (create) => (type, props, key) => {
    if (type === "div" && props.onPointerEnter) row = props;
    if (type === "button" && props["aria-label"]?.startsWith("查看")) trigger = props;
    return create(type, props, key);
  };
  const { ModelRow } = load("../src/components/credits/CreditsPanel.tsx", {
    "react/jsx-runtime": { ...runtime, jsx: record(runtime.jsx), jsxs: record(runtime.jsxs) },
    "@tauri-apps/plugin-shell": {},
    "@/components/ui/Modal": {},
    "@/components/ui/AgentIcon": {},
    "@/components/ui/Pressable": { Pressable: (props) => { copy = props; return React.createElement("button", props); } },
    "./ModelInfoPopover": {},
    "@/hooks/useCredits": {},
    "@/lib/api/credits": {},
    sonner: { toast: { success() {}, error() {} } },
  }, "\nmodule.exports.ModelRow = ModelRow;", { navigator: { clipboard: { writeText: async (value) => { copied.push(value); } } } });
  const html = renderToStaticMarkup(React.createElement(ModelRow, {
    account, model, infoId: "fixture-model-info", onPreview: (...args) => previewed.push(args),
    onToggle: (...args) => toggled.push(args), onLeave() {}, onBlur() {},
  }));
  assert.match(html, /aria-describedby="fixture-model-info"/);
  assert.match(html, /aria-expanded="true"/);
  const anchor = {};
  row.ref.current = anchor;
  row.onPointerEnter({ currentTarget: anchor });
  assert.equal(previewed[0][0].model.id, model.id);
  assert.equal(previewed[0][1], anchor);
  trigger.onFocus();
  assert.equal(previewed[1][2], true);
  trigger.onClick();
  assert.equal(toggled.length, 1);
  copy.onClick();
  assert.deepEqual(copied, ["qoder/glm-5.3-flash"]);
  assert.equal(toggled.length, 1);
});
