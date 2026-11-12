import { useEffect, useState } from "react";
import { updateApi } from "@/lib/api";

export type AvailableUpdate = {
  version: string;
  body: string;
};

/** 静默检查的间隔：启动后每隔 6 小时复查一次。 */
const CHECK_INTERVAL_MS = 6 * 60 * 60 * 1000;
/** 检查失败后的重试间隔，避免断网时每 6 小时才重试一次。 */
const FAILURE_RETRY_MS = 15 * 60 * 1000;
const CHECKED_AT_KEY = "ai-toolkit-update-checked-at";
const UPDATE_INFO_KEY = "ai-toolkit-update-info";

type Listener = (update: AvailableUpdate | null) => void;

let cachedUpdate: AvailableUpdate | null = null;
let currentVersion: string | null = null;
let started = false;
let timer: ReturnType<typeof setInterval> | undefined;
let running: Promise<void> | null = null;
let lastCheckedAt = 0;
let lastCheckFailed = false;
let stopOnlineListener: (() => void) | undefined;
const listeners = new Set<Listener>();

function readStorage(key: string): string | null {
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
}

function writeStorage(key: string, value: string | null) {
  try {
    if (value === null) window.localStorage.removeItem(key);
    else window.localStorage.setItem(key, value);
  } catch {
    // 隐私模式或配额已满时降级为纯内存，不阻断检查流程。
  }
}

function parseVersionParts(value: string): number[] | null {
  const match = /^v?(\d+)\.(\d+)\.(\d+)/.exec(value.trim());
  return match ? match.slice(1, 4).map(Number) : null;
}

/** 比较 `x.y.z` 形式的版本号，无法解析时返回 null 表示不比较。 */
function compareVersions(a: string, b: string): number | null {
  const left = parseVersionParts(a);
  const right = parseVersionParts(b);
  if (!left || !right) return null;
  for (let i = 0; i < 3; i += 1) {
    if (left[i] !== right[i]) return left[i] - right[i];
  }
  return 0;
}

/**
 * 已安装的版本不该继续显示徽标。当前版本未知时保持原样，
 * 等 `useAppVersion` 给出真实版本后由调用方重新过滤。
 */
function isAlreadyInstalled(update: AvailableUpdate) {
  if (!currentVersion) return false;
  const order = compareVersions(update.version, currentVersion);
  return order !== null && order <= 0;
}

function publish(update: AvailableUpdate | null) {
  const available = update && !isAlreadyInstalled(update) ? update : null;
  cachedUpdate = available;
  writeStorage(UPDATE_INFO_KEY, available ? JSON.stringify(available) : null);
  listeners.forEach((listener) => listener(available));
}

/** 冷启动时先用上次缓存恢复徽标，避免每次都等网络请求回来才出现。 */
function restoreCachedUpdate() {
  const raw = readStorage(UPDATE_INFO_KEY);
  if (!raw) return;
  try {
    const parsed = JSON.parse(raw) as Partial<AvailableUpdate>;
    if (typeof parsed.version === "string" && parsed.version) {
      publish({ version: parsed.version, body: typeof parsed.body === "string" ? parsed.body : "" });
      return;
    }
  } catch {
    // 缓存损坏时当作没有缓存。
  }
  writeStorage(UPDATE_INFO_KEY, null);
}

function isCheckDue() {
  const last = lastCheckedAt || Number(readStorage(CHECKED_AT_KEY));
  if (!Number.isFinite(last) || last <= 0) return true;
  return Date.now() - last >= CHECK_INTERVAL_MS;
}

/**
 * 执行一次检查并广播结果。失败时写入较短的退避时间戳，
 * 让下一轮定时器或网络恢复时能尽早重试。
 */
function runCheck(): Promise<void> {
  if (running) return running;
  running = updateApi.checkUpdate()
    .then((result) => {
      lastCheckedAt = Date.now();
      lastCheckFailed = false;
      writeStorage(CHECKED_AT_KEY, String(lastCheckedAt));
      publish(result.available && result.version
        ? { version: result.version, body: result.body || "" }
        : null);
    })
    .catch((err) => {
      lastCheckFailed = true;
      lastCheckedAt = Date.now() - CHECK_INTERVAL_MS + FAILURE_RETRY_MS;
      writeStorage(CHECKED_AT_KEY, String(lastCheckedAt));
      throw err;
    })
    .finally(() => {
      running = null;
    });
  return running;
}

/** 后台检查的失败只记录日志，不打扰用户。 */
function runSilently() {
  void runCheck().catch((err) => {
    console.warn("检查更新失败:", err instanceof Error ? err.message : err);
  });
}

function start() {
  if (started) return;
  started = true;

  restoreCachedUpdate();

  if (isCheckDue()) runSilently();

  timer = setInterval(() => {
    if (isCheckDue()) runSilently();
  }, FAILURE_RETRY_MS);

  // 断网期间检查失败后，网络恢复时立即重试，不必等定时器。
  const onOnline = () => {
    if (lastCheckFailed || isCheckDue()) runSilently();
  };
  window.addEventListener("online", onOnline);
  stopOnlineListener = () => window.removeEventListener("online", onOnline);
}

function subscribe(listener: Listener) {
  listeners.add(listener);
  start();
  return () => {
    listeners.delete(listener);
    if (listeners.size === 0) {
      if (timer) {
        clearInterval(timer);
        timer = undefined;
      }
      stopOnlineListener?.();
      stopOnlineListener = undefined;
      started = false;
    }
  };
}

/** 供设置页手动检查复用：立即检查并同步全局徽标状态。 */
export async function checkNow(): Promise<AvailableUpdate | null> {
  await runCheck();
  return cachedUpdate && !isAlreadyInstalled(cachedUpdate) ? cachedUpdate : null;
}

/**
 * 启动时静默检查新版本，之后每 6 小时复查一次（失败则 15 分钟后重试）。
 * 只把结果交给调用方渲染角标，不弹窗；失败时仅记录日志。
 *
 * `installedVersion` 用于过滤缓存里已经装上的版本，避免升级后仍显示角标。
 */
export function useStartupUpdateCheck(installedVersion?: string): AvailableUpdate | null {
  const [update, setUpdate] = useState<AvailableUpdate | null>(cachedUpdate);

  useEffect(() => {
    if (!installedVersion || installedVersion === currentVersion) return;
    currentVersion = installedVersion;
    if (cachedUpdate && isAlreadyInstalled(cachedUpdate)) {
      publish(null);
    }
  }, [installedVersion]);

  useEffect(() => subscribe(setUpdate), []);

  return update;
}
