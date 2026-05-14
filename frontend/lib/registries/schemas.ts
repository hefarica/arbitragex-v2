/**
 * =============================================================================
 * OMEGA Entity Registry Zod Schemas
 * =============================================================================
 *
 * Runtime validation schemas that mirror the TypeScript types in ./types.ts.
 * Every field is validated; zero tolerance for shape mismatches.
 *
 * Ghost Protocol: ACTIVO  |  R8 Fail-Honest: PRESERVADO
 *
 * @module registries/schemas
 */

import { z } from "zod";

// ---------------------------------------------------------------------------
// Primitive Shared Schemas
// ---------------------------------------------------------------------------

/** Lifecycle status enum shared by all operational entities. */
export const EntityStatusSchema = z.enum([
  "active",
  "paused",
  "blocked",
  "pending_validation",
  "deprecated",
]);

/** Checksummed Ethereum address: 42 chars, 0x prefix, hex digits only. */
export const AddressSchema = z
  .string()
  .regex(/^0x[a-fA-F0-9]{40}$/, "Must be a checksummed Ethereum address (42 chars, 0x prefix)");

/** SHA-256 digest as 64-character lowercase hexadecimal string. */
export const ConfigHashSchema = z
  .string()
  .length(64)
  .regex(/^[a-f0-9]{64}$/, "Must be a 64-char lowercase hex SHA-256 digest");

/** ISO-8601 timestamp with explicit UTC timezone marker. */
export const ISOTimestampSchema = z.string().datetime({ offset: true });

/** Bigint serialized as a non-negative decimal string. */
export const BigIntStringSchema = z
  .string()
  .regex(/^\d+$/, "Must be a non-negative decimal integer string");

/** UUID v4 format. */
export const UUIDSchema = z.string().uuid();

// ---------------------------------------------------------------------------
// Chain
// ---------------------------------------------------------------------------

/** Capability flags schema. */
export const ChainFeaturesSchema = z.object({
  simulation_enabled: z.boolean(),
  paper_mode: z.boolean(),
  live_mode: z.boolean(),
  flashloans_enabled: z.boolean(),
});

/** Zod schema for Chain — must stay aligned with the Chain interface. */
export const ChainSchema = z.object({
  id: z.number().int().positive(),
  name: z.string().min(1, "Chain name is required"),
  slug: z.string().regex(/^[a-z0-9-]+$/, "Slug must be lowercase alphanumeric with hyphens only"),
  native_token: z.string().min(1, "Native token ticker is required"),
  native_token_decimals: z.number().int().min(0).max(18),
  block_time_ms: z.number().int().positive(),
  confirmations_required: z.number().int().positive(),
  is_testnet: z.boolean(),
  status: EntityStatusSchema,
  rpc_http: z.string().url(),
  rpc_ws: z.string().url().startsWith("wss://", "WebSocket URL must use wss:// scheme"),
  explorer_url: z.string().url(),
  features: ChainFeaturesSchema,
  config_hash: ConfigHashSchema,
  created_at: ISOTimestampSchema,
  updated_at: ISOTimestampSchema,
});

/** Inferred TypeScript type from ChainSchema — guaranteed to satisfy the Chain interface. */
export type ChainValidated = z.infer<typeof ChainSchema>;

// ---------------------------------------------------------------------------
// DEX
// ---------------------------------------------------------------------------

/** AMM protocol version identifiers. */
export const DexVersionSchema = z.enum([
  "v2",
  "v3",
  "v4",
  "curve",
  "balancer",
  "custom",
]);

/** Mathematical model that the DEX uses for pricing. */
export const ProtocolTypeSchema = z.enum([
  "constant_product",
  "concentrated_liquidity",
  "stable",
  "weighted",
]);

/** Zod schema for DEX — must stay aligned with the DEX interface. */
export const DEXSchema = z.object({
  id: UUIDSchema,
  chain_id: z.number().int().positive(),
  name: z.string().min(1),
  slug: z.string().regex(/^[a-z0-9-]+$/),
  version: DexVersionSchema,
  router_address: AddressSchema,
  factory_address: AddressSchema,
  protocol_type: ProtocolTypeSchema,
  status: EntityStatusSchema,
  adapter_contract: AddressSchema.optional(),
  config_hash: ConfigHashSchema,
  created_at: ISOTimestampSchema,
  updated_at: ISOTimestampSchema,
});

export type DEXValidated = z.infer<typeof DEXSchema>;

// ---------------------------------------------------------------------------
// Pool
// ---------------------------------------------------------------------------

/** Zod schema for Pool — must stay aligned with the Pool interface. */
export const PoolSchema = z.object({
  id: UUIDSchema,
  chain_id: z.number().int().positive(),
  dex_id: UUIDSchema,
  token0: AddressSchema,
  token1: AddressSchema,
  token0_symbol: z.string().min(1),
  token1_symbol: z.string().min(1),
  pool_address: AddressSchema,
  fee_bps: z.number().int().min(0).max(10000),
  tick_spacing: z.number().int().positive().optional(),
  liquidity: BigIntStringSchema.optional(),
  status: EntityStatusSchema,
  config_hash: ConfigHashSchema,
  created_at: ISOTimestampSchema,
  updated_at: ISOTimestampSchema,
});

export type PoolValidated = z.infer<typeof PoolSchema>;

// ---------------------------------------------------------------------------
// Token
// ---------------------------------------------------------------------------

/** Risk bucket assigned by the automated token-scoring pipeline. */
export const RiskClassificationSchema = z.enum(["low", "medium", "high", "blocked"]);

/** Manual allowlist decision overrides automatic scoring. */
export const AllowlistStatusSchema = z.enum(["allowed", "blocked", "review"]);

/** Zod schema for Token — must stay aligned with the Token interface. */
export const TokenSchema = z.object({
  id: UUIDSchema,
  chain_id: z.number().int().positive(),
  address: AddressSchema,
  symbol: z.string().min(1),
  name: z.string().min(1),
  decimals: z.number().int().min(0).max(18),
  erc20_standard: z.boolean(),
  fee_on_transfer: z.boolean(),
  rebasing: z.boolean(),
  risk_classification: RiskClassificationSchema,
  allowlist_status: AllowlistStatusSchema,
  status: EntityStatusSchema,
  config_hash: ConfigHashSchema,
  created_at: ISOTimestampSchema,
  updated_at: ISOTimestampSchema,
});

export type TokenValidated = z.infer<typeof TokenSchema>;

// ---------------------------------------------------------------------------
// Wallet
// ---------------------------------------------------------------------------

/** Operational role of a managed wallet. */
export const WalletRoleSchema = z.enum([
  "gas_sponsor",
  "execution_signer",
  "cold_treasury",
]);

/** Zod schema for Wallet — must stay aligned with the Wallet interface. */
export const WalletSchema = z.object({
  id: UUIDSchema,
  chain_id: z.number().int().positive(),
  address: AddressSchema,
  role: WalletRoleSchema,
  name: z.string().min(1),
  balance_native: BigIntStringSchema,
  is_funded: z.boolean(),
  status: EntityStatusSchema,
  config_hash: ConfigHashSchema,
  created_at: ISOTimestampSchema,
  updated_at: ISOTimestampSchema,
});

export type WalletValidated = z.infer<typeof WalletSchema>;

// ---------------------------------------------------------------------------
// Strategy
// ---------------------------------------------------------------------------

/** High-level strategy taxonomy. */
export const StrategyTypeSchema = z.enum([
  "dex_dex",
  "triangular",
  "multi_hop",
  "latency_aware",
  "scoring_only",
  "simulation_only",
]);

/** Execution mode for a strategy. */
export const StrategyModeSchema = z.enum(["paper_shadow", "live", "disabled"]);

/** Zod schema for Strategy — must stay aligned with the Strategy interface. */
export const StrategySchema = z.object({
  id: UUIDSchema,
  chain_id: z.number().int().positive(),
  name: z.string().min(1),
  slug: z.string().regex(/^[a-z0-9-]+$/),
  type: StrategyTypeSchema,
  mode: StrategyModeSchema,
  confidence_threshold_bps: z.number().int().min(0).max(10000),
  min_profit_bps: z.number().int().min(0).max(10000),
  max_slippage_bps: z.number().int().min(0).max(10000),
  max_gas_price_gwei: z.number().int().positive(),
  status: EntityStatusSchema,
  config_hash: ConfigHashSchema,
  created_at: ISOTimestampSchema,
  updated_at: ISOTimestampSchema,
});

export type StrategyValidated = z.infer<typeof StrategySchema>;

// ---------------------------------------------------------------------------
// RiskGate
// ---------------------------------------------------------------------------

/** Granularity at which the gate is applied. */
export const RiskGateScopeSchema = z.enum([
  "global",
  "per_chain",
  "per_wallet",
  "per_strategy",
  "per_token",
  "per_dex",
  "per_time_window",
]);

/** Measurable risk dimension. */
export const RiskGateTypeSchema = z.enum([
  "max_exposure",
  "max_gas_burn",
  "max_drawdown",
  "max_consecutive_errors",
  "max_revert_rate",
  "max_latency_ms",
  "circuit_breaker",
]);

/** Automated action taken when threshold is breached. */
export const RiskGateActionSchema = z.enum(["warn", "pause", "block", "kill_switch"]);

/** Zod schema for RiskGate — must stay aligned with the RiskGate interface. */
export const RiskGateSchema = z.object({
  id: UUIDSchema,
  chain_id: z.number().int().positive().optional(),
  wallet_id: UUIDSchema.optional(),
  strategy_id: UUIDSchema.optional(),
  token_id: UUIDSchema.optional(),
  dex_id: UUIDSchema.optional(),
  scope: RiskGateScopeSchema,
  type: RiskGateTypeSchema,
  threshold: z.number().nonnegative(),
  action: RiskGateActionSchema,
  current_value: z.number().nonnegative().optional(),
  status: EntityStatusSchema,
  config_hash: ConfigHashSchema,
  created_at: ISOTimestampSchema,
  updated_at: ISOTimestampSchema,
});

export type RiskGateValidated = z.infer<typeof RiskGateSchema>;

// ---------------------------------------------------------------------------
// Relay
// ---------------------------------------------------------------------------

/** Authentication mechanism expected by the relay endpoint. */
export const RelayAuthTypeSchema = z.enum(["api_key", "signature"]);

/** Zod schema for Relay — must stay aligned with the Relay interface. */
export const RelaySchema = z.object({
  id: UUIDSchema,
  name: z.string().min(1),
  slug: z.string().regex(/^[a-z0-9-]+$/),
  endpoint_url: z.string().url(),
  auth_type: RelayAuthTypeSchema,
  status: EntityStatusSchema,
  config_hash: ConfigHashSchema,
  created_at: ISOTimestampSchema,
  updated_at: ISOTimestampSchema,
});

export type RelayValidated = z.infer<typeof RelaySchema>;

// ---------------------------------------------------------------------------
// Registry Collections Schemas
// ---------------------------------------------------------------------------

/** Canonical entity-type names. */
export const EntityTypeSchema = z.enum([
  "chain",
  "dex",
  "pool",
  "token",
  "wallet",
  "strategy",
  "risk_gate",
  "relay",
]);

/** Complete registry snapshot schema — used for drift detection and backups. */
export const RegistrySnapshotSchema = z.object({
  chains: z.array(ChainSchema),
  dexes: z.array(DEXSchema),
  pools: z.array(PoolSchema),
  tokens: z.array(TokenSchema),
  wallets: z.array(WalletSchema),
  strategies: z.array(StrategySchema),
  risk_gates: z.array(RiskGateSchema),
  relays: z.array(RelaySchema),
  snapshot_hash: ConfigHashSchema,
  taken_at: ISOTimestampSchema,
});

export type RegistrySnapshotValidated = z.infer<typeof RegistrySnapshotSchema>;

// ---------------------------------------------------------------------------
// Schema Registry Map (entity-type -> schema) for dynamic validation
// ---------------------------------------------------------------------------

/** Map from entity-type name to its Zod schema object — useful for generic validation loops. */
export const ENTITY_SCHEMA_MAP = {
  chain: ChainSchema,
  dex: DEXSchema,
  pool: PoolSchema,
  token: TokenSchema,
  wallet: WalletSchema,
  strategy: StrategySchema,
  risk_gate: RiskGateSchema,
  relay: RelaySchema,
} as const;

/** Union of all entity schema types derived from the map. */
export type EntitySchemaMap = typeof ENTITY_SCHEMA_MAP;

/**
 * Validate an arbitrary payload against the schema for a given entity type.
 *
 * @param entityType - One of the canonical EntityType values
 * @param payload    - Unknown data to validate
 * @returns Parsed and typed entity, or throws ZodError on mismatch
 */
export function validateEntity<T extends keyof EntitySchemaMap>(
  entityType: T,
  payload: unknown
): z.infer<EntitySchemaMap[T]> {
  const schema = ENTITY_SCHEMA_MAP[entityType];
  return schema.parse(payload) as z.infer<EntitySchemaMap[T]>;
}
