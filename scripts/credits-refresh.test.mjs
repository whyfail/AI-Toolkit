import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { test } from "node:test";
import { runInNewContext } from "node:vm";
import ts from "typescript";

const require = createRequire(import.meta.url);
const source = readFileSync(new URL("../src/hooks/useCredits.ts", import.meta.url), "utf8");
const { outputText } = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2020 },
});

function mount(panelActive = true) {
  const cleanups = [];
  const active = [];
  const timers = new Map();
  let timerId = 0;
  let invalidations = 0;
  let networkRecoveries = 0;
  let unsubscribed = 0;
  let handler;
  let resolveSubscription;
  const subscription = new Promise((resolve) => { resolveSubscription = resolve; });
  const surface = () => {
    const handlers = new Map();
    return {
      addEventListener(name, callback) {
        if (!handlers.has(name)) handlers.set(name, new Set());
        handlers.get(name).add(callback);
      },
      removeEventListener(name, callback) { handlers.get(name)?.delete(callback); },
      emit(name) { for (const callback of handlers.get(name) ?? []) callback(); },
      listenerCount() { return [...handlers.values()].reduce((sum, callbacks) => sum + callbacks.size, 0); },
    };
  };
  const window = surface();
  const document = Object.assign(surface(), { visibilityState: "visible", focused: true, hasFocus() { return this.focused; } });
  const stubs = {
    react: { useEffect: (effect) => cleanups.push(effect()) },
    "@tanstack/react-query": { useQueryClient: () => ({ invalidateQueries: async () => { invalidations++; } }) },
    "@tauri-apps/api/event": { listen: (name, callback) => {
      assert.equal(name, "credits-updated"); handler = callback; return subscription;
    } },
    "@/lib/api/credits": { creditsApi: {
      setViewActive: async (value) => { active.push(value); },
      networkRestored: async () => { networkRecoveries++; },
    } },
    sonner: { toast: { error: () => assert.fail("background refresh must stay quiet") } },
  };
  const module = { exports: {} };
  runInNewContext(outputText, {
    require: (id) => stubs[id] ?? require(id), module, exports: module.exports, window, document,
    setTimeout: (callback) => { timers.set(++timerId, callback); return timerId; },
    clearTimeout: (id) => timers.delete(id),
  });
  module.exports.useCreditsRuntime(panelActive);
  return {
    active, window, document, timers,
    updated: () => handler(),
    resolve: () => resolveSubscription(() => { unsubscribed++; }),
    flush: () => { const callbacks = [...timers.values()]; timers.clear(); for (const callback of callbacks) callback(); },
    unmount: () => { for (const cleanup of cleanups) cleanup(); },
    get invalidations() { return invalidations; },
    get unsubscribed() { return unsubscribed; },
    get networkRecoveries() { return networkRecoveries; },
  };
}

test("后台事件合并更新页面，连续事件不会让更新无限延后", async () => {
  const runtime = mount(); runtime.resolve(); await Promise.resolve();
  for (let index = 0; index < 100; index++) runtime.updated();
  assert.equal(runtime.timers.size, 1);
  runtime.flush(); assert.equal(runtime.invalidations, 1);
  runtime.updated(); runtime.flush(); assert.equal(runtime.invalidations, 2);
  runtime.unmount();
  assert.equal(runtime.unsubscribed, 1);
  assert.equal(runtime.window.listenerCount(), 0);
  assert.equal(runtime.document.listenerCount(), 0);
});

test("切换窗口状态只报告前台活跃情况，网络恢复单独通知后台", () => {
  const runtime = mount();
  assert.deepEqual(runtime.active, [true]);
  runtime.document.focused = false; runtime.window.emit("blur");
  runtime.document.focused = true; runtime.window.emit("focus");
  runtime.document.visibilityState = "hidden"; runtime.document.emit("visibilitychange");
  assert.deepEqual(runtime.active, [true, false, true, false]);
  runtime.window.emit("online"); assert.equal(runtime.networkRecoveries, 1);
  runtime.unmount(); assert.equal(runtime.active.at(-1), false);
});

test("其他页面保持后台模式，仍能处理网络恢复", () => {
  const runtime = mount(false);
  runtime.window.emit("focus"); assert.deepEqual(runtime.active, [false, false]);
  runtime.window.emit("online"); assert.equal(runtime.networkRecoveries, 1);
  runtime.unmount();
});

test("异步订阅尚未完成便卸载时，只清理一次且不再刷新页面", async () => {
  const runtime = mount();
  runtime.unmount(); runtime.resolve(); await Promise.resolve();
  assert.equal(runtime.unsubscribed, 1);
  assert.equal(runtime.timers.size, 0);
  assert.equal(runtime.invalidations, 0);
});
