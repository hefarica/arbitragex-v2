/**
 * =============================================================================
 * OMEGA registries/types-omni — completes the 12-entity canonical fabric
 * =============================================================================
 *
 * Extends `frontend/lib/registries/types.ts` (which declared chain/dex/pool/
 * token/wallet/strategy) with the 6 missing canonical entities:
 *
 *   - Rpc            (chain-scoped)
 *   - Contract       (chain-scoped, deterministic CREATE2)
 *   - RiskGate       (global, scoped via `scope` discriminator)
 *   - CapitalGate    (global, $0 default until operator signs)
 *   - Relay          (chain-scoped)
 *   - Agent          (global, Sindicato OMEGA roster)
 *
 * Mirrors `backend/api-server/src/lib/registry-engine.ts` Zod schemas
 * line-for-line.  Zero-`any` policy enforced.
 *
 * @module registries/types-omni
 */

import type { Address, ConfigHash, EntityStatus, ISOTimestamp, UUID } from "./types";

// ---------------------------------------------------------------------------
// Rpc
// ---------------------------------------------------------------------------

export type RpcTransport = "http" | "https" | "ws" | "wss" | "ipc";
export type RpcTier = "primary" | "secondary" | "fallback" | "archive";
export type RpcAuthKind = "none" | "header" | "bearer" | "basic" | "query";

export interface RpcEntity {
  id: UUID;
  chain_id: number;
  url: string;
  transport: RpcTransport;
  tier: RpcTier;
  auth_kind: RpcAuthKind;
  auth_credential_id: UUID | null;
  weight: number;
  rate_limit_rps: number | null;
  max_concurrency: number;
  enabled: boolean;
  status: EntityStatus;
  config_hash: ConfigHash;
  last_probe_at: ISOTimestamp | null;
  last_latency_ms: number | null;
  notes: string | null;
  created_at: ISOTimestamp;
  updated_at: ISOTimestamp;
  created_by: string;
  updated_by: string;
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

export type ContractKind =
  | "factory"
  | "resolution_core"
  | "adapter_uniswap_v2"
  | "adapter_uniswap_v3"
  | "adapter_balancer"
  | "adapter_curve"
  | "adapter_pancake_v3"
  | "adapter_gmx"
  | "adapter_synthetix"
  | "flashloan_aave"
  | "flashloan_balancer"
  | "flashloan_dydx"
  | "flashloan_uniswap_v3"
  | "wallet_topology"
  | "gas_sponsor"
  | "cold_treasury"
  | "execution_signer_guard"
  | "allowance_manager"
  | "admin_timelock"
  | "custom";

export type ProxyKind = "uups" | "transparent" | "beacon" | "minimal" | "none";

export interface ContractEntity {
  id: UUID;
  chain_id: number;
  label: string;
  address: Address;
  deployer: Address;
  salt: `0x${string}`;
  init_code_hash: `0x${string}`;
  abi_version: string;
  contract_kind: ContractKind;
  proxy_kind: ProxyKind | null;
  implementation: Address | null;
  deploy_tx: `0x${string}` | null;
  deploy_block: number | null;
  verified: boolean;
  enabled: boolean;
  status: EntityStatus;
  config_hash: ConfigHash;
  metadata: Record<string, unknown>;
  created_at: ISOTimestamp;
  updated_at: ISOTimestamp;
  created_by: string;
  updated_by: string;
}

// ---------------------------------------------------------------------------
// RiskGate
// ---------------------------------------------------------------------------

export type RiskScope =
  | "global"
  | "chain"
  | "wallet"
  | "strategy"
  | "token"
  | "route"
  | "dex"
  | "time_window";

export type RiskGateKind =
  | "min_confidence"
  | "max_slippage_bps"
  | "min_liquidity_usd"
  | "max_price_impact_bps"
  | "min_spectral_gap"
  | "max_decoherence"
  | "blacklist_token"
  | "blacklist_route"
  | "time_lock"
  | "circuit_breaker";

export type Comparator = "lt" | "lte" | "gt" | "gte" | "eq" | "neq" | "in" | "nin";

export type BreachAction =
  | "reject"
  | "warn"
  | "throttle"
  | "pause_chain"
  | "pause_global";

export interface RiskGateEntity {
  id: UUID;
  scope: RiskScope;
  scope_ref: string | null;
  gate_kind: RiskGateKind;
  threshold_value: number;
  comparator: Comparator;
  action_on_breach: BreachAction;
  enabled: boolean;
  status: EntityStatus;
  config_hash: ConfigHash;
  created_at: ISOTimestamp;
  updated_at: ISOTimestamp;
  created_by: string;
  updated_by: string;
}

// ---------------------------------------------------------------------------
// CapitalGate
// ---------------------------------------------------------------------------

export type CapitalScope =
  | "global"
  | "chain"
  | "wallet"
  | "strategy"
  | "token"
  | "route";

export interface CapitalGateEntity {
  id: UUID;
  scope: CapitalScope;
  scope_ref: string | null;
  name: string;
  capital_cap_usd: number;
  max_gas_burn_usd: number;
  max_drawdown_pct: number;
  max_consecutive_err: number;
  max_revert_rate_pct: number;
  max_latency_ms: number;
  window_seconds: number;
  enabled: boolean;
  status: EntityStatus;
  config_hash: ConfigHash;
  created_at: ISOTimestamp;
  updated_at: ISOTimestamp;
  created_by: string;
  updated_by: string;
}

// ---------------------------------------------------------------------------
// Relay
// ---------------------------------------------------------------------------

export type RelayKind =
  | "flashbots"
  | "bloxroute"
  | "eden"
  | "manifold"
  | "public"
  | "custom";

export interface RelayEntity {
  id: UUID;
  chain_id: number;
  name: string;
  url: string;
  relay_kind: RelayKind;
  auth_credential_id: UUID | null;
  priority: number;
  timeout_ms: number;
  enabled: boolean;
  status: EntityStatus;
  config_hash: ConfigHash;
  last_health_at: ISOTimestamp | null;
  last_health_ok: boolean | null;
  created_at: ISOTimestamp;
  updated_at: ISOTimestamp;
  created_by: string;
  updated_by: string;
}

// ---------------------------------------------------------------------------
// Agent
// ---------------------------------------------------------------------------

export type AgentRank = "phd" | "nobel" | "principal" | "architect" | "auditor";

export interface AgentEntity {
  id: UUID;
  role: string;
  discipline: string;
  rank: AgentRank;
  active: boolean;
  capabilities: string[];
  evidence_bindings: string[];
  status: EntityStatus;
  config_hash: ConfigHash;
  created_at: ISOTimestamp;
  updated_at: ISOTimestamp;
  created_by: string;
  updated_by: string;
}

// ---------------------------------------------------------------------------
// Union extension — 12 canonical entity types
// ---------------------------------------------------------------------------

export type OmniMutableEntityType =
  | "chain"
  | "rpc"
  | "dex"
  | "pool"
  | "token"
  | "wallet"
  | "strategy"
  | "contract"
  | "risk_gate"
  | "capital_gate"
  | "relay"
  | "agent";

export type OmniEntity =
  | RpcEntity
  | ContractEntity
  | RiskGateEntity
  | CapitalGateEntity
  | RelayEntity
  | AgentEntity;

// ---------------------------------------------------------------------------
// Runtime ack / drift surfaces (mirror system-manifest.ts)
// ---------------------------------------------------------------------------

export type AckLayer =
  | "api"
  | "persistence"
  | "redis_pubsub"
  | "searcher_rs"
  | "arc_swap"
  | "frontend_refresh"
  | "readiness"
  | "audit";

export type AckStatus =
  | "pending"
  | "received"
  | "applied"
  | "rejected"
  | "timeout"
  | "failed";

export interface RuntimeAckRecord {
  id: UUID;
  event_id: UUID;
  resource: string;
  chain_id: number | null;
  idempotency_key: string;
  config_hash_before: ConfigHash | null;
  config_hash_after: ConfigHash;
  worker_id: string;
  layer: AckLayer;
  status: AckStatus;
  latency_ms: number | null;
  error: string | null;
  received_at: ISOTimestamp;
}

export interface DriftObservation {
  id: UUID;
  resource: string;
  chain_id: number | null;
  layer_a: string;
  layer_b: string;
  hash_a: ConfigHash | null;
  hash_b: ConfigHash | null;
  diff_count: number;
  diff_summary: Record<string, unknown>;
  severity: "info" | "warn" | "error" | "critical";
  observed_at: ISOTimestamp;
  resolved_at: ISOTimestamp | null;
}

export interface FeatureManifestEntry {
  feature_key: string;
  description: string;
  layer: "backend" | "contract" | "runtime" | "frontend" | "edge";
  state_hash: ConfigHash;
  panel_path: string;
  required: boolean;
  updated_at: ISOTimestamp;
}
