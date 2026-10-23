import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { beforeEach, test } from "node:test";
import { runInNewContext } from "node:vm";
import { Children, createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import ts from "typescript";

const require = createRequire(import.meta.url);
let state;
const toast = Object.assign(() => {}, { error() {}, success() {} });
const stubs = {
  sonner: { toast },
  "./ModelInfoPopover": { ModelInfoPopover: () => null },
  "@tauri-apps/plugin-shell": { open: async (url) => { state.opened.push(url); } },
  "@/components/ui/Modal": { Modal: () => null },
  "@/components/ui/AgentIcon": { AgentIcon: ({ name }) => createElement("span", { role: "img", "aria-label": name, title: name }) },
  "@/components/ui/Pressable": {
    Pressable: ({ children, ...props }) => {
      const label = Children.toArray(children).filter((child) => typeof child === "string").join("");
      state.buttons.push({ label, ...props });
      return createElement("button", props, children);
    },
  },
  "@/hooks/useCredits": {
    useCreditsAction: (action) => ({
      isPending: false,
      mutate: (input, options) => {
        state.pending.push(Promise.resolve().then(() => action(input)).then((value) => options?.onSuccess?.(value)));
      },
    }),
  },
  "@/lib/api": {
    agentApi: { launchAgent: async (...args) => { state.launches.push(args); } },
  },
  "@/lib/api/credits": {
    creditsApi: {
      refresh: async () => [],
      claim: async (id) => { state.claims.push(id); return []; },
      setAccountEnabled: async () => {},
    },
  },
};
const source = readFileSync(new URL("../src/components/credits/CreditsPanel.tsx", import.meta.url), "utf8");
const { outputText } = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.ReactJSX, target: ts.ScriptTarget.ES2020 },
});
const module = { exports: {} };
runInNewContext(`${outputText}\nmodule.exports.AccountCard = AccountCard; module.exports.AccountsView = AccountsView;`, {
  require: (id) => stubs[id] ?? require(id), module, exports: module.exports,
});
const { AccountCard, AccountsView } = module.exports;

beforeEach(() => { state = { buttons: [], pending: [], launches: [], claims: [], opened: [] }; });

function renderAccount(overrides = {}, props = {}) {
  const account = {
    id: "test-account", platform: "qoder", userId: "test-user", label: "测试账户",
    enabled: true, status: "ready", lastError: null, snapshot: null,
    ...overrides,
  };
  return renderToStaticMarkup(createElement(AccountCard, { account, onDelete() {}, ...props }));
}

function snapshot(rewards = []) {
  return { balance: 0, unit: "credits", quota: null, packs: [], rewards, models: [], fetchedAt: 0, stale: false, diagnostic: null };
}

test("Qoder 资格未同步时与 TRAE 一样等待刷新，不显示活动说明", () => {
  const html = renderAccount();
  assert.match(html, /刷新后查看领取资格/);
  assert.doesNotMatch(html, /每日 100 积分|10:00|查看活动规则/);
  assert.equal(state.buttons.find((button) => button.label === "领取积分").disabled, true);
  assert.doesNotMatch(html, /去 Qoder CN 领取|桌面端领取/);
});

test("刷新失败保留凭据错误，不能假装拥有领取资格", () => {
  const html = renderAccount({ status: "error", lastError: "无法读取系统凭据库，请检查钥匙串权限", snapshot: { ...snapshot(), stale: true } });
  assert.match(html, /无法读取系统凭据库/);
  assert.doesNotMatch(html, /每日 100 积分|查看活动规则/);
  assert.match(html, /当前没有可领取的积分任务/);
  assert.equal(state.buttons.find((button) => button.label === "领取积分").disabled, true);
});

test("后台同步状态与历史余额同时可见，失败不会把余额显示为零", () => {
  const html = renderAccount({ status: "error", lastError: "网络连接失败",
    snapshot: { ...snapshot(), balance: 123.45, fetchedAt: 1792390604612, stale: true } }, { refreshing: true });
  assert.match(html, /123\.45/);
  assert.match(html, /同步中 · 上次成功同步/);
  assert.match(html, /同步失败，显示历史余额/);
});

test("自动刷新默认开启，旧设置与显式关闭均能正确显示", () => {
  const data = { accounts: [], rewards: [], settings: { autoClaim: false, port: 17860 } };
  const render = () => renderToStaticMarkup(createElement(AccountsView, { data, onAdd() {} }));
  assert.match(render(), /id="credits-auto-refresh"[^>]*checked/);
  data.settings.autoRefresh = false;
  assert.doesNotMatch(render(), /id="credits-auto-refresh"[^>]*checked/);
});

test("每日积分在工具内直接领取，不启动客户端或打开网页", async () => {
  renderAccount({ snapshot: snapshot([{ id: "today", kind: "credits", name: "每日 100 积分", status: "claimable", amount: 100 }]) });
  const button = state.buttons.find((button) => button.label === "领取积分");
  assert.equal(button.disabled, false);
  button.onClick();
  await Promise.all(state.pending);
  assert.deepEqual(state.claims, ["test-account"]);
  assert.equal(state.launches.length, 0);
  assert.equal(state.opened.length, 0);
});

test("Qoder 只显示积分奖励及状态，隐藏网页活动和优惠券", () => {
  const html = renderAccount({ snapshot: snapshot([
    { id: "credits", kind: "credits", name: "每日积分", status: "claimable", amount: 100 },
    { id: "campaign", kind: "activity", name: "每日活动", status: "claimable", actionUrl: "https://qoder.com.cn/activities" },
    { id: "coupon", kind: "coupon", name: "优惠券", status: "unavailable", actionUrl: null },
  ]) });
  assert.match(html, /每日积分 · 100/);
  assert.match(html, /可领取/);
  assert.doesNotMatch(html, /平台活动|每日活动|优惠券|查看活动规则|活动详情|打开活动页|30 天/);
  assert.equal(state.buttons.find((button) => button.label === "领取积分").disabled, false);
});

test("明确可直接领取的积分继续调用原有领取接口", async () => {
  renderAccount({ snapshot: snapshot([{ id: "credits", kind: "credits", name: "积分奖励", status: "claimable", amount: 100, actionUrl: null }]) });
  state.buttons.find((button) => button.label === "领取积分").onClick();
  await Promise.all(state.pending);
  assert.deepEqual(state.claims, ["test-account"]);
  assert.equal(state.launches.length, 0);
});

test("只有 Qoder 每日积分可领取时也启用批量领取，并调用统一接口", async () => {
  const account = { id: "qoder-only", platform: "qoder", label: "Qoder", enabled: true, status: "ready", snapshot: snapshot([{ id: "today", kind: "credits", name: "每日 100 积分", status: "claimable", amount: 100 }]) };
  renderToStaticMarkup(createElement(AccountsView, { data: { accounts: [account], rewards: [], settings: { autoClaim: false, port: 17860 } }, onAdd() {} }));
  const button = state.buttons.find((button) => button.label === "一键领取积分");
  assert.equal(button.disabled, false);
  button.onClick();
  await Promise.all(state.pending);
  assert.deepEqual(state.claims, [undefined]);
  assert.equal(state.launches.length, 0);
  assert.equal(state.opened.length, 0);
});

test("已领取或停用的 Qoder 账户不能重复发起领取", () => {
  renderAccount({ snapshot: snapshot([{ id: "today", kind: "credits", name: "每日 100 积分", status: "claimed", amount: 100 }]) });
  assert.equal(state.buttons.find((button) => button.label === "领取积分").disabled, true);
  state.buttons = [];
  renderAccount({ enabled: false, snapshot: snapshot([{ id: "today", kind: "credits", name: "每日 100 积分", status: "claimable", amount: 100 }]) });
  assert.equal(state.buttons.find((button) => button.label === "领取积分").disabled, true);
});

test("领取结果不明确时显示待确认，不显示已到账金额", () => {
  const html = renderToStaticMarkup(createElement(AccountsView, {
    data: { accounts: [], settings: { autoClaim: false, port: 17860 }, rewards: [{ accountId: "qoder", rewardId: "today", name: "每日积分", kind: "credits", status: "pending", amount: null, message: "上游结果待确认", createdAt: 0 }] },
    onAdd() {},
  }));
  assert.match(html, /待确认/);
  assert.doesNotMatch(html, /等待授权|\+100/);
});

test("其他平台保留原有领取按钮，不显示 Qoder 活动", () => {
  const html = renderAccount({ platform: "traework", snapshot: snapshot() });
  assert.doesNotMatch(html, /每日 100 积分|去 Qoder CN 领取/);
  assert.match(html, /当前没有可领取的积分任务/);
  assert.equal(state.buttons.find((button) => button.label === "领取积分").disabled, true);
});
