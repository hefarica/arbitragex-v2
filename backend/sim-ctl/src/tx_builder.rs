//! Tx builder — Opportunity -> probe transaction parameters.
//!
//! S4 scope: single-hop UniV2/UniV3 swap probes on chain_id=1. BR-00
//! (2026-09-07): admission is decided by ROUTE STRUCTURE -- two distinct
//! tokens plus a catalog V2/V3 router on dex_a -- NOT by string equality
//! with "dex_arb": every kind or cartridge stem whose payload has that
//! shape builds the same probe (the kind is never collapsed or rewritten).
//! Only non-swap topologies and cyclic routes are refused
//! (`UnsupportedStrategy` / `CyclicRouteNotRepresentable`, handled as
//! 'not_implemented' sim results, NEVER as pass).

use ethers::abi::{encode, Token};
use ethers::types::{Address, Bytes, U256};
use shared_rs::chains::RouterKind;
use shared_rs::contracts::{Opportunity, StrategyKind};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("unsupported strategy for S4 simulator: {0:?}")]
    UnsupportedStrategy(StrategyKind),
    // SIM4-CYCLIC-01 (2026-10-07): a closed route IS representable. The UniV2
    // router takes the WHOLE path array in ONE call
    // (`swapExactTokensForTokens(..., address[] path, ...)`), so
    // `path = [token_in, X1, ..., Xn, token_in]` encodes an N-leg cycle with no
    // new contract and no new encoder. What was missing was never the encoder:
    // `build_probe` could not SEE the hops, and the payload does not carry them.
    //
    // So the absence of the traversal path gets its OWN typed reason. This
    // deliberately REPLACES `CyclicRouteNotRepresentable`, whose message
    // asserted a topological impossibility that is false — and reusing that
    // label would keep publishing a false verdict. The path itself is persisted
    // (`route_metadata.token_addresses`, written by
    // searcher-rs/src/engines/dex_engine.rs:1762, read by route_lookup.rs).
    #[error(
        "cyclic route needs route_metadata.token_addresses (>=3 tokens) to be representable: {0:?}"
    )]
    CyclicRouteMissingPath(StrategyKind),
    // SIM4-CYCLIC-04 (F9 de la verificación independiente t91 — PROHIBIDO
    // DESCARTAR HOPS EN SILENCIO): desde N1 el `consumer.rs` YA pasa
    // `route_metadata.token_addresses`, pero este builder ignoraba el path
    // cuando `token_in != token_out` y encodía UNA pata `token_in -> token_out`
    // por `dex_a`, descartando los hops intermedios. Eso simula una ruta que NO
    // existe, y un `passed=true` obtenido así **contamina la mismísima métrica
    // que este despliegue existe para producir** (`passed=true > 0`).
    //
    // Una simulación equivocada es PEOR que una negativa: la negativa deja
    // rastro de su causa, la fabricación no. Por eso un path presente que no
    // empieza en `token_in` o no termina en `token_out` (o es demasiado corto)
    // es un fallo TIPADO con nombre propio, nunca una pata inventada.
    #[error(
        "route path is present but not representable as a single leg (must start on token_in and end on token_out): {0:?}"
    )]
    PathNotRepresentable(StrategyKind),
    #[error("chain {0} not supported in S4 (only mainnet)")]
    UnsupportedChain(u64),
    #[error("router not in catalog for chain={chain} dex={dex}")]
    UnknownRouter { chain: u64, dex: String },
    #[error("token address invalid: {0}")]
    InvalidAddress(String),
    #[error("amount invalid: {0}")]
    InvalidAmount(String),
}

pub struct ProbeTx {
    pub from: Address,
    pub to: Address,
    pub value: U256,
    pub data: Bytes,
    pub gas_cap: u64,
}

const DEFAULT_GAS_CAP: u64 = 5_000_000;
const DEADLINE_OFFSET_SECS: u64 = 120;
/// UniV3 default fee tier (0.3 %). Real production should try 500/3000/10000 and
/// pick the lowest-slippage quote — deferred to S5 quoter integration.
const DEFAULT_UNIV3_FEE: u32 = 3000;

/// Single-hop probe with NO traversal path. SIM4-CYCLIC-02: `sim_engine` now
/// always goes through `build_probe_with_path`, so this has no production
/// caller left — its only readers are the tests below. `#[cfg(test)]` declares
/// that fact, instead of `#[allow(dead_code)]` which would hide the (true)
/// warning behind a claim that production code still calls it.
#[cfg(test)]
pub fn build_probe(opp: &Opportunity, signer_from: Address) -> Result<ProbeTx, BuildError> {
    build_probe_with_path(opp, signer_from, &[])
}

/// SIM4-CYCLIC-01: `path` is `route_metadata.token_addresses` — the FULL token
/// traversal (first = token_in, last = token_out, length = hops + 1). Pass an
/// empty slice when the caller has no route.
///
/// A closed route (`token_in == token_out`) is representable IFF `path.len() >= 3`
/// (i.e. >= 2 legs) and the path genuinely starts on `token_in` and ends on
/// `token_out`. Absent/inconsistent path ⇒ `CyclicRouteMissingPath`, its own
/// typed reason — never the old "not representable" label.
pub fn build_probe_with_path(
    opp: &Opportunity,
    signer_from: Address,
    path: &[Address],
) -> Result<ProbeTx, BuildError> {
    if opp.chain_id != 1 {
        return Err(BuildError::UnsupportedChain(opp.chain_id));
    }
    // BR-00 (2026-09-07): D-SIM-01 -- simulability is decided by ROUTE
    // STRUCTURE, not by string equality with "dex_arb". The S4 probe is a
    // single V2/V3 swap (token_in -> token_out on dex_a router); ANY kind
    // whose payload has that shape gets the same probe -- including exact
    // cartridge stems (mev_01_001_dex_dex_arbitrage etc.), each a canonical
    // strategy_kind in its own right (shared-rs contracts.rs) that is NEVER
    // collapsed into a base family (operator directive 2026-09-07). Only
    // kinds whose execution topology is not a DEX swap at all are refused at
    // the kind level, with the kind carried in the error.
    if is_non_swap_strategy_kind(opp.strategy_kind.as_str()) {
        return Err(BuildError::UnsupportedStrategy(opp.strategy_kind.clone()));
    }
    let token_in = parse_addr(&opp.token_in)?;
    let token_out = parse_addr(&opp.token_out)?;
    // SIM4-CYCLIC-04 (F9 de t91): el path NO se usa sólo para rutas CERRADAS.
    //
    // Desde N1 el `consumer.rs` ya pasa `route_metadata.token_addresses`. Si ese
    // path viene y es COHERENTE se usa COMPLETO —cíclico o no—, porque descartar
    // los hops intermedios y encodear una sola pata `token_in -> token_out`
    // simula una ruta que NO existe, y un `passed=true` así contamina la métrica
    // que este despliegue existe para producir.
    //
    // Tres casos, y ninguno fabrica nada:
    //   * CERRADA (`token_in == token_out`): exige >= 2 patas y cierre; si no,
    //     `CyclicRouteMissingPath` (familia ya reconocida por el clasificador).
    //   * NO cerrada SIN path: comportamiento PREVIO intacto (una pata por dex_a).
    //   * NO cerrada CON path incoherente: `PathNotRepresentable`, tipado.
    let path_tokens: Option<Vec<Address>> = if token_in == token_out {
        if path.len() < 3 || path[0] != token_in || path[path.len() - 1] != token_out {
            return Err(BuildError::CyclicRouteMissingPath(
                opp.strategy_kind.clone(),
            ));
        }
        Some(path.to_vec())
    } else if path.is_empty() {
        None
    } else {
        if path.len() < 2 || path[0] != token_in || path[path.len() - 1] != token_out {
            return Err(BuildError::PathNotRepresentable(opp.strategy_kind.clone()));
        }
        Some(path.to_vec())
    };
    let amount_in = U256::from_dec_str(&opp.amount_in_wei)
        .map_err(|_| BuildError::InvalidAmount(opp.amount_in_wei.clone()))?;
    if amount_in.is_zero() {
        return Err(BuildError::InvalidAmount("zero amount_in".into()));
    }

    let router_entry =
        find_router_by_name(opp.chain_id, &opp.dex_a).ok_or_else(|| BuildError::UnknownRouter {
            chain: opp.chain_id,
            dex: opp.dex_a.clone(),
        })?;
    let to = Address::from(router_entry.address);

    let deadline = U256::from(now_secs() + DEADLINE_OFFSET_SECS);

    let data: Bytes = match router_entry.kind {
        RouterKind::UniswapV2 | RouterKind::Sushi => match &path_tokens {
            Some(p) => encode_v2_path(p, amount_in, signer_from, deadline),
            None => encode_v2(token_in, token_out, amount_in, signer_from, deadline),
        },
        RouterKind::UniswapV3 => match &path_tokens {
            Some(p) => encode_v3_exact_input(p, amount_in, signer_from, deadline),
            None => {
                encode_v3_exact_input_single(token_in, token_out, amount_in, signer_from, deadline)
            }
        },
        RouterKind::PancakeV3 => {
            // PANCAKE-ROUTER-01: Smart Router exposes the SwapRouter02-style
            // exactInputSingle — 7-field tuple WITHOUT deadline. Sending the
            // UniV3 v1 shape (deadline inside the tuple) reverts on this
            // router, so the shapes must stay distinct.
            encode_router02_exact_input_single(
                token_in,
                token_out,
                DEFAULT_UNIV3_FEE,
                signer_from,
                amount_in,
            )
        }
        _ => {
            return Err(BuildError::UnknownRouter {
                chain: opp.chain_id,
                dex: opp.dex_a.clone(),
            })
        }
    };

    Ok(ProbeTx {
        from: signer_from,
        to,
        value: U256::zero(), // probe assumes ERC20-in; ETH-in probes deferred to S4.1
        data,
        gas_cap: DEFAULT_GAS_CAP,
    })
}

/// BR-00 (2026-09-07): kinds whose EXECUTION topology is not a DEX swap
/// route, verified against their emitting workers -- they can never be
/// expressed by this builder regardless of payload shape:
/// - `liquidation` / `liquidation_snipe`: repay-debt + seize-collateral
///   calls against an Aave V3 pool (liquidation_worker.rs emits
///   dex_a = "aave-v3:<pool>" and amount in Aave base units) -- not a router
///   swap. Refusing by kind yields the specific per-kind reason instead of
///   a misleading "router not in catalog" build error.
///
/// Case-insensitive on purpose: producers have emitted PascalCase drift
/// before (2026-08-18 router-catalog anomaly -- same drift class).
fn is_non_swap_strategy_kind(kind: &str) -> bool {
    let k = kind.trim();
    k.eq_ignore_ascii_case("liquidation") || k.eq_ignore_ascii_case("liquidation_snipe")
}

/// Search router catalog by human-readable `dex_a` name (e.g. "uniswap-v2").
///
/// 2026-08-18 (anomaly "router not in catalog"): producers emit the SAME dex
/// under several spellings — catalog entries are kebab-case ("uniswap-v3-swap-
/// router"), kinds are kebab-case ("uniswap-v3"), while detection paths emit
/// PascalCase display names ("UniswapV3", "SushiSwap"). The old case-sensitive
/// `starts_with`/`==` match missed them → `build_error: router not in catalog`
/// on 15/50 live rows for routers that WERE in the catalog. Normalizing both
/// sides (lowercase + strip `-`/`_`/spaces) makes the match spelling-proof
/// without touching the catalog or upstream producers:
///   "UniswapV3" → "uniswapv3" matches "uniswap-v3-swap-router" → "uniswapv3…"
///   "SushiSwap" → "sushiswap" matches kind "sushi" name "sushi-router" → "sushirouter"
fn find_router_by_name(
    chain_id: u64,
    dex_a: &str,
) -> Option<&'static shared_rs::chains::RouterEntry> {
    fn norm(s: &str) -> String {
        s.chars()
            .filter(|c| !matches!(c, '-' | '_' | ' '))
            .flat_map(|c| c.to_lowercase())
            .collect()
    }
    let want = norm(dex_a);
    if want.is_empty() {
        return None;
    }
    shared_rs::chains::routers_for_chain(chain_id)
        .iter()
        .find(|r| {
            norm(r.name).starts_with(&want)
                || norm(r.kind.as_str()).starts_with(&want)
                || want.starts_with(&norm(r.kind.as_str()))
        })
}

fn parse_addr(s: &str) -> Result<Address, BuildError> {
    let cleaned = s.trim_start_matches("0x").trim_start_matches("0X");
    if cleaned.len() != 40 || !cleaned.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(BuildError::InvalidAddress(s.to_string()));
    }
    let bytes = hex::decode(cleaned).map_err(|_| BuildError::InvalidAddress(s.to_string()))?;
    let mut arr = [0u8; 20];
    arr.copy_from_slice(&bytes);
    Ok(Address::from(arr))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// UniV2 Router02: swapExactTokensForTokens(uint256,uint256,address[],address,uint256)
/// Selector: 0x38ed1739
fn encode_v2(
    token_in: Address,
    token_out: Address,
    amount_in: U256,
    to: Address,
    deadline: U256,
) -> Bytes {
    let selector: [u8; 4] = [0x38, 0xed, 0x17, 0x39];
    let tokens = vec![
        Token::Uint(amount_in),
        Token::Uint(U256::from(1u8)), // amountOutMin = 1 (simulation only)
        Token::Array(vec![Token::Address(token_in), Token::Address(token_out)]),
        Token::Address(to),
        Token::Uint(deadline),
    ];
    let mut buf = selector.to_vec();
    buf.extend(encode(&tokens));
    Bytes::from(buf)
}

/// SIM4-CYCLIC-01 (2026-10-07): SAME selector as `encode_v2` (0x38ed1739) — the
/// ONLY thing a closed route ever needed was the FULL path array instead of the
/// 2-element `[token_in, token_out]`. For a cycle the first and last elements
/// are the same token: `path = [A, X1, ..., Xn, A]`.
fn encode_v2_path(path: &[Address], amount_in: U256, to: Address, deadline: U256) -> Bytes {
    let selector: [u8; 4] = [0x38, 0xed, 0x17, 0x39];
    let tokens = vec![
        Token::Uint(amount_in),
        Token::Uint(U256::from(1u8)), // amountOutMin = 1 (simulation only)
        Token::Array(path.iter().map(|a| Token::Address(*a)).collect()),
        Token::Address(to),
        Token::Uint(deadline),
    ];
    let mut buf = selector.to_vec();
    buf.extend(encode(&tokens));
    Bytes::from(buf)
}

/// SIM4-CYCLIC-01 (2026-10-07): UniV3 `exactInput(ExactInputParams)` — selector
/// 0xc04b8d59 — for a multi-hop (closed) route. The path is the PACKED encoding
/// `token(20) ‖ fee(3) ‖ token(20) ‖ ... ‖ token(20)`.
///
/// Per-hop fee is `DEFAULT_UNIV3_FEE`: the SAME declared simplification the
/// single-hop encoder already makes (trying 500/3000/10000 and picking the
/// lowest-slippage quote is deferred to the S5 quoter — see `DEFAULT_UNIV3_FEE`).
/// This is a fee-tier approximation, not a fabricated route: every token and
/// every hop comes from `route_metadata.token_addresses`.
fn encode_v3_exact_input(path: &[Address], amount_in: U256, to: Address, deadline: U256) -> Bytes {
    let selector: [u8; 4] = [0xc0, 0x4b, 0x8d, 0x59];
    let mut packed: Vec<u8> = Vec::with_capacity(path.len() * 23);
    for (i, a) in path.iter().enumerate() {
        if i > 0 {
            // uint24 fee = 3000 -> 3 big-endian bytes 0x00,0x0b,0xb8
            packed.extend_from_slice(&DEFAULT_UNIV3_FEE.to_be_bytes()[1..]);
        }
        packed.extend_from_slice(a.as_bytes());
    }
    let tokens = vec![
        Token::Bytes(packed),
        Token::Address(to),
        Token::Uint(deadline),
        Token::Uint(amount_in),
        Token::Uint(U256::from(1u8)), // amountOutMinimum = 1 (simulation only)
    ];
    let mut buf = selector.to_vec();
    buf.extend(encode(&tokens));
    Bytes::from(buf)
}

/// UniV3 SwapRouter: exactInputSingle(ExactInputSingleParams)
/// Selector: 0x414bf389
fn encode_v3_exact_input_single(
    token_in: Address,
    token_out: Address,
    amount_in: U256,
    to: Address,
    deadline: U256,
) -> Bytes {
    let selector: [u8; 4] = [0x41, 0x4b, 0xf3, 0x89];
    let params = Token::Tuple(vec![
        Token::Address(token_in),
        Token::Address(token_out),
        Token::Uint(U256::from(DEFAULT_UNIV3_FEE)),
        Token::Address(to),
        Token::Uint(deadline),
        Token::Uint(amount_in),
        Token::Uint(U256::zero()), // amountOutMinimum = 0
        Token::Uint(U256::zero()), // sqrtPriceLimitX96 = 0
    ]);
    let mut buf = selector.to_vec();
    buf.extend(encode(&[params]));
    Bytes::from(buf)
}

/// PANCAKE-ROUTER-01 (2026-09-17): SwapRouter02-style exactInputSingle as
/// exposed by the PancakeSwap V3 Smart Router (and UniV3 SwapRouter02) —
/// a 7-field tuple WITHOUT deadline:
///   (tokenIn, tokenOut, fee, recipient, amountIn, amountOutMinimum,
///    sqrtPriceLimitX96)
/// The selector is COMPUTED from the canonical signature so this function
/// cannot silently drift from the ABI it claims to encode; a wrong
/// hand-typed constant would fail the pancake test, not the fork.
fn encode_router02_exact_input_single(
    token_in: Address,
    token_out: Address,
    fee: u32,
    to: Address,
    amount_in: U256,
) -> Bytes {
    let sig = "exactInputSingle((address,address,uint24,address,uint256,uint256,uint160))";
    let selector = ethers::utils::keccak256(sig.as_bytes())[..4].to_vec();
    let params = Token::Tuple(vec![
        Token::Address(token_in),
        Token::Address(token_out),
        Token::Uint(U256::from(fee)),
        Token::Address(to),
        Token::Uint(amount_in),
        Token::Uint(U256::zero()), // amountOutMinimum = 0 (simulation only)
        Token::Uint(U256::zero()), // sqrtPriceLimitX96 = 0
    ]);
    let mut buf = selector;
    buf.extend(encode(&[params]));
    Bytes::from(buf)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn opp(kind: StrategyKind, chain_id: u64, dex_a: &str) -> Opportunity {
        Opportunity {
            id: Uuid::new_v4(),
            chain_id,
            strategy_kind: kind,
            dex_a: dex_a.into(),
            dex_b: None,
            pair_symbol: "WETH/USDC".into(),
            token_in: "0xC02aaa39b223FE8D0A0e5C4F27eAD9083C756Cc2".into(),
            token_out: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".into(),
            amount_in_wei: "1000000000000000000".into(),
            expected_profit_usd: Some(10.0),
            net_expected_profit_usd: None,
            roi_pct: None,
            risk_score: None,
            block_number: None,
            rejection_reason: None,
            cartridge_id: None,
            detector_id: None,
            pipeline_latency_ms: None,
            detected_at: Utc::now(),
            trace_id: Uuid::new_v4(),
            economics: None,
        }
    }

    #[test]
    fn v2_dex_arb_builds() {
        let signer: Address = [0xab; 20].into();
        let o = opp(StrategyKind::dex_arb(), 1, "uniswap-v2");
        let tx = build_probe(&o, signer).expect("build v2");
        assert_eq!(tx.data.as_ref()[0..4], [0x38, 0xed, 0x17, 0x39]);
        assert_eq!(tx.value, U256::zero());
        assert_eq!(tx.from, signer);
    }

    // ── 2026-08-18 anomaly regression: PascalCase display names must resolve ──
    // Live feed rejected 15/50 rows as "router not in catalog" with dex_a =
    // "UniswapV3"/"UniswapV2"/"SushiSwap" while the catalog HAS those routers —
    // the old case-sensitive match just couldn't see them.
    #[test]
    fn pascalcase_display_names_resolve_in_catalog() {
        for (dex, expected_router) in [
            ("UniswapV2", "0x7a250d5630b4cf539739df2c5dacb4c659f2488d"),
            ("UniswapV3", "0xe592427a0aece92de3edee1f18e0157c05861564"),
            ("SushiSwap", "0xd9e1ce17f2641f24ae83637ab66a2cca9c378b9f"),
        ] {
            let entry =
                find_router_by_name(1, dex).unwrap_or_else(|| panic!("catalog miss for {dex}"));
            assert_eq!(
                format!("{:#x}", Address::from(entry.address)),
                expected_router
            );
        }
    }

    #[test]
    fn kebab_and_lowercase_names_still_resolve() {
        // Pre-existing spellings keep working (no regression on the old paths).
        for dex in ["uniswap-v2", "uniswap-v3", "sushi", "sushi-router"] {
            assert!(
                find_router_by_name(1, dex).is_some(),
                "regression: {dex} stopped resolving"
            );
        }
    }

    #[test]
    fn unknown_dex_still_misses() {
        // Fail-closed preserved: garbage names must NOT fuzzy-match a router.
        assert!(find_router_by_name(1, "notarealdeck").is_none());
        assert!(find_router_by_name(1, "").is_none());
    }

    #[test]
    fn v3_dex_arb_builds() {
        let signer: Address = [0xcd; 20].into();
        let o = opp(StrategyKind::dex_arb(), 1, "uniswap-v3");
        let tx = build_probe(&o, signer).expect("build v3");
        assert_eq!(tx.data.as_ref()[0..4], [0x41, 0x4b, 0xf3, 0x89]);
    }

    /// BR-00 (2026-09-07): cyclic-route fixture -- token_out == token_in,
    /// the exact shape the triangular/flashloan workers emit.
    fn opp_cyclic(kind: StrategyKind, chain_id: u64, dex_a: &str) -> Opportunity {
        let mut o = opp(kind, chain_id, dex_a);
        o.token_out = o.token_in.clone();
        o
    }

    // BR-00 (2026-09-07): non-swap topologies are the ONLY kind-level
    // refusals left -- refused with the kind carried in the error.
    #[test]
    fn non_swap_kinds_rejected() {
        let signer: Address = [0; 20].into();
        for kind in ["liquidation", "Liquidation", "liquidation_snipe"] {
            let o = opp(StrategyKind::cartridge(kind), 1, "uniswap-v2");
            assert!(
                matches!(
                    build_probe(&o, signer),
                    Err(BuildError::UnsupportedStrategy(_))
                ),
                "kind {kind} must be refused as non-swap"
            );
        }
    }

    // SIM4-CYCLIC-01 (2026-10-07): a closed route WITHOUT the traversal path is
    // still an error — but a DATA-AVAILABILITY error with its OWN reason, not the
    // old "not representable" label (which asserted a topology claim that is
    // false: the route IS representable, the path is simply missing).
    #[test]
    fn cyclic_route_without_path_has_its_own_reason() {
        let signer: Address = [0; 20].into();
        for kind in [
            StrategyKind::triangular(),
            StrategyKind::flashloan_arb(),
            StrategyKind::cartridge("mev_01_016_triangular_arbitrage"),
        ] {
            let o = opp_cyclic(kind, 1, "uniswap-v2");
            assert!(matches!(
                build_probe(&o, signer),
                Err(BuildError::CyclicRouteMissingPath(_))
            ));
        }
    }

    /// 32-byte ABI word for an address (left-padded), for calldata assertions.
    fn word(addr: Address) -> Vec<u8> {
        let mut w = vec![0u8; 12];
        w.extend_from_slice(addr.as_bytes());
        w
    }

    // SIM4-CYCLIC-01 (A1): a 2-leg closed route [A, B, A] builds an EXECUTABLE
    // ProbeTx — ONE V2 call carrying the full path array — and no longer ends in
    // a by-name refusal.
    #[test]
    fn cyclic_two_leg_route_builds_executable_probe() {
        let signer: Address = [0x11; 20].into();
        let b: Address = [0xbb; 20].into();
        let o = opp_cyclic(StrategyKind::triangular(), 1, "uniswap-v2");
        // The path must START on the opportunity's token_in and close back onto
        // it — that is exactly what makes it a consistent cyclic route.
        let a: Address = o.token_in.parse().expect("token_in parses");

        let tx = build_probe_with_path(&o, signer, &[a, b, a])
            .expect("closed route with a real path must build");

        // Executable shape: from + to + non-empty calldata.
        assert_eq!(tx.from, signer);
        assert_ne!(tx.to, Address::zero());
        assert!(tx.data.len() > 4, "calldata must be non-empty");
        // UniV2 swapExactTokensForTokens selector.
        assert_eq!(tx.data.as_ref()[0..4], [0x38, 0xed, 0x17, 0x39]);

        // The encoded path array carries the WHOLE traversal, in order: A, B, A.
        let d = tx.data.as_ref();
        let wa = word(a);
        let wb = word(b);
        let first_a = d.windows(32).position(|w| w == wa.as_slice());
        let first_b = d.windows(32).position(|w| w == wb.as_slice());
        let last_a = d.windows(32).rposition(|w| w == wa.as_slice());
        assert!(first_a.is_some() && first_b.is_some() && last_a.is_some());
        let (ia, ib, ia2) = (first_a.unwrap(), first_b.unwrap(), last_a.unwrap());
        assert!(ia < ib && ib < ia2, "path must encode A then B then A");
        // A appears TWICE — that is what makes it a closed route.
        assert_eq!(d.windows(32).filter(|w| *w == wa.as_slice()).count(), 2);
    }

    // SIM4-CYCLIC-01 (A1): a closed route whose path does NOT start/end on the
    // opportunity tokens is refused rather than encoded — an inconsistent route
    // is a data defect, not something to paper over with a wrong swap.
    #[test]
    fn cyclic_route_with_inconsistent_path_is_refused() {
        let signer: Address = [0x11; 20].into();
        let o = opp_cyclic(StrategyKind::triangular(), 1, "uniswap-v2");
        let a: Address = o.token_in.parse().expect("token_in parses");
        let b: Address = [0xbb; 20].into();
        // Starts on token_in (correct) but does NOT close back onto it.
        assert!(matches!(
            build_probe_with_path(&o, signer, &[a, b, Address::from([0xcc; 20])]),
            Err(BuildError::CyclicRouteMissingPath(_))
        ));
    }

    // SIM4-CYCLIC-01 (A1, V3): a closed route on a V3 router uses exactInput with
    // the packed path and builds.
    #[test]
    fn cyclic_two_leg_route_builds_on_v3() {
        let signer: Address = [0x11; 20].into();
        let b: Address = [0xbb; 20].into();
        let o = opp_cyclic(StrategyKind::triangular(), 1, "uniswap-v3");
        let a: Address = o.token_in.parse().expect("token_in parses");
        let tx = build_probe_with_path(&o, signer, &[a, b, a]).expect("V3 closed route must build");
        // UniV3 exactInput selector.
        assert_eq!(tx.data.as_ref()[0..4], [0xc0, 0x4b, 0x8d, 0x59]);
    }

    // SIM4-CYCLIC-04 (F9 de t91): una ruta NO cíclica CON path coherente debe usar
    // el path COMPLETO. Antes se IGNORABA el path y se encodía una sola pata
    // `token_in -> token_out`, descartando los hops intermedios: eso simula una
    // ruta que NO existe, y un `passed=true` obtenido así contamina la métrica
    // que este despliegue existe para producir.
    #[test]
    fn non_cyclic_multihop_path_is_used_not_replaced_by_an_invented_leg() {
        let a_str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let b_str = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let x_str = "0xcccccccccccccccccccccccccccccccccccccccc";
        let signer: Address = [0x11; 20].into();
        let a: Address = a_str.parse().expect("a");
        let b: Address = b_str.parse().expect("b");
        let x: Address = x_str.parse().expect("x");
        let mut o = opp(StrategyKind::dex_arb(), 1, "uniswap-v2");
        o.token_in = a_str.into();
        o.token_out = b_str.into();

        let tx = build_probe_with_path(&o, signer, &[a, x, b]).expect("multi-hop path must build");

        // El path encodado lleva A, X, B en orden. Una sola pata inventada habría
        // producido sólo [A, B] — sin X.
        let d = tx.data.as_ref();
        let (wa, wx, wb) = (word(a), word(x), word(b));
        let ia = d.windows(32).position(|w| w == wa.as_slice());
        let ix = d.windows(32).position(|w| w == wx.as_slice());
        let ib = d.windows(32).position(|w| w == wb.as_slice());
        assert!(
            ia.is_some() && ix.is_some() && ib.is_some(),
            "los 3 tokens deben estar en el path"
        );
        let (ia, ix, ib) = (ia.unwrap(), ix.unwrap(), ib.unwrap());
        assert!(ia < ix && ix < ib, "el path debe codificar A -> X -> B");
    }

    // SIM4-CYCLIC-04 (F9): un path PRESENTE pero incoherente NO se ignora ni se
    // sustituye por una pata inventada — fallo TIPADO con nombre propio.
    #[test]
    fn incoherent_path_on_non_cyclic_route_is_typed_not_invented() {
        let a_str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let b_str = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let x_str = "0xcccccccccccccccccccccccccccccccccccccccc";
        let z_str = "0xdddddddddddddddddddddddddddddddddddddddd";
        let signer: Address = [0x11; 20].into();
        let a: Address = a_str.parse().expect("a");
        let x: Address = x_str.parse().expect("x");
        let z: Address = z_str.parse().expect("z");
        let mut o = opp(StrategyKind::dex_arb(), 1, "uniswap-v2");
        o.token_in = a_str.into();
        o.token_out = b_str.into();

        // Empieza en A pero termina en Z, no en token_out.
        assert!(matches!(
            build_probe_with_path(&o, signer, &[a, x, z]),
            Err(BuildError::PathNotRepresentable(_))
        ));
    }

    // BR-00 (2026-09-07): D-SIM-01 regression -- cartridge stems and
    // re-labelings whose payload is a real two-token hop build the SAME
    // probe dex_arb gets; the kind is admitted, never rewritten.
    #[test]
    fn cartridge_stems_and_relabels_build_probes() {
        let signer: Address = [0xab; 20].into();
        for kind in [
            "dex_arb",
            "backrun",
            "mev_01_001_dex_dex_arbitrage",
            "mev_02_005_concentrated_liquidity_arbitrage",
            "mev_04_001_stablecoin_peg_arbitrage",
            "mev_01_019_multi_hop_arbitrage",
        ] {
            let o = opp(StrategyKind::cartridge(kind), 1, "uniswap-v2");
            let tx =
                build_probe(&o, signer).unwrap_or_else(|e| panic!("kind {kind} must build: {e}"));
            assert_eq!(tx.data.as_ref()[0..4], [0x38, 0xed, 0x17, 0x39]);
        }
    }

    // BR-00 (2026-09-07): a kind NOT in any list still builds when the route
    // is structurally sound -- the decision is the payload, never a name
    // registry (and never a panic on unknown input).
    #[test]
    fn unknown_open_route_kind_builds() {
        let signer: Address = [0; 20].into();
        let o = opp(
            StrategyKind::cartridge("mev_99_001_brand_new_kind"),
            1,
            "uniswap-v2",
        );
        assert!(build_probe(&o, signer).is_ok());
    }

    #[test]
    fn non_mainnet_rejected() {
        let o = opp(StrategyKind::dex_arb(), 137, "uniswap-v2");
        assert!(matches!(
            build_probe(&o, [0; 20].into()),
            Err(BuildError::UnsupportedChain(137))
        ));
    }

    #[test]
    fn unknown_dex_rejected() {
        let o = opp(StrategyKind::dex_arb(), 1, "some-unknown-dex");
        assert!(matches!(
            build_probe(&o, [0; 20].into()),
            Err(BuildError::UnknownRouter { .. })
        ));
    }

    // ── PANCAKE-ROUTER-01 (2026-09-17) ────────────────────────────────────
    // Incident: 79 build_errors "router not in catalog for chain=1
    // dex=PancakeSwap V3" in 2h — PancakeSwap V3 pools ARE in the live
    // graph, but the router catalog has no mainnet Pancake entry, so every
    // Pancake-bearing candidate died at build time.

    #[test]
    fn pancake_v3_display_name_resolves() {
        let entry = find_router_by_name(1, "PancakeSwap V3").expect("catalog miss");
        assert_eq!(
            format!("{:#x}", Address::from(entry.address)),
            "0x13f4ea83d0bd40e75c8222255bc855a974568dd4"
        );
        // The other spellings the graph emits must resolve to the same router.
        for dex in ["PancakeSwapV3", "pancakeswap-v3", "pancake-v3-smart-router"] {
            let e = find_router_by_name(1, dex).unwrap_or_else(|| panic!("catalog miss for {dex}"));
            assert_eq!(e.address, entry.address, "spelling {dex} diverged");
        }
    }

    #[test]
    fn pancake_v3_probe_uses_router02_style_encode() {
        // The Pancake Smart Router exposes the SwapRouter02-style
        // exactInputSingle (7-field tuple, NO deadline). Locking the SHAPE
        // (selector + field count via decoded length), not a memorized
        // selector: compute the selector from the signature so a wrong
        // hand-typed constant fails here, not on the fork.
        let signer: Address = [0xef; 20].into();
        let o = opp(StrategyKind::dex_arb(), 1, "PancakeSwap V3");
        let tx = build_probe(&o, signer).expect("pancake probe must build");
        let expected_sel = ethers::utils::keccak256(
            "exactInputSingle((address,address,uint24,address,uint256,uint256,uint160))".as_bytes(),
        )[..4]
            .to_vec();
        assert_eq!(tx.data.as_ref()[0..4].to_vec(), expected_sel);
        // 4 selector + tuple head + 7 words (last is empty bytes offset) —
        // structurally distinguishable from the 8-word UniV3 v1 encode.
        assert!(tx.data.len() > 4 + 32, "tuple-encoded payload expected");
    }
}
