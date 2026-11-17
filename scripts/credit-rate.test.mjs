import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { test } from "node:test";
import { runInNewContext } from "node:vm";
import ts from "typescript";

const require = createRequire(import.meta.url);
function load(path) {
  const source = readFileSync(new URL(path, import.meta.url), "utf8");
  // Rewrite the "@/lib/api/credits" alias to a relative import.
  const rewritten = source.replace('"@/lib/api/credits"', '"../src/lib/api/credits"');
  const { outputText } = ts.transpileModule(rewritten, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2020 },
  });
  const module = { exports: {} };
  runInNewContext(outputText, { require, module, exports: module.exports });
  return module.exports;
}

const { displayedCreditRate, formatMultiplier } = load("../src/lib/credits/creditRate.ts");

// Fixed instants: 2026-11-16 10:00 and 23:30 UTC+8; 2026-11-16 00:30 UTC+8.
const T0 = Date.parse("2026-11-16T10:00:00+08:00");
const T_NIGHT = Date.parse("2026-11-16T23:30:00+08:00");
const T_EARLY = Date.parse("2026-11-16T00:30:00+08:00");
const clock = (ms) => ({ now: () => ms, localOffsetMinutes: () => -480 });
const rate = (multiplier, promotions = []) => ({ creditRate: { multiplier, promotions } });

test("基准倍率直接展示，异常值视为未知而不是 1×", () => {
  // Field-wise comparison: values returned from the VM realm differ in prototype.
  const base = displayedCreditRate(rate(0.21), clock(T0));
  assert.equal(base.multiplier, 0.21);
  assert.equal(base.promotional, false);
  assert.equal(base.promotion, null);
  assert.equal(base.base, 0.21);
  assert.equal(displayedCreditRate(rate(0), clock(T0)).multiplier, 0);
  assert.equal(displayedCreditRate({ creditRate: { multiplier: null } }, clock(T0)).multiplier, null);
  assert.equal(displayedCreditRate({ creditRate: { multiplier: -1 } }, clock(T0)).multiplier, null);
  assert.equal(displayedCreditRate({}, clock(T0)).multiplier, null);
});

test("WorkBuddy 旧快照中的 x 前缀倍率可直接展示，说明文本保持未知", () => {
  for (const declaredText of ["x0.50", "X 0.50", "0.50x", "×0.50", "0.50 倍"]) {
    assert.equal(displayedCreditRate({ creditRate: { multiplier: null, declaredText, source: "enterprise-models/v3-config" } }, clock(T0)).multiplier, 0.5);
  }
  assert.equal(displayedCreditRate({ creditRate: { declaredText: "x0.00", source: "enterprise-models/v3-config" } }, clock(T0)).multiplier, 0);
  for (const declaredText of ["", "按用量计费", "限时0.5倍", "-1x", "x0.5x", "Infinity"]) {
    assert.equal(displayedCreditRate({ creditRate: { declaredText, source: "enterprise-models/v3-config" } }, clock(T0)).multiplier, null);
  }
  const disabledTrae = { creditRate: { multiplier: null, declaredText: "0.78x", source: "get_detail_param" } };
  assert.equal(displayedCreditRate(disabledTrae, clock(T0)).multiplier, null);
});

test("WorkBuddy 带 credits 单位的旧缓存展示数值倍率", () => {
  for (const declaredText of ["x0.18 credits", "0.18x credits", "0.18 credits", "  X 0.18 CREDITS  ", "0.18× credit", "x0.18\tcredits"]) {
    const model = { creditRate: { multiplier: null, declaredText, source: "enterprise-models/v3-config" } };
    assert.equal(displayedCreditRate(model, clock(T0)).multiplier, 0.18, declaredText);
  }
  const free = { creditRate: { multiplier: null, declaredText: "x0.00 credits", source: "enterprise-models/v3-config" } };
  assert.equal(displayedCreditRate(free, clock(T0)).multiplier, 0);
  for (const declaredText of ["credits", "x-0.18 credits", "NaN credits", "inf credits", "x0.18 credits per request", "限时 0.18 credits"]) {
    const model = { creditRate: { multiplier: null, declaredText, source: "enterprise-models/v3-config" } };
    assert.equal(displayedCreditRate(model, clock(T0)).multiplier, null, declaredText);
  }
});

test("促销生效时优先展示优惠倍率，discountedCredits 优先于折扣系数", () => {
  const promotion = { id: "p", priority: 1, startsAt: T0 - 1000, endsAt: T0 + 1000, discountedCredits: 0.1 };
  const shown = displayedCreditRate(rate(0.21, [promotion]), clock(T0));
  assert.equal(shown.multiplier, 0.1);
  assert.equal(shown.promotional, true);
  // No discountedCredits: base × factor.
  const factor = { id: "f", priority: 1, startsAt: T0 - 1000, endsAt: T0 + 1000, discountFactor: 0.5 };
  assert.equal(displayedCreditRate(rate(0.6, [factor]), clock(T0)).multiplier, 0.3);
  // Factor without a usable base stays unknown.
  assert.equal(displayedCreditRate({ creditRate: { promotions: [factor] } }, clock(T0)).multiplier, null);
});

test("促销窗口按左闭右开判断，未开始或已结束显示基准倍率", () => {
  const starts = { id: "s", priority: 1, startsAt: T0 + 1000, discountedCredits: 0.1 };
  assert.equal(displayedCreditRate(rate(0.5, [starts]), clock(T0)).multiplier, 0.5);
  const ends = { id: "e", priority: 1, endsAt: T0, discountedCredits: 0.1 };
  assert.equal(displayedCreditRate(rate(0.5, [ends]), clock(T0)).multiplier, 0.5);
  const inclusive = { id: "i", priority: 1, endsAt: T0 + 1, discountedCredits: 0.1 };
  assert.equal(displayedCreditRate(rate(0.5, [inclusive]), clock(T0)).multiplier, 0.1);
});

test("每日时段按时区判断，支持跨午夜窗口", () => {
  // 08:00–22:00 UTC+8: active at 10:00, inactive at 23:30.
  const day = { id: "d", priority: 1, dailyStartMinute: 480, dailyEndMinute: 1320, timezoneOffsetSeconds: 28800, discountedCredits: 0.1 };
  assert.equal(displayedCreditRate(rate(0.5, [day]), clock(T0)).promotional, true);
  assert.equal(displayedCreditRate(rate(0.5, [day]), clock(T_NIGHT)).promotional, false);
  // 23:00–01:00 overnight: active at 23:30 and 00:30, inactive at 10:00.
  const overnight = { id: "o", priority: 1, dailyStartMinute: 1380, dailyEndMinute: 60, timezoneOffsetSeconds: 28800, discountedCredits: 0.2 };
  assert.equal(displayedCreditRate(rate(0.5, [overnight]), clock(T_NIGHT)).promotional, true);
  assert.equal(displayedCreditRate(rate(0.5, [overnight]), clock(T_EARLY)).promotional, true);
  assert.equal(displayedCreditRate(rate(0.5, [overnight]), clock(T0)).promotional, false);
  // No declared timezone falls back to the device timezone.
  const device = { id: "z", priority: 1, dailyStartMinute: 480, dailyEndMinute: 1320, discountedCredits: 0.1 };
  assert.equal(displayedCreditRate(rate(0.5, [device]), { now: () => T0, localOffsetMinutes: () => -480 }).promotional, true);
  assert.equal(displayedCreditRate(rate(0.5, [device]), { now: () => T0, localOffsetMinutes: () => 0 }).promotional, false);
});

test("多活动按优先级取最高，同优先级保持目录顺序，禁用活动被跳过", () => {
  const low = { id: "low", priority: 1, discountedCredits: 0.2 };
  const high = { id: "high", priority: 5, discountedCredits: 0.1 };
  const shown = displayedCreditRate(rate(0.5, [low, high]), clock(T0));
  assert.equal(shown.multiplier, 0.1);
  assert.equal(shown.promotion.id, "high");
  // Same priority keeps the first entry.
  const first = { id: "first", priority: 3, discountedCredits: 0.3 };
  const second = { id: "second", priority: 3, discountedCredits: 0.4 };
  assert.equal(displayedCreditRate(rate(0.5, [first, second]), clock(T0)).promotion.id, "first");
  // Disabled or condition-less entries never apply.
  const disabled = { id: "x", priority: 9, enabled: false, discountedCredits: 0.01 };
  assert.equal(displayedCreditRate(rate(0.5, [disabled, low]), clock(T0)).promotion.id, "low");
  // A promotion without any usable discount expression is ignored.
  const empty = { id: "y", priority: 9 };
  assert.equal(displayedCreditRate(rate(0.5, [empty]), clock(T0)).promotional, false);
  const notice = { id: "notice", kind: "badge", priority: 10 };
  assert.equal(displayedCreditRate(rate(0.5, [low, notice]), clock(T0)).multiplier, 0.5);
});

test("官方 schedule 支持多时段、全天与 IANA 时区的夏令时", () => {
  const promotion = { id: "official", priority: 1, discountedCredits: 0.06, timezone: "Asia/Shanghai",
    dailyWindows: [{ startMinute: 0, endMinute: 540 }, { startMinute: 720, endMinute: 840 }, { startMinute: 1080, endMinute: 0 }] };
  assert.equal(displayedCreditRate(rate(0.12, [promotion]), clock(T0)).promotional, false);
  assert.equal(displayedCreditRate(rate(0.12, [promotion]), clock(T_NIGHT)).multiplier, 0.06);
  assert.equal(displayedCreditRate(rate(0.12, [promotion]), clock(Date.parse("2026-11-16T12:00:00+08:00"))).promotional, true);
  assert.equal(displayedCreditRate(rate(0.12, [promotion]), clock(Date.parse("2026-11-16T14:00:00+08:00"))).promotional, false);
  const allDay = { ...promotion, dailyWindows: [{ startMinute: 0, endMinute: 0 }] };
  assert.equal(displayedCreditRate(rate(0.12, [allDay]), clock(T0)).promotional, true);
  const legacyDay = { id: "legacy", priority: 1, discountedCredits: 0.06, dailyStartMinute: 0, dailyEndMinute: 1440 };
  assert.equal(displayedCreditRate(rate(0.12, [legacyDay]), clock(T0)).promotional, true);
  const newYork = { ...promotion, timezone: "America/New_York", dailyWindows: [{ startMinute: 480, endMinute: 540 }] };
  for (const instant of ["2026-07-01T12:30:00Z", "2026-11-16T13:30:00Z"]) {
    assert.equal(displayedCreditRate(rate(0.12, [newYork]), clock(Date.parse(instant))).promotional, true);
  }
});

test("倍率标签保留有效小数，消除浮点计算噪声", () => {
  assert.equal(formatMultiplier(0.21), "0.21×");
  assert.equal(formatMultiplier(1), "1×");
  assert.equal(formatMultiplier(1.5), "1.5×");
  assert.equal(formatMultiplier(0.1 + 0.2), "0.3×");
  assert.equal(formatMultiplier(0.004), "0.004×");
  assert.equal(formatMultiplier(0.21 * 0.5), "0.105×");
});
