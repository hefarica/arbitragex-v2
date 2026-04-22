/**
 * Thin client for the edge. Only uses endpoints the edge actually exposes.
 *
 * Honesty contract: every helper returns a discriminated union
 *   { ok: true, data } | { ok: false, error }
 * Pages consume `error` verbatim; we NEVER fabricate data on failure.
 */

const EDGE_URL = process.env.NEXT_PUBLIC_EDGE_URL || "http://localhost:8787";

type Result<T> = { ok: true; data: T } | { ok: false; error: string };

async function get<T>(path: string): Promise<Result<T>> {
  try {
    const r = await fetch(`${EDGE_URL}${path}`, {
      next: { revalidate: 0 },
      headers: { accept: "application/json" },
    });
    const text = await r.text();
    if (!r.ok) {
      return { ok: false, error: `edge HTTP ${r.status}${text ? `: ${text.slice(0, 200)}` : ""}` };
    }
    return { ok: true, data: JSON.parse(text) as T };
  } catch (e) {
    return { ok: false, error: (e as Error).message };
  }
}

// ─────── /status ───────

export type StatusResponse = {
  ok: boolean;
  services: Record<string, { ok: boolean; status?: number; detail?: string }>;
  killswitch: KillSwitchState | null;
  env: string;
  version: string;
  ts: string;
};
export type KillSwitchState = {
  enabled: boolean;
  reason: string | null;
  triggered_by: string | null;
  updated_at: string;
};
export async function getStatus() { return get<StatusResponse>("/status"); }

// ─────── /api/opportunities/live ───────

export type OpportunityRow = {
  id: string;
  chain_id: number;
  strategy_kind: string;
  dex_a: string;
  dex_b: string | null;
  pair_symbol: string | null;
  token_in: string;
  token_out: string;
  amount_in_wei: string;
  expected_profit_usd: number | null;
  roi_pct: number | null;
  risk_score: number | null;
  block_number: number | null;
  status: string;
  detected_at: string;
  trace_id: string;
};
export type OpportunitiesLive = {
  count: number;
  window: string;
  items: OpportunityRow[];
  ts: string;
};
export async function getOpportunitiesLive(limit = 50) {
  return get<OpportunitiesLive>(`/api/opportunities/live?limit=${limit}`);
}

// ─────── /api/risk/alerts ───────

export type RiskAlertRow = {
  id: string;
  event_type: "circuit_breaker" | "kill_switch" | "blacklist_hit" | "degradation" | "manual";
  severity: "info" | "warning" | "critical";
  source_service: string;
  payload: unknown;
  trace_id: string | null;
  opportunity_id: string | null;
  created_at: string;
};
export type RiskAlertsResponse = {
  window_hours: number;
  killswitch: KillSwitchState | null;
  alerts: RiskAlertRow[];
  ts: string;
};
export async function getRiskAlerts(hours = 24) {
  return get<RiskAlertsResponse>(`/api/risk/alerts?hours=${hours}`);
}

// ─────── /api/executions/recent ───────

export type ExecutionRow = {
  id: string;
  tx_hash: string | null;
  bundle_hash: string | null;
  relay_name: string;
  status: "submitted" | "included" | "reverted" | "dropped" | "replaced" | "not_implemented";
  block_included: number | null;
  gas_used_wei: string | null;
  gas_price_effective_wei: string | null;
  expected_profit_usd: number | null;
  actual_profit_usd: number | null;
  error_message: string | null;
  trace_id: string;
  submitted_at: string;
  confirmed_at: string | null;
  chain_id: number;
  strategy_kind: string;
  pair_symbol: string | null;
};
export type ExecutionsRecent = {
  count: number;
  items: ExecutionRow[];
  ts: string;
};
export async function getExecutionsRecent(limit = 50) {
  return get<ExecutionsRecent>(`/api/executions/recent?limit=${limit}`);
}

// ─────── /api/recon/summary ───────

export type ReconSummary = {
  window_hours: number;
  totals: {
    total: number;
    included: number;
    reverted: number;
    dropped: number;
    avg_pnl_included_usd: number | null;
    avg_confirm_latency_ms: number | null;
  };
  revert_rate: number | null;
  top_strategies: Array<{
    strategy_kind: string;
    chain_id: number;
    sample_count: number;
    success_rate: number | null;
    revert_rate: number | null;
    avg_profit_usd: number | null;
    score: number | null;
    window_end: string;
  }>;
  critical_anomalies_24h: Array<{
    event_type: string;
    severity: string;
    source_service: string;
    payload: unknown;
    created_at: string;
  }>;
  ts: string;
};
export async function getReconSummary(hours = 1) {
  return get<ReconSummary>(`/api/recon/summary?hours=${hours}`);
}

// ─────── /api/config/current ───────

export type AppConfigView = {
  system: { env: string; kill_switch_enabled_default: boolean; service_name_prefix: string };
  risk: Record<string, number | boolean>;
  execution: Record<string, number | boolean | string>;
  observability: Record<string, boolean | string>;
  chains: Array<{ chain_id: number; name: string; enabled: boolean }>;
  relays: Array<{ name: string; enabled: boolean; chains: number[]; endpoint: string }>;
  scoring: Record<string, number>;
  token_safety: Record<string, string | number>;
  circuit_breakers: Array<{ name: string; threshold: number; window_ms: number; cooldown_ms: number }>;
};
export async function getConfigCurrent() {
  return get<AppConfigView>("/api/config/current");
}

// ─────── Admin: killswitch toggle (POST, edge does not proxy yet) ───────
// This call goes direct to api-server because the edge worker is intentionally
// read-only. In production the operator console runs behind the ops.* tunnel,
// which reaches api-server via nginx + basic-auth.

export async function toggleKillswitch(
  enabled: boolean,
  reason: string,
  adminToken: string,
): Promise<Result<KillSwitchState>> {
  try {
    const r = await fetch(`${EDGE_URL.replace(/\/$/, "")}/admin/killswitch`, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        "x-arbx-admin-token": adminToken,
      },
      body: JSON.stringify({ enabled, reason }),
    });
    const text = await r.text();
    if (!r.ok) return { ok: false, error: `HTTP ${r.status}: ${text.slice(0, 200)}` };
    return { ok: true, data: JSON.parse(text) as KillSwitchState };
  } catch (e) {
    return { ok: false, error: (e as Error).message };
  }
}
