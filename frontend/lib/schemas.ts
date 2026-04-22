import { z } from "zod";

export const KillSwitchStateSchema = z.object({
  enabled: z.boolean(),
  reason: z.string().nullable(),
  triggered_by: z.string().nullable(),
  updated_at: z.string(),
});

export const StatusResponseSchema = z.object({
  ok: z.boolean(),
  services: z.record(
    z.string(),
    z.object({
      ok: z.boolean(),
      status: z.number().optional(),
      detail: z.string().optional(),
    }),
  ),
  killswitch: KillSwitchStateSchema.nullable(),
  env: z.string(),
  version: z.string(),
  ts: z.string(),
});

export const OpportunityRowSchema = z.object({
  id: z.string(),
  chain_id: z.number(),
  strategy_kind: z.string(),
  dex_a: z.string(),
  dex_b: z.string().nullable(),
  pair_symbol: z.string().nullable(),
  token_in: z.string(),
  token_out: z.string(),
  amount_in_wei: z.string(),
  expected_profit_usd: z.number().nullable(),
  roi_pct: z.number().nullable(),
  risk_score: z.number().nullable(),
  block_number: z.number().nullable(),
  status: z.string(),
  detected_at: z.string(),
  trace_id: z.string(),
});

export const OpportunitiesLiveSchema = z.object({
  count: z.number(),
  window: z.string(),
  items: z.array(OpportunityRowSchema),
  ts: z.string(),
});

export const RiskAlertRowSchema = z.object({
  id: z.string(),
  event_type: z.enum(["circuit_breaker", "kill_switch", "blacklist_hit", "degradation", "manual"]),
  severity: z.enum(["info", "warning", "critical"]),
  source_service: z.string(),
  payload: z.unknown(),
  trace_id: z.string().nullable(),
  opportunity_id: z.string().nullable(),
  created_at: z.string(),
});

export const RiskAlertsResponseSchema = z.object({
  window_hours: z.number(),
  killswitch: KillSwitchStateSchema.nullable(),
  alerts: z.array(RiskAlertRowSchema),
  ts: z.string(),
});

export const ExecutionRowSchema = z.object({
  id: z.string(),
  tx_hash: z.string().nullable(),
  bundle_hash: z.string().nullable(),
  relay_name: z.string(),
  status: z.enum(["submitted", "included", "reverted", "dropped", "replaced", "not_implemented"]),
  block_included: z.number().nullable(),
  gas_used_wei: z.string().nullable(),
  gas_price_effective_wei: z.string().nullable(),
  expected_profit_usd: z.number().nullable(),
  actual_profit_usd: z.number().nullable(),
  error_message: z.string().nullable(),
  trace_id: z.string(),
  submitted_at: z.string(),
  confirmed_at: z.string().nullable(),
  chain_id: z.number(),
  strategy_kind: z.string(),
  pair_symbol: z.string().nullable(),
});

export const ExecutionsRecentSchema = z.object({
  count: z.number(),
  items: z.array(ExecutionRowSchema),
  ts: z.string(),
});

export const ReconSummarySchema = z.object({
  window_hours: z.number(),
  totals: z.object({
    total: z.number(),
    included: z.number(),
    reverted: z.number(),
    dropped: z.number(),
    avg_pnl_included_usd: z.number().nullable(),
    avg_confirm_latency_ms: z.number().nullable(),
  }),
  revert_rate: z.number().nullable(),
  top_strategies: z.array(
    z.object({
      strategy_kind: z.string(),
      chain_id: z.number(),
      sample_count: z.number(),
      success_rate: z.number().nullable(),
      revert_rate: z.number().nullable(),
      avg_profit_usd: z.number().nullable(),
      score: z.number().nullable(),
      window_end: z.string(),
    }),
  ),
  critical_anomalies_24h: z.array(
    z.object({
      event_type: z.string(),
      severity: z.string(),
      source_service: z.string(),
      payload: z.unknown(),
      created_at: z.string(),
    }),
  ),
  ts: z.string(),
});

export const ReconTimeseriesPointSchema = z.object({
  bucket_start: z.string(),
  attempts: z.number(),
  included: z.number(),
  reverted: z.number(),
  avg_pnl_included_usd: z.number().nullable(),
  revert_rate: z.number().nullable(),
});

export const ReconTimeseriesResponseSchema = z.object({
  window_hours: z.number(),
  bucket_minutes: z.number(),
  points: z.array(ReconTimeseriesPointSchema),
  ts: z.string(),
});

export const AppConfigViewSchema = z.object({
  system: z.object({
    env: z.string(),
    kill_switch_enabled_default: z.boolean(),
    service_name_prefix: z.string(),
  }),
  risk: z.record(z.string(), z.union([z.number(), z.boolean()])),
  execution: z.record(z.string(), z.union([z.number(), z.boolean(), z.string()])),
  observability: z.record(z.string(), z.union([z.boolean(), z.string()])),
  chains: z.array(
    z.object({
      chain_id: z.number(),
      name: z.string(),
      enabled: z.boolean(),
    }),
  ),
  relays: z.array(
    z.object({
      name: z.string(),
      enabled: z.boolean(),
      chains: z.array(z.number()),
      endpoint: z.string(),
    }),
  ),
  scoring: z.record(z.string(), z.number()),
  token_safety: z.record(z.string(), z.union([z.string(), z.number()])),
  circuit_breakers: z.array(
    z.object({
      name: z.string(),
      threshold: z.number(),
      window_ms: z.number(),
      cooldown_ms: z.number(),
    }),
  ),
});

export const RelayRowSchema = z.object({
  name: z.string(),
  chain_id: z.number(),
  endpoint: z.string().nullable(),
  auth_scheme: z.enum(["none", "x-flashbots-signature", "bearer", "header-auth", "custom"]),
  enabled: z.boolean(),
  priority: z.number(),
});

export const RelaysResponseSchema = z.object({
  count: z.number(),
  items: z.array(RelayRowSchema),
  ts: z.string(),
});

export const OnboardingStatusSchema = z.object({
  org_id: z.string(),
  phase_1_completed_at: z.string().nullable(),
  phase_1_completed_by: z.string().nullable(),
  phase_1_vault_sealed_healthy: z.boolean(),
  phase_2_completed_at: z.string().nullable(),
  phase_2_completed_by: z.string().nullable(),
  phase_2_rpc_probe_ok: z.boolean(),
  phase_3_completed_at: z.string().nullable(),
  phase_3_completed_by: z.string().nullable(),
  phase_4_completed_at: z.string().nullable(),
  phase_4_completed_by: z.string().nullable(),
  phase_4_signer_zero_balance_verified: z.boolean(),
  phase_5_completed_at: z.string().nullable(),
  phase_5_completed_by: z.string().nullable(),
  phase_5_paper_mode_off_at: z.string().nullable(),
  created_at: z.string(),
  updated_at: z.string(),
});

export const OnboardingPhase1ResultSchema = z.object({
  phase_1_completed_at: z.string(),
  phase_1_completed_by: z.string(),
  phase_1_vault_sealed_healthy: z.boolean(),
});

// ─────── Derived types ───────

export type KillSwitchState = z.infer<typeof KillSwitchStateSchema>;
export type StatusResponse = z.infer<typeof StatusResponseSchema>;
export type OpportunityRow = z.infer<typeof OpportunityRowSchema>;
export type OpportunitiesLive = z.infer<typeof OpportunitiesLiveSchema>;
export type RiskAlertRow = z.infer<typeof RiskAlertRowSchema>;
export type RiskAlertsResponse = z.infer<typeof RiskAlertsResponseSchema>;
export type ExecutionRow = z.infer<typeof ExecutionRowSchema>;
export type ExecutionsRecent = z.infer<typeof ExecutionsRecentSchema>;
export type ReconSummary = z.infer<typeof ReconSummarySchema>;
export type ReconTimeseriesPoint = z.infer<typeof ReconTimeseriesPointSchema>;
export type ReconTimeseriesResponse = z.infer<typeof ReconTimeseriesResponseSchema>;
export type AppConfigView = z.infer<typeof AppConfigViewSchema>;
export type RelayRow = z.infer<typeof RelayRowSchema>;
export type RelaysResponse = z.infer<typeof RelaysResponseSchema>;
export type OnboardingStatus = z.infer<typeof OnboardingStatusSchema>;
export type OnboardingPhase1Result = z.infer<typeof OnboardingPhase1ResultSchema>;
