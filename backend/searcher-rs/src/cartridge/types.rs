//! Cartridge type definitions — shared across the cartridge subsystem.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Metadata returned by `init_strategy()` in every cartridge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartridgeMetadata {
    /// Human-readable strategy name (e.g. "DEX Arbitrage V2/V3").
    pub name: String,
    /// Semantic version string (e.g. "1.0.0").
    pub version: String,
    /// Author identifier (operator alias or wallet).
    pub author: String,
    /// Brief description of what the strategy does.
    pub description: String,
    /// Target chain IDs this cartridge is designed for (empty = all chains).
    pub target_chains: Vec<u64>,
    /// Strategy category for UI grouping.
    pub category: String,
    /// Minimum interval between evaluations in milliseconds.
    pub min_eval_interval_ms: u64,
}

/// Lifecycle state of a loaded cartridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CartridgeState {
    /// Cartridge compiled and validated, ready for evaluation.
    Active,
    /// Cartridge loaded but paused by operator.
    Paused,
    /// Cartridge failed validation or compilation.
    Failed,
    /// Cartridge is being hot-swapped (transient state).
    Reloading,
}

/// Result of evaluating a cartridge's `evaluate_opportunity` function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartridgeEvalResult {
    /// Whether the cartridge found a viable opportunity.
    pub is_opportunity: bool,
    /// Estimated gross profit in the base token (before gas).
    pub estimated_profit: f64,
    /// Confidence score 0.0 - 1.0.
    pub confidence: f64,
    /// Strategy-specific metadata (route details, pool addresses, etc.).
    pub metadata: HashMap<String, serde_json::Value>,
    /// Suggested execution urgency: "immediate", "next_block", "monitor".
    pub urgency: String,
    /// Explicit reason string the cartridge returned (e.g. "v3_sizing_pending",
    /// "insufficient_pools", "below_gas_threshold"). `None` if the cartridge omitted it.
    /// Observability-only (FASE B Paso 9): lets the durable outcomes series explain WHY
    /// is_opportunity=false (structural ceiling vs real insufficiency). Shadow/telemetry.
    #[serde(default)]
    pub reason: Option<String>,
}

/// Payload assembled by `build_payload` for Web3 execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartridgePayload {
    /// Target contract address for execution.
    pub target_contract: String,
    /// Encoded calldata (hex string).
    pub calldata: String,
    /// Value in wei to send with the transaction.
    pub value_wei: String,
    /// Gas limit suggestion from the cartridge.
    pub gas_limit: u64,
    /// Maximum priority fee per gas (in gwei).
    pub max_priority_fee_gwei: f64,
    /// Deadline timestamp (unix seconds) after which the tx should not execute.
    pub deadline_ts: u64,
    /// Flash loan parameters if the strategy requires borrowed capital.
    pub flash_loan: Option<FlashLoanParams>,
}

/// Flash loan parameters for strategies that require borrowed capital.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashLoanParams {
    /// Protocol to borrow from: "aave_v3", "balancer", "uniswap_v3".
    pub protocol: String,
    /// Token address to borrow.
    pub token: String,
    /// Amount to borrow (decimal string).
    pub amount: String,
}

/// Event published on Redis when a cartridge is injected/updated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartridgeInjectionEvent {
    /// Unique cartridge identifier (UUID).
    pub cartridge_id: String,
    /// Event type: "inject", "update", "remove", "pause", "resume".
    pub event_type: String,
    /// SHA-256 hash of the script content (for dedup).
    pub content_hash: String,
    /// Chain ID scope (0 = all chains).
    pub chain_id: u64,
    /// Timestamp of the event.
    pub timestamp: DateTime<Utc>,
    /// Operator who triggered the injection.
    pub actor: String,
}

/// Internal record for a compiled and validated cartridge.
#[derive(Debug, Clone)]
pub struct CompiledCartridge {
    /// Unique identifier.
    pub id: String,
    /// Compiled Rhai AST (wrapped in Arc for cheap cloning).
    pub ast: std::sync::Arc<rhai::AST>,
    /// Metadata extracted from `init_strategy()`.
    pub metadata: CartridgeMetadata,
    /// Current lifecycle state.
    pub state: CartridgeState,
    /// Content hash for dedup on reload.
    pub content_hash: String,
    /// When this cartridge was last compiled.
    pub compiled_at: DateTime<Utc>,
    /// Number of successful evaluations.
    pub eval_count: u64,
    /// Number of evaluation errors.
    pub error_count: u64,
}
