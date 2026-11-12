import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { test } from "node:test";
import { runInNewContext } from "node:vm";
import ts from "typescript";

const require = createRequire(import.meta.url);

const source = readFileSync(new URL("../src/hooks/useStartupUpdateCheck.ts", import.meta.url), "utf8");
const { outputText } = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.ReactJSX, target: ts.ScriptTarget.ES2020 },
});

function createStorage() {
  const data = new Map();
  return {
    data,
    getItem: (key) => (data.has(key) ? data.get(key) : null),
    setItem: (key, value) => data.set(key, String(value)),
    removeItem: (key) => data.delete(key),
  };
}

/** 每个用例独立加载模块，避免模块级状态互相污染。 */
function load(options = {}) {
  const responses = [...(options.responses ?? [])];
  const calls = { checkUpdate: 0 };
  const listeners = new Map();
  const localStorage = createStorage();
  const windowObject = {
    localStorage,
    addEventListener: (type, fn) => listeners.set(type, fn),
    removeEventListener: (type) => listeners.delete(type),
  };
  const setIntervals = [];
  const clearedIntervals = new Set();
  const effects = [];
  const updates = [];

  const module = { exports: {} };
  runInNewContext(outputText, {
    require: (id) => {
      if (id === "react") return {
        useEffect: (effect) => effects.push(effect),
        useState: (value) => [value, (next) => updates.push(next)],
      };
      if (id === "@/lib/api") {
        return {
          updateApi: {
            async checkUpdate() {
              calls.checkUpdate += 1;
              const next = responses.shift();
              if (next instanceof Error) throw next;
              return next ?? { available: false, version: "", body: null, date: null };
            },
          },
        };
      }
      return require(id);
    },
    module,
    exports: module.exports,
    window: windowObject,
    console: { warn: () => {}, error: () => {}, log: () => {} },
    setInterval: (fn, ms) => {
      setIntervals.push({ fn, ms });
      return setIntervals.length;
    },
    clearInterval: (id) => clearedIntervals.add(id),
    Date,
    Number,
    JSON,
    String,
    Set,
    Map,
    Promise,
    RegExp,
  });

  function mount(version) {
    effects.length = 0;
    module.exports.useStartupUpdateCheck(version);
    const cleanups = effects.map((effect) => effect());
    return () => cleanups.forEach((cleanup) => cleanup?.());
  }

  return { ...module.exports, calls, listeners, setIntervals, clearedIntervals, localStorage, mount, updates };
}

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

test("已安装版本不低于缓存版本时清除缓存，避免升级后仍显示徽标", async () => {
  const mod = load();
  mod.localStorage.setItem("ai-toolkit-update-info", JSON.stringify({ version: "2.2.4", body: "old" }));
  // 模拟上次检查刚发生，本次启动不重新请求。
  mod.localStorage.setItem("ai-toolkit-update-checked-at", String(Date.now()));

  const unmount = mod.mount("2.2.4");
  assert.equal(mod.updates.at(-1), null);
  assert.equal(mod.localStorage.getItem("ai-toolkit-update-info"), null);
  assert.equal(mod.calls.checkUpdate, 0);
  unmount();
});

test("checkNow 在发现新版本时返回版本与更新说明", async () => {
  const mod = load({
    responses: [{ available: true, version: "9.9.9", body: "说明", date: null }],
  });

  const update = await mod.checkNow();
  assert.equal(update.version, "9.9.9");
  assert.equal(update.body, "说明");
  assert.equal(mod.calls.checkUpdate, 1);
  assert.equal(JSON.parse(mod.localStorage.getItem("ai-toolkit-update-info")).version, "9.9.9");
});

test("检查失败时抛出原始错误并写入 15 分钟退避时间戳", async () => {
  const mod = load({ responses: [new Error("无法连接到更新服务器")] });
  const before = Date.now();

  await assert.rejects(() => mod.checkNow(), /无法连接到更新服务器/);

  const checkedAt = Number(mod.localStorage.getItem("ai-toolkit-update-checked-at"));
  const sixHours = 6 * 60 * 60 * 1000;
  const retryIn = checkedAt + sixHours - before;
  assert.ok(retryIn > 14 * 60 * 1000 && retryIn <= 15 * 60 * 1000 + 1000, `退避时间异常: ${retryIn}`);
  assert.equal(mod.localStorage.getItem("ai-toolkit-update-info"), null);
});

test("已是最新版本时清除缓存结果并返回 null", async () => {
  const mod = load({
    responses: [{ available: false, version: "", body: null, date: null }],
  });
  mod.localStorage.setItem("ai-toolkit-update-info", JSON.stringify({ version: "2.2.5", body: "x" }));

  assert.equal(await mod.checkNow(), null);
  assert.equal(mod.localStorage.getItem("ai-toolkit-update-info"), null);
});

test("后台检查与手动检查共享进行中的请求和结果", async () => {
  let resolve;
  const response = new Promise((done) => { resolve = done; });
  const mod = load({ responses: [response] });
  const unmount = mod.mount("2.2.4");
  const checks = [mod.checkNow(), mod.checkNow()];
  assert.equal(mod.calls.checkUpdate, 1);
  resolve({ available: true, version: "9.9.9", body: "shared" });
  for (const update of await Promise.all(checks)) {
    assert.equal(update?.version, "9.9.9");
  }
  unmount();
});

test("检查失败后网络恢复立即重试，并保留周期检查间隔", async () => {
  const mod = load({ responses: [new Error("offline"), { available: true, version: "9.9.9" }] });
  const unmount = mod.mount("2.2.4");
  await flush();
  assert.equal(mod.calls.checkUpdate, 1);
  mod.listeners.get("online")();
  await flush();
  assert.equal(mod.calls.checkUpdate, 2);
  assert.equal(mod.updates.at(-1)?.version, "9.9.9");
  mod.setIntervals[0].fn();
  await flush();
  assert.equal(mod.calls.checkUpdate, 2);
  unmount();
});

test("存储不可访问时使用内存中的检查时间", async () => {
  const mod = load();
  for (const name of ["getItem", "setItem", "removeItem"]) {
    mod.localStorage[name] = () => { throw new Error("storage unavailable"); };
  }
  const unmount = mod.mount("2.2.4");
  await flush();
  mod.setIntervals[0].fn();
  await flush();
  assert.equal(mod.calls.checkUpdate, 1);
  unmount();
});

test("StrictMode 重挂载复用请求，卸载后清理定时器和网络监听", async () => {
  let resolve;
  const response = new Promise((done) => { resolve = done; });
  const mod = load({ responses: [response] });
  mod.mount("2.2.4")();
  assert.equal(mod.listeners.size, 0);
  assert.ok(mod.clearedIntervals.has(1));
  const unmount = mod.mount("2.2.4");
  assert.equal(mod.calls.checkUpdate, 1);
  resolve({ available: true, version: "9.9.9", body: "remounted" });
  await flush();
  assert.equal(mod.updates.at(-1)?.version, "9.9.9");
  unmount();
  assert.equal(mod.listeners.size, 0);
  assert.ok(mod.clearedIntervals.has(2));
});
