/**
 * =============================================================================
 * OMEGA Entity Registry Canonical Types
 * =============================================================================
 *
 * Toda entidad operacional vive en un registry, nunca dispersa en componentes.
 * Frontend, backend y runtime comparten estos tipos canonicos.
 *
 * Ghost Protocol: ACTIVO  |  R8 Fail-Honest: PRESERVADO
 * Zero-any policy. Timestamps ISO8601. Addresses checksummed. Config hashes sha256.
 *
 * @module registries/types
 */

// ---------------------------------------------------------------------------
// Primitive Shared Types
// ---------------------------------------------------------------------------

/** Lifecycle status applicable to every operational entity in the system. */
export type EntityStatus =
  | "active"
  | "paused"
  | "blocked"
  | "pending_validation"
  | "deprecated";

/** Ethereum address: checksummed, 42 characters, 0x-prefixed. */
export type Address = `0x${string}`;

/** SHA-256 digest in lowercase hex (64 characters). */
export type ConfigHash = string;

/** ISO-8601 timestamp with timezone (e.g. 2024-01-15T09:30:00.000Z). */
export type ISOTimestamp = string;

/** Bigint serialized as decimal string (avoids JSON precision loss). */
export type BigIntString = string;

/** UUID v4 identifier. */
export type UUID = string;

// ---------------------------------------------------------------------------
// Chain
// ---------------------------------------------------------------------------

/** Capability flags that control execution behaviour on a per-chain basis. */
export interface ChainFeatures {
  /** Whether transaction simulation is available on this chain. */
  simulation_enabled: boolean;
  /** Paper-shadow (dry-run) mode is permitted. */
  paper_mode: boolean;
  /** Live execution with real capital is permitted. */
  live_mode: boolean;
  /** Flash-loan liquidity exists and contracts are deployed. */
  flashloans_enabled: boolean;
}

/**
 * A blockchain network supported by the ArbitrageX-V2 OMEGA engine.
 *
 * No chain_id hardcoded — every chain is an explicit registry entry.
 */
export interface Chain {
  /** Numeric chainId as returned by eth_chainId (e.g. 1, 42161, 8453). */
  id: number;
  /** Human-readable name (e.g. "Ethereum", "Arbitrum One"). */
  name: string;
  /** URL-safe slug used in routes and cache keys (e.g. "arbitrum"). */
  slug: string;
  /** Ticker of the native gas token (e.g. "ETH", "MATIC"). */
  native_token: string;
  /** Decimal places of the native token (almost always 18). */
  native_token_decimals: number;
  /** Average block time in milliseconds (e.g. 12000 for Ethereum L1). */
  block_time_ms: number;
  /** Number of confirmations required before an action is considered final. */
  confirmations_required: number;
  /** True if this is a testnet / devnet entry. */
  is_testnet: boolean;
  /** Current lifecycle status. */
  status: EntityStatus;
  /** HTTP RPC endpoint (may include API-key placeholders). */
  rpc_http: string;
  /** WebSocket RPC endpoint. */
  rpc_ws: string;
  /** Block-explorer base URL. */
  explorer_url: string;
  /** Capability flags. */
  features: ChainFeatures;
  /** SHA-256 hash of the canonical JSON representation of this record. */
  config_hash: ConfigHash;
  /** Record creation timestamp. */
  created_at: ISOTimestamp;
  /** Last mutation timestamp. */
  updated_at: ISOTimestamp;
}

// ---------------------------------------------------------------------------
// DEX
// ---------------------------------------------------------------------------

/** AMM protocol version identifiers. */
export type DexVersion = "v2" | "v3" | "v4" | "curve" | "balancer" | "custom";

/** Mathematical model that the DEX uses for pricing. */
export type ProtocolType =
  | "constant_product"
  | "concentrated_liquidity"
  | "stable"
  | "weighted";

/**
 * A decentralised exchange / AMM protocol deployed on a specific chain.
 *
 * The adapter_contract field points to the ArbitrageX adapter that knows how
 * to encode swaps for this DEX.
 */
export interface DEX {
  /** UUID primary key. */
  id: UUID;
  /** Chain on which this DEX is deployed. */
  chain_id: number;
  /** Human-readable name (e.g. "Uniswap V3", "Curve"). */
  name: string;
  /** URL-safe slug. */
  slug: string;
  /** Protocol version. */
  version: DexVersion;
  /** Checksummed router / entry-point contract address. */
  router_address: Address;
  /** Checksummed factory contract address. */
  factory_address: Address;
  /** Pricing model employed by the protocol. */
  protocol_type: ProtocolType;
  /** Lifecycle status. */
  status: EntityStatus;
  /** On-chain ArbitrageX adapter deployed for this DEX (undefined if not yet deployed). */
  adapter_contract?: Address;
  /** Integrity hash of the canonical record. */
  config_hash: ConfigHash;
  created_at: ISOTimestamp;
  updated_at: ISOTimestamp;
}

// ---------------------------------------------------------------------------
// Pool
// ---------------------------------------------------------------------------

/**
 * A liquidity pool belonging to a specific DEX on a specific chain.
 *
 * tick_spacing and liquidity are optional because they only make sense for
 * concentrated-liquidity AMMs (Uniswap V3-style).
 */
export interface Pool {
  /** UUID primary key. */
  id: UUID;
  /** Parent chain. */
  chain_id: number;
  /** Parent DEX (foreign key to DEX registry). */
  dex_id: UUID;
  /** Checksummed address of token0. */
  token0: Address;
  /** Checksummed address of token1. */
  token1: Address;
  /** Symbol of token0 (denormalised for quick display). */
  token0_symbol: string;
  /** Symbol of token1. */
  token1_symbol: string;
  /** Checksummed pool contract address. */
  pool_address: Address;
  /** Fee tier in basis points (e.g. 30 = 0.30 %). */
  fee_bps: number;
  /** Tick spacing for concentrated-liquidity pools (e.g. 60, 200). */
  tick_spacing?: number;
  /** Current liquidity as a bigint string (undefined for non-CL pools). */
  liquidity?: BigIntString;
  /** Lifecycle status. */
  status: EntityStatus;
  /** Integrity hash. */
  config_hash: ConfigHash;
  created_at: ISOTimestamp;
  updated_at: ISOTimestamp;
}

// ---------------------------------------------------------------------------
// Token
// ---------------------------------------------------------------------------

/** Risk bucket assigned by the automated token-scoring pipeline. */
export type RiskClassification = "low" | "medium" | "high" | "blocked";

/** Manual allowlist decision overrides automatic scoring. */
export type AllowlistStatus = "allowed" | "blocked" | "review";

/**
 * An ERC-20 token that the engine may encounter during route discovery.
 *
 * fee_on_transfer and rebasing flags are kill-switches: when true the token
 * is excluded from any arbitrage path because it breaks atomic-swap invariants.
 */
export interface Token {
  /** UUID primary key. */
  id: UUID;
  /** Chain where the token contract is deployed. */
  chain_id: number;
  /** Checksummed ERC-20 contract address. */
  address: Address;
  /** Token ticker (e.g. "WETH", "USDC"). */
  symbol: string;
  /** Full descriptive name (e.g. "Wrapped Ether"). */
  name: string;
  /** ERC-20 decimals field (e.g. 6 for USDC, 18 for DAI). */
  decimals: number;
  /** Whether the contract conforms to ERC-20 (checked by bytecode fingerprint). */
  erc20_standard: boolean;
  /** True if the contract deducts a fee on every transfer (tax token). */
  fee_on_transfer: boolean;
  /** True if the balance can change externally (rebase / stETH-style). */
  rebasing: boolean;
  /** Automatic risk classification result. */
  risk_classification: RiskClassification;
  /** Manual governance decision. */
  allowlist_status: AllowlistStatus;
  /** Lifecycle status. */
  status: EntityStatus;
  /** Integrity hash. */
  config_hash: ConfigHash;
  created_at: ISOTimestamp;
  updated_at: ISOTimestamp;
}

// ---------------------------------------------------------------------------
// Wallet
// ---------------------------------------------------------------------------

/** Operational role of a managed wallet. */
export type WalletRole = "gas_sponsor" | "execution_signer" | "cold_treasury";

/**
 * An externally-owned account (EOA) or smart wallet managed by the system.
 *
 * execution_signer wallets are expected to hold zero native balance because
 * gas is sponsored by a gas_sponsor via ERC-4337 paymasters or meta-transactions.
 */
export interface Wallet {
  /** UUID primary key. */
  id: UUID;
  /** Chain this wallet operates on. */
  chain_id: number;
  /** Checksummed address. */
  address: Address;
  /** Operational role within the architecture. */
  role: WalletRole;
  /** Human-readable label (e.g. "Gas Sponsor #1", "Cold Treasury"). */
  name: string;
  /** Native-token balance in wei (string to avoid JSON precision loss). */
  balance_native: BigIntString;
  /** True if the wallet holds sufficient balance for its role. */
  is_funded: boolean;
  /** Lifecycle status. */
  status: EntityStatus;
  /** Integrity hash. */
  config_hash: ConfigHash;
  created_at: ISOTimestamp;
  updated_at: ISOTimestamp;
}

// ---------------------------------------------------------------------------
// Strategy
// ---------------------------------------------------------------------------

/** High-level strategy taxonomy. */
export type StrategyType =
  | "dex_dex"
  | "triangular"
  | "multi_hop"
  | "latency_aware"
  | "scoring_only"
  | "simulation_only";

/** Execution mode for a strategy. */
export type StrategyMode = "paper_shadow" | "live" | "disabled";

/**
 * A trading strategy configuration entry.
 *
 * Strategies are chain-scoped.  A single logical strategy (e.g. "dex-dex")
 * may have one registry row per chain so that parameters can be tuned
 * independently.
 */
export interface Strategy {
  /** UUID primary key. */
  id: UUID;
  /** Chain this strategy instance targets. */
  chain_id: number;
  /** Human-readable name. */
  name: string;
  /** URL-safe slug. */
  slug: string;
  /** Strategy taxonomy. */
  type: StrategyType;
  /** Current execution mode. */
  mode: StrategyMode;
  /** Minimum confidence score (basis points) required to submit a trade. */
  confidence_threshold_bps: number;
  /** Minimum expected profit in basis points before a route is considered viable. */
  min_profit_bps: number;
  /** Maximum acceptable slippage in basis points. */
  max_slippage_bps: number;
  /** Maximum gas price in gwei the strategy is willing to pay. */
  max_gas_price_gwei: number;
  /** Lifecycle status. */
  status: EntityStatus;
  /** Integrity hash. */
  config_hash: ConfigHash;
  created_at: ISOTimestamp;
  updated_at: ISOTimestamp;
}

// ---------------------------------------------------------------------------
// RiskGate
// ---------------------------------------------------------------------------

/** Granularity at which the gate is applied. */
export type RiskGateScope =
  | "global"
  | "per_chain"
  | "per_wallet"
  | "per_strategy"
  | "per_token"
  | "per_dex"
  | "per_time_window";

/** Measurable risk dimension. */
export type RiskGateType =
  | "max_exposure"
  | "max_gas_burn"
  | "max_drawdown"
  | "max_consecutive_errors"
  | "max_revert_rate"
  | "max_latency_ms"
  | "circuit_breaker";

/** Automated action taken when threshold is breached. */
export type RiskGateAction = "warn" | "pause" | "block" | "kill_switch";

/**
 * A configurable safety fence that monitors a single risk dimension.
 *
 * Risk gates are evaluated continuously by the Guardian runtime component.
 * When current_value crosses threshold the configured action is executed
 * automatically without human intervention.
 */
export interface RiskGate {
  /** UUID primary key. */
  id: UUID;
  /** Target chain (undefined = global / cross-chain gate). */
  chain_id?: number;
  /** Target wallet (undefined = not wallet-scoped). */
  wallet_id?: UUID;
  /** Target strategy (undefined = not strategy-scoped). */
  strategy_id?: UUID;
  /** Target token (undefined = not token-scoped). */
  token_id?: UUID;
  /** Target DEX (undefined = not DEX-scoped). */
  dex_id?: UUID;
  /** Granularity scope. */
  scope: RiskGateScope;
  /** Risk dimension being monitored. */
  type: RiskGateType;
  /** Numeric threshold that triggers the action. */
  threshold: number;
  /** Action to execute on breach. */
  action: RiskGateAction;
  /** Last known measured value (undefined until first evaluation). */
  current_value?: number;
  /** Lifecycle status. */
  status: EntityStatus;
  /** Integrity hash. */
  config_hash: ConfigHash;
  created_at: ISOTimestamp;
  updated_at: ISOTimestamp;
}

// ---------------------------------------------------------------------------
// Relay
// ---------------------------------------------------------------------------

/** Authentication mechanism expected by the relay endpoint. */
export type RelayAuthType = "api_key" | "signature";

/**
 * A private-transaction relay (MEV-Share, Flashbots Protect, BloXroute, etc).
 *
 * Relays are NOT chain-scoped; a single relay may accept bundles for multiple
 * chains differentiated by endpoint path or headers.
 */
export interface Relay {
  /** UUID primary key. */
  id: UUID;
  /** Human-readable name (e.g. "Flashbots Fast", "BloXroute BDN"). */
  name: string;
  /** URL-safe slug. */
  slug: string;
  /** HTTPS endpoint base URL. */
  endpoint_url: string;
  /** How the engine authenticates with the relay. */
  auth_type: RelayAuthType;
  /** Lifecycle status. */
  status: EntityStatus;
  /** Integrity hash. */
  config_hash: ConfigHash;
  created_at: ISOTimestamp;
  updated_at: ISOTimestamp;
}

// ---------------------------------------------------------------------------
// Registry Collections (convenience types for bulk operations)
// ---------------------------------------------------------------------------

/** Canonical name of every entity type that lives in the registry. */
export type EntityType =
  | "chain"
  | "dex"
  | "pool"
  | "token"
  | "wallet"
  | "strategy"
  | "risk_gate"
  | "relay";

/** Complete snapshot of the entire registry (used for drift detection and backups). */
export interface RegistrySnapshot {
  chains: Chain[];
  dexes: DEX[];
  pools: Pool[];
  tokens: Token[];
  wallets: Wallet[];
  strategies: Strategy[];
  risk_gates: RiskGate[];
  relays: Relay[];
  snapshot_hash: ConfigHash;
  taken_at: ISOTimestamp;
}

/** Union of all registry entity interfaces (useful for generic handlers). */
export type RegistryEntity =
  | Chain
  | DEX
  | Pool
  | Token
  | Wallet
  | Strategy
  | RiskGate
  | Relay;
