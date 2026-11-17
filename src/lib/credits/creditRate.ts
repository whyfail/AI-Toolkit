import type { CreditModel, CreditPromotion, CreditRate } from "@/lib/api/credits";

/** Stable "now" hook for tests; production callers use the default. */
export interface RateClock {
  now?: () => number;
  /** Local timezone offset in minutes (Date.prototype.getTimezoneOffset semantics). */
  localOffsetMinutes?: () => number;
}

const defaultClock: Required<RateClock> = {
  now: () => Date.now(),
  localOffsetMinutes: () => new Date().getTimezoneOffset(),
};

const finiteNumber = (value: unknown): value is number =>
  typeof value === "number" && Number.isFinite(value);

const validMultiplier = (value: unknown): value is number =>
  finiteNumber(value) && value >= 0;

const DAY_MS = 86_400_000;

function declaredMultiplier(value: unknown): number | null {
  if (typeof value !== "string") return null;
  const declared = value.trim().replace(/\s+credits?$/i, "").trimEnd();
  const match = declared.match(/^(?:[xX×]\s*(\d+(?:\.\d+)?|\.\d+)|(\d+(?:\.\d+)?|\.\d+)\s*[xX×倍]?)$/);
  if (!match) return null;
  const multiplier = Number(match[1] ?? match[2]);
  return validMultiplier(multiplier) ? multiplier : null;
}

function minuteInTimezone(promotion: CreditPromotion, timestampMs: number, localOffsetMinutes: number): number {
  if (promotion.timezone) {
    try {
      const parts = new Intl.DateTimeFormat("en-GB", {
        timeZone: promotion.timezone, hour: "2-digit", minute: "2-digit", hourCycle: "h23",
      }).formatToParts(timestampMs);
      return Number(parts.find((part) => part.type === "hour")?.value) * 60
        + Number(parts.find((part) => part.type === "minute")?.value);
    } catch {
      // Match the official client's local-time fallback for an invalid timezone.
    }
  }
  const offsetSeconds = finiteNumber(promotion.timezoneOffsetSeconds)
    ? promotion.timezoneOffsetSeconds : -localOffsetMinutes * 60;
  const localMs = ((timestampMs + offsetSeconds * 1000) % DAY_MS + DAY_MS) % DAY_MS;
  return Math.floor(localMs / 60_000);
}

/**
 * Resolve whether one promotion is effective at `timestampMs`; false when it
 * is disabled or outside its window. Windows are left-closed/right-open and
 * support overnight spans and full-day windows.
 */
function promotionActive(promotion: CreditPromotion, timestampMs: number, localOffsetMinutes: number): boolean {
  if (promotion.enabled === false) return false;
  if (promotion.startsAt != null && timestampMs < promotion.startsAt) return false;
  if (promotion.endsAt != null && timestampMs >= promotion.endsAt) return false;
  const windows = promotion.dailyWindows?.length ? promotion.dailyWindows
    : promotion.dailyStartMinute != null || promotion.dailyEndMinute != null
      ? [{ startMinute: promotion.dailyStartMinute, endMinute: promotion.dailyEndMinute }] : [];
  if (!windows.length) return true;
  const minute = minuteInTimezone(promotion, timestampMs, localOffsetMinutes);
  return windows.some(({ startMinute: start, endMinute: end }) => {
    if (start == null || end == null || !Number.isInteger(start) || !Number.isInteger(end)
      || start < 0 || start > 1440 || end < 0 || end > 1440) return false;
    if (start === end) return true;
    return start < end ? minute >= start && minute < end : minute >= start || minute < end;
  });
}

export interface RateDisplay {
  /** The multiplier the list should show; null means unknown. */
  multiplier: number | null;
  /** Whether the shown value comes from a currently active promotion. */
  promotional: boolean;
  /** The active promotion when `promotional` is true. */
  promotion: CreditPromotion | null;
  /** Base multiplier from the catalog, when known. */
  base: number | null;
}

/**
 * Compute the displayed credit rate for a model at the current moment.
 * - Active promotions win over the base rate (highest priority, catalog order).
 * - `discountedCredits` is preferred; otherwise base × discountFactor.
 * - An unresolved activity uses the declared base rate; missing rates stay unknown.
 */
export function displayedCreditRate(model: Pick<CreditModel, "creditRate">, clock: RateClock = {}): RateDisplay {
  const now = clock.now ?? defaultClock.now;
  const localOffsetMinutes = clock.localOffsetMinutes ?? defaultClock.localOffsetMinutes;
  const rate: CreditRate | null | undefined = model.creditRate;
  // Recover WorkBuddy snapshots produced before prefix notation was supported.
  const cachedBase = rate?.source === "enterprise-models/v3-config" ? declaredMultiplier(rate.declaredText) : null;
  const base = validMultiplier(rate?.multiplier) ? rate.multiplier : cachedBase;
  const timestampMs = now();
  let best: CreditPromotion | null = null;
  for (const promotion of rate?.promotions ?? []) {
    const active = promotionActive(promotion, timestampMs, localOffsetMinutes());
    if (active !== true) continue;
    if (best == null || promotion.priority > best.priority) {
      best = promotion;
    }
  }
  if (best && (best.kind == null || best.kind === "discount")) {
    const multiplier = validMultiplier(best.discountedCredits) ? best.discountedCredits
      : base != null && finiteNumber(best.discountFactor) && best.discountFactor > 0 && best.discountFactor < 1
        ? base * best.discountFactor : null;
    if (multiplier != null) {
      return { multiplier, promotional: true, promotion: best, base };
    }
  }
  return { multiplier: base, promotional: false, promotion: null, base };
}

/** Preserve small catalog rates while removing floating-point multiplication noise. */
export function formatMultiplier(multiplier: number): string {
  return `${Number(multiplier.toFixed(10))}×`;
}
