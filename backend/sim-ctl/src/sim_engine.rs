//! Simulation engine — orchestrates fork snapshot, tx build, eth_call,
//! result extraction. Pure orchestration; persistence + stream publish live
//! in the consumer.
//!
//! Output is always a SimulationResult, even for unsupported / error cases —
//! those just carry `passed = false` and a documented `fail_reason`.

use crate::fork_manager::ForkManager;
use crate::tx_builder::{build_probe_with_path, BuildError};
use chrono::Utc;
use ethers::abi::{decode as abi_decode, ParamType};
use ethers::core::types::transaction::eip2718::TypedTransaction;
use ethers::prelude::*;
use shared_rs::candidates::DecimalsMap;
use shared_rs::contracts::{Opportunity, SimulationResult, SimulatorKind};
use sim_ctl::signer_funding::SignerFunder;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, warn};

pub struct SimEngine {
    pub fork: Option<ForkManager>,
    pub signer_from: Address,
    pub timeout: Duration,
    pub max_slippage_for_pass_pct: f64,
    /// SIM-FUND-01 (2026-09-17): seeds the signer's token balance on the
    /// fork (anvil_setStorageAt, sentinel-verified slot) so the probe's
    /// `transferFrom(signer, …)` can actually settle. `None` keeps the
    /// pre-fix behavior (unfunded probes → TRANSFER_FROM_FAILED/STF).
    pub funder: Option<SignerFunder>,
}

impl SimEngine {
    /// Route-less entry point: byte-identical to the pre-SIM4-CYCLIC-02
    /// behaviour (delegates with an empty path).
    ///
    /// F2-SIM-DECIMALS (2026-10-23): this entry has NO decimals source, so it
    /// passes an EMPTY map and the slippage verdict becomes
    /// `slippage_not_computable:decimals_unknown:<token>` instead of the old
    /// invented `(18, 18)`. The route-aware consumer path (`simulate_with_route`)
    /// is the one that carries the resolved map.
    pub async fn simulate(&self, opp: &Opportunity) -> SimulationResult {
        self.simulate_with_route(opp, &[], &DecimalsMap::new())
            .await
    }

    /// SIM4-CYCLIC-02: `route_path` is `route_metadata.token_addresses` — the
    /// full token traversal (first = token_in, last = token_out, length =
    /// hops + 1). Empty means "no route available": a closed route then fails
    /// with its OWN typed reason (`cyclic_route_missing_route_metadata:<kind>`),
    /// never a fabricated probe and never a silent pass.
    ///
    /// F2-SIM-DECIMALS (2026-10-23): `decimals` is the REAL map the consumer
    /// already resolved (`route_lookup::CandidateInputs::resolved_decimals`:
    /// `tokens` table + the row's own claim). Before this parameter existed the
    /// builder assumed `(18, 18)`, so a 6↔18 pair produced a slippage that was
    /// pure scale (0 % / ≈99.7 %) and `passed` was an artifact.
    pub async fn simulate_with_route(
        &self,
        opp: &Opportunity,
        route_path: &[Address],
        decimals: &DecimalsMap,
    ) -> SimulationResult {
        let trace_id = opp.trace_id;
        let id = opp.id;

        // Guard: if no fork (no ANVIL_URL or failed to connect), honest 501-style result.
        let Some(fork) = &self.fork else {
            return Self::not_implemented(
                id,
                trace_id,
                "anvil_fork_not_configured (set ANVIL_URL + profile sim)",
            );
        };

        // Build probe.
        let probe = match build_probe_with_path(opp, self.signer_from, route_path) {
            Ok(p) => p,
            // BR-00 (2026-09-07): D-SIM-01 -- the reason now names the EXACT
            // kind it refused (base kind or cartridge stem, never collapsed).
            // Closes the BR-00-VERIFY 2.3 observability gap: a stray
            // ":dex_arb" suffix on one of these rows is now visible evidence
            // of upstream kind drift instead of an undiagnosable aggregate.
            Err(BuildError::UnsupportedStrategy(kind)) => {
                return Self::not_implemented(
                    id,
                    trace_id,
                    &format!("strategy_not_simulatable_in_s4:{}", kind.as_str()),
                );
            }
            // SIM4-CYCLIC-01 (2026-10-07): a closed route is NO LONGER refused by
            // SHAPE. A closed route is representable — the V2 router takes the
            // whole path array in one call — so this arm now fires only when the
            // traversal path is ABSENT, which is a DATA-AVAILABILITY gap, not a
            // topological one.
            //
            // It gets its OWN reason family on purpose: the old
            // `strategy_cyclic_route_not_simulatable_in_s4` label asserted an
            // impossibility that is FALSE, and reusing it would keep publishing a
            // false verdict on 50k rows.
            //
            // Resolution path: `route_metadata.token_addresses` already carries
            // the full traversal (written by searcher-rs
            // `build_route_metadata_from_plan`, read by `route_lookup.rs`); feed
            // it to `tx_builder::build_probe_with_path`.
            Err(BuildError::CyclicRouteMissingPath(kind)) => {
                return Self::not_implemented(
                    id,
                    trace_id,
                    &format!("cyclic_route_missing_route_metadata:{}", kind.as_str()),
                );
            }
            // SIM4-CYCLIC-04 (F9 de t91): un path PRESENTE pero incoherente con
            // `token_in`/`token_out` NO se ignora ni se sustituye por una pata
            // inventada. Se declara con su propio nombre, y entra TAMBIÉN en el
            // clasificador (`persistence.rs`): si no, se repetiría el agujero de
            // F-01 con otro nombre — un gap de capacidad convertido en rechazo
            // silencioso.
            Err(BuildError::PathNotRepresentable(kind)) => {
                return Self::not_implemented(
                    id,
                    trace_id,
                    &format!("route_path_not_representable:{}", kind.as_str()),
                );
            }
            Err(BuildError::UnsupportedChain(c)) => {
                return Self::not_implemented(
                    id,
                    trace_id,
                    &format!("chain_{c}_not_supported_in_s4"),
                );
            }
            Err(e) => return Self::failed(id, trace_id, &format!("build_error: {e}")),
        };

        // Snapshot + eth_call + revert.
        let handle = match fork.acquire().await {
            Ok(h) => h,
            Err(e) => {
                warn!(event = "sim.acquire_failed", error = %e);
                return Self::failed(id, trace_id, "fork_acquire_failed");
            }
        };

        // SIM-FUND-01 (2026-09-17): fund the signer for token_in INSIDE the
        // snapshot window — the probe swaps `amount_in` of token_in and the
        // router pulls it with transferFrom(signer). Without this, every
        // honest probe reverted TRANSFER_FROM_FAILED (V2) / STF (V3) because
        // the signer is an observation address with zero balance on the
        // fork. Fail-closed: an unfundable token is a typed failure, never a
        // fabricated pass.
        if let Some(funder) = &self.funder {
            if let (Ok(token_in), Ok(amount_in)) = (
                opp.token_in.parse::<Address>(),
                U256::from_dec_str(&opp.amount_in_wei),
            ) {
                if let Err(reason) = funder.ensure_funded(token_in, probe.from, amount_in).await {
                    warn!(event = "sim.funding_failed", id = %id, reason = %reason);
                    let _ = handle.release().await;
                    return Self::failed(id, trace_id, &reason);
                }
            }
        }

        let tx_req = TransactionRequest::new()
            .from(probe.from)
            .to(probe.to)
            .value(probe.value)
            .data(probe.data.clone())
            .gas(probe.gas_cap);
        let typed: TypedTransaction = tx_req.into();

        let call_fut = async {
            let out = fork.provider.call(&typed, None).await?;
            let gas = fork.provider.estimate_gas(&typed, None).await?;
            Ok::<(Bytes, U256), ethers::providers::ProviderError>((out, gas))
        };

        let result = timeout(self.timeout, call_fut).await;

        let release_res = handle.release().await;
        if let Err(e) = release_res {
            warn!(event = "sim.revert_failed", error = %e);
        }

        let (output, gas_used) = match result {
            Ok(Ok((o, g))) => (o, g),
            Ok(Err(e)) => {
                // ProviderError can be a revert. Extract reason when possible.
                let fail_reason = extract_revert_reason(&e);
                return Self::failed(id, trace_id, &fail_reason);
            }
            Err(_) => return Self::failed(id, trace_id, "sim_timeout"),
        };

        // Decode output → slippage vs opp.expected.
        // BR-00 (2026-09-07): decode by ABI shape (the probe tx shape is
        // chosen by the router, never by the strategy kind) -- every
        // structurally-built probe decodes identically.
        //
        // F2-SIM-DECIMALS (2026-10-23): el veredicto se decide con los decimales
        // REALES del par comparado. Un veredicto AUSENTE se declara con su
        // familia propia (`slippage_not_computable:*`) y NUNCA se pliega a
        // `slippage_too_high` (que afirmaría una medición que no ocurrió).
        let actual_out = decode_amount_out(&output);
        let slippage = compute_slippage(opp, actual_out, route_path, decimals);

        let (slippage_pct, fail_reason) = match slippage {
            Ok(s) if s <= self.max_slippage_for_pass_pct => (Some(s), None),
            Ok(s) => (Some(s), Some("slippage_too_high".to_string())),
            Err(gap) => (None, Some(gap.as_fail_reason())),
        };
        let passed = fail_reason.is_none();

        SimulationResult {
            opportunity_id: id,
            passed,
            gas_estimate_wei: Some(gas_used.to_string()),
            gas_price_wei: None, // derived at submit time in S5
            slippage_pct,
            revert_risk_pct: None, // SIM-09 fix (2026-09-24): was fabricated Some(0.5)/Some(50.0) — no risk model computes this
            simulated_profit_usd: None, // computed from counter-trade in S5
            simulator: SimulatorKind::Anvil,
            fail_reason,
            simulated_at: Utc::now(),
            trace_id,
        }
    }

    fn not_implemented(id: uuid::Uuid, trace_id: uuid::Uuid, reason: &str) -> SimulationResult {
        SimulationResult {
            opportunity_id: id,
            passed: false,
            gas_estimate_wei: None,
            gas_price_wei: None,
            slippage_pct: None,
            revert_risk_pct: None,
            simulated_profit_usd: None,
            simulator: SimulatorKind::NotImplemented,
            fail_reason: Some(reason.to_string()),
            simulated_at: Utc::now(),
            trace_id,
        }
    }

    fn failed(id: uuid::Uuid, trace_id: uuid::Uuid, reason: &str) -> SimulationResult {
        SimulationResult {
            opportunity_id: id,
            passed: false,
            gas_estimate_wei: None,
            gas_price_wei: None,
            slippage_pct: None,
            revert_risk_pct: Some(100.0),
            simulated_profit_usd: None,
            simulator: SimulatorKind::Anvil,
            fail_reason: Some(reason.to_string()),
            simulated_at: Utc::now(),
            trace_id,
        }
    }
}

/// Attempts to decode an amountOut from eth_call return bytes. For V2 router,
/// the return is uint[] with one element per hop; last element is the final out.
/// For V3 exactInputSingle, return is a single uint256 = amountOut.
// BR-00 (2026-09-07): decode by ABI SHAPE, not by kind string -- the probe tx
// shape is chosen by the ROUTER (V2 => uint[], V3 => uint256), never by the
// strategy kind, so every structurally-built probe decodes the same way
// regardless of kind or cartridge stem.
// Array shape goes FIRST: a uint[] head word is the 32-byte data offset, so
// decoding it as a bare uint256 "succeeds" with the constant 32 -- the old
// uint256-first order (pre-existing, kind-gated to dex_arb) misread every V2
// probe return as amountOut=32. A bare uint256 (V3) can only decode as an
// array when its value is exactly 32 AND more words follow, so the fallback
// order is unambiguous in both directions.
fn decode_amount_out(output: &Bytes) -> Option<U256> {
    if let Ok(toks) = abi_decode(&[ParamType::Array(Box::new(ParamType::Uint(256)))], output) {
        if let Some(arr) = toks.first().and_then(|t| t.clone().into_array()) {
            return arr.last().and_then(|t| t.clone().into_uint());
        }
    }
    if let Ok(toks) = abi_decode(&[ParamType::Uint(256)], output) {
        if let Some(t) = toks.first() {
            return t.clone().into_uint();
        }
    }
    None
}

/// F2-SIM-DECIMALS (2026-10-23): por qué NO se pudo producir un veredicto de
/// slippage. Cada variante es una AUSENCIA DE MEDICIÓN declarada, no un
/// veredicto de mercado (`None ≠ Some(0)`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlippageGap {
    /// El `eth_call` no devolvió bytes decodificables como amountOut.
    OutputUndecodable,
    /// `amount_in_wei` no es un decimal parseable (el consumidor lo valida
    /// antes; se declara, no se inventa).
    AmountInUnparseable,
    /// Falta el decimal de uno de los dos tokens comparados. NUNCA se asume 18.
    DecimalsUnknown { token: String },
    /// Los tokens comparados tienen decimales DISTINTOS: su cociente en
    /// unidades crudas difiere por 10^(a−b) con independencia de la ejecución.
    CrossDecimals { decimals_in: u8, decimals_out: u8 },
}

impl SlippageGap {
    /// La etiqueta persistida. `slippage_not_computable:*` es una familia de
    /// GAP DE CAPACIDAD (registrada en `persistence::is_sim_capability_gap`):
    /// el motor no puede medir, y eso no es un juicio sobre la oportunidad.
    pub fn as_fail_reason(&self) -> String {
        match self {
            SlippageGap::OutputUndecodable => "output_undecodable".to_string(),
            SlippageGap::AmountInUnparseable => {
                "slippage_not_computable:amount_in_unparseable".to_string()
            }
            SlippageGap::DecimalsUnknown { token } => {
                format!("slippage_not_computable:decimals_unknown:{token}")
            }
            SlippageGap::CrossDecimals {
                decimals_in,
                decimals_out,
            } => format!("slippage_not_computable:cross_decimals_{decimals_in}_{decimals_out}"),
        }
    }
}

/// El token en el que el probe TERMINA: `route_path.last()` cuando hay ruta
/// (para un ciclo es `token_in` — el token de cierre que el builder encoda) y
/// `token_out` cuando no hay ruta (sonda de una pata por `dex_a`).
///
/// F2-SIM-DECIMALS: comparar contra `token_out` a secas era el error que hacía
/// que un ciclo WETH→USDC→WETH se leyera como \"cross-decimal\" (WETH 18 contra
/// USDC 6) cuando en realidad el probe devuelve WETH, el MISMO token que entró.
fn settled_output_token(opp: &Opportunity, route_path: &[Address]) -> String {
    match route_path.last() {
        Some(addr) => format!("{addr:#x}").to_lowercase(),
        None => opp.token_out.to_lowercase(),
    }
}

/// Slippage relativo del probe, con los decimales REALES del par comparado.
///
/// Reglas (F2-SIM-DECIMALS, 2026-10-23):
///   * Sin salida decodificable ⇒ [`SlippageGap::OutputUndecodable`] (sin cambios).
///   * Decimals REALES de `token_in` y del token de cierre; si falta alguno ⇒
///     [`SlippageGap::DecimalsUnknown`]. **NUNCA** se asume 18.
///   * Decimales distintos ⇒ [`SlippageGap::CrossDecimals`]: el cociente en
///     unidades crudas difiere por 10^(a−b) y no mide ejecución. Antes esta rama
///     era CÓDIGO MUERTO (`token_decimals()` devolvía `(18,18)` siempre), así que
///     un par 6→18 daba **0 % de slippage ⇒ PASS FALSO** y el inverso ≈99.7 %
///     ⇒ **FAIL FALSO** (medido en t13 sobre 0/516,460 filas).
///   * Mismo decimal ⇒ heurística legacy `(amt_in − actual)/amt_in`, sin cambios.
///     Para una ruta CERRADA el token de cierre ES `token_in`, así que el
///     cociente sí mide el resultado del ciclo; para una ruta ABIERTA entre dos
///     activos distintos sigue siendo una heurística (la cotización S6 la
///     reemplaza — FOLLOWUP, fuera de esta unidad).
fn compute_slippage(
    opp: &Opportunity,
    actual_out: Option<U256>,
    route_path: &[Address],
    decimals: &DecimalsMap,
) -> Result<f64, SlippageGap> {
    let actual = actual_out.ok_or(SlippageGap::OutputUndecodable)?;
    let amt_in =
        U256::from_dec_str(&opp.amount_in_wei).map_err(|_| SlippageGap::AmountInUnparseable)?;
    if amt_in.is_zero() {
        return Ok(100.0);
    }
    let token_in = opp.token_in.to_lowercase();
    let token_out_settled = settled_output_token(opp, route_path);
    let dec_in = decimals
        .get(&token_in)
        .ok_or_else(|| SlippageGap::DecimalsUnknown {
            token: token_in.clone(),
        })?;
    let dec_out = decimals
        .get(&token_out_settled)
        .ok_or_else(|| SlippageGap::DecimalsUnknown {
            token: token_out_settled.clone(),
        })?;
    if dec_in != dec_out {
        // Cross-decimal pair: the raw-unit ratio is meaningless (R8).
        debug!(
            event = "sim.slippage_cross_decimals_not_computable",
            dec_in,
            dec_out,
            token_in = %token_in,
            token_out = %token_out_settled,
            "not computable — a 10^(dec_in-dec_out) scale, not execution quality"
        );
        return Err(SlippageGap::CrossDecimals {
            decimals_in: dec_in,
            decimals_out: dec_out,
        });
    }
    // Same-decimals heuristic (legacy): (amt_in - actual) / amt_in * 100.
    let diff = if actual >= amt_in {
        U256::zero()
    } else {
        amt_in - actual
    };
    let pct = (diff.as_u128() as f64 / amt_in.as_u128() as f64) * 100.0;
    debug!(event = "sim.slippage_heuristic", pct, decimals = dec_in);
    Ok(pct)
}

/// SIM-04 fix (2026-09-24): char-boundary-safe truncation (twin in
/// revm_backend.rs). The previous `&s[..s.len().min(200)]` panicked on
/// multibyte UTF-8 codepoints at the byte-200 boundary.
fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

fn extract_revert_reason(e: &ethers::providers::ProviderError) -> String {
    let s = e.to_string();
    if s.contains("revert") || s.contains("execution reverted") {
        // Grab content after "reverted: " if present.
        if let Some(idx) = s.find("reverted:") {
            return s[idx..].chars().take(200).collect();
        }
    }
    format!("rpc_error: {}", truncate_chars(&s, 200))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod br00_decode_tests {
    use super::*;
    use ethers::abi::{encode as abi_encode, Token};

    /// V3 exactInputSingle returns a bare uint256 amountOut.
    #[test]
    fn decode_v3_uint256_shape() {
        let out = Bytes::from(abi_encode(&[Token::Uint(U256::from(123_456u64))]));
        assert_eq!(decode_amount_out(&out), Some(U256::from(123_456u64)));
    }

    /// V2 swapExactTokensForTokens returns uint[] amounts -- the LAST element
    /// is the final out (single-hop probes return a 1-element array).
    #[test]
    fn decode_v2_array_shape_takes_last() {
        let arr = Token::Array(vec![
            Token::Uint(U256::from(1u64)),
            Token::Uint(U256::from(2u64)),
        ]);
        let out = Bytes::from(abi_encode(&[arr]));
        assert_eq!(decode_amount_out(&out), Some(U256::from(2u64)));
    }

    /// Garbage / empty output returns None -- fail-closed, never a
    /// fabricated amountOut (R8: None = not computed).
    #[test]
    fn decode_garbage_returns_none() {
        assert_eq!(decode_amount_out(&Bytes::from(vec![0xde, 0xad])), None);
        assert_eq!(decode_amount_out(&Bytes::default()), None);
    }
}

/// F2-SIM-DECIMALS (2026-10-23): el criterio de `passed` deja de ser un
/// artefacto de escala. Estos tests fijan las CUATRO ramas del veredicto con
/// decimales REALES (WETH 18, USDC 6, UNI 18 — los del pipeline).
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod f2_decimals_tests {
    use super::*;

    const WETH: &str = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"; // 18
    const USDC: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"; // 6
    const UNI: &str = "0x1f9840a85d5af5bf1d1762f925bdaddc4201f984"; // 18

    fn opp(token_in: &str, token_out: &str, amount_in_wei: &str) -> Opportunity {
        Opportunity {
            id: uuid::Uuid::new_v4(),
            chain_id: 1,
            strategy_kind: shared_rs::contracts::StrategyKind::dex_arb(),
            dex_a: "uniswap-v2".into(),
            dex_b: None,
            pair_symbol: "WETH/USDC".into(),
            token_in: token_in.into(),
            token_out: token_out.into(),
            amount_in_wei: amount_in_wei.into(),
            expected_profit_usd: Some(1.0),
            net_expected_profit_usd: None,
            roi_pct: None,
            risk_score: None,
            block_number: None,
            rejection_reason: None,
            cartridge_id: None,
            detector_id: None,
            pipeline_latency_ms: None,
            detected_at: Utc::now(),
            trace_id: uuid::Uuid::new_v4(),
            economics: None,
        }
    }

    fn map(pairs: &[(&str, u8)]) -> DecimalsMap {
        let mut m = DecimalsMap::new();
        for (addr, dec) in pairs {
            m.insert((*addr).to_string(), *dec);
        }
        m
    }

    /// (a) Par 6↔18 con decimals REALES: el veredicto es NO COMPUTADO, y el
    /// valor que producía el hardcode `(18,18)` (0.0 ⇒ PASS) ya NO se emite.
    /// Este es el corazón de F2: antes, USDC→WETH daba slippage 0 % con
    /// CUALQUIER salida; ahora se declara que no se puede medir.
    #[test]
    fn cross_decimals_6_18_is_not_computable_and_never_a_false_pass() {
        // 1 USDC-in (6 dec) y una salida de 1e18 wei (1 WETH, 18 dec).
        let o = opp(USDC, WETH, "1000000");
        let actual = U256::from(10u64).pow(18.into());
        let real = map(&[(USDC, 6), (WETH, 18)]);

        let verdict = compute_slippage(&o, Some(actual), &[], &real);
        assert_eq!(
            verdict,
            Err(SlippageGap::CrossDecimals {
                decimals_in: 6,
                decimals_out: 18
            }),
            "a 6↔18 pair cannot be measured by raw units"
        );
        let reason = verdict.unwrap_err().as_fail_reason();
        assert_eq!(reason, "slippage_not_computable:cross_decimals_6_18");
        assert_ne!(reason, "slippage_too_high", "not a market verdict");

        // EL ARTEFACTO, medido: con el mapa inventado (18,18) el mismo probe
        // daba 0.0 % ⇒ `passed = true` sin medir nada.
        let invented = map(&[(USDC, 18), (WETH, 18)]);
        let artifact = compute_slippage(&o, Some(actual), &[], &invented).expect("legacy path");
        assert_eq!(artifact, 0.0, "the old hardcode produced a 0% false pass");

        // Y el inverso (WETH→USDC) con el mapa inventado daba ≈99.7 % ⇒ FAIL falso.
        let o_rev = opp(WETH, USDC, "1000000000000000000");
        let artifact_rev = compute_slippage(&o_rev, Some(U256::from(1_000_000u64)), &[], &invented)
            .expect("legacy");
        assert!(
            artifact_rev > 99.0,
            "the old hardcode produced ≈99.7% on a perfect trade, got {artifact_rev}"
        );
        assert_eq!(
            compute_slippage(&o_rev, Some(U256::from(1_000_000u64)), &[], &real),
            Err(SlippageGap::CrossDecimals {
                decimals_in: 18,
                decimals_out: 6
            })
        );
    }

    /// (b) No-regresión 18↔18: la heurística legacy queda EXACTAMENTE igual.
    #[test]
    fn same_decimals_18_18_keeps_the_legacy_heuristic() {
        let amt = U256::from(10u64).pow(18.into());
        let actual = amt * U256::from(95u32) / U256::from(100u32); // 5 % menos
        let o = opp(WETH, UNI, "1000000000000000000");
        let pct = compute_slippage(&o, Some(actual), &[], &map(&[(WETH, 18), (UNI, 18)]))
            .expect("18↔18 stays computable");
        assert!((pct - 5.0).abs() < 1e-9, "expected 5.0, got {pct}");

        // Profit del ciclo (salida > entrada) sigue midiendo 0 % de pérdida.
        let better = amt + U256::from(1u32);
        let pnl = compute_slippage(&o, Some(better), &[], &map(&[(WETH, 18), (UNI, 18)]))
            .expect("computable");
        assert_eq!(pnl, 0.0);
    }

    /// (c) CICLO CERRADO: el par comparado es (token_in, path[-1]) — el token de
    /// CIERRE —, no `token_out` (el contra-token del par que persiste
    /// TWO-LEG-01). Con `token_out=USDC(6)` y un ciclo WETH→USDC→WETH el probe
    /// devuelve WETH(18): medible. Comparar contra `token_out` habría declarado
    /// un falso `cross_decimals_18_6`.
    #[test]
    fn closed_cycle_compares_the_closing_token_not_the_counter_token() {
        let amt = U256::from(10u64).pow(18.into());
        let actual = amt * U256::from(98u32) / U256::from(100u32); // 2 % menos
        let o = opp(WETH, USDC, "1000000000000000000");
        let path = vec![
            WETH.parse::<Address>().unwrap(),
            USDC.parse::<Address>().unwrap(),
            WETH.parse::<Address>().unwrap(),
        ];
        let pct = compute_slippage(&o, Some(actual), &path, &map(&[(WETH, 18), (USDC, 6)]))
            .expect("a closed cycle returns token_in, so its decimals are comparable");
        assert!((pct - 2.0).abs() < 1e-9, "expected 2.0, got {pct}");

        // Sin el path (sonda de una pata) el mismo row SÍ cruza decimales: es la
        // diferencia exacta entre medir un ciclo y medir un swap simple.
        assert_eq!(
            compute_slippage(&o, Some(actual), &[], &map(&[(WETH, 18), (USDC, 6)])),
            Err(SlippageGap::CrossDecimals {
                decimals_in: 18,
                decimals_out: 6
            })
        );
    }

    /// (d) Decimales AUSENTES: se declara NO COMPUTADO con el token nombrado y
    /// NUNCA se inventa 18 (R8).
    #[test]
    fn unknown_decimals_is_declared_not_invented() {
        let o = opp(WETH, UNI, "1000000000000000000");
        let actual = U256::from(10u64).pow(18.into());

        // Falta el token de entrada.
        assert_eq!(
            compute_slippage(&o, Some(actual), &[], &map(&[(UNI, 18)])),
            Err(SlippageGap::DecimalsUnknown {
                token: WETH.to_string()
            })
        );
        // Falta el token de salida.
        assert_eq!(
            compute_slippage(&o, Some(actual), &[], &map(&[(WETH, 18)])),
            Err(SlippageGap::DecimalsUnknown {
                token: UNI.to_string()
            })
        );
        // Mapa completamente vacío: el motivo nombra el token, no un 18 implícito.
        let reason = compute_slippage(&o, Some(actual), &[], &DecimalsMap::new())
            .unwrap_err()
            .as_fail_reason();
        assert!(
            reason.starts_with("slippage_not_computable:decimals_unknown:"),
            "got {reason}"
        );
        assert!(
            reason.contains(WETH),
            "the reason must name the token: {reason}"
        );
    }

    /// (e) Sin salida decodificable se mantiene la etiqueta previa
    /// (`output_undecodable`), que ya está reconocida por el clasificador.
    #[test]
    fn no_output_keeps_the_undecodable_label() {
        let o = opp(WETH, UNI, "1000000000000000000");
        let gap = compute_slippage(&o, None, &[], &map(&[(WETH, 18), (UNI, 18)])).unwrap_err();
        assert_eq!(gap, SlippageGap::OutputUndecodable);
        assert_eq!(gap.as_fail_reason(), "output_undecodable");
    }
}
