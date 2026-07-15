//! arbx-sim-engine — local MCP (stdio) server for ArbitrageX v2.
//!
//! Exposes the REAL `searcher-rs` kernels to Claude as MCP tools so route
//! discovery / sizing run as tool calls instead of "write a script". Runs
//! entirely local over stdio (no market data leaves the machine).
//!
//! ## Honesty contract (RULE 00 / Zero-Mocks)
//! Only what actually exists is wired to real code; everything else fails
//! HONESTLY (never fabricates a route or a profit):
//!   - `run_route_discovery`  → REAL  `route_discovery::find_routes` (bounded DFS, 2–3 hops, pure)
//!   - `compute_sizing`       → REAL  `amm_math::v2_amount_out` (V2 CPMM eval at a given amount_in)
//!   - `simulate_bundle`      → HONEST `requires_infra` (REVM fork sim needs an RPC fork; not in this scaffold)
//!   - `run_mmbf_discovery`   → HONEST `not_implemented` (MMBF line-graph = Phase 2, arXiv:2406.16573)
//!   - `compute_mpo_sizing`   → HONEST `not_implemented` (Marginal Price Optimization = Phase 3, arXiv:2502.08258)
//!
//! ## NO-ACTIVE
//! Shadow/read-only. No executor, no wallets, no capital, no broadcast, no
//! Redis/`arbx:opps:detected` writes. Pure in-memory simulation only.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use ethers::types::{Address, H256, U256};
use rmcp::{
    handler::server::{router::tool::ToolRouter, tool::Parameters, wrapper::Json},
    tool, tool_handler, tool_router,
    transport::stdio,
    ServerHandler, ServiceExt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use prioritization_spine::round_trip_executor::RoundTripContext;
use searcher_rs::amm_math::v2_amount_out;
use searcher_rs::route_discovery::graph_builder::TokenGraph;
use searcher_rs::route_discovery::types::{RouteDirection, RouteEdge};
use searcher_rs::route_discovery::unique_route_finder::{find_routes, RouteFinderConfig};
use searcher_rs::route_intent::ProtocolType;
use searcher_rs::sim_orchestrator::{execute_round_trip_revm, RoundTripExecutionConfig};

// ─────────────────────────────────────────────────────────────────────────────
// Shared helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Server-side caps that bound graph construction + route enumeration memory
/// regardless of caller input (DoS guard for a tool that takes arbitrary JSON).
const MAX_INPUT_POOLS: usize = 5_000;
const MAX_ROUTES_CAP: usize = 2_000;
/// Uniswap-V2 fee must be strictly < 100% (10_000 bps): `amm_math::v2_amount_out`
/// computes `10_000u32 - fee_bps`, so `fee_bps >= 10_000` underflows (panic in
/// debug, silent wrap → fabricated profit in release). Reject at the boundary.
const MAX_FEE_BPS_EXCLUSIVE: u32 = 10_000;

fn parse_addr(s: &str) -> Result<Address, String> {
    Address::from_str(s.trim()).map_err(|_| format!("invalid_address: {s}"))
}

fn proto_str(p: ProtocolType) -> String {
    match p {
        ProtocolType::V2 => "v2",
        ProtocolType::V3 => "v3",
        ProtocolType::Curve => "curve",
        ProtocolType::Balancer => "balancer",
        ProtocolType::Unknown => "unknown",
    }
    .to_string()
}

/// Honest structured response for tools that are not (yet) backed by real code.
#[derive(Debug, Serialize, JsonSchema)]
struct StatusOutput {
    /// `not_implemented` | `requires_infra`
    status: String,
    /// machine-readable error code
    error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    phase: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reference: Option<String>,
    detail: String,
}

/// Result of `simulate_bundle` (REAL REVM round-trip via searcher_rs::sim_orchestrator).
#[derive(Debug, Serialize, JsonSchema)]
struct SimOutput {
    engine: String,
    chain_id: u64,
    /// Always `true` — the orchestrator structurally rejects live execution
    /// (`RoundTripExecutionConfig.paper_mode` must be true). NO-ACTIVE by design.
    paper_mode: bool,
    /// `true` only when the simulated executeArbitrage met the min-profit floor.
    passed: bool,
    simulated_profit_token_in_wei: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    intermediate_amount_out_wei: Option<String>,
    gas_used_total: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    fail_reason: Option<String>,
    /// Set when the call could not run (bad input or missing operator infra) —
    /// fail-honest, never a fabricated profit.
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    /// The exact env var that must be set to enable the REVM sim, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    needs_env: Option<String>,
}

/// Build a fail-honest `SimOutput` (no fabricated profit) for input/infra errors.
fn sim_err(chain_id: u64, error: &str, needs_env: Option<&str>, detail: &str) -> Json<SimOutput> {
    Json(SimOutput {
        engine: "execute_round_trip_revm (REAL, paper_mode=true)".to_string(),
        chain_id,
        paper_mode: true,
        passed: false,
        simulated_profit_token_in_wei: "0".to_string(),
        intermediate_amount_out_wei: None,
        gas_used_total: 0,
        fail_reason: Some(detail.to_string()),
        error: Some(error.to_string()),
        needs_env: needs_env.map(|s| s.to_string()),
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Tool 1 — run_route_discovery (REAL: bounded DFS over searcher_rs::find_routes)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct PoolInput {
    /// Pool address, 0x-prefixed.
    pool: String,
    /// token0 address, 0x-prefixed.
    token0: String,
    /// token1 address, 0x-prefixed.
    token1: String,
    /// Protocol: "v2" or "v3".
    protocol: String,
    /// Fee in basis points (e.g. 30 = 0.30%). Optional.
    #[serde(default)]
    fee_bps: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct RouteDiscoveryParams {
    /// EVM chain ID (e.g. 1 = Ethereum mainnet, 137 = Polygon, 42161 = Arbitrum).
    /// Provenance metadata only — the graph is built from the `pools` you provide.
    chain_id: u64,
    /// The pool set to build the token graph from (each pool → two directed edges).
    pools: Vec<PoolInput>,
    /// Max cycle length in hops; clamped to [2, 3] (Phase-1 DFS). Default 3.
    #[serde(default)]
    max_depth: Option<u8>,
    /// Cap on emitted routes (anti-explosion). Default 500.
    #[serde(default)]
    max_routes: Option<usize>,
    /// Restrict cycle start tokens (0x addresses). Empty = every token.
    #[serde(default)]
    base_tokens: Option<Vec<String>>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct RouteOut {
    route_hash: String,
    route_kind: String,
    hops: u8,
    tokens: Vec<String>,
    pools: Vec<String>,
    protocols: Vec<String>,
    directions: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct DiscoveryOutput {
    engine: String,
    chain_id: u64,
    routes_found: usize,
    /// R8 fail-honest: true ⇒ route cap stopped enumeration early (incomplete set).
    capped: bool,
    /// R8 fail-honest: true ⇒ a parallel pool was dropped by the per-pair cap.
    pools_truncated: bool,
    dropped_for_cap: usize,
    routes: Vec<RouteOut>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl DiscoveryOutput {
    fn err(chain_id: u64, msg: String) -> Self {
        Self {
            engine: "dfs_bounded (REAL searcher_rs::route_discovery::find_routes)".to_string(),
            chain_id,
            routes_found: 0,
            capped: false,
            pools_truncated: false,
            dropped_for_cap: 0,
            routes: Vec::new(),
            error: Some(msg),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tool 2 — compute_sizing (REAL: per-leg V2 swap eval via amm_math::v2_amount_out)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct LegReserve {
    /// reserve of the input token of this hop (decimal string, wei).
    reserve_in: String,
    /// reserve of the output token of this hop (decimal string, wei).
    reserve_out: String,
    /// fee bps for this hop. Default 30 (0.30%).
    #[serde(default)]
    fee_bps: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SizingParams {
    /// Ordered V2 hops (reserves per leg).
    legs: Vec<LegReserve>,
    /// amount_in (decimal string, wei).
    amount_in: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct SizingOutput {
    engine: String,
    amount_in: String,
    amount_out: String,
    /// `amount_out - amount_in` when positive, else "0".
    net_profit_wei: String,
    profitable: bool,
    legs: usize,
    note: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Tools 3/4/5 — honest params
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct SimulateBundleParams {
    /// EVM chain ID. The fork RPC is read from env `RPC_HTTP_<chain_id>` and the
    /// deployed ArbitrageExecutor from env `EXECUTOR_<chain_id>` (no-hardcode).
    chain_id: u64,
    /// token_in address (0x); amount_in is denominated in this token's wei.
    token_in: String,
    /// token_out address (0x).
    token_out: String,
    /// amount_in in wei (decimal string, > 0).
    amount_in_wei: String,
    /// Forward-leg router (0x): token_in → token_out.
    forward_router: String,
    /// Forward-leg path: ≥2 token addresses (e.g. [token_in, token_out]).
    forward_path: Vec<String>,
    /// Backward-leg router (0x): token_out → token_in.
    backward_router: String,
    /// Backward-leg path: ≥2 token addresses (e.g. [token_out, token_in]).
    backward_path: Vec<String>,
    /// Gas limit for the simulated executeArbitrage. Default 30_000_000.
    #[serde(default)]
    gas_limit: Option<u64>,
    /// Effective gas price (wei) for net accounting. Default 0 (profit is gross of gas).
    #[serde(default)]
    gas_price_wei: Option<String>,
    /// Optional route_hash (0x + 64 hex) passed through to calldata for byte-stability.
    #[serde(default)]
    route_hash: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct MmbfParams {
    chain_id: u64,
    #[serde(default)]
    max_hops: Option<u8>,
    #[serde(default)]
    source_token: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct MpoParams {
    token_path: Vec<String>,
    pools: Vec<String>,
    input_amount: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Server
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct SimEngine {
    tool_router: ToolRouter<SimEngine>,
}

#[tool_router]
impl SimEngine {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    /// REAL Phase-1 bounded-DFS cyclic route discovery (2–3 hops) over a provided
    /// pool set. Pure in-memory — calls `searcher_rs::route_discovery::find_routes`.
    #[tool(
        name = "run_route_discovery",
        description = "REAL bounded-DFS (2-3 hop) cyclic arbitrage route discovery over a provided pool set. Pure, no infra. Calls searcher_rs::route_discovery::find_routes. NOTE: this is the Phase-1 DFS; for 7-11 hop MMBF use run_mmbf_discovery (not yet implemented)."
    )]
    fn run_route_discovery(
        &self,
        Parameters(p): Parameters<RouteDiscoveryParams>,
    ) -> Json<DiscoveryOutput> {
        // DoS guard: bound graph construction regardless of caller input.
        if p.pools.len() > MAX_INPUT_POOLS {
            return Json(DiscoveryOutput::err(
                p.chain_id,
                format!(
                    "too_many_pools: {} > limit {MAX_INPUT_POOLS}",
                    p.pools.len()
                ),
            ));
        }
        let mut edges: Vec<RouteEdge> = Vec::new();
        for (i, pin) in p.pools.iter().enumerate() {
            let proto = match pin.protocol.to_lowercase().as_str() {
                "v2" => ProtocolType::V2,
                "v3" => ProtocolType::V3,
                other => {
                    return Json(DiscoveryOutput::err(
                        p.chain_id,
                        format!("pool[{i}]: unsupported protocol '{other}' (only v2/v3)"),
                    ))
                }
            };
            let (pool, t0, t1) = match (
                parse_addr(&pin.pool),
                parse_addr(&pin.token0),
                parse_addr(&pin.token1),
            ) {
                (Ok(a), Ok(b), Ok(c)) => (a, b, c),
                _ => {
                    return Json(DiscoveryOutput::err(
                        p.chain_id,
                        format!("pool[{i}]: invalid pool/token0/token1 address"),
                    ))
                }
            };
            // Two directed edges per pool (token0→token1 and token1→token0), mirroring
            // the graph_builder's directed-edge construction.
            for (ti, to) in [(t0, t1), (t1, t0)] {
                edges.push(RouteEdge {
                    chain_id: p.chain_id,
                    pool,
                    token_in: ti,
                    token_out: to,
                    token0: t0,
                    token1: t1,
                    protocol: proto,
                    fee_bps: pin.fee_bps,
                    // RULE 00: the MCP has no real reserves, so the honest value is
                    // `None` (graph_builder's contract: None = not computed). find_routes
                    // is topology-only and never reads this; never fabricate a magnitude.
                    liquidity_hint: None,
                    log_weight: None,
                    freshness_ts: 0,
                    blk: 0,
                    direction: RouteDirection::from_in_token0(ti, t0),
                });
            }
        }

        let mut adjacency: HashMap<Address, Vec<usize>> = HashMap::new();
        for (i, e) in edges.iter().enumerate() {
            adjacency.entry(e.token_in).or_default().push(i);
        }
        let graph = TokenGraph { edges, adjacency };

        let base_tokens: Vec<Address> = p
            .base_tokens
            .unwrap_or_default()
            .iter()
            .filter_map(|s| parse_addr(s).ok())
            .collect();

        let cfg = RouteFinderConfig {
            max_depth: p.max_depth.unwrap_or(3).clamp(2, 3),
            max_pools_per_pair: 8,
            // DoS guard: cap total emitted routes so a caller can't lift the cap to
            // ~infinity and exhaust the heap (seen-set + results vec grow with this).
            max_routes_per_tick: p.max_routes.unwrap_or(500).min(MAX_ROUTES_CAP),
            base_tokens,
            mode: "shadow".to_string(),
        };

        let outcome = find_routes(&graph, p.chain_id, &cfg);
        let routes: Vec<RouteOut> = outcome
            .routes
            .iter()
            .map(|r| RouteOut {
                route_hash: r.route_hash.clone(),
                route_kind: r.route_kind.as_str().to_string(),
                hops: r.hops,
                tokens: r.tokens.iter().map(|a| format!("{a:#x}")).collect(),
                pools: r.pools.iter().map(|a| format!("{a:#x}")).collect(),
                protocols: r.protocols.iter().map(|x| proto_str(*x)).collect(),
                directions: r
                    .directions
                    .iter()
                    .map(|d| d.as_str().to_string())
                    .collect(),
            })
            .collect();

        Json(DiscoveryOutput {
            engine: "dfs_bounded (REAL searcher_rs::route_discovery::find_routes)".to_string(),
            chain_id: p.chain_id,
            routes_found: outcome.routes.len(),
            capped: outcome.capped,
            pools_truncated: outcome.pools_truncated,
            dropped_for_cap: outcome.dropped_for_cap,
            routes,
            error: None,
        })
    }

    /// REAL evaluation of a multi-leg V2 route AT A GIVEN amount_in (not the optimal).
    /// Chains `searcher_rs::amm_math::v2_amount_out` over the legs. Optimal-size
    /// search (golden-section / MPO) is `compute_mpo_sizing` (not yet implemented).
    #[tool(
        name = "compute_sizing",
        description = "REAL evaluation of a multi-leg Uniswap-V2 route at a GIVEN amount_in via searcher_rs::amm_math::v2_amount_out (chained per leg). Returns amount_out + net profit. This is NOT the optimal-size search (that is compute_mpo_sizing, not yet implemented)."
    )]
    fn compute_sizing(&self, Parameters(p): Parameters<SizingParams>) -> Json<SizingOutput> {
        let amount_in = match U256::from_dec_str(p.amount_in.trim()) {
            Ok(v) => v,
            Err(_) => {
                return Json(SizingOutput {
                    engine: "v2_amount_out (REAL)".to_string(),
                    amount_in: p.amount_in.clone(),
                    amount_out: "0".to_string(),
                    net_profit_wei: "0".to_string(),
                    profitable: false,
                    legs: p.legs.len(),
                    note: "amount_in must be a decimal wei string".to_string(),
                    error: Some("invalid_amount_in".to_string()),
                })
            }
        };
        if p.legs.is_empty() {
            return Json(SizingOutput {
                engine: "v2_amount_out (REAL)".to_string(),
                amount_in: amount_in.to_string(),
                amount_out: "0".to_string(),
                net_profit_wei: "0".to_string(),
                profitable: false,
                legs: 0,
                note: "no legs provided".to_string(),
                error: Some("empty_legs".to_string()),
            });
        }

        let mut cur = amount_in;
        for (i, leg) in p.legs.iter().enumerate() {
            let (rin, rout) = match (
                U256::from_dec_str(leg.reserve_in.trim()),
                U256::from_dec_str(leg.reserve_out.trim()),
            ) {
                (Ok(a), Ok(b)) => (a, b),
                _ => {
                    return Json(SizingOutput {
                        engine: "v2_amount_out (REAL)".to_string(),
                        amount_in: amount_in.to_string(),
                        amount_out: "0".to_string(),
                        net_profit_wei: "0".to_string(),
                        profitable: false,
                        legs: p.legs.len(),
                        note: format!("leg[{i}]: reserves must be decimal wei strings"),
                        error: Some("invalid_reserves".to_string()),
                    })
                }
            };
            let fee = leg.fee_bps.unwrap_or(30);
            // HIGH-severity guard (3 reviewers converged): v2_amount_out computes
            // `10_000u32 - fee_bps`; fee_bps >= 10_000 underflows → panic (debug, kills
            // the stdio server) or wrap → fabricated inflated amount_out (release).
            // Fail-honest at the boundary instead.
            if fee >= MAX_FEE_BPS_EXCLUSIVE {
                return Json(SizingOutput {
                    engine: "v2_amount_out (REAL)".to_string(),
                    amount_in: amount_in.to_string(),
                    amount_out: "0".to_string(),
                    net_profit_wei: "0".to_string(),
                    profitable: false,
                    legs: p.legs.len(),
                    note: format!(
                        "leg[{i}]: fee_bps {fee} >= 10000 is invalid (would underflow V2 math)"
                    ),
                    error: Some("invalid_fee_bps".to_string()),
                });
            }
            cur = v2_amount_out(cur, rin, rout, fee);
        }

        let profitable = cur > amount_in;
        let net = if profitable {
            (cur - amount_in).to_string()
        } else {
            "0".to_string()
        };
        Json(SizingOutput {
            engine: "v2_amount_out (REAL searcher_rs::amm_math)".to_string(),
            amount_in: amount_in.to_string(),
            amount_out: cur.to_string(),
            net_profit_wei: net,
            profitable,
            legs: p.legs.len(),
            note: "Evaluation at the given amount_in. Optimal sizing (golden-section / MPO) = compute_mpo_sizing.".to_string(),
            error: None,
        })
    }

    /// REAL in-process REVM round-trip simulation of a 2-leg arbitrage against
    /// the deployed ArbitrageExecutor, via `searcher_rs::sim_orchestrator::execute_round_trip_revm`.
    /// Gated on operator infra (fork RPC + executor address, both from env, no-hardcode);
    /// runs ONLY in `paper_mode=true` (the orchestrator structurally rejects live exec).
    /// Fails honestly (never fabricates a profit) when infra/inputs are missing.
    #[tool(
        name = "simulate_bundle",
        description = "REAL REVM round-trip sim of a 2-leg arbitrage (token_in -> token_out -> token_in) against the deployed ArbitrageExecutor, via searcher_rs::sim_orchestrator::execute_round_trip_revm. Requires env RPC_HTTP_<chain_id> (fork RPC) + EXECUTOR_<chain_id> (executor address); paper_mode-only, never broadcasts. Returns simulated profit/gas or a fail-honest reason."
    )]
    fn simulate_bundle(&self, Parameters(p): Parameters<SimulateBundleParams>) -> Json<SimOutput> {
        // ── operator infra from env (no-hardcode; same convention as searcher-rs) ──
        let rpc_key = format!("RPC_HTTP_{}", p.chain_id);
        let rpc_url = match std::env::var(&rpc_key) {
            Ok(u) if !u.trim().is_empty() => u,
            _ => {
                return sim_err(
                    p.chain_id,
                    "rpc_http_unset",
                    Some(&rpc_key),
                    "REVM round-trip sim needs a fork RPC for real chain state; set this env to enable. Nothing fabricated (RULE 00).",
                )
            }
        };
        let exec_key = format!("EXECUTOR_{}", p.chain_id);
        let executor = match std::env::var(&exec_key).ok().and_then(|s| parse_addr(&s).ok()) {
            Some(a) => a,
            None => {
                return sim_err(
                    p.chain_id,
                    "executor_unset",
                    Some(&exec_key),
                    "REVM round-trip sim simulates a DEPLOYED ArbitrageExecutor; set its address env to enable.",
                )
            }
        };

        // ── parse route inputs ──
        let (token_in, token_out) = match (parse_addr(&p.token_in), parse_addr(&p.token_out)) {
            (Ok(a), Ok(b)) => (a, b),
            _ => {
                return sim_err(
                    p.chain_id,
                    "invalid_token_address",
                    None,
                    "token_in/token_out must be 0x addresses",
                )
            }
        };
        let (fwd_router, bwd_router) = match (
            parse_addr(&p.forward_router),
            parse_addr(&p.backward_router),
        ) {
            (Ok(a), Ok(b)) => (a, b),
            _ => {
                return sim_err(
                    p.chain_id,
                    "invalid_router_address",
                    None,
                    "forward_router/backward_router must be 0x addresses",
                )
            }
        };
        let parse_path = |v: &[String]| -> Option<Vec<Address>> {
            v.iter().map(|s| parse_addr(s).ok()).collect()
        };
        let fwd_path = match parse_path(&p.forward_path) {
            Some(x) if x.len() >= 2 => x,
            _ => {
                return sim_err(
                    p.chain_id,
                    "invalid_forward_path",
                    None,
                    "forward_path needs >=2 valid 0x addresses",
                )
            }
        };
        let bwd_path = match parse_path(&p.backward_path) {
            Some(x) if x.len() >= 2 => x,
            _ => {
                return sim_err(
                    p.chain_id,
                    "invalid_backward_path",
                    None,
                    "backward_path needs >=2 valid 0x addresses",
                )
            }
        };
        let amount_in = match U256::from_dec_str(p.amount_in_wei.trim()) {
            Ok(a) if !a.is_zero() => a,
            _ => {
                return sim_err(
                    p.chain_id,
                    "invalid_amount_in",
                    None,
                    "amount_in_wei must be a positive decimal wei string",
                )
            }
        };
        let gas_price_wei = match p
            .gas_price_wei
            .as_deref()
            .map(|s| U256::from_dec_str(s.trim()))
        {
            None => U256::zero(),
            Some(Ok(g)) => g,
            Some(Err(_)) => {
                return sim_err(
                    p.chain_id,
                    "invalid_gas_price_wei",
                    None,
                    "gas_price_wei must be a decimal wei string",
                )
            }
        };
        let route_hash: [u8; 32] = match p.route_hash.as_deref() {
            None => [0u8; 32],
            Some(h) => match H256::from_str(h) {
                Ok(x) => x.0,
                Err(_) => {
                    return sim_err(
                        p.chain_id,
                        "invalid_route_hash",
                        None,
                        "route_hash must be 0x + 64 hex",
                    )
                }
            },
        };
        let deadline = U256::from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
                .saturating_add(300),
        );

        let ctx = RoundTripContext {
            caller: executor,
            token_in,
            token_out,
            amount_in,
            forward_router: fwd_router,
            forward_path: fwd_path,
            backward_router: bwd_router,
            backward_path: bwd_path,
            deadline,
        };
        let config = RoundTripExecutionConfig {
            chain_id: p.chain_id,
            executor_address: executor,
            gas_limit: p.gas_limit.unwrap_or(30_000_000),
            gas_price_wei,
            route_hash,
            min_profit_wei: U256::from(1u8), // bare contract minimum (>0); economic floor is the caller's gate
            paper_mode: true, // structural NO-ACTIVE gate; orchestrator rejects false
        };

        // SimulatorV2::new is a cheap sync ctor; the RPC state fetch is lazy (LazyDb) inside simulate().
        let simulator = Arc::new(simulator_v2::SimulatorV2::new(rpc_url));
        let outcome = execute_round_trip_revm(&ctx, simulator, &config);

        Json(SimOutput {
            engine: "execute_round_trip_revm (REAL searcher_rs::sim_orchestrator + simulator-v2 REVM, paper_mode=true)".to_string(),
            chain_id: p.chain_id,
            paper_mode: true,
            passed: outcome.passed,
            simulated_profit_token_in_wei: outcome.simulated_profit_token_in.to_string(),
            intermediate_amount_out_wei: outcome.intermediate_amount_out.map(|x| x.to_string()),
            gas_used_total: outcome.gas_used_total,
            fail_reason: outcome.fail_reason,
            error: None,
            needs_env: None,
        })
    }

    /// HONEST stub: MMBF line-graph (7-11 hop) is Phase 2, not implemented.
    #[tool(
        name = "run_mmbf_discovery",
        description = "Modified Moore-Bellman-Ford over the line-graph for 7-11 hop exotic routes. NOT IMPLEMENTED (Phase 2). Use run_route_discovery for the real Phase-1 DFS (2-3 hops)."
    )]
    fn run_mmbf_discovery(&self, Parameters(_p): Parameters<MmbfParams>) -> Json<StatusOutput> {
        Json(StatusOutput {
            status: "not_implemented".to_string(),
            error: "mmbf_phase_2".to_string(),
            phase: Some(2),
            reference: Some("arXiv:2406.16573".to_string()),
            detail: "MMBF line-graph (N loops + N^2 non-loops, 7-11 hops) is Phase 2 and not built. searcher-rs only ships the Phase-1 bounded DFS. Use run_route_discovery. No fabricated routes (RULE 00).".to_string(),
        })
    }

    /// HONEST stub: Marginal Price Optimization is Phase 3, not implemented.
    #[tool(
        name = "compute_mpo_sizing",
        description = "Marginal Price Optimization (root-finding over a price vector, resolves v3_sizing_pending). NOT IMPLEMENTED (Phase 3). Use compute_sizing for a real V2 evaluation at a given amount_in."
    )]
    fn compute_mpo_sizing(&self, Parameters(_p): Parameters<MpoParams>) -> Json<StatusOutput> {
        Json(StatusOutput {
            status: "not_implemented".to_string(),
            error: "mpo_phase_3".to_string(),
            phase: Some(3),
            reference: Some("arXiv:2502.08258".to_string()),
            detail: "Marginal Price Optimization is Phase 3 (resolves v3_sizing_pending) and not built. Use compute_sizing for a real V2 amount_in evaluation. No fabricated sizing (RULE 00).".to_string(),
        })
    }
}

#[tool_handler]
impl ServerHandler for SimEngine {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Logs go to STDERR only — STDOUT is the MCP JSON-RPC channel and must stay clean.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_max_level(tracing::Level::INFO)
        .init();

    tracing::info!(
        event = "arbx_sim_engine.boot",
        "arbx-sim-engine MCP server starting over stdio (shadow/read-only; no capital/executor/broadcast)"
    );

    let service = SimEngine::new().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
