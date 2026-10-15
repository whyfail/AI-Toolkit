import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { test } from "node:test";
import { runInNewContext } from "node:vm";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import ts from "typescript";

const require = createRequire(import.meta.url);
const source = readFileSync(new URL("../src/components/DesktopRuntimeGuard.tsx", import.meta.url), "utf8");
const { outputText } = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    jsx: ts.JsxEmit.ReactJSX,
    target: ts.ScriptTarget.ES2020,
  },
});
const module = { exports: {} };
runInNewContext(outputText, { require, module, exports: module.exports });
const { DesktopRuntimeGuard } = module.exports;

test("浏览器缺少桌面桥接时显示启动指引，不挂载本地功能", (t) => {
  const previous = globalThis.isTauri;
  delete globalThis.isTauri;
  t.after(() => { globalThis.isTauri = previous; });

  function NativeApp() {
    assert.fail("浏览器中不应挂载会调用桌面接口的组件");
  }

  const html = renderToStaticMarkup(
    createElement(DesktopRuntimeGuard, null, createElement(NativeApp)),
  );
  assert.match(html, /请在桌面应用中打开/);
  assert.match(html, /pnpm tauri:dev/);
  assert.doesNotMatch(html, /暂无技能|已安装 0 个 Agent/);
});

test("桌面环境正常挂载本地功能", (t) => {
  const previous = globalThis.isTauri;
  globalThis.isTauri = true;
  t.after(() => { globalThis.isTauri = previous; });

  let mounted = false;
  function NativeApp() {
    mounted = true;
    return createElement("div", null, "本地技能列表");
  }

  const html = renderToStaticMarkup(
    createElement(DesktopRuntimeGuard, null, createElement(NativeApp)),
  );
  assert.equal(mounted, true);
  assert.match(html, /本地技能列表/);
  assert.doesNotMatch(html, /请在桌面应用中打开/);
});
