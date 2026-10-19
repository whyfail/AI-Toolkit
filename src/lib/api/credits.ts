import { invoke } from "@tauri-apps/api/core";

export type CreditPlatform = "workbuddy" | "traework" | "qoder";

export interface Credentials {
  accessToken: string;
  refreshToken?: string | null;
  userId?: string | null;
  deviceId?: string | null;
  expiresAt?: number | null;
  extra: unknown;
}

export interface CreditPack {
  id: string;
  name: string;
  remaining: number | null;
  expiresAt: number | null;
}

export interface CreditModel {
  id: string;
  upstreamId?: string | null;
  upstreamModelIds?: string[];
  name: string;
  canonicalId: string | null;
  supportsTools: boolean;
  contextWindow: number | null;
  contextWindowOptions?: number[];
  maxOutputTokens?: number | null;
  supportsImages?: boolean | null;
  supportsReasoning?: boolean | null;
  verified: boolean;
  verificationNote: string | null;
}

export interface CreditReward {
  id: string;
  name: string;
  kind: string;
  status: string;
  amount: number | null;
  nextAvailableAt: number | null;
  actionUrl: string | null;
}

export interface CreditSnapshot {
  balance: number | null;
  unit: string;
  quota: string | null;
  packs: CreditPack[];
  rewards: CreditReward[];
  models: CreditModel[];
  fetchedAt: number;
  stale: boolean;
  diagnostic: string | null;
}

export interface CreditAccount {
  id: string;
  platform: CreditPlatform;
  userId: string;
  label: string;
  enabled: boolean;
  status: string;
  lastError: string | null;
  snapshot: CreditSnapshot | null;
}

export interface RewardOutcome {
  accountId: string;
  rewardId: string;
  name: string;
  status: string;
  amount: number | null;
  kind: string;
  message: string;
  createdAt: number;
}

export interface ProxySettings {
  port: number;
  autoClaim: boolean;
}

export interface GatewayStatus {
  running: boolean;
  port: number;
  baseUrl: string;
  lastError: string | null;
}

export interface ProxyRequest {
  id: string;
  accountId: string;
  platform: CreditPlatform;
  model: string;
  elapsedMs: number;
  inputTokens: number | null;
  outputTokens: number | null;
  status: string;
  createdAt: number;
}

export interface CreditDashboard {
  accounts: CreditAccount[];
  settings: ProxySettings;
  gateway: GatewayStatus;
  rewards: RewardOutcome[];
  requests: ProxyRequest[];
}

export interface OAuthStart {
  flowId: string;
  authorizationUrl: string;
  expiresAt: number;
}

export type OAuthResult =
  | { status: "pending"; message?: string }
  | { status: "completed"; account: CreditAccount; message?: string }
  | { status: "failed"; message: string };

export const creditsApi = {
  dashboard: () => invoke<CreditDashboard>("credits_dashboard"),
  importLocal: (platform: CreditPlatform) =>
    invoke<CreditAccount>("credits_import_local", { platform }),
  setAccountEnabled: (accountId: string, enabled: boolean) =>
    invoke<void>("credits_set_account_enabled", { accountId, enabled }),
  deleteAccount: (accountId: string) => invoke<void>("credits_delete_account", { accountId }),
  refresh: (accountId?: string) => invoke<CreditAccount[]>("credits_refresh", { accountId }),
  claim: (accountId?: string) => invoke<RewardOutcome[]>("credits_claim", { accountId }),
  saveSettings: (settings: ProxySettings) =>
    invoke<ProxySettings>("credits_save_settings", { settings }),
  startProxy: () => invoke<GatewayStatus>("credits_start_proxy"),
  stopProxy: () => invoke<GatewayStatus>("credits_stop_proxy"),
  proxyKey: () => invoke<string>("credits_proxy_key"),
  probeProxy: () => invoke<string>("credits_probe_proxy"),
  verifyModel: (accountId: string, modelId: string) =>
    invoke<CreditModel>("credits_verify_model", { accountId, modelId }),
  oauthStart: (platform: CreditPlatform) => invoke<OAuthStart>("credits_oauth_start", { platform }),
  oauthPoll: (flowId: string) => invoke<OAuthResult>("credits_oauth_poll", { flowId }),
};
