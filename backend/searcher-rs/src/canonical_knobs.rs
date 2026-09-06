//! Canonical knobs — the 53 live-configuration surface: the ULTRA workbook's
//! 42 (sheet `01_CONFIG`, "CONFIGURACIÓN VIVA — knobs que cambian el SET de
//! rutas" — XLS-CANON-01) + 11 from QUOTEBASE-264 `01_CONFIG` (XLS-QB-03/05b/
//! 06/07 + 06_EDGE_MATH — min_net_bps, beam_k, quote_w_×5, discovery_sla_ms,
//! dirty_reeval_enabled, max_state_age_blocks, fe_prefilter_enabled).
//!
//! ## Authority & precedence (anti-regression §37)
//! Every knob resolves as **explicit operator env > deploy YAML (where an
//! equivalent exists) > workbook default below**. Today's deployed behavior is
//! therefore preserved: the workbook defaults are the CANONICAL surface (what
//! the Excel declares), never a silent hot-path override.
//!
//! ```text
//!   ARBX_KNOB_MAX_HOPS=7          (operator, explicit — wins)
//!   route_applicability.yaml      (deploy config — next)
//!   workbook default (below)      (canonical floor)
//! ```
//!
//! ## Mode-invariance (§34)
//! `execution_mode` / `selected_execution_mode` / `killswitch` are DECLARATIVE
//! and observability-only here. The actual mode authority is
//! `relays-client::live_exec_policy` (§34.3, default-deny) and the existing
//! kill-switch system. These fields NEVER gate or flip execution semantics;
//! `validate()` only checks that their values are canonical tokens.
//!
//! ## Units (19_SOURCE_FIELD_MAP discipline)
//! - `min_pool_liquidity_usd` is **USD at the route bottleneck** (05_RUTAS
//!   `Bottleneck_Liquidity_USD`), NOT the graph's normalized
//!   `min_liquidity_hint` (different unit — they are separate knobs).
//! - `max_pool_utilization_pct` is a fraction of TVL (0.2 = 20%).
//! - `*_bps` are basis points; `*_pct` are fractions in [0, 1].
use serde_json::json;

/// Canonical execution-mode tokens (§34.1 — LIVE_MAINNET is canonical).
pub const EXEC_MODES: [&str; 3] = ["LIVE_MAINNET", "TESTNET", "PAPER_SHADOW"];

/// Canonical financing-mode tokens (02_FINANCING — first-class modes).
pub const FINANCING_MODES: [&str; 4] = ["OWN_CAPITAL", "AAVE_FL", "BALANCER_FL", "V2_FLASH_SWAP"];

/// The 53 canonical knobs, field names exactly matching the workbook tokens
/// (snake_case), defaults exactly the `01_CONFIG` values: 42 from the ULTRA
/// workbook + 11 from QUOTEBASE-264's `01_CONFIG` (`min_net_bps`, `beam_k`,
/// the five `quote_w_*` weights, `discovery_sla_ms`,
/// `dirty_reeval_enabled`, `max_state_age_blocks`, and
/// `fe_prefilter_enabled` — XLS-QB-03/05b/06/07 + ARBX-0027/0024).
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalKnobs {
    // ── Discovery ────────────────────────────────────────────────────────
    pub max_hops: u8, // 7  (hops)
    pub min_hops: u8, // 2  (hops)
    // ── Financing (selected mode for KPIs/ranking; 02_FINANCING) ────────
    pub selected_financing: String, // OWN_CAPITAL
    // ── Graph pruning / sizing / filters (USD or unit per field) ─────────
    pub min_pool_liquidity_usd: f64, // 150_000 (USD, route bottleneck)
    pub max_pool_utilization_pct: f64, // 0.2    (% TVL, fraction)
    pub min_gross_edge_bps: f64,     // 12     (bps)
    pub max_gas_usd: f64,            // 120    (USD)
    pub max_freshness_s: u64,        // 15     (seconds — ULTRA 01_CONFIG r13)
    pub max_state_age_blocks: u64, // 2 (blocks — QUOTEBASE-264 01_CONFIG r14, gates.max_state_age_blocks)
    pub min_size_usd: f64,         // 10_000 (USD)
    // ── Ranking ──────────────────────────────────────────────────────────
    pub min_ev_usd: f64,       // 25     (USD, net of all costs)
    pub risk_haircut_pct: f64, // 0.1    (fraction)
    pub slippage_factor: f64,  // 0.06   (proxy; exact sim is truth)
    // ── Dynamic N-engine (QUOTEBASE-264 01_CONFIG, workbook #5 — XLS-QB-03).
    // Declared + validated here; consumption lands at their true layers
    // (same declarative-only precedent as `execution_mode`).
    pub min_net_bps: f64, // 5 (bps — Min_Net_bps: gate mínimo beneficio neto;
    //                       evaluation-layer net gate, amount-aware exact net
    //                       stays the truth — G-ECON doctrine, this is the floor)
    pub beam_k: u16, // 4 (branches/node — Beam_K: top-K outgoing branches kept
    //                  per expansion; DECLARED-ONLY: 0 hot-path consumers —
    //                  the dirty-pair scoped re-eval (XLS-QB-05b) scopes
    //                  WHICH routes re-price, not how many branches the DFS
    //                  expands. The effective expansion bound today is
    //                  max_pools_per_pair (unique_route_finder) + max_depth
    //                  + max_routes_per_tick; wiring beam_k into the DFS
    //                  would change the route-set (route-discovery is
    //                  §37 Level-1 frozen) and needs its own gated PR)
    pub dirty_reeval_enabled: bool, // false (Dirty_ReEval_Enabled — gates the
    //                                  scoped re-evaluation seeded by the
    //                                  dirty-pair engine; OFF = the drain
    //                                  stays observe-only with Dirty_Seeds
    //                                  telemetry, deployed behavior
    //                                  identical — XLS-QB-05b, ARBX-QB-05-009)
    pub fe_prefilter_enabled: bool, // false (06_EDGE_MATH REQ-QB-008 — gates
    //                                 the F_e cycle prefilter; OFF = no route
    //                                 is filtered and no fe_prefilter_* KPI is
    //                                 emitted, deployed behavior identical.
    //                                 The prefilter is a SIGNAL (señal≠prueba):
    //                                 the exact net gate — amount-aware +
    //                                 fees + gas + financing + tip + risk +
    //                                 sim — stays the only PASS authority)
    // ── QuoteScore weights (QUOTEBASE-264 01_CONFIG rows 15–19 — XLS-QB-06).
    // Sum MUST be 1.0 (validate); consumed by quote_score::quote_score.
    pub quote_w_prior: f64,          // 0.3 (Quote_w_Prior)
    pub quote_w_liquidity: f64,      // 0.3 (Quote_w_Liquidity)
    pub quote_w_venue_coverage: f64, // 0.2 (Quote_w_VenueCoverage)
    pub quote_w_stability: f64,      // 0.1 (Quote_w_Stability)
    pub quote_w_cross_dex: f64,      // 0.1 (Quote_w_CrossDex)
    // ── Discovery latency budget (QUOTEBASE-264 01_CONFIG r20 — XLS-QB-07).
    // Target p95 for discovery/ranking PRE-simulation (remote RPC and
    // simulation are explicitly OUT of this budget — 00_MANUAL r13).
    // Consumed by latency_budget (PASS_p95 gate, 10_LATENCY r18).
    pub discovery_sla_ms: f64, // 30 (Discovery_SLA_ms)
    // ── Runtime budgets ──────────────────────────────────────────────────
    pub emission_budget_routes_block: usize,  // 50_000
    pub candidate_budget_routes_block: usize, // 250_000
    pub block_cadence_s: u64,                 // 12
    pub cpu_op_budget_block: u64,             // 200_000_000
    pub estimated_cycles: u64,                // 100_000
    // ── Control (declarative only — see module docs §34) ────────────────
    pub execution_mode: String, // PAPER_SHADOW
    // ── Route kinds ──────────────────────────────────────────────────────
    pub enable_2v2: bool,        // true
    pub enable_v2v3: bool,       // true
    pub enable_triangular: bool, // true
    pub enable_nhop: bool,       // true
    // ── Algorithms (04_ALGORITMOS toggles) ───────────────────────────────
    pub enable_bfm: bool,         // true
    pub enable_mmbf: bool,        // true
    pub enable_johnson: bool,     // false (output-explosive)
    pub enable_bounded_dfs: bool, // true
    pub enable_rich: bool,        // true
    pub enable_convex_size: bool, // true
    // ── 264-strategy integration (11_STRATEGY_CATALOG / 15_STRAT_ROUTE_OPT)
    pub selected_strategy_id: String,    // MEV-01-001
    pub selected_execution_mode: String, // PAPER_SHADOW
    pub min_strategy_fit_pct: f64,       // 0.65
    pub min_operator_coverage_pct: f64,  // 0.5
    pub enable_observe_only: bool,       // false (never makes executable)
    pub strict_surface_match: bool,      // true
    pub strict_dependency_match: bool,   // true
    pub route_capacity: usize,           // 72
    // ── Operator weights (12_OPERATOR_CONTROL) ───────────────────────────
    pub primary_operator_weight: f64,   // 1.0
    pub secondary_operator_weight: f64, // 0.35
    // ── Rank composition (sum = 1.0) ─────────────────────────────────────
    pub rank_economic_weight: f64,     // 0.45
    pub rank_strategy_fit_weight: f64, // 0.35
    pub rank_operator_weight: f64,     // 0.20
    // ── Kill-switch (orthogonal control; declarative here) ───────────────
    pub killswitch: bool, // false (OFF)
}

impl Default for CanonicalKnobs {
    fn default() -> Self {
        Self {
            max_hops: 7,
            min_hops: 2,
            selected_financing: "OWN_CAPITAL".to_string(),
            min_pool_liquidity_usd: 150_000.0,
            max_pool_utilization_pct: 0.2,
            min_gross_edge_bps: 12.0,
            max_gas_usd: 120.0,
            max_freshness_s: 15,
            max_state_age_blocks: 2, // r14 Freshness (ARBX-0027)
            min_size_usd: 10_000.0,
            min_ev_usd: 25.0,
            risk_haircut_pct: 0.1,
            slippage_factor: 0.06,
            min_net_bps: 5.0,
            beam_k: 4,
            dirty_reeval_enabled: false,
            fe_prefilter_enabled: false,
            quote_w_prior: 0.3,
            quote_w_liquidity: 0.3,
            quote_w_venue_coverage: 0.2,
            quote_w_stability: 0.1,
            quote_w_cross_dex: 0.1,
            discovery_sla_ms: 30.0,
            emission_budget_routes_block: 50_000,
            candidate_budget_routes_block: 250_000,
            block_cadence_s: 12,
            cpu_op_budget_block: 200_000_000,
            estimated_cycles: 100_000,
            execution_mode: "PAPER_SHADOW".to_string(),
            enable_2v2: true,
            enable_v2v3: true,
            enable_triangular: true,
            enable_nhop: true,
            enable_bfm: true,
            enable_mmbf: true,
            enable_johnson: false,
            enable_bounded_dfs: true,
            enable_rich: true,
            enable_convex_size: true,
            selected_strategy_id: "MEV-01-001".to_string(),
            selected_execution_mode: "PAPER_SHADOW".to_string(),
            min_strategy_fit_pct: 0.65,
            min_operator_coverage_pct: 0.5,
            enable_observe_only: false,
            strict_surface_match: true,
            strict_dependency_match: true,
            route_capacity: 72,
            primary_operator_weight: 1.0,
            secondary_operator_weight: 0.35,
            rank_economic_weight: 0.45,
            rank_strategy_fit_weight: 0.35,
            rank_operator_weight: 0.20,
            killswitch: false,
        }
    }
}

fn env_str(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn env_bool(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

fn env_f64(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite())
        .unwrap_or(default)
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(default)
}

impl CanonicalKnobs {
    /// Explicit operator env (`ARBX_KNOB_*`) layered over the workbook
    /// defaults. Invalid numeric values fall back to the default (the boot
    /// `validate()` pass still fails loudly on invariant violations).
    pub fn from_env() -> Self {
        let d = Self::default();
        Self {
            max_hops: env_u64("ARBX_KNOB_MAX_HOPS", d.max_hops as u64) as u8,
            min_hops: env_u64("ARBX_KNOB_MIN_HOPS", d.min_hops as u64) as u8,
            selected_financing: env_str("ARBX_KNOB_SELECTED_FINANCING", &d.selected_financing),
            min_pool_liquidity_usd: env_f64(
                "ARBX_KNOB_MIN_POOL_LIQUIDITY_USD",
                d.min_pool_liquidity_usd,
            ),
            max_pool_utilization_pct: env_f64(
                "ARBX_KNOB_MAX_POOL_UTILIZATION_PCT",
                d.max_pool_utilization_pct,
            ),
            min_gross_edge_bps: env_f64("ARBX_KNOB_MIN_GROSS_EDGE_BPS", d.min_gross_edge_bps),
            max_gas_usd: env_f64("ARBX_KNOB_MAX_GAS_USD", d.max_gas_usd),
            max_freshness_s: env_u64("ARBX_KNOB_MAX_FRESHNESS_S", d.max_freshness_s),
            max_state_age_blocks: env_u64("ARBX_KNOB_MAX_STATE_AGE_BLOCKS", d.max_state_age_blocks),
            min_size_usd: env_f64("ARBX_KNOB_MIN_SIZE_USD", d.min_size_usd),
            min_ev_usd: env_f64("ARBX_KNOB_MIN_EV_USD", d.min_ev_usd),
            risk_haircut_pct: env_f64("ARBX_KNOB_RISK_HAIRCUT_PCT", d.risk_haircut_pct),
            slippage_factor: env_f64("ARBX_KNOB_SLIPPAGE_FACTOR", d.slippage_factor),
            min_net_bps: env_f64("ARBX_KNOB_MIN_NET_BPS", d.min_net_bps),
            beam_k: env_u64("ARBX_KNOB_BEAM_K", d.beam_k as u64) as u16,
            dirty_reeval_enabled: env_bool(
                "ARBX_KNOB_DIRTY_REEVAL_ENABLED",
                d.dirty_reeval_enabled,
            ),
            fe_prefilter_enabled: env_bool(
                "ARBX_KNOB_FE_PREFILTER_ENABLED",
                d.fe_prefilter_enabled,
            ),
            quote_w_prior: env_f64("ARBX_KNOB_QUOTE_W_PRIOR", d.quote_w_prior),
            quote_w_liquidity: env_f64("ARBX_KNOB_QUOTE_W_LIQUIDITY", d.quote_w_liquidity),
            quote_w_venue_coverage: env_f64(
                "ARBX_KNOB_QUOTE_W_VENUE_COVERAGE",
                d.quote_w_venue_coverage,
            ),
            quote_w_stability: env_f64("ARBX_KNOB_QUOTE_W_STABILITY", d.quote_w_stability),
            quote_w_cross_dex: env_f64("ARBX_KNOB_QUOTE_W_CROSS_DEX", d.quote_w_cross_dex),
            discovery_sla_ms: env_f64("ARBX_KNOB_DISCOVERY_SLA_MS", d.discovery_sla_ms),
            emission_budget_routes_block: env_u64(
                "ARBX_KNOB_EMISSION_BUDGET_ROUTES_BLOCK",
                d.emission_budget_routes_block as u64,
            ) as usize,
            candidate_budget_routes_block: env_u64(
                "ARBX_KNOB_CANDIDATE_BUDGET_ROUTES_BLOCK",
                d.candidate_budget_routes_block as u64,
            ) as usize,
            block_cadence_s: env_u64("ARBX_KNOB_BLOCK_CADENCE_S", d.block_cadence_s),
            cpu_op_budget_block: env_u64("ARBX_KNOB_CPU_OP_BUDGET_BLOCK", d.cpu_op_budget_block),
            estimated_cycles: env_u64("ARBX_KNOB_ESTIMATED_CYCLES", d.estimated_cycles),
            execution_mode: env_str("ARBX_KNOB_EXECUTION_MODE", &d.execution_mode),
            enable_2v2: env_bool("ARBX_KNOB_ENABLE_2V2", d.enable_2v2),
            enable_v2v3: env_bool("ARBX_KNOB_ENABLE_V2V3", d.enable_v2v3),
            enable_triangular: env_bool("ARBX_KNOB_ENABLE_TRIANGULAR", d.enable_triangular),
            enable_nhop: env_bool("ARBX_KNOB_ENABLE_NHOP", d.enable_nhop),
            enable_bfm: env_bool("ARBX_KNOB_ENABLE_BFM", d.enable_bfm),
            enable_mmbf: env_bool("ARBX_KNOB_ENABLE_MMBF", d.enable_mmbf),
            enable_johnson: env_bool("ARBX_KNOB_ENABLE_JOHNSON", d.enable_johnson),
            enable_bounded_dfs: env_bool("ARBX_KNOB_ENABLE_BOUNDED_DFS", d.enable_bounded_dfs),
            enable_rich: env_bool("ARBX_KNOB_ENABLE_RICH", d.enable_rich),
            enable_convex_size: env_bool("ARBX_KNOB_ENABLE_CONVEX_SIZE", d.enable_convex_size),
            selected_strategy_id: env_str(
                "ARBX_KNOB_SELECTED_STRATEGY_ID",
                &d.selected_strategy_id,
            ),
            selected_execution_mode: env_str(
                "ARBX_KNOB_SELECTED_EXECUTION_MODE",
                &d.selected_execution_mode,
            ),
            min_strategy_fit_pct: env_f64("ARBX_KNOB_MIN_STRATEGY_FIT_PCT", d.min_strategy_fit_pct),
            min_operator_coverage_pct: env_f64(
                "ARBX_KNOB_MIN_OPERATOR_COVERAGE_PCT",
                d.min_operator_coverage_pct,
            ),
            enable_observe_only: env_bool("ARBX_KNOB_ENABLE_OBSERVE_ONLY", d.enable_observe_only),
            strict_surface_match: env_bool(
                "ARBX_KNOB_STRICT_SURFACE_MATCH",
                d.strict_surface_match,
            ),
            strict_dependency_match: env_bool(
                "ARBX_KNOB_STRICT_DEPENDENCY_MATCH",
                d.strict_dependency_match,
            ),
            route_capacity: env_u64("ARBX_KNOB_ROUTE_CAPACITY", d.route_capacity as u64) as usize,
            primary_operator_weight: env_f64(
                "ARBX_KNOB_PRIMARY_OPERATOR_WEIGHT",
                d.primary_operator_weight,
            ),
            secondary_operator_weight: env_f64(
                "ARBX_KNOB_SECONDARY_OPERATOR_WEIGHT",
                d.secondary_operator_weight,
            ),
            rank_economic_weight: env_f64("ARBX_KNOB_RANK_ECONOMIC_WEIGHT", d.rank_economic_weight),
            rank_strategy_fit_weight: env_f64(
                "ARBX_KNOB_RANK_STRATEGY_FIT_WEIGHT",
                d.rank_strategy_fit_weight,
            ),
            rank_operator_weight: env_f64("ARBX_KNOB_RANK_OPERATOR_WEIGHT", d.rank_operator_weight),
            killswitch: env_bool("ARBX_KNOB_KILLSWITCH", d.killswitch),
        }
    }

    /// Structural + economic invariants (boot fails loudly when violated —
    /// config validation is defensive, not speculative).
    pub fn validate(&self) -> Result<(), String> {
        if !(2..=7).contains(&self.max_hops) {
            return Err(format!(
                "max_hops {} outside canonical 2..=7",
                self.max_hops
            ));
        }
        if self.min_hops < 2 || self.min_hops > self.max_hops {
            return Err(format!(
                "min_hops {} must be in 2..=max_hops({})",
                self.min_hops, self.max_hops
            ));
        }
        if !FINANCING_MODES.contains(&self.selected_financing.as_str()) {
            return Err(format!(
                "selected_financing {} not canonical (one of {:?})",
                self.selected_financing, FINANCING_MODES
            ));
        }
        if !(0.0..=1.0).contains(&self.max_pool_utilization_pct)
            || self.max_pool_utilization_pct <= 0.0
        {
            return Err("max_pool_utilization_pct must be in (0, 1]".to_string());
        }
        if self.min_gross_edge_bps < 0.0 {
            return Err("min_gross_edge_bps cannot be negative".to_string());
        }
        if self.max_gas_usd < 0.0 || self.min_size_usd < 0.0 || self.min_ev_usd < 0.0 {
            return Err("max_gas_usd / min_size_usd / min_ev_usd cannot be negative".to_string());
        }
        if self.min_pool_liquidity_usd < 0.0 {
            return Err("min_pool_liquidity_usd cannot be negative".to_string());
        }
        if !(0.0..=0.5).contains(&self.risk_haircut_pct) {
            return Err("risk_haircut_pct must be in [0, 0.5]".to_string());
        }
        if self.slippage_factor < 0.0 {
            return Err("slippage_factor cannot be negative".to_string());
        }
        // QUOTEBASE-264 01_CONFIG (XLS-QB-03): Min_Net_bps ≥ 0 finite; Beam_K
        // in 1..=256 (the per-hop-tier DirtySeed beam bounds run up to 256).
        if !self.min_net_bps.is_finite() || self.min_net_bps < 0.0 {
            return Err("min_net_bps must be finite and >= 0".to_string());
        }
        if !(1..=256).contains(&self.beam_k) {
            return Err(format!("beam_k {} outside 1..=256", self.beam_k));
        }
        // QUOTEBASE-264 01_CONFIG rows 15–19 (XLS-QB-06): each weight finite
        // in [0,1] and the five sum to 1.0.
        let qw = [
            self.quote_w_prior,
            self.quote_w_liquidity,
            self.quote_w_venue_coverage,
            self.quote_w_stability,
            self.quote_w_cross_dex,
        ];
        if qw.iter().any(|w| !w.is_finite() || *w < 0.0 || *w > 1.0) {
            return Err("quote weights must each be finite in [0, 1]".to_string());
        }
        let qw_sum: f64 = qw.iter().sum();
        if (qw_sum - 1.0).abs() > 1e-9 {
            return Err(format!(
                "quote weights must sum to 1.0 (got {qw_sum:.6} — 01_CONFIG rows 15–19)"
            ));
        }
        // QUOTEBASE-264 01_CONFIG r20 (XLS-QB-07): the discovery SLA must be
        // a finite positive duration (a zero/negative budget is meaningless).
        if !self.discovery_sla_ms.is_finite() || self.discovery_sla_ms <= 0.0 {
            return Err("discovery_sla_ms must be finite and > 0".to_string());
        }
        if self.emission_budget_routes_block == 0 || self.candidate_budget_routes_block == 0 {
            return Err("route budgets must be > 0".to_string());
        }
        if self.emission_budget_routes_block > self.candidate_budget_routes_block {
            return Err("emission budget cannot exceed candidate budget".to_string());
        }
        if self.block_cadence_s == 0 || self.cpu_op_budget_block == 0 {
            return Err("block_cadence_s / cpu_op_budget_block must be > 0".to_string());
        }
        // QUOTEBASE-264 01_CONFIG r14 (ARBX-0027): the block freshness budget
        // must be a sane positive bound — 0 would stale-out every observation,
        // an absurd value would disable the canonical unit entirely.
        if !(1..=1024).contains(&self.max_state_age_blocks) {
            return Err(format!(
                "max_state_age_blocks {} outside 1..=1024",
                self.max_state_age_blocks
            ));
        }
        if !EXEC_MODES.contains(&self.execution_mode.as_str()) {
            return Err(format!(
                "execution_mode {} not canonical (one of {:?})",
                self.execution_mode, EXEC_MODES
            ));
        }
        if !EXEC_MODES.contains(&self.selected_execution_mode.as_str()) {
            return Err(format!(
                "selected_execution_mode {} not canonical (one of {:?})",
                self.selected_execution_mode, EXEC_MODES
            ));
        }
        if !self.selected_strategy_id.starts_with("MEV-") {
            return Err(format!(
                "selected_strategy_id {} not a canonical MEV-XX-XXX id",
                self.selected_strategy_id
            ));
        }
        if !(0.0..=1.0).contains(&self.min_strategy_fit_pct)
            || !(0.0..=1.0).contains(&self.min_operator_coverage_pct)
        {
            return Err(
                "strategy fit / operator coverage thresholds must be in [0, 1]".to_string(),
            );
        }
        if self.route_capacity == 0 {
            return Err("route_capacity must be > 0".to_string());
        }
        if self.primary_operator_weight < 0.0 || self.secondary_operator_weight < 0.0 {
            return Err("operator weights cannot be negative".to_string());
        }
        if self.secondary_operator_weight > self.primary_operator_weight {
            return Err("secondary operator weight cannot exceed primary".to_string());
        }
        let w_sum =
            self.rank_economic_weight + self.rank_strategy_fit_weight + self.rank_operator_weight;
        if (w_sum - 1.0).abs() > 1e-9 {
            return Err(format!(
                "rank weights must sum to 1.0 (got {w_sum:.6}: {}/{}/{} — 01_CONFIG 'Sum=1')",
                self.rank_economic_weight, self.rank_strategy_fit_weight, self.rank_operator_weight
            ));
        }
        Ok(())
    }

    /// Serializable snapshot (boot log, Redis `arbx:config:canonical_knobs`,
    /// `GET /api/v1/config/canonical-knobs`). Values only — never secrets.
    ///
    /// Built with explicit `Map` inserts, NOT one `json!({...})` literal: a
    /// single 52-key `json!` macro expansion exceeds the crate's default
    /// `recursion_limit` (rust-check CI failure) — the incremental build stays
    /// under it without a crate-wide attribute.
    pub fn to_json(&self) -> serde_json::Value {
        let mut m = serde_json::Map::with_capacity(54);
        m.insert("max_hops".into(), json!(self.max_hops));
        m.insert("min_hops".into(), json!(self.min_hops));
        m.insert("selected_financing".into(), json!(self.selected_financing));
        m.insert(
            "min_pool_liquidity_usd".into(),
            json!(self.min_pool_liquidity_usd),
        );
        m.insert(
            "max_pool_utilization_pct".into(),
            json!(self.max_pool_utilization_pct),
        );
        m.insert("min_gross_edge_bps".into(), json!(self.min_gross_edge_bps));
        m.insert("max_gas_usd".into(), json!(self.max_gas_usd));
        m.insert("max_freshness_s".into(), json!(self.max_freshness_s));
        m.insert(
            "max_state_age_blocks".into(),
            json!(self.max_state_age_blocks),
        );
        m.insert("min_size_usd".into(), json!(self.min_size_usd));
        m.insert("min_ev_usd".into(), json!(self.min_ev_usd));
        m.insert("risk_haircut_pct".into(), json!(self.risk_haircut_pct));
        m.insert("slippage_factor".into(), json!(self.slippage_factor));
        m.insert("min_net_bps".into(), json!(self.min_net_bps));
        m.insert("beam_k".into(), json!(self.beam_k));
        m.insert(
            "dirty_reeval_enabled".into(),
            json!(self.dirty_reeval_enabled),
        );
        m.insert(
            "fe_prefilter_enabled".into(),
            json!(self.fe_prefilter_enabled),
        );
        m.insert("quote_w_prior".into(), json!(self.quote_w_prior));
        m.insert("quote_w_liquidity".into(), json!(self.quote_w_liquidity));
        m.insert(
            "quote_w_venue_coverage".into(),
            json!(self.quote_w_venue_coverage),
        );
        m.insert("quote_w_stability".into(), json!(self.quote_w_stability));
        m.insert("quote_w_cross_dex".into(), json!(self.quote_w_cross_dex));
        m.insert("discovery_sla_ms".into(), json!(self.discovery_sla_ms));
        m.insert(
            "emission_budget_routes_block".into(),
            json!(self.emission_budget_routes_block),
        );
        m.insert(
            "candidate_budget_routes_block".into(),
            json!(self.candidate_budget_routes_block),
        );
        m.insert("block_cadence_s".into(), json!(self.block_cadence_s));
        m.insert(
            "cpu_op_budget_block".into(),
            json!(self.cpu_op_budget_block),
        );
        m.insert("estimated_cycles".into(), json!(self.estimated_cycles));
        m.insert("execution_mode".into(), json!(self.execution_mode));
        m.insert("enable_2v2".into(), json!(self.enable_2v2));
        m.insert("enable_v2v3".into(), json!(self.enable_v2v3));
        m.insert("enable_triangular".into(), json!(self.enable_triangular));
        m.insert("enable_nhop".into(), json!(self.enable_nhop));
        m.insert("enable_bfm".into(), json!(self.enable_bfm));
        m.insert("enable_mmbf".into(), json!(self.enable_mmbf));
        m.insert("enable_johnson".into(), json!(self.enable_johnson));
        m.insert("enable_bounded_dfs".into(), json!(self.enable_bounded_dfs));
        m.insert("enable_rich".into(), json!(self.enable_rich));
        m.insert("enable_convex_size".into(), json!(self.enable_convex_size));
        m.insert(
            "selected_strategy_id".into(),
            json!(self.selected_strategy_id),
        );
        m.insert(
            "selected_execution_mode".into(),
            json!(self.selected_execution_mode),
        );
        m.insert(
            "min_strategy_fit_pct".into(),
            json!(self.min_strategy_fit_pct),
        );
        m.insert(
            "min_operator_coverage_pct".into(),
            json!(self.min_operator_coverage_pct),
        );
        m.insert(
            "enable_observe_only".into(),
            json!(self.enable_observe_only),
        );
        m.insert(
            "strict_surface_match".into(),
            json!(self.strict_surface_match),
        );
        m.insert(
            "strict_dependency_match".into(),
            json!(self.strict_dependency_match),
        );
        m.insert("route_capacity".into(), json!(self.route_capacity));
        m.insert(
            "primary_operator_weight".into(),
            json!(self.primary_operator_weight),
        );
        m.insert(
            "secondary_operator_weight".into(),
            json!(self.secondary_operator_weight),
        );
        m.insert(
            "rank_economic_weight".into(),
            json!(self.rank_economic_weight),
        );
        m.insert(
            "rank_strategy_fit_weight".into(),
            json!(self.rank_strategy_fit_weight),
        );
        m.insert(
            "rank_operator_weight".into(),
            json!(self.rank_operator_weight),
        );
        m.insert("killswitch".into(), json!(self.killswitch));
        m.insert(
            "source".into(),
            json!("canonical_knobs.rs (01_CONFIG ULTRA workbook)"),
        );
        serde_json::Value::Object(m)
    }
}

// ── ARBX-0027: freshness unification s↔blocks (REQ-QB-013) ────────────
//
// The workbook carries TWO freshness budgets for the same concept: the
// ULTRA legacy wall-clock budget `max_freshness_s` = 15 s (r13) and the
// QUOTEBASE canonical block budget `Max_State_Age_Blocks` = 2 blocks
// (01_CONFIG r14 → `gates.max_state_age_blocks`). This section unifies
// them with ONE explicit conversion, never a hidden equivalence:
//
// * seconds → blocks is a CEILING (`blocks_for_seconds`) — a partial block
//   still ages, so the seconds budget must be covered by whole blocks;
// * blocks → seconds is exact (`seconds_for_blocks`);
// * the conversion factor is ALWAYS a parameter (`block_time_s`) — callers
//   pass the chain's observed cadence (the `block_cadence_s` knob is the
//   workbook-declared 12 s default, but no chain constant is hardcoded
//   here: no per-chain assumption lives in this gate);
// * both budgets stay enforced — the effective bound in each unit is the
//   STRICTER of the two expressed in that unit, so tightening either knob
//   can only tighten the gate, never loosen it.

/// Whole blocks needed to span a seconds budget: `ceil(seconds / block_time)`.
/// A degenerate `block_time_s == 0` saturates to `u64::MAX` (a zero-cadence
/// chain imposes no block bound) — visible, never a silent zero bound.
pub fn blocks_for_seconds(seconds: u64, block_time_s: u64) -> u64 {
    if block_time_s == 0 {
        return u64::MAX;
    }
    seconds / block_time_s + u64::from(seconds % block_time_s != 0)
}

/// Exact seconds spanned by a blocks budget (saturating product).
pub fn seconds_for_blocks(blocks: u64, block_time_s: u64) -> u64 {
    blocks.saturating_mul(block_time_s)
}

/// The unified freshness budget (ARBX-0027): one cutoff, two workbook units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FreshnessBudget {
    /// QUOTEBASE canonical bound (01_CONFIG r14).
    pub max_state_age_blocks: u64,
    /// Legacy wall-clock bound (ULTRA 01_CONFIG r13).
    pub max_freshness_s: u64,
}

impl FreshnessBudget {
    pub fn from_knobs(k: &CanonicalKnobs) -> Self {
        Self {
            max_state_age_blocks: k.max_state_age_blocks,
            max_freshness_s: k.max_freshness_s,
        }
    }

    /// Effective BLOCK bound = min(canonical blocks, legacy seconds → blocks).
    pub fn effective_max_blocks(&self, block_time_s: u64) -> u64 {
        self.max_state_age_blocks
            .min(blocks_for_seconds(self.max_freshness_s, block_time_s))
    }

    /// Effective SECOND bound = min(legacy seconds, canonical blocks → seconds).
    pub fn effective_max_seconds(&self, block_time_s: u64) -> u64 {
        self.max_freshness_s
            .min(seconds_for_blocks(self.max_state_age_blocks, block_time_s))
    }
}

/// Observable verdict for a state/price observation (sheet 03 models
/// staleness as `Age_Blocks = Head_Block − Last_Block`). The canonical
/// block unit is checked first; a verdict may feed pricing/sizing only
/// when `Fresh` (see [`FreshnessVerdict::is_usable`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreshnessVerdict {
    /// Within both effective bounds. `age_blocks` carries the observed lag
    /// when the block pair was observable.
    Fresh { age_blocks: Option<u64> },
    /// Older than the effective block bound (canonical unit tripped).
    StaleBlocks { age_blocks: u64, max_blocks: u64 },
    /// Within the block bound but older than the effective second bound.
    StaleSeconds { age_s: u64, max_s: u64 },
    /// No observable dimension (neither a block pair nor a timestamp):
    /// fail-closed — unusable, never silently fresh (R8).
    UnknownAge,
}

impl FreshnessVerdict {
    /// Only `Fresh` may feed pricing/sizing decisions.
    pub fn is_usable(&self) -> bool {
        matches!(self, Self::Fresh { .. })
    }
}

/// Stale-state price gate (ARBX-0027, "stale-state price cubierto"):
/// a price/state observation is fresh iff its block lag is within the
/// effective block bound AND its wall-clock age is within the effective
/// second bound. Missing observability fails closed; each stale verdict
/// carries the exact (observed, bound) pair so rejection reasons stay
/// R8-observable downstream.
pub fn price_freshness(
    last_block: Option<u64>,
    head_block: Option<u64>,
    last_update_s: Option<u64>,
    now_s: u64,
    budget: &FreshnessBudget,
    block_time_s: u64,
) -> FreshnessVerdict {
    let mut age_blocks: Option<u64> = None;
    if let (Some(last), Some(head)) = (last_block, head_block) {
        let age = head.saturating_sub(last);
        let max = budget.effective_max_blocks(block_time_s);
        if age > max {
            return FreshnessVerdict::StaleBlocks {
                age_blocks: age,
                max_blocks: max,
            };
        }
        age_blocks = Some(age);
    }
    if let Some(ts) = last_update_s {
        let age = now_s.saturating_sub(ts);
        let max = budget.effective_max_seconds(block_time_s);
        if age > max {
            return FreshnessVerdict::StaleSeconds {
                age_s: age,
                max_s: max,
            };
        }
    }
    if age_blocks.is_none() && last_update_s.is_none() {
        return FreshnessVerdict::UnknownAge;
    }
    FreshnessVerdict::Fresh { age_blocks }
}

#[cfg(test)]
// Tests deliberately build single-field variants off `Default` then mutate —
// the clearest way to pin each knob's validation boundary.
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    /// The 42 defaults are EXACTLY the workbook `01_CONFIG` table — the
    /// canonical surface must never drift from the Excel (XLS-CANON-01).
    #[test]
    fn defaults_match_workbook_01_config_exactly() {
        let k = CanonicalKnobs::default();
        assert_eq!(k.max_hops, 7);
        assert_eq!(k.min_hops, 2);
        assert_eq!(k.selected_financing, "OWN_CAPITAL");
        assert_eq!(k.min_pool_liquidity_usd, 150_000.0);
        assert_eq!(k.max_pool_utilization_pct, 0.2);
        assert_eq!(k.min_gross_edge_bps, 12.0);
        assert_eq!(k.max_gas_usd, 120.0);
        assert_eq!(k.max_freshness_s, 15);
        assert_eq!(k.max_state_age_blocks, 2); // r14 (ARBX-0027)
        assert_eq!(k.min_size_usd, 10_000.0);
        assert_eq!(k.min_ev_usd, 25.0);
        assert_eq!(k.risk_haircut_pct, 0.1);
        assert_eq!(k.slippage_factor, 0.06);
        assert_eq!(k.min_net_bps, 5.0); // QUOTEBASE-264 01_CONFIG Min_Net_bps
        assert_eq!(k.beam_k, 4); // QUOTEBASE-264 01_CONFIG Beam_K
        assert_eq!(k.quote_w_prior, 0.3); // rows 15–19 (XLS-QB-06)
        assert_eq!(k.quote_w_liquidity, 0.3);
        assert_eq!(k.quote_w_venue_coverage, 0.2);
        assert_eq!(k.quote_w_stability, 0.1);
        assert_eq!(k.quote_w_cross_dex, 0.1);
        assert_eq!(k.discovery_sla_ms, 30.0); // r20 (XLS-QB-07)
        assert_eq!(k.emission_budget_routes_block, 50_000);
        assert_eq!(k.candidate_budget_routes_block, 250_000);
        assert_eq!(k.block_cadence_s, 12);
        assert_eq!(k.cpu_op_budget_block, 200_000_000);
        assert_eq!(k.estimated_cycles, 100_000);
        assert_eq!(k.execution_mode, "PAPER_SHADOW");
        assert!(k.enable_2v2);
        assert!(k.enable_v2v3);
        assert!(k.enable_triangular);
        assert!(k.enable_nhop);
        assert!(k.enable_bfm);
        assert!(k.enable_mmbf);
        assert!(!k.enable_johnson);
        assert!(k.enable_bounded_dfs);
        assert!(k.enable_rich);
        assert!(k.enable_convex_size);
        assert_eq!(k.selected_strategy_id, "MEV-01-001");
        assert_eq!(k.selected_execution_mode, "PAPER_SHADOW");
        assert_eq!(k.min_strategy_fit_pct, 0.65);
        assert_eq!(k.min_operator_coverage_pct, 0.5);
        assert!(!k.enable_observe_only);
        assert!(k.strict_surface_match);
        assert!(k.strict_dependency_match);
        assert_eq!(k.route_capacity, 72);
        assert_eq!(k.primary_operator_weight, 1.0);
        assert_eq!(k.secondary_operator_weight, 0.35);
        assert_eq!(k.rank_economic_weight, 0.45);
        assert_eq!(k.rank_strategy_fit_weight, 0.35);
        assert_eq!(k.rank_operator_weight, 0.20);
        assert!(!k.killswitch);
        assert!(k.validate().is_ok(), "workbook defaults must validate");
    }

    #[test]
    fn validate_rejects_invariant_violations() {
        let mut k = CanonicalKnobs::default();
        k.max_hops = 8;
        assert!(k.validate().is_err(), "max_hops outside 2..=7");

        let mut k = CanonicalKnobs::default();
        k.min_hops = 3;
        k.max_hops = 2;
        assert!(k.validate().is_err(), "min_hops > max_hops");

        let mut k = CanonicalKnobs::default();
        k.selected_financing = "MARGIN".into();
        assert!(k.validate().is_err(), "non-canonical financing mode");

        let mut k = CanonicalKnobs::default();
        k.max_pool_utilization_pct = 1.5;
        assert!(k.validate().is_err(), "utilization > 1");

        let mut k = CanonicalKnobs::default();
        k.rank_operator_weight = 0.5;
        assert!(k.validate().is_err(), "rank weights must sum 1.0");

        let mut k = CanonicalKnobs::default();
        k.emission_budget_routes_block = 300_000;
        k.candidate_budget_routes_block = 250_000;
        assert!(k.validate().is_err(), "emission > candidate budget");

        let mut k = CanonicalKnobs::default();
        k.execution_mode = "LIVE".into();
        assert!(k.validate().is_err(), "non-canonical exec mode token");

        let mut k = CanonicalKnobs::default();
        k.selected_strategy_id = "triangle-1".into();
        assert!(k.validate().is_err(), "non-canonical strategy id");

        let mut k = CanonicalKnobs::default();
        k.secondary_operator_weight = 2.0;
        assert!(k.validate().is_err(), "secondary weight > primary");

        let mut k = CanonicalKnobs::default();
        k.min_net_bps = -1.0;
        assert!(k.validate().is_err(), "min_net_bps negative");

        let mut k = CanonicalKnobs::default();
        k.beam_k = 0;
        assert!(k.validate().is_err(), "beam_k < 1");
        k.beam_k = 257;
        assert!(k.validate().is_err(), "beam_k > 256");

        let mut k = CanonicalKnobs::default();
        k.quote_w_prior = 0.5; // sum becomes 1.2
        assert!(k.validate().is_err(), "quote weights must sum 1.0");

        let mut k = CanonicalKnobs::default();
        k.quote_w_stability = -0.1;
        k.quote_w_cross_dex = 0.3; // sum still 1.0 but a weight is negative
        assert!(k.validate().is_err(), "negative quote weight rejected");

        let mut k = CanonicalKnobs::default();
        k.discovery_sla_ms = 0.0;
        assert!(k.validate().is_err(), "discovery_sla_ms must be > 0");
    }

    /// Env overrides win over defaults (single test fn — `set_var` is
    /// process-global; keep all env cases serial in one fn).
    #[test]
    fn env_overrides_win_over_workbook_defaults() {
        // Numeric + bool + string; cleanup restores the process env.
        let keys = [
            "ARBX_KNOB_MAX_HOPS",
            "ARBX_KNOB_MAX_GAS_USD",
            "ARBX_KNOB_ENABLE_JOHNSON",
            "ARBX_KNOB_SELECTED_FINANCING",
            "ARBX_KNOB_KILLSWITCH",
            "ARBX_KNOB_MIN_NET_BPS",
            "ARBX_KNOB_DIRTY_REEVAL_ENABLED",
            "ARBX_KNOB_FE_PREFILTER_ENABLED",
        ];
        let saved: Vec<Option<String>> = keys.iter().map(|k| std::env::var(k).ok()).collect();
        std::env::set_var("ARBX_KNOB_MAX_HOPS", "5");
        std::env::set_var("ARBX_KNOB_MAX_GAS_USD", "77.5");
        std::env::set_var("ARBX_KNOB_ENABLE_JOHNSON", "true");
        std::env::set_var("ARBX_KNOB_SELECTED_FINANCING", "AAVE_FL");
        std::env::set_var("ARBX_KNOB_KILLSWITCH", "ON");
        std::env::set_var("ARBX_KNOB_MIN_NET_BPS", "8.5");
        std::env::set_var("ARBX_KNOB_DIRTY_REEVAL_ENABLED", "true");
        std::env::set_var("ARBX_KNOB_FE_PREFILTER_ENABLED", "true");
        let k = CanonicalKnobs::from_env();
        assert_eq!(k.max_hops, 5);
        assert_eq!(k.max_gas_usd, 77.5);
        assert!(k.enable_johnson);
        assert_eq!(k.selected_financing, "AAVE_FL");
        assert!(k.killswitch);
        assert_eq!(k.min_net_bps, 8.5);
        assert!(
            k.dirty_reeval_enabled,
            "explicit operator override flips the scoped re-eval gate (XLS-QB-05b)"
        );
        assert!(
            k.fe_prefilter_enabled,
            "explicit operator override flips the F_e prefilter gate (ARBX-0024)"
        );
        assert!(
            k.validate().is_ok(),
            "explicit operator overrides must validate"
        );
        // Invalid numeric falls back to default (validate still guards ranges).
        std::env::set_var("ARBX_KNOB_MAX_HOPS", "not-a-number");
        let k = CanonicalKnobs::from_env();
        assert_eq!(k.max_hops, 7, "invalid env falls back to workbook default");
        for (k, s) in keys.iter().zip(saved.into_iter()) {
            match s {
                Some(v) => std::env::set_var(k, v),
                None => std::env::remove_var(k),
            }
        }
    }

    #[test]
    fn snapshot_json_has_all_53_knobs_and_source() {
        let j = CanonicalKnobs::default().to_json();
        let obj = j.as_object().expect("snapshot is an object");
        // 53 knob fields (ULTRA 01_CONFIG ×42 + QUOTEBASE-264 01_CONFIG ×11,
        // XLS-QB-03/05b/06/07 + ARBX-0027 max_state_age_blocks + ARBX-0024
        // fe_prefilter_enabled) + 1 source field.
        assert_eq!(obj.len(), 54);
        assert_eq!(
            obj["source"],
            "canonical_knobs.rs (01_CONFIG ULTRA workbook)"
        );
        assert_eq!(obj["max_hops"], 7);
        assert_eq!(obj["min_net_bps"], 5.0);
        assert_eq!(obj["beam_k"], 4);
        assert_eq!(
            obj["dirty_reeval_enabled"], false,
            "default OFF — observe-only drain (XLS-QB-05b)"
        );
        assert_eq!(
            obj["fe_prefilter_enabled"], false,
            "default OFF — signal only, net gate stays the proof (ARBX-0024)"
        );
        assert_eq!(obj["quote_w_prior"], 0.3);
        assert_eq!(obj["quote_w_cross_dex"], 0.1);
        assert_eq!(obj["discovery_sla_ms"], 30.0);
        assert_eq!(obj["max_state_age_blocks"], 2, "r14 (ARBX-0027)");
        assert_eq!(obj["killswitch"], false);
    }

    // ── ARBX-0027: freshness s↔blocks unification ────────────────────────

    /// Conversion must be exact where divisible, conservative (ceiling)
    /// where not, and never under-cover the seconds budget.
    #[test]
    fn freshness_conversion_exact_and_conservative() {
        assert_eq!(blocks_for_seconds(0, 12), 0);
        assert_eq!(blocks_for_seconds(24, 12), 2, "exact division");
        assert_eq!(blocks_for_seconds(15, 12), 2, "ceiling: 12 < 15 ≤ 24");
        assert_eq!(blocks_for_seconds(25, 12), 3, "ceiling: 24 < 25 ≤ 36");
        assert_eq!(blocks_for_seconds(13, 1), 13, "1 s cadence is identity");
        assert_eq!(
            blocks_for_seconds(7, 0),
            u64::MAX,
            "degenerate cadence saturates, never a silent zero bound"
        );
        assert_eq!(seconds_for_blocks(2, 12), 24);
        assert_eq!(seconds_for_blocks(0, 12), 0);
        assert_eq!(
            seconds_for_blocks(u64::MAX, 2),
            u64::MAX,
            "saturating product, no overflow panic"
        );
        assert_eq!(seconds_for_blocks(3, 0), 0);
        // Round-trip property: converting to blocks and back never shrinks
        // the seconds budget (the ceiling guarantees coverage).
        for &(s, bt) in &[(15u64, 12u64), (24, 12), (1, 7), (100, 3)] {
            assert!(
                seconds_for_blocks(blocks_for_seconds(s, bt), bt) >= s,
                "budget {}s @ {}s/block under-covers",
                s,
                bt
            );
        }
    }

    /// The effective bound in each unit is the STRICTER of the two budgets
    /// expressed in that unit — tightening either knob only tightens.
    #[test]
    fn freshness_effective_bounds_take_the_stricter_unit() {
        let b = FreshnessBudget {
            max_state_age_blocks: 2,
            max_freshness_s: 15,
        };
        // 15 s spans ceil(15/12)=2 blocks → both bounds agree.
        assert_eq!(b.effective_max_blocks(12), 2);
        assert_eq!(b.effective_max_seconds(12), 15);
        // 1 s cadence: 2 blocks = 2 s only → seconds bound tightens to 2.
        assert_eq!(b.effective_max_blocks(1), 2);
        assert_eq!(b.effective_max_seconds(1), 2);
        // 100 s cadence: 15 s spans 1 block only → block bound tightens to 1.
        assert_eq!(b.effective_max_blocks(100), 1);
        assert_eq!(b.effective_max_seconds(100), 15);
        // from_knobs reads both knob fields.
        let from = FreshnessBudget::from_knobs(&CanonicalKnobs::default());
        assert_eq!(from.max_state_age_blocks, 2);
        assert_eq!(from.max_freshness_s, 15);
    }

    /// Stale prices are rejected with the exact (observed, bound) pair, in
    /// the canonical block unit first, the legacy second unit second.
    #[test]
    fn stale_price_rejected_with_reason() {
        let b = FreshnessBudget {
            max_state_age_blocks: 2,
            max_freshness_s: 15,
        };
        // 3-block lag > 2 (both units would trip; block wins as canonical).
        assert_eq!(
            price_freshness(Some(10), Some(13), Some(1000), 1016, &b, 12),
            FreshnessVerdict::StaleBlocks {
                age_blocks: 3,
                max_blocks: 2
            }
        );
        // 1-block lag is fine, but 20 s old > 15 s trips the second bound.
        assert_eq!(
            price_freshness(Some(10), Some(11), Some(1000), 1020, &b, 12),
            FreshnessVerdict::StaleSeconds {
                age_s: 20,
                max_s: 15
            }
        );
        // Neither stale verdict may feed pricing.
        assert!(!FreshnessVerdict::StaleBlocks {
            age_blocks: 3,
            max_blocks: 2
        }
        .is_usable());
        assert!(!FreshnessVerdict::StaleSeconds {
            age_s: 20,
            max_s: 15
        }
        .is_usable());
    }

    /// A price within both bounds passes with its observed block lag.
    #[test]
    fn fresh_price_passes_both_dimensions() {
        let b = FreshnessBudget::from_knobs(&CanonicalKnobs::default());
        assert_eq!(
            price_freshness(Some(10), Some(11), Some(1000), 1005, &b, 12),
            FreshnessVerdict::Fresh {
                age_blocks: Some(1)
            }
        );
        // Timestamp-only observation: fresh, block lag unknown (not zero).
        assert_eq!(
            price_freshness(None, None, Some(1000), 1005, &b, 12),
            FreshnessVerdict::Fresh { age_blocks: None }
        );
        assert!(price_freshness(None, None, Some(1000), 1005, &b, 12).is_usable());
    }

    /// Missing observability fails closed — unusable, never silently fresh.
    #[test]
    fn missing_freshness_observability_fails_closed() {
        let b = FreshnessBudget::from_knobs(&CanonicalKnobs::default());
        assert_eq!(
            price_freshness(None, None, None, 1000, &b, 12),
            FreshnessVerdict::UnknownAge
        );
        // Half a block pair is no block evidence at all.
        assert_eq!(
            price_freshness(Some(10), None, None, 1000, &b, 12),
            FreshnessVerdict::UnknownAge
        );
        assert!(!price_freshness(None, None, None, 1000, &b, 12).is_usable());
    }

    /// The unified knobs follow the standard env>YAML>workbook precedence.
    #[test]
    fn state_age_blocks_env_override() {
        let key = "ARBX_KNOB_MAX_STATE_AGE_BLOCKS";
        let saved = std::env::var(key).ok();
        std::env::set_var(key, "6");
        let k = CanonicalKnobs::from_env();
        assert_eq!(k.max_state_age_blocks, 6);
        let b = FreshnessBudget::from_knobs(&k);
        // At 1 s cadence the override binds: 6 < the 15 s legacy span (15).
        assert_eq!(b.effective_max_blocks(1), 6);
        // At 12 s cadence the legacy seconds budget is stricter (15 s spans
        // only 2 blocks) — the override reaches the budget field, but the
        // unified gate still takes the stricter unit.
        assert_eq!(b.effective_max_blocks(12), 2);
        match saved {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
    }
}
