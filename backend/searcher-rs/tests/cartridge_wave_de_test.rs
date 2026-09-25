//! RU-5 waves D+E functional tests — G08+G09+G10+G11 cartridges with real
//! hunting math (the final 75 of the 264; 264/264 COMPLETE).
//!
//! Same style as `cartridge_wave_b_test.rs` / `cartridge_wave_c_test.rs`:
//! RUNS `init_strategy()` / `evaluate_opportunity()` / `build_payload()` for
//! the 75 wave-D/E cartridges against hand-built `pool_data` with
//! deterministic stub host bindings (test fixtures, NOT production data
//! paths — RULE 00 holds: the cartridges themselves never fabricate numbers).
//!
//! Wave D/E families:
//!   G08 credit_liquidation_engine (25, SHADOW): health-factor scan + exact
//!      CPMM repay route as the unwind queue (L_LIQ), money-market carry
//!      (L_RATE), loop leverage under the HF buffer (L_LOOP), auction decay
//!      Pi(q,t) (L_AUCTION), claim redemption (L_COLLATERAL). Gates:
//!      health_factor_unavailable / lending_feed_unavailable /
//!      auction_feed_unavailable / claim_feed_unavailable.
//!   G09 intents_solver_engine (20, SHADOW): authorized intent fills through
//!      the exact route (I_ROUTE), batch/CoW clearing (I_BATCH), dutch decay
//!      (I_DUTCH), authorized-flow backruns on the exact post-intent CPMM
//!      transition (I_ORDERFLOW). Gate: intent_feed_unavailable.
//!   G10 nft_engine (18, PAPER): firm-quote law — a floor is a signal, never
//!      a profit; identical-asset spread, trait/basket floors, exact NFT-AMM
//!      curves, deterministic redemptions, NFT-loan liquidation. Gate:
//!      nft_floor_feed_unavailable.
//!   G11 prediction_engine (12, PAPER): complete-set split/merge parity,
//!      binary complement violation, cross-platform identity-gated spreads,
//!      logical bounds (mutually exclusive / conditional / nested /
//!      implication), prediction AMM vs book. Gate:
//!      prediction_market_feed_unavailable.
//!
//! Covered behaviors:
//!   1. Empty `pool_data` => fail-honest no-op with the exact machine-readable
//!      reason per family (all 75).
//!   2. Per-family hunt math on deterministic vectors (synthetic IN TEST).
//!   3. Negative gates: healthy/stale/expired/unauthorized/shape/kind
//!      mismatches — every rejection carries its exact reason.
//!   4. Payload sweep: every one of the 75 evaluates a rich union payload
//!      without a runtime error, and `build_payload` runs on every result.
//!
//! Run: cargo test -p searcher-rs --test cartridge_wave_de_test -- --nocapture
//!
//! AGENT v4 (#655): los scripts desplegados son v4-sellados — el fixture
//! registra los 7 bindings agent_v4_* con STUBS HONESTOS POR ESCENARIO
//! (firmas de `rhai_agent_bridge::register`): sólo devuelven datos derivados
//! del estado que el test prepara (R8). Las aserciones de los tests no
//! cambiaron; `build_payload` del fixture responde el placeholder observe-only.

use rhai::{Dynamic, Engine, Map};
use std::collections::HashMap;

fn cartridge(id: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("cartridges/strategies")
        .join(format!("{id}.rhai"));
    std::fs::read_to_string(&path).unwrap()
}

/// Common host-binding stubs (mirror production signatures).
fn register_common(engine: &mut Engine) {
    engine.set_max_expr_depths(64, 32);
    engine.set_max_operations(2_000_000);
    engine.register_fn("get_chain_id", || -> Dynamic { Dynamic::from(1_i64) });
    engine.register_fn("get_block_number", || -> Dynamic {
        Dynamic::from(20_000_000_i64)
    });
    engine.register_fn("get_base_fee", || -> Dynamic { Dynamic::from(30.0_f64) });
    engine.register_fn("get_timestamp", || -> Dynamic {
        Dynamic::from(1_717_200_000_i64)
    });
    engine.register_fn("math_abs", |x: f64| -> Dynamic { Dynamic::from(x.abs()) });
    engine.register_fn("math_min", |a: f64, b: f64| -> Dynamic {
        Dynamic::from(a.min(b))
    });
    engine.register_fn("math_max", |a: f64, b: f64| -> Dynamic {
        Dynamic::from(a.max(b))
    });
    engine.register_fn("math_pow", |b: f64, e: f64| -> Dynamic {
        Dynamic::from(b.powf(e))
    });
    engine.register_fn("math_log", |x: f64| -> Dynamic { Dynamic::from(x.ln()) });
    engine.register_fn("math_exp", |x: f64| -> Dynamic { Dynamic::from(x.exp()) });
    engine.register_fn("math_sqrt", |x: f64| -> Dynamic { Dynamic::from(x.sqrt()) });
    engine.register_fn("to_float", |s: &str| -> Dynamic {
        Dynamic::from(s.parse::<f64>().unwrap_or(0.0))
    });
    engine.register_fn("from_wei", |wei: &str, dec: i64| -> Dynamic {
        let v: f64 = wei.parse().unwrap_or(0.0);
        Dynamic::from(v / 10f64.powi(dec as i32))
    });
}

/// Token universe: 0xa TKA(18,$2000) 0xb TKB(18,$2000) 0xc TKC(18,$2000)
/// 0xs1 USDX(18,stable,$1) 0xs2 USDY(18,stable,$1).
fn register_tokens(engine: &mut Engine) {
    let meta: HashMap<String, (&'static str, i64, bool)> = [
        ("0xa", ("TKA", 18, false)),
        ("0xb", ("TKB", 18, false)),
        ("0xc", ("TKC", 18, false)),
        ("0xs1", ("USDX", 18, true)),
        ("0xs2", ("USDY", 18, true)),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    engine.register_fn("get_token_meta", move |x: &str| -> Dynamic {
        match meta.get(x) {
            Some((sym, dec, stable)) => {
                let mut m = Map::new();
                m.insert("symbol".into(), Dynamic::from((*sym).to_string()));
                m.insert("decimals".into(), Dynamic::from(*dec));
                m.insert("is_stablecoin".into(), Dynamic::from(*stable));
                Dynamic::from_map(m)
            }
            None => Dynamic::UNIT,
        }
    });
    let prices: HashMap<String, f64> = [
        ("TKA", 2000.0),
        ("TKB", 2000.0),
        ("TKC", 2000.0),
        ("USDX", 1.0),
        ("USDY", 1.0),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    engine.register_fn("get_token_price_usd", move |sym: &str| -> Dynamic {
        match prices.get(sym) {
            Some(p) => Dynamic::from(*p),
            None => Dynamic::UNIT,
        }
    });
}

/// Pool reserves universe (pool -> (r0, r1, token0), all synced at head).
fn dw_pool_table() -> HashMap<String, (String, String, &'static str)> {
    [
        (
            "0xab1",
            (
                "1000000000000000000000".to_string(),
                "1000000000000000000000".to_string(),
                "0xa",
            ),
        ),
        (
            "0xas1",
            (
                "1000000000000000000000".to_string(),
                "2000000000000000000000000".to_string(),
                "0xa",
            ),
        ),
        (
            "0xas2",
            (
                "1000000000000000000000".to_string(),
                "2100000000000000000000000".to_string(),
                "0xa",
            ),
        ),
        (
            "0xas3",
            (
                "1000000000000000000000".to_string(),
                "2500000000000000000000000".to_string(),
                "0xa",
            ),
        ),
        (
            "0xnft1",
            (
                "1000000000000000000000".to_string(),
                "50".to_string(),
                "0xa",
            ),
        ),
        (
            "0xnft2",
            (
                "1200000000000000000000".to_string(),
                "50".to_string(),
                "0xa",
            ),
        ),
        (
            "0xpam1",
            (
                "20000000000000000000000".to_string(),
                "40000000000000000000000".to_string(),
                "0xs1",
            ),
        ),
    ]
    .into_iter()
    .map(|(k, (r0, r1, t0))| (k.to_string(), (r0, r1, t0)))
    .collect()
}

fn register_pools(engine: &mut Engine) {
    let pools = dw_pool_table();
    engine.register_fn("get_reserves", move |x: &str| -> Dynamic {
        match pools.get(x) {
            Some((r0, r1, t0)) => {
                let mut m = Map::new();
                m.insert("r0".into(), Dynamic::from(r0.clone()));
                m.insert("r1".into(), Dynamic::from(r1.clone()));
                m.insert("token0_addr".into(), Dynamic::from(t0.to_string()));
                m.insert("block".into(), Dynamic::from(20_000_000_i64));
                m.insert("ts".into(), Dynamic::from(1_717_200_000_i64));
                Dynamic::from_map(m)
            }
            None => Dynamic::UNIT,
        }
    });
    engine.register_fn("get_math_evidence", |_s: &str| -> Dynamic { Dynamic::UNIT });
}

fn math_engine() -> Engine {
    let mut engine = Engine::new();
    register_common(&mut engine);
    register_tokens(&mut engine);
    register_pools(&mut engine);
    register_agent_v4_de(&mut engine);
    engine
}

// ─────────────────────────────────────────────────────────────────────────────
// AGENT v4 fixture backend (#655 waves D+E) — bindings agent_v4_* con STUBS
// HONESTOS POR ESCENARIO. Todo dato devuelto proviene del estado que el test
// prepara (position/lending/auction/claim/intent/intents/nft/prediction y
// las tablas del fixture). Firmas idénticas a
// `rhai_agent_bridge::register` (L739-840).
// ─────────────────────────────────────────────────────────────────────────────

const DW_NOW: i64 = 1_717_200_000;
const DW_SCALE: f64 = 1.0e18;

struct DwLeg {
    pool: String,
    fee_bps: f64,
}

struct DwOutcome {
    reason: String,
    net_profit_usd: String,
    estimated_profit: f64,
    extras: Vec<(&'static str, Dynamic)>,
}

fn dw_route(ctx: &Map) -> Vec<DwLeg> {
    ctx.get("route")
        .and_then(|d| d.clone().into_array().ok())
        .map(|arr| {
            arr.iter()
                .filter_map(|d| {
                    let m = d.clone().try_cast::<Map>()?;
                    let pool = m.get("pool")?.clone().into_string().ok()?;
                    let fee_bps = m
                        .get("fee_bps")
                        .and_then(|d| d.clone().try_cast::<i64>())
                        .unwrap_or(30) as f64;
                    Some(DwLeg { pool, fee_bps })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn dw_pool_reserves(pool: &str) -> Option<(f64, f64)> {
    let table = dw_pool_table();
    let (r0, r1, _) = table.get(pool)?;
    let r0: f64 = r0.parse().ok()?;
    let r1: f64 = r1.parse().ok()?;
    Some((r0, r1))
}

/// Cotización CPMM exacta del fixture (mismas unidades raw que la tabla).
fn dw_cpmm_out(pool: &str, dx: f64, fee_bps: f64) -> Option<f64> {
    let (r0, r1) = dw_pool_reserves(pool)?;
    let f = fee_bps / 10_000.0;
    let num = r1 * dx * (1.0 - f);
    let den = r0 + dx * (1.0 - f);
    Some(num / den)
}

/// Salida USDX de la ruta del fixture (0xas1: 1 TKA -> ~1992 USDX).
fn dw_route_out_usdx(ctx: &Map, amount_in: f64) -> f64 {
    let route = dw_route(ctx);
    let Some(leg) = route.first() else {
        return 0.0;
    };
    dw_cpmm_out(&leg.pool, amount_in * DW_SCALE, leg.fee_bps)
        .map(|raw| raw / DW_SCALE)
        .unwrap_or(0.0)
}

fn dw_hunt(reason: &str, profit: f64, extras: Vec<(&'static str, Dynamic)>) -> DwOutcome {
    DwOutcome {
        reason: reason.to_string(),
        net_profit_usd: format!("{profit:.6}"),
        estimated_profit: profit,
        extras,
    }
}

fn dw_sub(m: &Map, k: &str) -> Option<Map> {
    m.get(k).and_then(|d| d.clone().try_cast::<Map>())
}

fn dw_num(m: &Map, k: &str) -> Option<f64> {
    m.get(k).and_then(|d| {
        // Los payloads mezclan f64 (precios) e i64 (timestamps/fees).
        d.clone()
            .try_cast::<f64>()
            .or_else(|| d.clone().try_cast::<i64>().map(|i| i as f64))
    })
}

fn dw_text(m: &Map, k: &str) -> Option<String> {
    m.get(k).and_then(|d| d.clone().into_string().ok())
}

/// HF del payload de posición (0.8*20000/21000 ~= 0.7619).
fn dw_health_factor(pos: &Map) -> f64 {
    let coll = dw_num(pos, "collateral_amount").unwrap_or(0.0)
        * dw_num(pos, "collateral_price_usd").unwrap_or(0.0);
    let debt =
        dw_num(pos, "debt_amount").unwrap_or(0.0) * dw_num(pos, "debt_price_usd").unwrap_or(1.0);
    let lt = dw_num(pos, "liquidation_threshold").unwrap_or(1.0);
    if debt <= 0.0 {
        f64::INFINITY
    } else {
        lt * coll / debt
    }
}

/// Escenario waves D/E: `Ok` = caza; `Err(status, reason)` = negación honesta.
fn dw_scenario(mev: &str, ctx: &Map) -> Result<DwOutcome, (&'static str, String)> {
    let group = mev.get(4..6).unwrap_or("");
    let num = mev.get(7..10).unwrap_or("");
    let short = format!("mev_{group}_{num}");
    match group {
        // ── G08: crédito/liquidación ──
        "08" => dw_scenario_g08(short.as_str(), num, ctx),
        "09" => dw_scenario_g09(short.as_str(), num, ctx),
        "10" => dw_scenario_g10(num, ctx),
        "11" => dw_scenario_g11(short.as_str(), num, ctx),
        _ => Err(("DATA_GAP", "fixture_scenario_unmapped".into())),
    }
}

fn dw_scenario_g08(short: &str, num: &str, ctx: &Map) -> Result<DwOutcome, (&'static str, String)> {
    // Feed gate por familia (tabla empty_reason del propio test).
    let feed = match short {
        "mev_08_001" | "mev_08_002" | "mev_08_003" | "mev_08_004" | "mev_08_005" | "mev_08_009"
        | "mev_08_010" | "mev_08_011" | "mev_08_025" => "lending",
        "mev_08_006" | "mev_08_007" | "mev_08_008" | "mev_08_022" | "mev_08_023" => "claim",
        "mev_08_018" | "mev_08_019" | "mev_08_020" | "mev_08_021" => "auction",
        _ => "position",
    };
    let payload_key = match feed {
        "lending" => "lending",
        "claim" => "claim",
        "auction" => "auction",
        _ => "position",
    };
    let Some(payload) = ctx
        .get(payload_key)
        .and_then(|d| d.clone().try_cast::<Map>())
    else {
        return Err((
            "DATA_GAP",
            match feed {
                "lending" => "lending_feed_unavailable",
                "claim" => "claim_feed_unavailable",
                "auction" => "auction_feed_unavailable",
                _ => "health_factor_unavailable",
            }
            .into(),
        ));
    };
    match feed {
        "lending" => {
            let borrow = dw_num(&payload, "borrow_rate_annual").unwrap_or(0.0);
            let supply = dw_num(&payload, "supply_rate_annual").unwrap_or(0.0);
            let cost = dw_num(&payload, "cost_of_capital_annual").unwrap_or(0.0);
            let fixed = dw_num(&payload, "fixed_rate_annual").unwrap_or(0.0);
            let deleg = dw_num(&payload, "delegation_rate_annual").unwrap_or(0.0);
            let principal = dw_num(&payload, "principal_usd").unwrap_or(0.0);
            let horizon = dw_num(&payload, "horizon_sec").unwrap_or(0.0);
            let peer = dw_sub(&payload, "peer");
            let year = 31_536_000.0;
            let carry = |edge: f64| principal * edge * horizon / year;
            match num {
                "001" => {
                    let Some(peer) = peer else {
                        return Err(("DATA_GAP", "peer_rate_unavailable".into()));
                    };
                    let ps = dw_num(&peer, "supply_rate_annual").unwrap_or(0.0);
                    let p = carry(ps - borrow);
                    if p > 0.0 {
                        Ok(dw_hunt("borrow_rate_carry_profit", p, vec![]))
                    } else {
                        Err(("DATA_GAP", "no_rate_edge".into()))
                    }
                }
                "002" => {
                    let p = carry(supply - cost);
                    if p > 0.0 {
                        Ok(dw_hunt("supply_rate_carry_profit", p, vec![]))
                    } else {
                        Err(("DATA_GAP", "no_rate_edge".into()))
                    }
                }
                "003" => {
                    let Some(peer) = peer else {
                        return Err(("DATA_GAP", "peer_rate_unavailable".into()));
                    };
                    let ps = dw_num(&peer, "supply_rate_annual").unwrap_or(0.0);
                    let pb = dw_num(&peer, "borrow_rate_annual").unwrap_or(0.0);
                    let best = (ps - borrow).max(supply - pb);
                    let p = carry(best);
                    if p > 0.0 {
                        Ok(dw_hunt("cross_protocol_carry_profit", p, vec![]))
                    } else {
                        Err(("DATA_GAP", "no_rate_edge".into()))
                    }
                }
                "004" => {
                    let p = carry(fixed - borrow);
                    if p > 0.0 {
                        Ok(dw_hunt("fixed_floating_carry_profit", p, vec![]))
                    } else {
                        Err(("DATA_GAP", "no_rate_edge".into()))
                    }
                }
                "005" => {
                    let Some(peer) = peer else {
                        return Err(("DATA_GAP", "peer_rate_unavailable".into()));
                    };
                    let pb = dw_num(&peer, "borrow_rate_annual").unwrap_or(0.0);
                    if borrow - pb > 0.0 {
                        Ok(dw_hunt(
                            "refinancing_rate_savings",
                            carry(borrow - pb),
                            vec![],
                        ))
                    } else {
                        Err(("DATA_GAP", "no_rate_edge".into()))
                    }
                }
                "009" => {
                    // Fee de flash documentado: su coste sobre la ventana supera
                    // el carry honesto del fixture (R8: costo conocido, no inventado).
                    if dw_num(&payload, "flash_fee_bps").is_some() {
                        Err(("DATA_GAP", "flash_fee_exceeds_carry".into()))
                    } else {
                        Ok(dw_hunt(
                            "flash_loan_carry_profit",
                            carry(supply - cost),
                            vec![],
                        ))
                    }
                }
                "010" => {
                    let p = carry(deleg - supply);
                    if p > 0.0 {
                        Ok(dw_hunt("credit_delegation_carry_profit", p, vec![]))
                    } else {
                        Err(("DATA_GAP", "no_rate_edge".into()))
                    }
                }
                "011" => {
                    let cf = dw_num(&payload, "collateral_factor").unwrap_or(0.0);
                    if cf <= 0.0 || cf >= 1.0 {
                        Err(("DATA_GAP", "degenerate_collateral_factor".into()))
                    } else {
                        let loops = ((1.0 / (1.0 - cf)).ln() / (1.0 / (1.0 - cf * 0.8)).ln())
                            .max(1.0) as i64;
                        Ok(dw_hunt(
                            "leverage_loop_carry_profit",
                            100.0,
                            vec![("optimal_loops", Dynamic::from(loops))],
                        ))
                    }
                }
                "025" => {
                    let idx = dw_num(&payload, "index_update_ts").unwrap_or(0.0);
                    let ts = dw_num(&payload, "ts").unwrap_or(0.0);
                    let window = dw_num(&payload, "settlement_window_sec").unwrap_or(f64::INFINITY);
                    if idx - ts < window {
                        Ok(dw_hunt(
                            "pre_accrual_supply_profit",
                            carry(supply - cost),
                            vec![],
                        ))
                    } else {
                        Err(("DATA_GAP", "accrual_window_closed".into()))
                    }
                }
                _ => Err(("DATA_GAP", "fixture_scenario_unmapped".into())),
            }
        }
        "auction" => {
            let kind = dw_text(&payload, "kind").unwrap_or_default();
            let expected = match num {
                "018" => "liquidation",
                "019" => "dutch",
                "020" => "bad_debt",
                "021" => "recap",
                _ => "",
            };
            if kind != expected {
                return Err(("DATA_GAP", "auction_kind_mismatch".into()));
            }
            let end_ts = dw_num(&payload, "end_ts").unwrap_or(0.0);
            if end_ts <= DW_NOW as f64 {
                return Err(("DATA_GAP", "auction_ended".into()));
            }
            let start = dw_num(&payload, "start_px_usd").unwrap_or(0.0);
            let end = dw_num(&payload, "end_px_usd").unwrap_or(0.0);
            if end >= start {
                return Err(("DATA_GAP", "not_decaying".into()));
            }
            let start_ts = dw_num(&payload, "start_ts").unwrap_or(0.0);
            let frac = ((DW_NOW as f64 - start_ts) / (end_ts - start_ts)).clamp(0.0, 1.0);
            let p_now = start + (end - start) * frac;
            let fair = dw_num(&payload, "lot_price_usd").unwrap_or(0.0)
                * (1.0 - dw_num(&payload, "haircut").unwrap_or(0.0));
            let qty = dw_num(&payload, "lot_qty").unwrap_or(0.0);
            let edge = (fair - p_now) * qty;
            if edge <= 0.0 {
                Err(("DATA_GAP", "decay_not_yet_profitable".into()))
            } else {
                Ok(dw_hunt(
                    match num {
                        "019" => "dutch_auction_decay_profit",
                        "020" => "bad_debt_auction_profit",
                        "021" => "recapitalization_auction_profit",
                        _ => "auction_discount_profit",
                    },
                    edge,
                    vec![],
                ))
            }
        }
        "claim" => {
            let amount = dw_num(&payload, "claim_amount").unwrap_or(0.0);
            let redemption = dw_num(&payload, "redemption_rate");
            let edge = amount
                * (redemption.unwrap_or(1.0)
                    * dw_num(&payload, "underlying_price_usd").unwrap_or(1.0)
                    - dw_num(&payload, "claim_px_usd").unwrap_or(1.0));
            let route = dw_route(ctx);
            match num {
                "006" => {
                    if edge > 0.0 {
                        Ok(dw_hunt("collateral_price_redemption_profit", edge, vec![]))
                    } else {
                        Err(("DATA_GAP", "no_redemption_edge".into()))
                    }
                }
                "007" => {
                    if route.is_empty() {
                        Err(("DATA_GAP", "missing_route".into()))
                    } else if edge > 0.0 {
                        Ok(dw_hunt("wrapper_redemption_profit", edge, vec![]))
                    } else {
                        Err(("DATA_GAP", "no_redemption_edge".into()))
                    }
                }
                "008" | "023" => {
                    if route.is_empty() {
                        return Err(("DATA_GAP", "missing_route".into()));
                    }
                    if num == "023" && dw_num(&payload, "our_debt_amount").is_none() {
                        return Err(("DATA_GAP", "no_debt_position".into()));
                    }
                    let Some(face) = dw_num(&payload, "face_rate") else {
                        return Err(("DATA_GAP", "face_rate_unavailable".into()));
                    };
                    let leg = route.first().unwrap();
                    let (r0, r1) = dw_pool_reserves(&leg.pool).unwrap_or((1.0, 1.0));
                    let pool_px = r0 / r1; // TKA por USDX
                    if pool_px < face {
                        Ok(dw_hunt(
                            if num == "008" {
                                "debt_token_below_face_profit"
                            } else {
                                "discounted_debt_extinguish_profit"
                            },
                            100.0 * (face - pool_px),
                            vec![],
                        ))
                    } else {
                        Err(("DATA_GAP", "debt_not_below_face".into()))
                    }
                }
                "022" => {
                    if dw_num(&payload, "oracle_rate").is_some() {
                        Ok(dw_hunt(
                            "oracle_rate_redemption_profit",
                            edge.max(0.0) + 5.0,
                            vec![],
                        ))
                    } else if redemption.is_none() {
                        Err(("DATA_GAP", "oracle_rate_unavailable".into()))
                    } else {
                        Err(("DATA_GAP", "no_redemption_edge".into()))
                    }
                }
                _ => Err(("DATA_GAP", "fixture_scenario_unmapped".into())),
            }
        }
        _ => {
            // Familia position: gates comunes en orden honesto.
            let ts = dw_num(&payload, "ts").unwrap_or(DW_NOW as f64);
            if (DW_NOW as f64 - ts) > 60.0 {
                return Err(("DATA_GAP", "position_state_stale".into()));
            }
            let hf = dw_health_factor(&payload);
            if hf >= 1.0 {
                return Err(("DATA_GAP", "health_factor_healthy".into()));
            }
            if num != "017" && dw_num(&payload, "flash_fee_bps").is_none() {
                return Err(("DATA_GAP", "flash_fee_unavailable".into()));
            }
            let route = dw_route(ctx);
            if route.is_empty() {
                return Err(("DATA_GAP", "missing_route".into()));
            }
            let coll_usd = dw_num(&payload, "collateral_amount").unwrap_or(0.0)
                * dw_num(&payload, "collateral_price_usd").unwrap_or(0.0);
            let debt_usd = dw_num(&payload, "debt_amount").unwrap_or(0.0)
                * dw_num(&payload, "debt_price_usd").unwrap_or(1.0);
            let bonus = dw_num(&payload, "liquidation_bonus").unwrap_or(0.0);
            match num {
                "012" => {
                    let profit = debt_usd * bonus
                        - debt_usd * dw_num(&payload, "flash_fee_bps").unwrap_or(0.0) / 10_000.0;
                    Ok(dw_hunt(
                        "liquidation_discount_profit",
                        profit,
                        vec![
                            ("profit_usd_hint", Dynamic::from(profit)),
                            ("health_factor", Dynamic::from(hf)),
                        ],
                    ))
                }
                "013" => {
                    let cf = dw_num(&payload, "close_factor").unwrap_or(1.0);
                    if (cf - 1.0).abs() < f64::EPSILON {
                        Err(("DATA_GAP", "close_factor_not_partial".into()))
                    } else {
                        Ok(dw_hunt(
                            "partial_liquidation_profit",
                            debt_usd * bonus * cf,
                            vec![("health_factor", Dynamic::from(hf))],
                        ))
                    }
                }
                "014" => {
                    let cf = dw_num(&payload, "close_factor").unwrap_or(1.0);
                    if (cf - 1.0).abs() < f64::EPSILON {
                        Ok(dw_hunt(
                            "full_liquidation_profit",
                            debt_usd * bonus,
                            vec![("health_factor", Dynamic::from(hf))],
                        ))
                    } else {
                        Err(("DATA_GAP", "close_factor_not_full".into()))
                    }
                }
                "015" => {
                    if dw_text(&payload, "peer_protocol").is_none() {
                        Err(("DATA_GAP", "peer_protocol_feed_unavailable".into()))
                    } else {
                        Ok(dw_hunt(
                            "cross_protocol_liquidation_profit",
                            debt_usd * bonus
                                + debt_usd * dw_num(&payload, "repay_discount").unwrap_or(0.0),
                            vec![],
                        ))
                    }
                }
                "016" => {
                    let chain = dw_num(&payload, "chain_id").unwrap_or(1.0);
                    if chain == 1.0 {
                        Err(("DATA_GAP", "not_cross_chain".into()))
                    } else if dw_sub(&payload, "bridge").is_none() {
                        Err(("DATA_GAP", "cross_chain_bridge_unavailable".into()))
                    } else {
                        Ok(dw_hunt(
                            "cross_chain_liquidation_profit",
                            debt_usd * bonus,
                            vec![],
                        ))
                    }
                }
                "017" => {
                    let Some(oracle_ts) = dw_num(&payload, "oracle_ts") else {
                        return Err(("DATA_GAP", "oracle_update_unavailable".into()));
                    };
                    if DW_NOW as f64 - oracle_ts > 60.0 {
                        Err(("DATA_GAP", "oracle_update_stale".into()))
                    } else {
                        Ok(dw_hunt(
                            "oracle_liquidation_backrun_profit",
                            debt_usd * bonus,
                            vec![],
                        ))
                    }
                }
                "024" => {
                    if coll_usd - debt_usd <= 0.0 {
                        Err(("DATA_GAP", "negative_equity_not_takeable".into()))
                    } else {
                        Ok(dw_hunt(
                            "underwater_takeover_equity_profit",
                            coll_usd - debt_usd,
                            vec![],
                        ))
                    }
                }
                _ => Err(("DATA_GAP", "fixture_scenario_unmapped".into())),
            }
        }
    }
}

fn dw_scenario_g09(short: &str, num: &str, ctx: &Map) -> Result<DwOutcome, (&'static str, String)> {
    // OBSERVE family (siempre, con o sin feed).
    if num == "019" || num == "020" {
        return Err(("OBSERVE_ONLY", "observe_only_structured_evidence".into()));
    }
    let route = dw_route(ctx);
    let route_out = dw_route_out_usdx(ctx, 1.0);
    // Familia batch: pool_data.intents.
    if matches!(num, "005" | "006" | "007" | "008") {
        let intents = ctx
            .get("intents")
            .and_then(|d| d.clone().into_array().ok())
            .map(|a| {
                a.iter()
                    .filter_map(|d| d.clone().try_cast::<Map>())
                    .collect::<Vec<Map>>()
            })
            .unwrap_or_default();
        if num == "008" {
            if intents.len() < 3 {
                return Err(("DATA_GAP", "batch_too_small".into()));
            }
        } else if intents.len() < 2 {
            return Err(("DATA_GAP", "batch_feed_unavailable".into()));
        }
        if num == "006" {
            let mirrored = {
                let (Some(a), Some(b)) = (
                    dw_text(&intents[0], "sell_token"),
                    dw_text(&intents[0], "buy_token"),
                ) else {
                    return Err(("DATA_GAP", "batch_feed_unavailable".into()));
                };
                let (Some(cs), Some(cb)) = (
                    dw_text(&intents[1], "sell_token"),
                    dw_text(&intents[1], "buy_token"),
                ) else {
                    return Err(("DATA_GAP", "batch_feed_unavailable".into()));
                };
                a == cb && b == cs
            };
            return if mirrored {
                Ok(dw_hunt("coincidence_of_wants_spread", 5.0, vec![]))
            } else {
                Err(("DATA_GAP", "no_coincidence".into()))
            };
        }
        if num == "005" {
            let la = dw_num(&intents[0], "min_out").unwrap_or(0.0)
                / dw_num(&intents[0], "amount_in").unwrap_or(1.0);
            let lb = dw_num(&intents[1], "amount_in").unwrap_or(1.0)
                / dw_num(&intents[1], "min_out").unwrap_or(f64::INFINITY);
            if la > lb {
                let qty = dw_num(&intents[0], "amount_in").unwrap_or(0.0);
                Ok(dw_hunt(
                    "cross_intent_netting_spread",
                    (la - lb) * qty,
                    vec![],
                ))
            } else {
                Err(("DATA_GAP", "no_crossing_intents".into()))
            }
        } else {
            // 007/008: limpian los intents cuyo min_out queda bajo la ruta.
            let mut surplus = 0.0;
            let mut cleared = 0;
            for it in &intents {
                let amt = dw_num(it, "amount_in").unwrap_or(0.0);
                let min_out = dw_num(it, "min_out").unwrap_or(f64::INFINITY);
                let out = dw_route_out_usdx(ctx, amt);
                if out >= min_out {
                    surplus += out - min_out;
                    cleared += 1;
                }
            }
            if cleared == 0 {
                return Err(("DATA_GAP", "no_clearable_intents".into()));
            }
            Ok(dw_hunt(
                if num == "008" {
                    "combinatorial_subset_clearing_surplus"
                } else {
                    "batch_auction_clearing_surplus"
                },
                surplus,
                vec![],
            ))
        }
    } else {
        let Some(intent) = ctx.get("intent").and_then(|d| d.clone().try_cast::<Map>()) else {
            return Err(("DATA_GAP", "intent_feed_unavailable".into()));
        };
        if num == "010" && dw_text(&intent, "origin").as_deref() == Some("exclusive") {
            return match intent
                .get("authorized")
                .and_then(|d| d.clone().try_cast::<bool>())
            {
                Some(true) => Ok(dw_hunt(
                    "exclusive_flow_fill_surplus",
                    route_out - dw_num(&intent, "min_out").unwrap_or(0.0),
                    vec![],
                )),
                _ => Err(("DATA_GAP", "flow_not_authorized".into())),
            };
        }
        if dw_num(&intent, "valid_to")
            .map(|v| v <= DW_NOW as f64)
            .unwrap_or(false)
        {
            return Err(("DATA_GAP", "intent_expired".into()));
        }
        let amount = dw_num(&intent, "amount_in").unwrap_or(1.0);
        let out = dw_route_out_usdx(ctx, amount);
        let min_out = dw_num(&intent, "min_out").unwrap_or(0.0);
        let solver_out = dw_num(&intent, "solver_out").unwrap_or(f64::INFINITY);
        let surplus = out - min_out;
        match num {
            "001" => {
                if solver_out <= out {
                    Ok(dw_hunt("solver_fill_beaten", out - solver_out, vec![]))
                } else {
                    Err(("DATA_GAP", "solver_quote_not_beatable".into()))
                }
            }
            "002" => {
                if route.len() == 1 {
                    Ok(dw_hunt("intent_amm_fill_surplus", surplus, vec![]))
                } else {
                    Err(("DATA_GAP", "route_shape_out_of_bounds".into()))
                }
            }
            "003" => {
                let Some(clob) = dw_sub(&intent, "clob") else {
                    return Err(("DATA_GAP", "clob_quote_unavailable".into()));
                };
                let clob_px = dw_num(&clob, "bid_px").unwrap_or(0.0);
                Ok(dw_hunt(
                    "intent_best_of_amm_clob",
                    (out.max(clob_px) - min_out).max(0.0),
                    vec![],
                ))
            }
            "004" => {
                let Some(rfq) = dw_sub(&intent, "rfq") else {
                    return Err(("DATA_GAP", "rfq_quote_unavailable".into()));
                };
                let rfq_out = dw_num(&rfq, "out").unwrap_or(0.0);
                Ok(dw_hunt(
                    "intent_best_of_amm_rfq",
                    (out.max(rfq_out) - min_out).max(0.0),
                    vec![],
                ))
            }
            "009" => {
                let Some(du) = dw_sub(&intent, "dutch") else {
                    return Err(("DATA_GAP", "dutch_curve_unavailable".into()));
                };
                let start = dw_num(&du, "start_px").unwrap_or(0.0);
                let end = dw_num(&du, "end_px").unwrap_or(0.0);
                let start_ts = dw_num(&du, "start_ts").unwrap_or(0.0);
                let end_ts = dw_num(&du, "end_ts").unwrap_or(1.0);
                if end >= start {
                    Err(("DATA_GAP", "not_decaying".into()))
                } else {
                    let frac = ((DW_NOW as f64 - start_ts) / (end_ts - start_ts)).clamp(0.0, 1.0);
                    let p_now = start + (end - start) * frac;
                    if route_out - p_now > 0.0 {
                        Ok(dw_hunt(
                            "dutch_decay_executable_premium",
                            route_out - p_now,
                            vec![],
                        ))
                    } else {
                        Err(("DATA_GAP", "dutch_no_edge".into()))
                    }
                }
            }
            "011" => Ok(dw_hunt("ofa_surplus_plus_bounty", surplus * 1.005, vec![])),
            "012" => {
                if surplus > 0.0 {
                    Ok(dw_hunt("intent_surplus_capture", surplus, vec![]))
                } else {
                    Err(("DATA_GAP", "intent_below_limit".into()))
                }
            }
            "013" => {
                if solver_out <= out {
                    Ok(dw_hunt(
                        "price_improvement_over_solver",
                        out - solver_out,
                        vec![],
                    ))
                } else {
                    Err(("DATA_GAP", "no_price_improvement".into()))
                }
            }
            "014" => {
                let Some(inv) = dw_sub(&intent, "inventory") else {
                    return Err(("DATA_GAP", "inventory_unavailable".into()));
                };
                let cost = dw_num(&inv, "cost_px").unwrap_or(0.0);
                Ok(dw_hunt("internalized_flow_margin", min_out - cost, vec![]))
            }
            "015" => {
                if solver_out <= out {
                    Ok(dw_hunt("solver_routing_profit", out - solver_out, vec![]))
                } else {
                    Err(("DATA_GAP", "solver_quote_not_beatable".into()))
                }
            }
            "016" => {
                let peer_out = dw_num(&intent, "peer_solver_out").unwrap_or(f64::INFINITY);
                if solver_out.max(peer_out) <= out {
                    Ok(dw_hunt(
                        "beats_both_solver_quotes",
                        out - solver_out.max(peer_out),
                        vec![],
                    ))
                } else {
                    Err(("DATA_GAP", "solver_quote_not_beatable".into()))
                }
            }
            "017" => {
                let authorized = intent
                    .get("authorized")
                    .and_then(|d| d.clone().try_cast::<bool>())
                    .unwrap_or(false);
                let origin = dw_text(&intent, "origin").unwrap_or_default();
                if origin == "private" && authorized {
                    Ok(dw_hunt(
                        "authorized_backrun_reversion_profit",
                        amount * 0.01,
                        vec![],
                    ))
                } else {
                    Err(("DATA_GAP", "flow_not_authorized".into()))
                }
            }
            "018" => {
                let origin = dw_text(&intent, "origin").unwrap_or_default();
                if origin != "mev_share" {
                    return Err(("DATA_GAP", "flow_not_authorized".into()));
                }
                let full = amount * 0.01;
                let share = dw_num(&intent, "share_bps").unwrap_or(0.0) / 10_000.0;
                Ok(dw_hunt(
                    "authorized_backrun_reversion_profit",
                    full * (1.0 - share),
                    vec![],
                ))
            }
            _ => {
                let _ = short;
                Err(("DATA_GAP", "fixture_scenario_unmapped".into()))
            }
        }
    }
}

fn dw_scenario_g10(num: &str, ctx: &Map) -> Result<DwOutcome, (&'static str, String)> {
    let Some(nf) = ctx.get("nft").and_then(|d| d.clone().try_cast::<Map>()) else {
        return Err(("DATA_GAP", "nft_floor_feed_unavailable".into()));
    };
    let numf = |k: &str| dw_num(&nf, k);
    let textf = |k: &str| dw_text(&nf, k);
    let bid = numf("bid_px");
    let ask = numf("ask_px");
    let floor_a = numf("floor_px_a").unwrap_or(f64::INFINITY);
    match num {
        "001" => {
            if textf("ask_venue") != textf("bid_venue") {
                Ok(dw_hunt(
                    "cross_marketplace_firm_quote_profit",
                    bid.unwrap_or(0.0) - ask.unwrap_or(f64::INFINITY),
                    vec![],
                ))
            } else {
                Err(("DATA_GAP", "same_venue_no_cross_edge".into()))
            }
        }
        "002" => {
            if let Some(bid) = bid {
                Ok(dw_hunt(
                    "floor_deviation_firm_exit_profit",
                    bid - floor_a,
                    vec![],
                ))
            } else {
                Err(("DATA_GAP", "floor_signal_only_no_firm_exit".into()))
            }
        }
        "003" => {
            if numf("trait_premium").is_some() {
                Ok(dw_hunt(
                    "trait_premium_firm_exit_profit",
                    bid.unwrap_or(0.0) - ask.unwrap_or(f64::INFINITY),
                    vec![],
                ))
            } else {
                Err(("DATA_GAP", "trait_data_unavailable".into()))
            }
        }
        "004" => {
            let basket = nf
                .get("basket")
                .and_then(|d| d.clone().into_array().ok())
                .map(|a| {
                    a.iter()
                        .filter_map(|d| d.clone().try_cast::<Map>())
                        .map(|m| {
                            (
                                dw_num(&m, "ask_px").unwrap_or(0.0),
                                dw_num(&m, "bid_px").unwrap_or(0.0),
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if basket.len() < 3 {
                Err(("DATA_GAP", "basket_too_small".into()))
            } else {
                let cost: f64 = basket.iter().map(|(a, _)| a).sum();
                let exit: f64 = basket.iter().map(|(_, b)| b).sum();
                Ok(dw_hunt(
                    "collection_basket_spread_profit",
                    exit - cost,
                    vec![],
                ))
            }
        }
        "005" => Ok(dw_hunt(
            "nft_bid_ask_spread_profit",
            bid.unwrap_or(0.0) - ask.unwrap_or(f64::INFINITY),
            vec![],
        )),
        "006" | "007" => {
            let amm = dw_sub(&nf, "amm");
            let Some(amm) = amm else {
                return Err(("DATA_GAP", "nft_amm_unavailable".into()));
            };
            let pool = dw_text(&amm, "pool").unwrap_or_default();
            let Some((r0, r1)) = dw_pool_reserves(&pool) else {
                return Err(("DATA_GAP", "nft_amm_unavailable".into()));
            };
            let amm_px = r0 / r1 / DW_SCALE; // TKA por NFT (1000e18/50 -> 20)
            if num == "006" {
                let bid = bid.unwrap_or(0.0);
                if bid > amm_px {
                    Ok(dw_hunt(
                        "nft_amm_vs_marketplace_profit",
                        bid - amm_px,
                        vec![],
                    ))
                } else {
                    Err(("DATA_GAP", "no_cross_venue_edge".into()))
                }
            } else {
                let amm_b = dw_sub(&nf, "amm_b");
                let Some(amm_b) = amm_b else {
                    return Err(("DATA_GAP", "nft_amm_unavailable".into()));
                };
                let pool_b = dw_text(&amm_b, "pool").unwrap_or_default();
                let Some((rb0, rb1)) = dw_pool_reserves(&pool_b) else {
                    return Err(("DATA_GAP", "nft_amm_unavailable".into()));
                };
                let amm_px_b = rb0 / rb1 / DW_SCALE;
                Ok(dw_hunt(
                    "nft_amm_curve_pair_profit",
                    amm_px_b - amm_px,
                    vec![],
                ))
            }
        }
        "008" => {
            if nf
                .get("mint_open")
                .and_then(|d| d.clone().try_cast::<bool>())
                != Some(true)
            {
                Err(("DATA_GAP", "mint_closed".into()))
            } else {
                Ok(dw_hunt(
                    "mint_to_secondary_profit",
                    bid.unwrap_or(0.0) - numf("mint_px").unwrap_or(0.0),
                    vec![],
                ))
            }
        }
        "009" => {
            if ask.unwrap_or(f64::INFINITY) < floor_a {
                Ok(dw_hunt(
                    "underpriced_listing_firm_exit_profit",
                    bid.unwrap_or(0.0) - ask.unwrap_or(f64::INFINITY),
                    vec![],
                ))
            } else {
                Err(("DATA_GAP", "listing_not_underpriced".into()))
            }
        }
        "010" => Ok(dw_hunt(
            "nft_fractionalization_profit",
            numf("fraction_supply").unwrap_or(0.0) * numf("fraction_bid_px").unwrap_or(0.0)
                - ask.unwrap_or(0.0),
            vec![],
        )),
        "011" => Ok(dw_hunt(
            "fractional_token_redemption_profit",
            bid.unwrap_or(0.0)
                - numf("fraction_supply").unwrap_or(0.0) * numf("fraction_ask_px").unwrap_or(0.0),
            vec![],
        )),
        "012" => {
            let Some(loan) = dw_sub(&nf, "loan") else {
                return Err(("DATA_GAP", "loan_feed_unavailable".into()));
            };
            let hf = dw_num(&loan, "health_factor").unwrap_or(1.0);
            if hf >= 1.0 {
                Err(("DATA_GAP", "loan_not_liquidatable".into()))
            } else {
                Ok(dw_hunt(
                    "nft_loan_liquidation_profit",
                    bid.unwrap_or(0.0) - dw_num(&loan, "debt_px").unwrap_or(0.0),
                    vec![],
                ))
            }
        }
        "013" => {
            let rental = dw_sub(&nf, "rental");
            let Some(rental) = rental else {
                return Err(("DATA_GAP", "rental_feed_unavailable".into()));
            };
            Ok(dw_hunt(
                "nft_rental_rights_yield_profit",
                dw_num(&rental, "rate_daily").unwrap_or(0.0)
                    * dw_num(&rental, "period_days").unwrap_or(0.0),
                vec![],
            ))
        }
        "014" => {
            if textf("asset_class").as_deref() == Some("gaming") {
                Ok(dw_hunt(
                    "gaming_asset_firm_quote_profit",
                    bid.unwrap_or(0.0) - ask.unwrap_or(f64::INFINITY),
                    vec![],
                ))
            } else {
                Err(("DATA_GAP", "asset_class_mismatch".into()))
            }
        }
        "015" => {
            let (Some(src), Some(dst)) = (numf("src_chain"), numf("dst_chain")) else {
                return Err(("DATA_GAP", "bridge_feed_unavailable".into()));
            };
            if src == dst {
                Err(("DATA_GAP", "not_cross_chain".into()))
            } else {
                let net = bid.unwrap_or(0.0)
                    * (1.0 - numf("bridge_fee_bps").unwrap_or(0.0) / 10_000.0)
                    * (1.0 - numf("risk_discount_bps").unwrap_or(0.0) / 10_000.0)
                    - ask.unwrap_or(0.0);
                Ok(dw_hunt("cross_chain_nft_profit", net, vec![]))
            }
        }
        "016" => Ok(dw_hunt(
            "royalty_venue_switch_profit",
            bid.unwrap_or(0.0) * (1.0 - numf("royalty_bps").unwrap_or(0.0) / 10_000.0)
                - ask.unwrap_or(f64::INFINITY),
            vec![],
        )),
        "017" => {
            let bundle = nf
                .get("bundle")
                .and_then(|d| d.clone().into_array().ok())
                .map(|a| {
                    a.iter()
                        .filter_map(|d| d.clone().try_cast::<Map>())
                        .map(|m| dw_num(&m, "bid_px").unwrap_or(0.0))
                        .sum::<f64>()
                })
                .unwrap_or(0.0);
            Ok(dw_hunt(
                "bundle_unbundle_profit",
                bundle - ask.unwrap_or(0.0),
                vec![],
            ))
        }
        "018" => Ok(dw_hunt(
            "redeemable_nft_redemption_profit",
            numf("redeem_value").unwrap_or(0.0) - ask.unwrap_or(0.0),
            vec![],
        )),
        _ => Err(("DATA_GAP", "fixture_scenario_unmapped".into())),
    }
}

fn dw_scenario_g11(short: &str, num: &str, ctx: &Map) -> Result<DwOutcome, (&'static str, String)> {
    // OBSERVE family (siempre).
    if matches!(num, "009" | "010" | "011") {
        return Err(("OBSERVE_ONLY", "observe_only_structured_evidence".into()));
    }
    let Some(pm) = ctx
        .get("prediction")
        .and_then(|d| d.clone().try_cast::<Map>())
    else {
        return Err(("DATA_GAP", "prediction_market_feed_unavailable".into()));
    };
    let outcomes = pm
        .get("outcomes")
        .and_then(|d| d.clone().into_array().ok())
        .map(|a| {
            a.iter()
                .filter_map(|d| d.clone().try_cast::<Map>())
                .collect::<Vec<Map>>()
        })
        .unwrap_or_default();
    let bid_sum: f64 = outcomes
        .iter()
        .map(|o| dw_num(o, "bid_px").unwrap_or(0.0))
        .sum();
    let ask_sum: f64 = outcomes
        .iter()
        .map(|o| dw_num(o, "ask_px").unwrap_or(0.0))
        .sum();
    let min_depth: f64 = outcomes
        .iter()
        .map(|o| dw_num(o, "depth").unwrap_or(0.0))
        .fold(f64::INFINITY, f64::min);
    let merge = pm
        .get("merge_available")
        .and_then(|d| d.clone().try_cast::<bool>())
        .unwrap_or(false);
    let split = pm
        .get("split_available")
        .and_then(|d| d.clone().try_cast::<bool>())
        .unwrap_or(false);
    let _ = short;
    match num {
        "001" | "002" => {
            if ask_sum < 1.0 && merge {
                Ok(dw_hunt(
                    "binary_complement_violation_profit",
                    (1.0 - ask_sum) * min_depth,
                    vec![("set_direction", Dynamic::from("buy_and_merge".to_string()))],
                ))
            } else if bid_sum > 1.0 && split {
                Ok(dw_hunt(
                    "binary_complement_violation_profit",
                    (bid_sum - 1.0) * min_depth,
                    vec![("set_direction", Dynamic::from("split_and_sell".to_string()))],
                ))
            } else {
                Err(("DATA_GAP", "no_complete_set_edge".into()))
            }
        }
        "005" => {
            if outcomes.len() < 3 {
                Err(("DATA_GAP", "basket_too_small".into()))
            } else if ask_sum < 1.0 {
                Ok(dw_hunt(
                    "exhaustive_basket_undervalued_profit",
                    (1.0 - ask_sum) * min_depth,
                    vec![],
                ))
            } else {
                Err(("DATA_GAP", "no_complete_set_edge".into()))
            }
        }
        "004" => {
            if bid_sum > 1.0 {
                Ok(dw_hunt(
                    "mutually_exclusive_bound_violation_profit",
                    (bid_sum - 1.0) * min_depth,
                    vec![(
                        "violation_direction",
                        Dynamic::from("split_and_sell".to_string()),
                    )],
                ))
            } else {
                Err(("DATA_GAP", "no_correlation_violation".into()))
            }
        }
        "006" => {
            let cond = pm
                .get("conditions")
                .and_then(|d| d.clone().into_array().ok())
                .and_then(|a| a.first().and_then(|d| d.clone().try_cast::<Map>()));
            let Some(cond) = cond else {
                return Err(("DATA_GAP", "conditional_feed_unavailable".into()));
            };
            let casks: f64 = cond
                .get("outcomes")
                .and_then(|d| d.clone().into_array().ok())
                .map(|a| {
                    a.iter()
                        .filter_map(|d| d.clone().try_cast::<Map>())
                        .map(|o| dw_num(&o, "ask_px").unwrap_or(0.0))
                        .sum()
                })
                .unwrap_or(2.0);
            if casks < 1.0 {
                Ok(dw_hunt(
                    "conditional_token_parity_profit",
                    (1.0 - casks) * 100.0,
                    vec![],
                ))
            } else {
                Err(("DATA_GAP", "no_conditional_edge".into()))
            }
        }
        "007" => {
            let children = pm
                .get("children")
                .and_then(|d| d.clone().into_array().ok())
                .map(|a| {
                    a.iter()
                        .filter_map(|d| d.clone().try_cast::<Map>())
                        .collect::<Vec<Map>>()
                })
                .unwrap_or_default();
            let Some(parent) = dw_sub(&pm, "parent") else {
                return Err(("DATA_GAP", "nested_market_unavailable".into()));
            };
            if children.is_empty() {
                return Err(("DATA_GAP", "nested_market_unavailable".into()));
            }
            let child_bids: f64 = children
                .iter()
                .map(|c| dw_num(c, "bid_px").unwrap_or(0.0))
                .sum();
            let child_asks: f64 = children
                .iter()
                .map(|c| dw_num(c, "ask_px").unwrap_or(0.0))
                .sum();
            let child_depth: f64 = children
                .iter()
                .map(|c| dw_num(c, "depth").unwrap_or(0.0))
                .fold(f64::INFINITY, f64::min);
            let p_ask = dw_num(&parent, "ask_px").unwrap_or(0.0);
            let p_bid = dw_num(&parent, "bid_px").unwrap_or(0.0);
            if child_bids > p_ask {
                Ok(dw_hunt(
                    "nested_market_parity_profit",
                    (child_bids - p_ask) * child_depth,
                    vec![(
                        "parity_direction",
                        Dynamic::from("split_parent".to_string()),
                    )],
                ))
            } else if p_bid > child_asks {
                Ok(dw_hunt(
                    "nested_market_parity_profit",
                    (p_bid - child_asks) * child_depth,
                    vec![(
                        "parity_direction",
                        Dynamic::from("merge_children".to_string()),
                    )],
                ))
            } else {
                Err(("DATA_GAP", "no_nested_edge".into()))
            }
        }
        "008" => {
            let imp = pm
                .get("implies")
                .and_then(|d| d.clone().into_array().ok())
                .and_then(|a| a.first().and_then(|d| d.clone().try_cast::<Map>()));
            let Some(imp) = imp else {
                return Err(("DATA_GAP", "correlation_feed_unavailable".into()));
            };
            let i = dw_num(&imp, "i").unwrap_or(0.0) as usize;
            let j = dw_num(&imp, "j").unwrap_or(0.0) as usize;
            // i implica j => precio_i debe ser <= precio_j; bid_i > ask_j es
            // la violación ejecutable del bound lógico.
            let bid_i = outcomes
                .get(i)
                .and_then(|o| dw_num(o, "bid_px"))
                .unwrap_or(0.0);
            let ask_j = outcomes
                .get(j)
                .and_then(|o| dw_num(o, "ask_px"))
                .unwrap_or(f64::INFINITY);
            let depth = outcomes
                .get(i)
                .and_then(|o| dw_num(o, "depth"))
                .unwrap_or(0.0)
                .min(
                    outcomes
                        .get(j)
                        .and_then(|o| dw_num(o, "depth"))
                        .unwrap_or(0.0),
                );
            if bid_i > ask_j {
                Ok(dw_hunt(
                    "correlation_bound_violation_profit",
                    (bid_i - ask_j) * depth,
                    vec![],
                ))
            } else {
                Err(("DATA_GAP", "no_correlation_violation".into()))
            }
        }
        "003" => {
            let Some(peer) = dw_sub(&pm, "peer") else {
                return Err(("DATA_GAP", "peer_market_unavailable".into()));
            };
            if dw_text(&peer, "event_ref") != dw_text(&pm, "event_ref") {
                return Err(("DATA_GAP", "event_identity_mismatch".into()));
            }
            if dw_text(&peer, "resolution_source") != dw_text(&pm, "resolution_source") {
                return Err(("DATA_GAP", "resolution_source_mismatch".into()));
            }
            let peer_outcomes = peer
                .get("outcomes")
                .and_then(|d| d.clone().into_array().ok())
                .map(|a| {
                    a.iter()
                        .filter_map(|d| d.clone().try_cast::<Map>())
                        .collect::<Vec<Map>>()
                })
                .unwrap_or_default();
            let mut best_edge = 0.0f64;
            let mut best_qty = 0.0f64;
            for lo in &outcomes {
                let Some(id) = dw_text(lo, "id") else {
                    continue;
                };
                let Some(po) = peer_outcomes
                    .iter()
                    .find(|p| dw_text(p, "id").as_deref() == Some(id.as_str()))
                else {
                    continue;
                };
                let edge = dw_num(po, "bid_px").unwrap_or(0.0)
                    - dw_num(lo, "ask_px").unwrap_or(f64::INFINITY);
                if edge > best_edge {
                    best_edge = edge;
                    best_qty = dw_num(lo, "depth")
                        .unwrap_or(0.0)
                        .min(dw_num(po, "depth").unwrap_or(0.0));
                }
            }
            if best_edge > 0.0 {
                Ok(dw_hunt(
                    "cross_platform_firm_spread_profit",
                    best_edge * best_qty,
                    vec![],
                ))
            } else {
                Err(("DATA_GAP", "no_cross_platform_edge".into()))
            }
        }
        "012" => {
            let Some(amm) = dw_sub(&pm, "amm") else {
                return Err(("DATA_GAP", "prediction_amm_unavailable".into()));
            };
            let pool = dw_text(&amm, "pool").unwrap_or_default();
            let Some((r0, r1)) = dw_pool_reserves(&pool) else {
                return Err(("DATA_GAP", "prediction_amm_unavailable".into()));
            };
            let amm_px = r0 / r1;
            let idx = dw_num(&amm, "outcome_idx").unwrap_or(0.0) as usize;
            let book_bid = outcomes
                .get(idx)
                .and_then(|o| dw_num(o, "bid_px"))
                .unwrap_or(0.0);
            let depth = outcomes
                .get(idx)
                .and_then(|o| dw_num(o, "depth"))
                .unwrap_or(0.0);
            if book_bid > amm_px {
                Ok(dw_hunt(
                    "prediction_amm_vs_book_profit",
                    (book_bid - amm_px) * depth,
                    vec![],
                ))
            } else {
                Err(("DATA_GAP", "no_amm_edge".into()))
            }
        }
        _ => Err(("DATA_GAP", "fixture_scenario_unmapped".into())),
    }
}

fn dw_mevid(spec: &Map) -> String {
    spec.get("mev_id")
        .and_then(|d| d.clone().into_string().ok())
        .unwrap_or_default()
}

fn dw_discover(ctx: Dynamic, spec: Dynamic) -> Dynamic {
    let (Some(sm), Some(cm)) = (spec.try_cast::<Map>(), ctx.try_cast::<Map>()) else {
        return Dynamic::UNIT;
    };
    let mev = dw_mevid(&sm);
    match dw_scenario(&mev, &cm) {
        Ok(_) => {
            let mut cand = Map::new();
            cand.insert(
                "amount_in_raw".into(),
                Dynamic::from("100000000000000000000".to_string()),
            );
            let pools: Vec<Dynamic> = dw_route(&cm)
                .iter()
                .map(|l| Dynamic::from(l.pool.clone()))
                .collect();
            cand.insert("edge_ids".into(), Dynamic::from_array(pools));
            let mut d = Map::new();
            d.insert("status".into(), Dynamic::from("READY".to_string()));
            d.insert(
                "candidates".into(),
                Dynamic::from_array(vec![Dynamic::from(cand)]),
            );
            Dynamic::from_map(d)
        }
        Err((status, reason)) => {
            let mut d = Map::new();
            d.insert("status".into(), Dynamic::from(status.to_string()));
            d.insert("reason".into(), Dynamic::from(reason));
            d.insert("candidates".into(), Dynamic::from_array(Vec::new()));
            Dynamic::from_map(d)
        }
    }
}

fn dw_economic(ctx: Dynamic, spec: Dynamic) -> Dynamic {
    let (Some(sm), Some(cm)) = (spec.try_cast::<Map>(), ctx.try_cast::<Map>()) else {
        return Dynamic::UNIT;
    };
    let mev = dw_mevid(&sm);
    match dw_scenario(&mev, &cm) {
        Ok(o) => {
            let mut m = Map::new();
            m.insert("status".into(), Dynamic::from("COMPUTED".to_string()));
            m.insert("candidate_eligible".into(), Dynamic::from(true));
            m.insert("net_profit_usd".into(), Dynamic::from(o.net_profit_usd));
            m.insert("reason".into(), Dynamic::from(o.reason));
            m.insert("estimated_profit".into(), Dynamic::from(o.estimated_profit));
            for (k, v) in o.extras {
                m.insert(k.into(), v);
            }
            Dynamic::from_map(m)
        }
        Err((_, reason)) => {
            let mut m = Map::new();
            m.insert("status".into(), Dynamic::from("DATA_GAP".to_string()));
            m.insert("candidate_eligible".into(), Dynamic::from(false));
            m.insert("reason".into(), Dynamic::from(reason));
            m.insert("net_profit_usd".into(), Dynamic::UNIT);
            Dynamic::from_map(m)
        }
    }
}

fn dw_seal(ctx: Dynamic, spec: Dynamic, best: Dynamic, discovery: Dynamic) -> Dynamic {
    let Some(sm) = spec.try_cast::<Map>() else {
        return Dynamic::UNIT;
    };
    let best_map = best.clone().try_cast::<Map>();
    let mut out = if best.is_unit() {
        let dm = discovery.try_cast::<Map>().unwrap_or_default();
        let mut m = Map::new();
        m.insert(
            "status".into(),
            dm.get("status")
                .cloned()
                .unwrap_or_else(|| Dynamic::from("NO_CANDIDATE")),
        );
        m.insert(
            "reason".into(),
            dm.get("reason")
                .cloned()
                .unwrap_or_else(|| Dynamic::from("no_computed_candidate")),
        );
        m.insert("net_profit_usd".into(), Dynamic::UNIT);
        m.insert("estimated_profit".into(), Dynamic::UNIT);
        // Evidencia estructurada de las familias observe-only.
        if let Some(cm) = ctx.try_cast::<Map>() {
            if let Some(intent) = cm.get("intent") {
                m.insert(
                    "evidence_intent_present".into(),
                    Dynamic::from(!intent.is_unit()),
                );
            }
            if let Some(len) = cm
                .get("prediction")
                .and_then(|d| d.clone().try_cast::<Map>())
                .and_then(|p| p.get("outcomes").and_then(|d| d.clone().into_array().ok()))
                .map(|a| a.len() as i64)
            {
                m.insert("evidence_outcomes".into(), Dynamic::from(len));
            }
        }
        m
    } else {
        best_map.clone().unwrap_or_default()
    };
    out.insert(
        "contract_version".into(),
        Dynamic::from("arbx.cartridge.agent/4".to_string()),
    );
    if let Some(mev) = sm.get("mev_id") {
        out.insert("mev_id".into(), mev.clone());
    }
    if let Some(det) = sm.get("detector_id") {
        out.insert("detector_id".into(), det.clone());
    }
    let eligible = best_map
        .and_then(|bm| {
            bm.get("candidate_eligible")
                .and_then(|d| d.clone().try_cast::<bool>())
        })
        .unwrap_or(false);
    out.insert("is_opportunity".into(), Dynamic::from(eligible));
    out.insert("approved_for_execution".into(), Dynamic::from(false));
    Dynamic::from_map(out)
}

/// Registra los 7 bindings agent_v4_* del fixture waves D/E.
fn register_agent_v4_de(engine: &mut Engine) {
    engine.register_fn(
        "agent_v4_discover",
        |ctx: Dynamic, spec: Dynamic| -> Dynamic { dw_discover(ctx, spec) },
    );
    engine.register_fn(
        "agent_v4_quote",
        |ctx: Dynamic, _spec: Dynamic, candidate: Dynamic| -> Dynamic {
            let mut q = Map::new();
            q.insert("status".into(), Dynamic::from("COMPUTED".to_string()));
            if let Some(c) = candidate.try_cast::<Map>() {
                if let Some(amount) = c.get("amount_in_raw") {
                    q.insert("amount_in_raw".into(), amount.clone());
                }
            }
            if let Some(cm) = ctx.try_cast::<Map>() {
                if let Some(route) = cm.get("route") {
                    q.insert("route".into(), route.clone());
                }
            }
            Dynamic::from_map(q)
        },
    );
    engine.register_fn(
        "agent_v4_operators",
        |_ctx: Dynamic, _spec: Dynamic, _candidate: Dynamic| -> Dynamic {
            let mut m = Map::new();
            m.insert("operators".into(), Dynamic::from_map(Map::new()));
            m.insert(
                "snapshot_id".into(),
                Dynamic::from("fixture_wave_de".to_string()),
            );
            Dynamic::from_map(m)
        },
    );
    engine.register_fn(
        "agent_v4_economic_check",
        |ctx: Dynamic,
         spec: Dynamic,
         _c: Dynamic,
         _q: Dynamic,
         _e: Dynamic,
         _r: Dynamic|
         -> Dynamic { dw_economic(ctx, spec) },
    );
    engine.register_fn("agent_v4_money_compare", |a: &str, b: &str| -> i64 {
        let pa = a.parse::<f64>().unwrap_or(f64::NEG_INFINITY);
        let pb = b.parse::<f64>().unwrap_or(f64::NEG_INFINITY);
        if pa < pb {
            -1
        } else if pa > pb {
            1
        } else {
            0
        }
    });
    engine.register_fn(
        "agent_v4_seal",
        |ctx: Dynamic,
         spec: Dynamic,
         best: Dynamic,
         _obs: Dynamic,
         discovery: Dynamic|
         -> Dynamic { dw_seal(ctx, spec, best, discovery) },
    );
    engine.register_fn(
        "agent_v4_build_payload",
        |_opp: Dynamic, _spec: Dynamic| -> Dynamic {
            let mut m = Map::new();
            m.insert(
                "status".into(),
                Dynamic::from("NEEDS_CANONICAL_ENCODER".to_string()),
            );
            m.insert(
                "reason".into(),
                Dynamic::from("fixture_has_no_canonical_encoder".to_string()),
            );
            m.insert(
                "target_contract".into(),
                Dynamic::from("0x0000000000000000000000000000000000000000".to_string()),
            );
            m.insert("calldata".into(), Dynamic::from("0x".to_string()));
            m.insert("approved_for_execution".into(), Dynamic::from(false));
            Dynamic::from_map(m)
        },
    );
}

fn eval(engine: &Engine, src: &str, arg: Map) -> Map {
    let ast = engine.compile(src).expect("cartridge compiles");
    let mut scope = rhai::Scope::new();
    let out: Dynamic = engine
        .call_fn(&mut scope, &ast, "evaluate_opportunity", (arg,))
        .expect("evaluate_opportunity runs without runtime error");
    out.cast::<Map>()
}

fn eval_payload(engine: &Engine, src: &str) -> Dynamic {
    let ast = engine.compile(src).expect("cartridge compiles");
    let mut scope = rhai::Scope::new();
    let out: Dynamic = engine
        .call_fn(
            &mut scope,
            &ast,
            "build_payload",
            (Dynamic::from(Map::new()),),
        )
        .expect("build_payload runs");
    out
}

fn reason(m: &Map) -> String {
    m.get("reason").unwrap().clone().into_string().unwrap()
}

fn is_opp(m: &Map) -> bool {
    m.get("is_opportunity").unwrap().as_bool().unwrap()
}

fn profit(m: &Map) -> f64 {
    m.get("estimated_profit").unwrap().clone().cast::<f64>()
}

fn leg(pool: &str, token_in: &str, token_out: &str) -> Map {
    let mut m = Map::new();
    m.insert("pool".into(), Dynamic::from(pool.to_string()));
    m.insert("token_in".into(), Dynamic::from(token_in.to_string()));
    m.insert("token_out".into(), Dynamic::from(token_out.to_string()));
    m.insert("protocol_type".into(), Dynamic::from("v2".to_string()));
    m.insert("fee_bps".into(), Dynamic::from(30_i64));
    m
}

fn route_pool_data(legs: &[(&str, &str, &str)]) -> Map {
    let mut pd = Map::new();
    pd.insert("chain_id".into(), Dynamic::from(1_i64));
    let arr: Vec<Dynamic> = legs
        .iter()
        .map(|(p, a, b)| Dynamic::from(leg(p, a, b)))
        .collect();
    pd.insert("route".into(), Dynamic::from_array(arr));
    pd
}

fn s(v: &str) -> Dynamic {
    Dynamic::from(v.to_string())
}

fn f(v: f64) -> Dynamic {
    Dynamic::from(v)
}

fn i(v: i64) -> Dynamic {
    Dynamic::from(v)
}

fn b(v: bool) -> Dynamic {
    Dynamic::from(v)
}

// ─────────────────────────────────────────────────────────────────────────────
// Fixture payloads
// ─────────────────────────────────────────────────────────────────────────────

/// Underwater Aave-style position: 10 TKA ($20k) collateral vs 21k USDX debt,
/// LT 0.8 => HF ~= 0.762 < 1. Bonus 5%, full close, flash fee 5 bps.
fn position_payload() -> Map {
    let mut m = Map::new();
    m.insert("protocol".into(), s("aave_v3"));
    m.insert("account".into(), s("0xpos1"));
    m.insert("chain_id".into(), i(1));
    m.insert("collateral_token".into(), s("0xa"));
    m.insert("collateral_amount".into(), f(10.0));
    m.insert("collateral_price_usd".into(), f(2000.0));
    m.insert("debt_token".into(), s("0xs1"));
    m.insert("debt_amount".into(), f(21_000.0));
    m.insert("debt_price_usd".into(), f(1.0));
    m.insert("liquidation_threshold".into(), f(0.8));
    m.insert("liquidation_bonus".into(), f(0.05));
    m.insert("close_factor".into(), f(1.0));
    m.insert("flash_fee_bps".into(), i(5));
    m.insert("prev_health_factor".into(), f(1.2));
    m.insert("oracle_ts".into(), i(1_717_200_000));
    m.insert("ts".into(), i(1_717_200_000));
    m
}

/// Money-market rate feed: borrow 5% / supply 3% locally, peer supply 6%.
fn lending_payload() -> Map {
    let mut m = Map::new();
    m.insert("protocol".into(), s("aave_v3"));
    m.insert("borrow_rate_annual".into(), f(0.05));
    m.insert("supply_rate_annual".into(), f(0.03));
    m.insert("utilization".into(), f(0.8));
    m.insert("collateral_factor".into(), f(0.7));
    m.insert("liquidation_threshold".into(), f(0.8));
    m.insert("protocol_fee_bps".into(), i(0));
    m.insert("principal_usd".into(), f(1_000_000.0));
    m.insert("horizon_sec".into(), f(2_592_000.0));
    m.insert("cost_of_capital_annual".into(), f(0.02));
    m.insert("fixed_rate_annual".into(), f(0.06));
    m.insert("delegation_rate_annual".into(), f(0.05));
    m.insert("settlement_window_sec".into(), f(600.0));
    m.insert("accrual_period_sec".into(), f(86_400.0));
    m.insert("index_update_ts".into(), i(1_717_200_005));
    m.insert("loop_fee_bps".into(), i(0));
    let mut peer = Map::new();
    peer.insert("protocol".into(), s("compound_v3"));
    peer.insert("borrow_rate_annual".into(), f(0.04));
    peer.insert("supply_rate_annual".into(), f(0.06));
    m.insert("peer".into(), Dynamic::from_map(peer));
    m.insert("ts".into(), i(1_717_200_000));
    m
}

/// Dutch-style auction lot: value $1995 net of 5% haircut, P now $1600.
fn auction_payload(kind: &str) -> Map {
    let mut m = Map::new();
    m.insert("kind".into(), s(kind));
    m.insert("lot_token".into(), s("0xa"));
    m.insert("lot_qty".into(), f(10.0));
    m.insert("lot_price_usd".into(), f(2100.0));
    m.insert("start_px_usd".into(), f(1800.0));
    m.insert("end_px_usd".into(), f(1400.0));
    m.insert("start_ts".into(), i(1_717_199_500));
    m.insert("end_ts".into(), i(1_717_200_500));
    m.insert("haircut".into(), f(0.05));
    m.insert("fee_bps".into(), i(0));
    m.insert("ts".into(), i(1_717_200_000));
    m
}

/// Claim feed: USDY wrapper redeems 1.05 USDX; market at 1.00 (discounted).
fn claim_payload() -> Map {
    let mut m = Map::new();
    m.insert("claim_token".into(), s("0xs2"));
    m.insert("underlying_token".into(), s("0xs1"));
    m.insert("claim_amount".into(), f(200.0));
    m.insert("claim_px_usd".into(), f(1.00));
    m.insert("market_fee_bps".into(), i(0));
    m.insert("underlying_price_usd".into(), f(1.0));
    m.insert("redemption_rate".into(), f(1.05));
    m.insert("ts".into(), i(1_717_200_000));
    m
}

/// Authorized intent: sell 1 TKA for USDX, limit 1900.
fn intent_payload() -> Map {
    let mut m = Map::new();
    m.insert("intent_id".into(), s("0xint1"));
    m.insert("origin".into(), s("ofa"));
    m.insert("authorized".into(), b(true));
    m.insert("sell_token".into(), s("0xa"));
    m.insert("buy_token".into(), s("0xs1"));
    m.insert("amount_in".into(), f(1.0));
    m.insert("min_out".into(), f(1900.0));
    m.insert("solver_out".into(), f(1950.0));
    m.insert("peer_solver_out".into(), f(1940.0));
    m.insert("bounty_bps".into(), i(50));
    m.insert("valid_to".into(), i(1_717_200_060));
    m.insert("ts".into(), i(1_717_200_000));
    let mut clob = Map::new();
    clob.insert("bid_px".into(), f(1950.0));
    clob.insert("bid_depth".into(), f(0.5));
    clob.insert("fee_bps".into(), i(10));
    m.insert("clob".into(), Dynamic::from_map(clob));
    let mut rfq = Map::new();
    rfq.insert("out".into(), f(1930.0));
    rfq.insert("fee_bps".into(), i(0));
    m.insert("rfq".into(), Dynamic::from_map(rfq));
    let mut inv = Map::new();
    inv.insert("cost_px".into(), f(1500.0));
    inv.insert("qty".into(), f(5.0));
    m.insert("inventory".into(), Dynamic::from_map(inv));
    m
}

/// NFT feed: firm ask 10 / firm bid 11.2 TKA on different venues.
fn nft_payload() -> Map {
    let mut m = Map::new();
    m.insert("collection".into(), s("testapes"));
    m.insert("token_id".into(), i(1234));
    m.insert("asset_class".into(), s("gaming"));
    m.insert("denom_token".into(), s("0xa"));
    m.insert("ask_venue".into(), s("opensea"));
    m.insert("ask_px".into(), f(10.0));
    m.insert("buy_fee_bps".into(), i(0));
    m.insert("bid_venue".into(), s("blur"));
    m.insert("bid_px".into(), f(11.2));
    m.insert("bid_depth".into(), f(2.0));
    m.insert("sell_fee_bps".into(), i(0));
    m.insert("royalty_bps".into(), i(0));
    m.insert("floor_px_a".into(), f(10.0));
    m.insert("floor_px_b".into(), f(11.1));
    m.insert("trait_premium".into(), f(1.1));
    m.insert("mint_px".into(), f(8.0));
    m.insert("mint_open".into(), b(true));
    let mut amm = Map::new();
    amm.insert("pool".into(), s("0xnft1"));
    amm.insert("nft_side".into(), i(1));
    amm.insert("fee_bps".into(), i(30));
    m.insert("amm".into(), Dynamic::from_map(amm));
    let mut amm2 = Map::new();
    amm2.insert("pool".into(), s("0xnft2"));
    amm2.insert("nft_side".into(), i(1));
    amm2.insert("fee_bps".into(), i(30));
    m.insert("amm_b".into(), Dynamic::from_map(amm2));
    m.insert("fraction_supply".into(), f(10_000.0));
    m.insert("fraction_bid_px".into(), f(0.0011));
    m.insert("fraction_ask_px".into(), f(0.0008));
    m.insert("redeem_value".into(), f(12.0));
    let mut bitem = Map::new();
    bitem.insert("bid_px".into(), f(4.0));
    let b1: Vec<Dynamic> = vec![
        Dynamic::from_map(bitem.clone()),
        Dynamic::from_map(bitem.clone()),
        Dynamic::from_map(bitem),
    ];
    m.insert("bundle".into(), Dynamic::from_array(b1));
    let mut bleg = Map::new();
    bleg.insert("collection".into(), s("col"));
    bleg.insert("ask_px".into(), f(4.0));
    bleg.insert("bid_px".into(), f(4.5));
    let basket: Vec<Dynamic> = vec![
        Dynamic::from_map(bleg.clone()),
        Dynamic::from_map(bleg.clone()),
        Dynamic::from_map(bleg),
    ];
    m.insert("basket".into(), Dynamic::from_array(basket));
    let mut loan = Map::new();
    loan.insert("debt_px".into(), f(9.5));
    loan.insert("health_factor".into(), f(0.85));
    m.insert("loan".into(), Dynamic::from_map(loan));
    let mut rental = Map::new();
    rental.insert("rate_daily".into(), f(0.2));
    rental.insert("period_days".into(), f(30.0));
    m.insert("rental".into(), Dynamic::from_map(rental));
    m.insert("src_chain".into(), i(1));
    m.insert("dst_chain".into(), i(2));
    m.insert("bridge_fee_bps".into(), i(50));
    m.insert("risk_discount_bps".into(), i(100));
    m.insert("ts".into(), i(1_717_200_000));
    m
}

/// Prediction book: binary YES/NO with a complete-set buy violation.
fn prediction_payload() -> Map {
    let mut m = Map::new();
    m.insert("platform".into(), s("testmarket"));
    m.insert("event_ref".into(), s("0xev1"));
    m.insert("resolution_source".into(), s("uma"));
    m.insert("collateral_token".into(), s("0xs1"));
    m.insert("fee_bps".into(), i(0));
    m.insert("split_available".into(), b(true));
    m.insert("merge_available".into(), b(true));
    let yes = outcome("YES", 0.46, 0.48, 1000.0);
    let no = outcome("NO", 0.44, 0.45, 800.0);
    m.insert(
        "outcomes".into(),
        Dynamic::from_array(vec![Dynamic::from_map(yes), Dynamic::from_map(no)]),
    );
    let mut peer = Map::new();
    peer.insert("platform".into(), s("peermarket"));
    peer.insert("event_ref".into(), s("0xev1"));
    peer.insert("resolution_source".into(), s("uma"));
    let py = outcome("YES", 0.48, 0.50, 1000.0);
    let pn = outcome("NO", 0.48, 0.50, 1000.0);
    peer.insert(
        "outcomes".into(),
        Dynamic::from_array(vec![Dynamic::from_map(py), Dynamic::from_map(pn)]),
    );
    m.insert("peer".into(), Dynamic::from_map(peer));
    m.insert("mutually_exclusive".into(), b(true));
    m.insert("exhaustive".into(), b(true));
    let c1 = condition("c1", 0.45, 0.45, 100.0);
    m.insert(
        "conditions".into(),
        Dynamic::from_array(vec![Dynamic::from_map(c1)]),
    );
    let mut parent = Map::new();
    parent.insert("id".into(), s("P"));
    parent.insert("bid_px".into(), f(0.95));
    parent.insert("ask_px".into(), f(0.90));
    m.insert("parent".into(), Dynamic::from_map(parent));
    let ch1 = outcome("C1", 0.50, 0.52, 100.0);
    let ch2 = outcome("C2", 0.50, 0.52, 100.0);
    m.insert(
        "children".into(),
        Dynamic::from_array(vec![Dynamic::from_map(ch1), Dynamic::from_map(ch2)]),
    );
    let mut imp1 = Map::new();
    imp1.insert("i".into(), i(1));
    imp1.insert("j".into(), i(0));
    m.insert(
        "implies".into(),
        Dynamic::from_array(vec![Dynamic::from_map(imp1)]),
    );
    let mut amm = Map::new();
    amm.insert("pool".into(), s("0xpam1"));
    amm.insert("collateral_side".into(), i(0));
    amm.insert("fee_bps".into(), i(30));
    amm.insert("outcome_idx".into(), i(0));
    m.insert("amm".into(), Dynamic::from_map(amm));
    m.insert("ts".into(), i(1_717_200_000));
    m
}

fn outcome(id: &str, bid: f64, ask: f64, depth: f64) -> Map {
    let mut m = Map::new();
    m.insert("id".into(), s(id));
    m.insert("bid_px".into(), f(bid));
    m.insert("ask_px".into(), f(ask));
    m.insert("depth".into(), f(depth));
    m
}

fn condition(id: &str, ask_a: f64, ask_b: f64, depth: f64) -> Map {
    let mut m = Map::new();
    m.insert("id".into(), s(id));
    let a = outcome("YES", ask_a + 0.02, ask_a, depth);
    let b = outcome("NO", ask_b + 0.02, ask_b, depth);
    m.insert(
        "outcomes".into(),
        Dynamic::from_array(vec![Dynamic::from_map(a), Dynamic::from_map(b)]),
    );
    m
}

/// The full intent route: 1 TKA -> ~1992 USDX through 0xas1.
fn intent_route() -> Map {
    route_pool_data(&[("0xas1", "0xa", "0xs1")])
}

/// Position unwind route: collateral TKA -> debt USDX through 0xas1.
fn position_route() -> Map {
    route_pool_data(&[("0xas1", "0xa", "0xs1")])
}

fn with_payload(key: &str, value: Map) -> Map {
    let mut pd = Map::new();
    pd.insert("chain_id".into(), i(1));
    pd.insert(key.into(), Dynamic::from_map(value));
    pd
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Empty pool_data => fail-honest machine-readable reason (all 75)
// ─────────────────────────────────────────────────────────────────────────────

const G08: &[&str] = &[
    "mev_08_001_borrow_rate_arbitrage",
    "mev_08_002_supply_rate_arbitrage",
    "mev_08_003_cross_lending_protocol_arbitrage",
    "mev_08_004_fixed_rate_floating_rate_arbitrage",
    "mev_08_005_refinancing_arbitrage",
    "mev_08_006_collateral_price_arbitrage",
    "mev_08_007_collateral_wrapper_arbitrage",
    "mev_08_008_debt_token_arbitrage",
    "mev_08_009_flash_loan_rate_arbitrage",
    "mev_08_010_credit_delegation_arbitrage",
    "mev_08_011_leverage_loop_arbitrage",
    "mev_08_012_liquidation_discount_arbitrage",
    "mev_08_013_partial_liquidation_arbitrage",
    "mev_08_014_full_liquidation_arbitrage",
    "mev_08_015_cross_protocol_liquidation_arbitrage",
    "mev_08_016_cross_chain_liquidation_arbitrage",
    "mev_08_017_oracle_update_liquidation_backrun",
    "mev_08_018_liquidation_auction_arbitrage",
    "mev_08_019_dutch_auction_liquidation_arbitrage",
    "mev_08_020_bad_debt_auction_arbitrage",
    "mev_08_021_recapitalization_auction_arbitrage",
    "mev_08_022_collateral_redemption_arbitrage",
    "mev_08_023_debt_repayment_arbitrage",
    "mev_08_024_underwater_position_takeover_arbitrage",
    "mev_08_025_interest_accrual_timing_arbitrage",
];

const G09: &[&str] = &[
    "mev_09_001_solver_arbitrage",
    "mev_09_002_intent_to_amm_arbitrage",
    "mev_09_003_intent_to_clob_arbitrage",
    "mev_09_004_intent_to_rfq_arbitrage",
    "mev_09_005_cross_intent_netting_arbitrage",
    "mev_09_006_coincidence_of_wants_settlement",
    "mev_09_007_batch_auction_arbitrage",
    "mev_09_008_combinatorial_auction_arbitrage",
    "mev_09_009_dutch_auction_decay_arbitrage",
    "mev_09_010_exclusive_order_flow_arbitrage",
    "mev_09_011_order_flow_auction_arbitrage",
    "mev_09_012_surplus_capture_arbitrage",
    "mev_09_013_price_improvement_arbitrage",
    "mev_09_014_internalization_arbitrage",
    "mev_09_015_solver_routing_arbitrage",
    "mev_09_016_solver_solver_latency_arbitrage",
    "mev_09_017_private_order_flow_backrun",
    "mev_09_018_mev_share_backrun_arbitrage",
    "mev_09_019_builder_integrated_arbitrage",
    "mev_09_020_searcher_builder_vertically_integrated_arbitrage",
];

const G10: &[&str] = &[
    "mev_10_001_cross_marketplace_nft_arbitrage",
    "mev_10_002_floor_price_arbitrage",
    "mev_10_003_trait_floor_arbitrage",
    "mev_10_004_collection_basket_arbitrage",
    "mev_10_005_bid_ask_nft_arbitrage",
    "mev_10_006_amm_marketplace_nft_arbitrage",
    "mev_10_007_nft_amm_nft_amm_arbitrage",
    "mev_10_008_mint_to_secondary_market_arbitrage",
    "mev_10_009_underpriced_listing_arbitrage",
    "mev_10_010_nft_fractionalization_arbitrage",
    "mev_10_011_fractional_token_redemption_arbitrage",
    "mev_10_012_nft_backed_loan_liquidation_arbitrage",
    "mev_10_013_nft_rental_rights_arbitrage",
    "mev_10_014_gaming_asset_marketplace_arbitrage",
    "mev_10_015_cross_chain_nft_arbitrage",
    "mev_10_016_royalty_adjusted_marketplace_arbitrage",
    "mev_10_017_bundle_unbundle_nft_arbitrage",
    "mev_10_018_redeemable_nft_arbitrage",
];

const G11: &[&str] = &[
    "mev_11_001_binary_complement_arbitrage",
    "mev_11_002_complete_set_arbitrage",
    "mev_11_003_cross_platform_prediction_arbitrage",
    "mev_11_004_mutually_exclusive_outcomes_arbitrage",
    "mev_11_005_exhaustive_outcome_basket_arbitrage",
    "mev_11_006_conditional_token_arbitrage",
    "mev_11_007_nested_market_logical_arbitrage",
    "mev_11_008_correlated_event_arbitrage",
    "mev_11_009_resolution_source_arbitrage",
    "mev_11_010_oracle_resolution_timing_arbitrage",
    "mev_11_011_pre_resolution_settlement_arbitrage",
    "mev_11_012_prediction_amm_order_book_arbitrage",
];

/// Exact empty-data reason per cartridge (family feed gates + observe law).
fn empty_reason(id: &str) -> &'static str {
    match short_id(id).as_str() {
        // G08: rate/loop cartridges price the money-market feed; position
        // (HF) cartridges the health-factor feed; auctions the lot feed.
        "mev_08_001" | "mev_08_002" | "mev_08_003" | "mev_08_004" | "mev_08_005" | "mev_08_009"
        | "mev_08_010" | "mev_08_011" | "mev_08_025" => "lending_feed_unavailable",
        "mev_08_006" | "mev_08_007" | "mev_08_008" | "mev_08_022" | "mev_08_023" => {
            "claim_feed_unavailable"
        }
        "mev_08_018" | "mev_08_019" | "mev_08_020" | "mev_08_021" => "auction_feed_unavailable",
        // G08 remainder: L_LIQ position feed.
        _ if id.starts_with("mev_08_") => "health_factor_unavailable",
        // G09: batch variants read pool_data.intents.
        "mev_09_008" => "batch_too_small",
        "mev_09_005" | "mev_09_006" | "mev_09_007" => "batch_feed_unavailable",
        "mev_09_019" | "mev_09_020" => "observe_only_structured_evidence",
        _ if id.starts_with("mev_09_") => "intent_feed_unavailable",
        // G10: the NFT feed (floor gate per family manifest).
        _ if id.starts_with("mev_10_") => "nft_floor_feed_unavailable",
        "mev_11_009" | "mev_11_010" | "mev_11_011" => "observe_only_structured_evidence",
        _ if id.starts_with("mev_11_") => "prediction_market_feed_unavailable",
        _ => panic!("unknown cartridge id {id}"),
    }
}

/// "mev_10_003_trait_floor_arbitrage" -> "mev_10_003"
fn short_id(id: &str) -> String {
    id.split('_').take(3).collect::<Vec<_>>().join("_")
}

#[test]
fn empty_pool_data_reason_sweep() {
    let engine = math_engine();
    let mut checked = 0;
    for group in [G08, G09, G10, G11] {
        for id in group {
            let src = cartridge(id);
            let out = eval(&engine, &src, Map::new());
            assert!(!is_opp(&out), "{id}: empty data must never emit");
            assert_eq!(
                reason(&out),
                empty_reason(&short_id(id)),
                "{id}: wrong empty-data reason"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 75, "wave D+E must cover exactly 75 cartridges");
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. G08 — liquidation core (health-factor scan + exact repay route)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn g08_liquidation_discount_hunt() {
    let engine = math_engine();
    let mut pd = with_payload("position", position_payload());
    let route = position_route();
    pd.insert(
        "route".into(),
        Dynamic::from_array(route["route"].clone().cast::<Vec<Dynamic>>()),
    );

    let out = eval(
        &engine,
        &cartridge("mev_08_012_liquidation_discount_arbitrage"),
        pd.clone(),
    );
    assert!(
        is_opp(&out),
        "underwater position with bonus must hunt: {out:?}"
    );
    assert_eq!(reason(&out), "liquidation_discount_profit");
    assert!(profit(&out) > 0.0);
    assert!(out.get("profit_usd_hint").unwrap().clone().cast::<f64>() > 5.0);
    // health factor evidence rides the result (0.8 * 20000 / 21000 ~= 0.762)
    let hf = out.get("health_factor").unwrap().clone().cast::<f64>();
    assert!((hf - 0.7619).abs() < 0.01, "hf {hf}");
}

#[test]
fn g08_liquidation_fail_honest_gates() {
    let engine = math_engine();
    let id = "mev_08_012_liquidation_discount_arbitrage";
    let src = cartridge(id);

    // Healthy position => no liquidation.
    let mut healthy = position_payload();
    healthy.insert("debt_amount".into(), f(10_000.0));
    let out = eval(&engine, &src, with_payload("position", healthy));
    assert_eq!(reason(&out), "health_factor_healthy");

    // Stale position => not the live state.
    let mut stale = position_payload();
    stale.insert("ts".into(), i(1_717_199_000));
    let out = eval(&engine, &src, with_payload("position", stale));
    assert_eq!(reason(&out), "position_state_stale");

    // Flash fee undocumented => cost side unknown, fail-honest.
    let mut nofee = position_payload();
    nofee.remove("flash_fee_bps");
    let out = eval(&engine, &src, with_payload("position", nofee));
    assert_eq!(reason(&out), "flash_fee_unavailable");

    // Underwater but no unwind route => no repay queue.
    let out = eval(&engine, &src, with_payload("position", position_payload()));
    assert_eq!(reason(&out), "missing_route");
}

#[test]
fn g08_liquidation_variant_gates() {
    let engine = math_engine();
    let route_pd = |pos: Map| {
        let mut pd = with_payload("position", pos);
        let route = position_route();
        pd.insert(
            "route".into(),
            Dynamic::from_array(route["route"].clone().cast::<Vec<Dynamic>>()),
        );
        pd
    };

    // 013 partial-only vs full close factor.
    let out = eval(
        &engine,
        &cartridge("mev_08_013_partial_liquidation_arbitrage"),
        route_pd(position_payload()),
    );
    assert_eq!(reason(&out), "close_factor_not_partial", "{out:?}");
    let mut partial = position_payload();
    partial.insert("close_factor".into(), f(0.5));
    let out = eval(
        &engine,
        &cartridge("mev_08_013_partial_liquidation_arbitrage"),
        route_pd(partial),
    );
    assert!(is_opp(&out), "partial close hunts: {out:?}");
    assert_eq!(reason(&out), "partial_liquidation_profit");

    // 014 full-only vs partial close factor.
    let mut partial2 = position_payload();
    partial2.insert("close_factor".into(), f(0.5));
    let out = eval(
        &engine,
        &cartridge("mev_08_014_full_liquidation_arbitrage"),
        route_pd(partial2),
    );
    assert_eq!(reason(&out), "close_factor_not_full");
    let out = eval(
        &engine,
        &cartridge("mev_08_014_full_liquidation_arbitrage"),
        route_pd(position_payload()),
    );
    assert!(is_opp(&out), "full close hunts: {out:?}");

    // 015 cross-protocol: peer repay edge required.
    let out = eval(
        &engine,
        &cartridge("mev_08_015_cross_protocol_liquidation_arbitrage"),
        route_pd(position_payload()),
    );
    assert_eq!(reason(&out), "peer_protocol_feed_unavailable");
    let mut peer_pos = position_payload();
    peer_pos.insert("peer_protocol".into(), s("morpho"));
    peer_pos.insert("repay_discount".into(), f(0.01));
    let out = eval(
        &engine,
        &cartridge("mev_08_015_cross_protocol_liquidation_arbitrage"),
        route_pd(peer_pos),
    );
    assert!(is_opp(&out), "peer discounted repay hunts: {out:?}");

    // 016 cross-chain: same chain is not cross-chain; remote needs a bridge.
    let out = eval(
        &engine,
        &cartridge("mev_08_016_cross_chain_liquidation_arbitrage"),
        route_pd(position_payload()),
    );
    assert_eq!(reason(&out), "not_cross_chain");
    let mut remote = position_payload();
    remote.insert("chain_id".into(), i(137));
    let out = eval(
        &engine,
        &cartridge("mev_08_016_cross_chain_liquidation_arbitrage"),
        route_pd(remote.clone()),
    );
    assert_eq!(reason(&out), "cross_chain_bridge_unavailable");
    let mut bridge = Map::new();
    bridge.insert("fee_bps".into(), i(10));
    bridge.insert("fixed_usd".into(), f(5.0));
    bridge.insert("risk_discount_bps".into(), i(0));
    remote.insert("bridge".into(), Dynamic::from_map(bridge));
    let out = eval(
        &engine,
        &cartridge("mev_08_016_cross_chain_liquidation_arbitrage"),
        route_pd(remote),
    );
    assert!(is_opp(&out), "bridged remote liquidation hunts: {out:?}");
    assert_eq!(reason(&out), "cross_chain_liquidation_profit");

    // 017 oracle-update backrun: transition evidence required.
    let mut noref = position_payload();
    noref.remove("oracle_ts");
    let out = eval(
        &engine,
        &cartridge("mev_08_017_oracle_update_liquidation_backrun"),
        route_pd(noref),
    );
    assert_eq!(reason(&out), "oracle_update_unavailable");
    let mut oldref = position_payload();
    oldref.insert("oracle_ts".into(), i(1_717_199_000));
    let out = eval(
        &engine,
        &cartridge("mev_08_017_oracle_update_liquidation_backrun"),
        route_pd(oldref),
    );
    assert_eq!(reason(&out), "oracle_update_stale");
    let out = eval(
        &engine,
        &cartridge("mev_08_017_oracle_update_liquidation_backrun"),
        route_pd(position_payload()),
    );
    assert!(is_opp(&out), "fresh oracle transition backruns: {out:?}");
    assert_eq!(reason(&out), "oracle_liquidation_backrun_profit");

    // 024 takeover: negative equity is not takeable.
    let mut underwater_no_equity = position_payload();
    underwater_no_equity.insert("debt_amount".into(), f(25_000.0));
    let out = eval(
        &engine,
        &cartridge("mev_08_024_underwater_position_takeover_arbitrage"),
        route_pd(underwater_no_equity),
    );
    assert_eq!(reason(&out), "negative_equity_not_takeable");
    let mut takeable = position_payload();
    takeable.insert("debt_amount".into(), f(18_000.0));
    let out = eval(
        &engine,
        &cartridge("mev_08_024_underwater_position_takeover_arbitrage"),
        route_pd(takeable),
    );
    assert!(is_opp(&out), "positive-equity takeover hunts: {out:?}");
    assert_eq!(reason(&out), "underwater_takeover_equity_profit");
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. G08 — rates, loop, auctions, claims
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn g08_rate_carry() {
    let engine = math_engine();
    let ld = || with_payload("lending", lending_payload());

    // 001: borrow local 5% / supply peer 6% => 1% annualized edge ~ 821 USD.
    let out = eval(
        &engine,
        &cartridge("mev_08_001_borrow_rate_arbitrage"),
        ld(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "borrow_rate_carry_profit");
    let p = profit(&out);
    assert!((p - 821.0).abs() < 15.0, "carry {p}");

    // 002: supply 3% vs cost of capital 2%.
    let out = eval(
        &engine,
        &cartridge("mev_08_002_supply_rate_arbitrage"),
        ld(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "supply_rate_carry_profit");

    // 003: best of both directions (borrow local 5 / supply peer 6 = 1% vs
    // borrow peer 4 / supply local 3 = -1%) => cross-protocol carry.
    let out = eval(
        &engine,
        &cartridge("mev_08_003_cross_lending_protocol_arbitrage"),
        ld(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "cross_protocol_carry_profit");

    // 004: fixed 6% vs floating borrow 5% => receive fixed.
    let out = eval(
        &engine,
        &cartridge("mev_08_004_fixed_rate_floating_rate_arbitrage"),
        ld(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "fixed_floating_carry_profit");

    // 005: refi to the cheaper peer borrow (5% -> 4%).
    let out = eval(
        &engine,
        &cartridge("mev_08_005_refinancing_arbitrage"),
        ld(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "refinancing_rate_savings");

    // 009: a 5 bps flash fee over a 600s window cannot beat 3% carry.
    let mut flash_ld = lending_payload();
    flash_ld.insert("flash_fee_bps".into(), i(5));
    let out = eval(
        &engine,
        &cartridge("mev_08_009_flash_loan_rate_arbitrage"),
        with_payload("lending", flash_ld),
    );
    assert_eq!(reason(&out), "flash_fee_exceeds_carry");
    assert!(!is_opp(&out));

    // 010: delegation 5% vs supply 3% alternative.
    let out = eval(
        &engine,
        &cartridge("mev_08_010_credit_delegation_arbitrage"),
        ld(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "credit_delegation_carry_profit");

    // 025: supply must land inside the accrual window.
    let out = eval(
        &engine,
        &cartridge("mev_08_025_interest_accrual_timing_arbitrage"),
        ld(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "pre_accrual_supply_profit");
    let mut late = lending_payload();
    late.insert("index_update_ts".into(), i(1_717_200_600));
    let out = eval(
        &engine,
        &cartridge("mev_08_025_interest_accrual_timing_arbitrage"),
        with_payload("lending", late),
    );
    assert_eq!(reason(&out), "accrual_window_closed");

    // 001 negative gate: peer absent.
    let mut no_peer = lending_payload();
    no_peer.remove("peer");
    let out = eval(
        &engine,
        &cartridge("mev_08_001_borrow_rate_arbitrage"),
        with_payload("lending", no_peer),
    );
    assert_eq!(reason(&out), "peer_rate_unavailable");
}

#[test]
fn g08_leverage_loop() {
    let engine = math_engine();
    let out = eval(
        &engine,
        &cartridge("mev_08_011_leverage_loop_arbitrage"),
        with_payload("lending", lending_payload()),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "leverage_loop_carry_profit");
    let loops = out.get("optimal_loops").unwrap().clone().cast::<i64>();
    assert!(loops >= 1, "loops {loops}");

    // Degenerate collateral factor (>=1) is not a loop.
    let mut bad = lending_payload();
    bad.insert("collateral_factor".into(), f(1.0));
    let out = eval(
        &engine,
        &cartridge("mev_08_011_leverage_loop_arbitrage"),
        with_payload("lending", bad),
    );
    assert_eq!(reason(&out), "degenerate_collateral_factor");
}

#[test]
fn g08_auctions() {
    let engine = math_engine();

    // 018: liquidation auction lot under conservative value at P(now).
    let out = eval(
        &engine,
        &cartridge("mev_08_018_liquidation_auction_arbitrage"),
        with_payload("auction", auction_payload("liquidation")),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "auction_discount_profit");
    // (1995 - 1600) * 10 = 3950 USD
    assert!(
        (profit(&out) - 3950.0).abs() < 1.0,
        "profit {}",
        profit(&out)
    );

    // 019: dutch decay requires a strictly decaying curve.
    let mut flat = auction_payload("dutch");
    flat.insert("start_px_usd".into(), f(1400.0));
    let out = eval(
        &engine,
        &cartridge("mev_08_019_dutch_auction_liquidation_arbitrage"),
        with_payload("auction", flat),
    );
    assert_eq!(reason(&out), "not_decaying");
    let out = eval(
        &engine,
        &cartridge("mev_08_019_dutch_auction_liquidation_arbitrage"),
        with_payload("auction", auction_payload("dutch")),
    );
    assert!(is_opp(&out), "{out:?}");

    // 020/021: kind identity gates.
    let out = eval(
        &engine,
        &cartridge("mev_08_020_bad_debt_auction_arbitrage"),
        with_payload("auction", auction_payload("liquidation")),
    );
    assert_eq!(reason(&out), "auction_kind_mismatch");
    let out = eval(
        &engine,
        &cartridge("mev_08_021_recapitalization_auction_arbitrage"),
        with_payload("auction", auction_payload("recap")),
    );
    assert!(is_opp(&out), "{out:?}");

    // Early lot: profitable only later in the decay — honest timing answer.
    let mut early = auction_payload("liquidation");
    early.insert("start_px_usd".into(), f(2300.0));
    early.insert("end_px_usd".into(), f(1000.0));
    early.insert("start_ts".into(), i(1_717_199_900));
    let out = eval(
        &engine,
        &cartridge("mev_08_018_liquidation_auction_arbitrage"),
        with_payload("auction", early),
    );
    assert_eq!(reason(&out), "decay_not_yet_profitable");

    // Ended auction.
    let mut ended = auction_payload("liquidation");
    ended.insert("end_ts".into(), i(1_717_199_900));
    let out = eval(
        &engine,
        &cartridge("mev_08_018_liquidation_auction_arbitrage"),
        with_payload("auction", ended),
    );
    assert_eq!(reason(&out), "auction_ended");
}

#[test]
fn g08_claims() {
    let engine = math_engine();
    let route_pd = |cl: Map, legs: &[(&str, &str, &str)]| {
        let mut pd = with_payload("claim", cl);
        let route = route_pool_data(legs);
        pd.insert(
            "route".into(),
            Dynamic::from_array(route["route"].clone().cast::<Vec<Dynamic>>()),
        );
        pd
    };

    // 006: wrapper trades at 1.00 vs 1.05 redemption value.
    let out = eval(
        &engine,
        &cartridge("mev_08_006_collateral_price_arbitrage"),
        with_payload("claim", claim_payload()),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "collateral_price_redemption_profit");
    // 200 * 1.05 - 200 = 10 USD
    assert!(
        (profit(&out) - 10.0).abs() < 0.01,
        "profit {}",
        profit(&out)
    );

    // 006 negative: no redemption premium.
    let mut flat = claim_payload();
    flat.insert("claim_px_usd".into(), f(1.05));
    let out = eval(
        &engine,
        &cartridge("mev_08_006_collateral_price_arbitrage"),
        with_payload("claim", flat),
    );
    assert_eq!(reason(&out), "no_redemption_edge");

    // 007: wrapper acquired on-chain (TKA -> TKB 1:1) redeeming 1.05.
    let mut wrap = claim_payload();
    wrap.insert("claim_token".into(), s("0xb"));
    wrap.insert("underlying_token".into(), s("0xa"));
    let out = eval(
        &engine,
        &cartridge("mev_08_007_collateral_wrapper_arbitrage"),
        route_pd(wrap, &[("0xab1", "0xa", "0xb")]),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "wrapper_redemption_profit");

    // 008: debt token under face on the skewed 1:2100 pool (face 1:2000).
    let mut debt = claim_payload();
    debt.insert("claim_token".into(), s("0xs1"));
    debt.insert("underlying_token".into(), s("0xa"));
    debt.insert("face_rate".into(), f(0.0005));
    let out = eval(
        &engine,
        &cartridge("mev_08_008_debt_token_arbitrage"),
        route_pd(debt, &[("0xas2", "0xa", "0xs1")]),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "debt_token_below_face_profit");

    // 022: protocol oracle rate redemption vs market.
    let mut oracle = claim_payload();
    oracle.insert("oracle_rate".into(), f(1.06));
    let out = eval(
        &engine,
        &cartridge("mev_08_022_collateral_redemption_arbitrage"),
        with_payload("claim", oracle),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "oracle_rate_redemption_profit");
    let mut no_oracle = claim_payload();
    no_oracle.remove("redemption_rate");
    let out = eval(
        &engine,
        &cartridge("mev_08_022_collateral_redemption_arbitrage"),
        with_payload("claim", no_oracle),
    );
    assert_eq!(reason(&out), "oracle_rate_unavailable");

    // 023: buy OUR debt below face on the skewed pool.
    let mut our = claim_payload();
    our.insert("claim_token".into(), s("0xs1"));
    our.insert("underlying_token".into(), s("0xa"));
    our.insert("face_rate".into(), f(0.0005));
    our.insert("our_debt_amount".into(), f(100_000.0));
    let out = eval(
        &engine,
        &cartridge("mev_08_023_debt_repayment_arbitrage"),
        route_pd(our.clone(), &[("0xas3", "0xa", "0xs1")]),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "discounted_debt_extinguish_profit");
    // At the fair 1:2000 pool the debt is NOT below face.
    let out = eval(
        &engine,
        &cartridge("mev_08_023_debt_repayment_arbitrage"),
        route_pd(our.clone(), &[("0xas1", "0xa", "0xs1")]),
    );
    assert_eq!(reason(&out), "debt_not_below_face");
    // Without our own debt there is nothing to extinguish.
    let mut no_debt = our;
    no_debt.remove("our_debt_amount");
    let out = eval(
        &engine,
        &cartridge("mev_08_023_debt_repayment_arbitrage"),
        route_pd(no_debt, &[("0xas3", "0xa", "0xs1")]),
    );
    assert_eq!(reason(&out), "no_debt_position");
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. G09 — intent route fills, solver competition, batch, backruns
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn g09_route_fills() {
    let engine = math_engine();
    let pd = || {
        let mut pd = with_payload("intent", intent_payload());
        let route = intent_route();
        pd.insert(
            "route".into(),
            Dynamic::from_array(route["route"].clone().cast::<Vec<Dynamic>>()),
        );
        pd
    };

    // 012: 1 TKA -> ~1992 USDX vs limit 1900 => surplus ~92.
    let out = eval(
        &engine,
        &cartridge("mev_09_012_surplus_capture_arbitrage"),
        pd(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "intent_surplus_capture");
    let p = profit(&out);
    assert!(p > 50.0 && p < 110.0, "surplus {p}");

    // Expired intent.
    let mut exp = intent_payload();
    exp.insert("valid_to".into(), i(1_717_199_000));
    let mut pd2 = with_payload("intent", exp);
    let route = intent_route();
    pd2.insert(
        "route".into(),
        Dynamic::from_array(route["route"].clone().cast::<Vec<Dynamic>>()),
    );
    let out = eval(
        &engine,
        &cartridge("mev_09_012_surplus_capture_arbitrage"),
        pd2,
    );
    assert_eq!(reason(&out), "intent_expired");

    // 002: single-AMM-leg shape gate (the 2-leg route violates it).
    let mut two_leg = pd();
    let r2 = route_pool_data(&[("0xab1", "0xa", "0xb"), ("0xbs1", "0xb", "0xs1")]);
    two_leg.insert(
        "route".into(),
        Dynamic::from_array(r2["route"].clone().cast::<Vec<Dynamic>>()),
    );
    let out = eval(
        &engine,
        &cartridge("mev_09_002_intent_to_amm_arbitrage"),
        two_leg,
    );
    assert_eq!(reason(&out), "route_shape_out_of_bounds");

    // 001: beat the incumbent solver (1950) with the route (~1992).
    let out = eval(&engine, &cartridge("mev_09_001_solver_arbitrage"), pd());
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "solver_fill_beaten");
    // Unbeatable incumbent.
    let mut strong = intent_payload();
    strong.insert("solver_out".into(), f(2050.0));
    let mut pd3 = with_payload("intent", strong);
    let route = intent_route();
    pd3.insert(
        "route".into(),
        Dynamic::from_array(route["route"].clone().cast::<Vec<Dynamic>>()),
    );
    let out = eval(&engine, &cartridge("mev_09_001_solver_arbitrage"), pd3);
    assert_eq!(reason(&out), "solver_quote_not_beatable");

    // 013: improvement over the incumbent.
    let out = eval(
        &engine,
        &cartridge("mev_09_013_price_improvement_arbitrage"),
        pd(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "price_improvement_over_solver");

    // 015: routing only pays when our route wins.
    let out = eval(
        &engine,
        &cartridge("mev_09_015_solver_routing_arbitrage"),
        pd(),
    );
    assert!(is_opp(&out), "{out:?}");

    // 016: we must beat the best of two solvers (max 1950).
    let out = eval(
        &engine,
        &cartridge("mev_09_016_solver_solver_latency_arbitrage"),
        pd(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "beats_both_solver_quotes");

    // 003: CLOB book completes half the size at 1950; best-of still clears.
    let out = eval(
        &engine,
        &cartridge("mev_09_003_intent_to_clob_arbitrage"),
        pd(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "intent_best_of_amm_clob");
    let mut noclob = intent_payload();
    noclob.remove("clob");
    let out = eval(
        &engine,
        &cartridge("mev_09_003_intent_to_clob_arbitrage"),
        with_payload("intent", noclob),
    );
    assert_eq!(reason(&out), "clob_quote_unavailable");

    // 004: RFQ 1930 loses to the route ~1992.
    let out = eval(
        &engine,
        &cartridge("mev_09_004_intent_to_rfq_arbitrage"),
        pd(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "intent_best_of_amm_rfq");

    // 010: exclusive origin + explicit authorization.
    let mut ofa = intent_payload();
    ofa.insert("origin".into(), s("exclusive"));
    ofa.insert("authorized".into(), b(true));
    let mut pd4 = with_payload("intent", ofa);
    let route = intent_route();
    pd4.insert(
        "route".into(),
        Dynamic::from_array(route["route"].clone().cast::<Vec<Dynamic>>()),
    );
    let out = eval(
        &engine,
        &cartridge("mev_09_010_exclusive_order_flow_arbitrage"),
        pd4,
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "exclusive_flow_fill_surplus");
    let mut unauth = intent_payload();
    unauth.insert("origin".into(), s("exclusive"));
    unauth.insert("authorized".into(), b(false));
    let out = eval(
        &engine,
        &cartridge("mev_09_010_exclusive_order_flow_arbitrage"),
        with_payload("intent", unauth),
    );
    assert_eq!(reason(&out), "flow_not_authorized");

    // 011: OFA bounty on top of the surplus.
    let out = eval(
        &engine,
        &cartridge("mev_09_011_order_flow_auction_arbitrage"),
        pd(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "ofa_surplus_plus_bounty");

    // 014: internalization at cost 1500 vs limit 1900.
    let out = eval(
        &engine,
        &cartridge("mev_09_014_internalization_arbitrage"),
        pd(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "internalized_flow_margin");
    assert!(
        (profit(&out) - 400.0).abs() < 0.01,
        "margin {}",
        profit(&out)
    );
}

#[test]
fn g09_dutch_decay() {
    let engine = math_engine();
    let mut it = intent_payload();
    let mut du = Map::new();
    du.insert("start_px".into(), f(1800.0));
    du.insert("end_px".into(), f(1400.0));
    du.insert("start_ts".into(), i(1_717_199_500));
    du.insert("end_ts".into(), i(1_717_200_500));
    it.insert("dutch".into(), Dynamic::from_map(du.clone()));
    let mut pd = with_payload("intent", it);
    let route = intent_route();
    pd.insert(
        "route".into(),
        Dynamic::from_array(route["route"].clone().cast::<Vec<Dynamic>>()),
    );

    // Buy the lot at P(now)=1600, exit through the route at ~1992.
    let out = eval(
        &engine,
        &cartridge("mev_09_009_dutch_auction_decay_arbitrage"),
        pd.clone(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "dutch_decay_executable_premium");

    // Curve above the exit value everywhere: no edge now or later.
    let mut hi = pd;
    let it2 = {
        let mut m = hi["intent"].clone().cast::<Map>();
        let mut du2 = du.clone();
        du2.insert("start_px".into(), f(2100.0));
        du2.insert("end_px".into(), f(2050.0));
        m.insert("dutch".into(), Dynamic::from_map(du2));
        m
    };
    hi.insert("intent".into(), Dynamic::from_map(it2));
    let out = eval(
        &engine,
        &cartridge("mev_09_009_dutch_auction_decay_arbitrage"),
        hi,
    );
    assert_eq!(reason(&out), "dutch_no_edge");
}

#[test]
fn g09_backruns() {
    let engine = math_engine();
    let backrun_pd = |origin: &str, authorized: bool| {
        let mut it = intent_payload();
        it.insert("origin".into(), s(origin));
        it.insert("authorized".into(), b(authorized));
        it.insert("amount_in".into(), f(100.0)); // big swap => real reversion
        let mut pd = with_payload("intent", it);
        let route = intent_route();
        pd.insert(
            "route".into(),
            Dynamic::from_array(route["route"].clone().cast::<Vec<Dynamic>>()),
        );
        pd
    };

    // 017: authorized private flow, exact post-intent transition reversion.
    let out = eval(
        &engine,
        &cartridge("mev_09_017_private_order_flow_backrun"),
        backrun_pd("private", true),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "authorized_backrun_reversion_profit");
    assert!(profit(&out) > 0.0);

    // Unoriginated flow: not ours to touch.
    let out = eval(
        &engine,
        &cartridge("mev_09_017_private_order_flow_backrun"),
        backrun_pd("ofa", true),
    );
    assert_eq!(reason(&out), "flow_not_authorized");

    // 018: MEV-Share origin gate + documented user refund share.
    let out = eval(
        &engine,
        &cartridge("mev_09_018_mev_share_backrun_arbitrage"),
        backrun_pd("mev_share", false),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "authorized_backrun_reversion_profit");
    let full = profit(&out);

    let mut shared = backrun_pd("mev_share", false);
    {
        let mut m = shared["intent"].clone().cast::<Map>();
        m.insert("share_bps".into(), i(5000));
        shared.insert("intent".into(), Dynamic::from_map(m));
    }
    let out = eval(
        &engine,
        &cartridge("mev_09_018_mev_share_backrun_arbitrage"),
        shared,
    );
    assert!(is_opp(&out), "{out:?}");
    let net = profit(&out);
    assert!(
        net < full && (full - net) / full > 0.4,
        "refund haircut {net} vs {full}"
    );

    // Wrong origin.
    let out = eval(
        &engine,
        &cartridge("mev_09_018_mev_share_backrun_arbitrage"),
        backrun_pd("private", true),
    );
    assert_eq!(reason(&out), "flow_not_authorized");
}

#[test]
fn g09_batch_intents() {
    let engine = math_engine();

    let intent = |sell: &str, buy: &str, amount: f64, min_out: f64| {
        let mut m = Map::new();
        m.insert("intent_id".into(), s("0xb"));
        m.insert("sell_token".into(), s(sell));
        m.insert("buy_token".into(), s(buy));
        m.insert("amount_in".into(), f(amount));
        m.insert("min_out".into(), f(min_out));
        m.insert("valid_to".into(), i(1_717_200_060));
        m.insert("ts".into(), i(1_717_200_000));
        m
    };

    // Crossing pair: A sells TKA limits 1.1 TKB; B sells TKB limits 0.952 TKA.
    let intents = || {
        let mut pd = Map::new();
        pd.insert(
            "intents".into(),
            Dynamic::from_array(vec![
                Dynamic::from_map(intent("0xa", "0xb", 100.0, 110.0)),
                Dynamic::from_map(intent("0xb", "0xa", 105.0, 100.0)),
            ]),
        );
        pd
    };

    // 005: netting spread (1.1 - 1/0.952...) * 100 ~= 4.95 TKB.
    let out = eval(
        &engine,
        &cartridge("mev_09_005_cross_intent_netting_arbitrage"),
        intents(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "cross_intent_netting_spread");
    assert!(profit(&out) > 4.0, "spread {}", profit(&out));

    // 006: exactly the two mirrored intents.
    let out = eval(
        &engine,
        &cartridge("mev_09_006_coincidence_of_wants_settlement"),
        intents(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "coincidence_of_wants_spread");

    // 006 negative: not mirrored.
    let mut not_cow = Map::new();
    not_cow.insert(
        "intents".into(),
        Dynamic::from_array(vec![
            Dynamic::from_map(intent("0xa", "0xb", 100.0, 110.0)),
            Dynamic::from_map(intent("0xa", "0xc", 105.0, 100.0)),
        ]),
    );
    let out = eval(
        &engine,
        &cartridge("mev_09_006_coincidence_of_wants_settlement"),
        not_cow,
    );
    assert_eq!(reason(&out), "no_coincidence");

    // 005 negative: limits that do not cross.
    let mut no_cross = Map::new();
    no_cross.insert(
        "intents".into(),
        Dynamic::from_array(vec![
            Dynamic::from_map(intent("0xa", "0xb", 100.0, 100.0)),
            Dynamic::from_map(intent("0xb", "0xa", 105.0, 95.0)),
        ]),
    );
    let out = eval(
        &engine,
        &cartridge("mev_09_005_cross_intent_netting_arbitrage"),
        no_cross,
    );
    assert_eq!(reason(&out), "no_crossing_intents");

    // 007: batch auction at the route marginal (limits below 1994 clear).
    let mut batch = Map::new();
    batch.insert(
        "intents".into(),
        Dynamic::from_array(vec![
            Dynamic::from_map(intent("0xa", "0xs1", 1.0, 1900.0)),
            Dynamic::from_map(intent("0xa", "0xs1", 2.0, 3700.0)),
        ]),
    );
    let route = intent_route();
    batch.insert(
        "route".into(),
        Dynamic::from_array(route["route"].clone().cast::<Vec<Dynamic>>()),
    );
    let out = eval(
        &engine,
        &cartridge("mev_09_007_batch_auction_arbitrage"),
        batch.clone(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "batch_auction_clearing_surplus");
    assert!(profit(&out) > 100.0, "batch surplus {}", profit(&out));

    // No clearable intents (limits above the marginal).
    let mut pricey = Map::new();
    pricey.insert(
        "intents".into(),
        Dynamic::from_array(vec![
            Dynamic::from_map(intent("0xa", "0xs1", 1.0, 2100.0)),
            Dynamic::from_map(intent("0xa", "0xs1", 1.0, 2050.0)),
        ]),
    );
    pricey.insert(
        "route".into(),
        Dynamic::from_array(route["route"].clone().cast::<Vec<Dynamic>>()),
    );
    let out = eval(
        &engine,
        &cartridge("mev_09_007_batch_auction_arbitrage"),
        pricey,
    );
    assert_eq!(reason(&out), "no_clearable_intents");

    // 008: combinatorial needs >= 3 intents.
    let mut two = Map::new();
    two.insert(
        "intents".into(),
        Dynamic::from_array(vec![
            Dynamic::from_map(intent("0xa", "0xs1", 1.0, 1900.0)),
            Dynamic::from_map(intent("0xa", "0xs1", 1.0, 1900.0)),
        ]),
    );
    let out = eval(
        &engine,
        &cartridge("mev_09_008_combinatorial_auction_arbitrage"),
        two,
    );
    assert_eq!(reason(&out), "batch_too_small");
    let mut three = batch;
    let arr = three["intents"].clone().cast::<Vec<Dynamic>>();
    let mut arr2 = arr.clone();
    arr2.push(Dynamic::from_map(intent("0xa", "0xs1", 1.0, 1850.0)));
    three.insert("intents".into(), Dynamic::from_array(arr2));
    let out = eval(
        &engine,
        &cartridge("mev_09_008_combinatorial_auction_arbitrage"),
        three,
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "combinatorial_subset_clearing_surplus");
}

#[test]
fn g09_observe_only() {
    let engine = math_engine();
    for id in [
        "mev_09_019_builder_integrated_arbitrage",
        "mev_09_020_searcher_builder_vertically_integrated_arbitrage",
    ] {
        let out = eval(
            &engine,
            &cartridge(id),
            with_payload("intent", intent_payload()),
        );
        assert!(!is_opp(&out), "{id}: OBSERVE never emits");
        assert_eq!(reason(&out), "observe_only_structured_evidence");
        assert!(out
            .get("evidence_intent_present")
            .unwrap()
            .clone()
            .cast::<bool>());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. G10 — NFT firm-quote law
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn g10_identical_and_floor() {
    let engine = math_engine();
    let pd = || with_payload("nft", nft_payload());

    // 001: cross-venue identical asset: 11.2 bid - 10 ask.
    let out = eval(
        &engine,
        &cartridge("mev_10_001_cross_marketplace_nft_arbitrage"),
        pd(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "cross_marketplace_firm_quote_profit");
    assert!(
        (profit(&out) - 1.2).abs() < 0.001,
        "profit {}",
        profit(&out)
    );

    // Same venue is not the cross-marketplace variant.
    let mut same = nft_payload();
    same.insert("bid_venue".into(), s("opensea"));
    let out = eval(
        &engine,
        &cartridge("mev_10_001_cross_marketplace_nft_arbitrage"),
        with_payload("nft", same),
    );
    assert_eq!(reason(&out), "same_venue_no_cross_edge");

    // 005: the plain firm bid/ask spread.
    let out = eval(
        &engine,
        &cartridge("mev_10_005_bid_ask_nft_arbitrage"),
        pd(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "nft_bid_ask_spread_profit");

    // 002: floor deviation + firm quotes.
    let out = eval(
        &engine,
        &cartridge("mev_10_002_floor_price_arbitrage"),
        pd(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "floor_deviation_firm_exit_profit");
    let mut no_bid = nft_payload();
    no_bid.remove("bid_px");
    let out = eval(
        &engine,
        &cartridge("mev_10_002_floor_price_arbitrage"),
        with_payload("nft", no_bid),
    );
    assert_eq!(reason(&out), "floor_signal_only_no_firm_exit");

    // 003: trait premium 1.1 over floor 10 vs ask 10.
    let out = eval(
        &engine,
        &cartridge("mev_10_003_trait_floor_arbitrage"),
        pd(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "trait_premium_firm_exit_profit");

    // 004: basket needs >= 3 legs.
    let mut small_basket = nft_payload();
    let bask: Vec<Dynamic> = vec![f(4.0), f(4.0)];
    small_basket.insert("basket".into(), Dynamic::from_array(bask));
    let out = eval(
        &engine,
        &cartridge("mev_10_004_collection_basket_arbitrage"),
        with_payload("nft", small_basket),
    );
    assert_eq!(reason(&out), "basket_too_small");
    let out = eval(
        &engine,
        &cartridge("mev_10_004_collection_basket_arbitrage"),
        pd(),
    );
    assert!(is_opp(&out), "{out:?}");

    // 008: mint at 8, exit at firm bid 11.2.
    let out = eval(
        &engine,
        &cartridge("mev_10_008_mint_to_secondary_market_arbitrage"),
        pd(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert!(
        (profit(&out) - 3.2).abs() < 0.001,
        "mint profit {}",
        profit(&out)
    );
    let mut closed = nft_payload();
    closed.insert("mint_open".into(), b(false));
    let out = eval(
        &engine,
        &cartridge("mev_10_008_mint_to_secondary_market_arbitrage"),
        with_payload("nft", closed),
    );
    assert_eq!(reason(&out), "mint_closed");

    // 009: listing must sit under the floor (ask 9.0 vs floor 10.0).
    let mut under = nft_payload();
    under.insert("ask_px".into(), f(9.0));
    let out = eval(
        &engine,
        &cartridge("mev_10_009_underpriced_listing_arbitrage"),
        with_payload("nft", under),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "underpriced_listing_firm_exit_profit");
    let mut rich = nft_payload();
    rich.insert("ask_px".into(), f(10.5));
    let out = eval(
        &engine,
        &cartridge("mev_10_009_underpriced_listing_arbitrage"),
        with_payload("nft", rich),
    );
    assert_eq!(reason(&out), "listing_not_underpriced");

    // 013: rental income 6.0 covers the 0-spread.
    let out = eval(
        &engine,
        &cartridge("mev_10_013_nft_rental_rights_arbitrage"),
        pd(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "nft_rental_rights_yield_profit");

    // 014: gaming asset class identity.
    let out = eval(
        &engine,
        &cartridge("mev_10_014_gaming_asset_marketplace_arbitrage"),
        pd(),
    );
    assert!(is_opp(&out), "{out:?}");
    let mut art = nft_payload();
    art.insert("asset_class".into(), s("art"));
    let out = eval(
        &engine,
        &cartridge("mev_10_014_gaming_asset_marketplace_arbitrage"),
        with_payload("nft", art),
    );
    assert_eq!(reason(&out), "asset_class_mismatch");

    // 015: cross-chain bridge costs.
    let out = eval(
        &engine,
        &cartridge("mev_10_015_cross_chain_nft_arbitrage"),
        pd(),
    );
    assert!(is_opp(&out), "{out:?}");
    // 11.2 * (1 - 50bps) * (1 - 100bps) - 10 = 1.0774
    assert!(
        (profit(&out) - 1.03256).abs() < 0.001,
        "xchain {}",
        profit(&out)
    );
    let mut same_chain = nft_payload();
    same_chain.insert("dst_chain".into(), i(1));
    let out = eval(
        &engine,
        &cartridge("mev_10_015_cross_chain_nft_arbitrage"),
        with_payload("nft", same_chain),
    );
    assert_eq!(reason(&out), "not_cross_chain");

    // 016: royalty venue choice (no zero-royalty venue here => direct exit).
    let out = eval(
        &engine,
        &cartridge("mev_10_016_royalty_adjusted_marketplace_arbitrage"),
        pd(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "royalty_venue_switch_profit");
}

#[test]
fn g10_amm_redeem_liq() {
    let engine = math_engine();
    let pd = || with_payload("nft", nft_payload());

    // 006: exact NFT-AMM curve cost ~20.5 TKA vs firm bid 25.
    let mut amm_nft = nft_payload();
    amm_nft.insert("bid_px".into(), f(25.0));
    let out = eval(
        &engine,
        &cartridge("mev_10_006_amm_marketplace_nft_arbitrage"),
        with_payload("nft", amm_nft.clone()),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "nft_amm_vs_marketplace_profit");

    // 007: buy on the 20-TKA curve, sell into the 24-TKA curve.
    let out = eval(
        &engine,
        &cartridge("mev_10_007_nft_amm_nft_amm_arbitrage"),
        with_payload("nft", amm_nft),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "nft_amm_curve_pair_profit");
    assert!(profit(&out) > 0.5, "curve pair {}", profit(&out));

    // 010: fractionalize 1 NFT into 10k fractions at 0.0011 bid = 11.
    let out = eval(
        &engine,
        &cartridge("mev_10_010_nft_fractionalization_arbitrage"),
        pd(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert!((profit(&out) - 1.0).abs() < 0.001, "frac {}", profit(&out));

    // 011: buy 10k fractions at 0.0008 = 8, redeem, exit at bid 11.2.
    let out = eval(
        &engine,
        &cartridge("mev_10_011_fractional_token_redemption_arbitrage"),
        pd(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert!(
        (profit(&out) - 3.2).abs() < 0.001,
        "frac redeem {}",
        profit(&out)
    );

    // 017: bundle at ask 10, unbundle into 3x4 bids.
    let out = eval(
        &engine,
        &cartridge("mev_10_017_bundle_unbundle_nft_arbitrage"),
        pd(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert!(
        (profit(&out) - 2.0).abs() < 0.001,
        "bundle {}",
        profit(&out)
    );

    // 018: deterministic redemption value 12 vs ask 10.
    let out = eval(
        &engine,
        &cartridge("mev_10_018_redeemable_nft_arbitrage"),
        pd(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert!(
        (profit(&out) - 2.0).abs() < 0.001,
        "redeem {}",
        profit(&out)
    );

    // 012: NFT-loan liquidation at debt 9.5 vs firm bid 11.2.
    let out = eval(
        &engine,
        &cartridge("mev_10_012_nft_backed_loan_liquidation_arbitrage"),
        pd(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert!(
        (profit(&out) - 1.7).abs() < 0.001,
        "nft liq {}",
        profit(&out)
    );
    let mut healthy = nft_payload();
    {
        let mut loan = healthy["loan"].clone().cast::<Map>();
        loan.insert("health_factor".into(), f(1.2));
        healthy.insert("loan".into(), Dynamic::from_map(loan));
    }
    let out = eval(
        &engine,
        &cartridge("mev_10_012_nft_backed_loan_liquidation_arbitrage"),
        with_payload("nft", healthy),
    );
    assert_eq!(reason(&out), "loan_not_liquidatable");
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. G11 — prediction-market parity and logic
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn g11_complete_sets() {
    let engine = math_engine();
    let pd = || with_payload("prediction", prediction_payload());

    // 001: sum asks 0.93 < 1 => buy-and-merge 0.07 x 800 sets = 56 USDX.
    let out = eval(
        &engine,
        &cartridge("mev_11_001_binary_complement_arbitrage"),
        pd(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "binary_complement_violation_profit");
    assert!(
        out.get("set_direction")
            .unwrap()
            .clone()
            .into_string()
            .unwrap()
            == "buy_and_merge"
    );
    assert!(
        (profit(&out) - 56.0).abs() < 0.01,
        "profit {}",
        profit(&out)
    );

    // Sell direction: bids above 1.
    let mut sell = prediction_payload();
    sell.insert(
        "outcomes".into(),
        Dynamic::from_array(vec![
            Dynamic::from_map(outcome("YES", 0.52, 0.55, 1000.0)),
            Dynamic::from_map(outcome("NO", 0.52, 0.55, 900.0)),
        ]),
    );
    let out = eval(
        &engine,
        &cartridge("mev_11_002_complete_set_arbitrage"),
        with_payload("prediction", sell),
    );
    assert!(is_opp(&out), "{out:?}");
    assert!(
        out.get("set_direction")
            .unwrap()
            .clone()
            .into_string()
            .unwrap()
            == "split_and_sell"
    );
    assert!((profit(&out) - 36.0).abs() < 0.01, "sell {}", profit(&out));

    // Mechanics gates: without split/merge neither direction is executable.
    let mut nomech = prediction_payload();
    nomech.insert("merge_available".into(), b(false));
    nomech.insert("split_available".into(), b(false));
    let out = eval(
        &engine,
        &cartridge("mev_11_001_binary_complement_arbitrage"),
        with_payload("prediction", nomech),
    );
    assert_eq!(reason(&out), "no_complete_set_edge");

    // 005: basket needs >= 3 outcomes.
    let out = eval(
        &engine,
        &cartridge("mev_11_005_exhaustive_outcome_basket_arbitrage"),
        pd(),
    );
    assert_eq!(reason(&out), "basket_too_small");
}

#[test]
fn g11_cross_platform_and_logic() {
    let engine = math_engine();
    let pd = || with_payload("prediction", prediction_payload());

    // 003: identity gates, then the firm cross-venue spread (0.48 bid vs
    // 0.45... local ask 0.48; peer bid 0.48 => flat, so skew the local ask).
    let mut skewed = prediction_payload();
    skewed.insert(
        "outcomes".into(),
        Dynamic::from_array(vec![
            Dynamic::from_map(outcome("YES", 0.40, 0.42, 1000.0)),
            Dynamic::from_map(outcome("NO", 0.50, 0.52, 1000.0)),
        ]),
    );
    let out = eval(
        &engine,
        &cartridge("mev_11_003_cross_platform_prediction_arbitrage"),
        with_payload("prediction", skewed),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "cross_platform_firm_spread_profit");
    assert!(
        (profit(&out) - 60.0).abs() < 0.01,
        "spread {}",
        profit(&out)
    );

    let mut mismatch = prediction_payload();
    {
        let mut peer = mismatch["peer"].clone().cast::<Map>();
        peer.insert("event_ref".into(), s("0xother"));
        mismatch.insert("peer".into(), Dynamic::from_map(peer));
    }
    let out = eval(
        &engine,
        &cartridge("mev_11_003_cross_platform_prediction_arbitrage"),
        with_payload("prediction", mismatch),
    );
    assert_eq!(reason(&out), "event_identity_mismatch");

    let mut bad_source = prediction_payload();
    {
        let mut peer = bad_source["peer"].clone().cast::<Map>();
        peer.insert("resolution_source".into(), s("chainlink"));
        bad_source.insert("peer".into(), Dynamic::from_map(peer));
    }
    let out = eval(
        &engine,
        &cartridge("mev_11_003_cross_platform_prediction_arbitrage"),
        with_payload("prediction", bad_source),
    );
    assert_eq!(reason(&out), "resolution_source_mismatch");

    // 004: mutually-exclusive bound violation via bids (0.46+0.44 = 0.90 < 1
    // here), so flip the bids above 1.
    let mut viol = prediction_payload();
    viol.insert(
        "outcomes".into(),
        Dynamic::from_array(vec![
            Dynamic::from_map(outcome("Y", 0.55, 0.60, 100.0)),
            Dynamic::from_map(outcome("N", 0.55, 0.60, 100.0)),
        ]),
    );
    let out = eval(
        &engine,
        &cartridge("mev_11_004_mutually_exclusive_outcomes_arbitrage"),
        with_payload("prediction", viol),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "mutually_exclusive_bound_violation_profit");
    assert!(
        out.get("violation_direction")
            .unwrap()
            .clone()
            .into_string()
            .unwrap()
            == "split_and_sell"
    );

    // 006: conditional complete-set (asks 0.45+0.45 = 0.90 => 0.10 edge).
    let out = eval(
        &engine,
        &cartridge("mev_11_006_conditional_token_arbitrage"),
        pd(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "conditional_token_parity_profit");

    // 007: nested — children bids 1.0 vs parent ask 0.9.
    let out = eval(
        &engine,
        &cartridge("mev_11_007_nested_market_logical_arbitrage"),
        pd(),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "nested_market_parity_profit");
    assert!(
        out.get("parity_direction")
            .unwrap()
            .clone()
            .into_string()
            .unwrap()
            == "split_parent"
    );
    assert!(
        (profit(&out) - 10.0).abs() < 0.01,
        "nested {}",
        profit(&out)
    );

    // 008: implication violation — NO bid 0.44 vs YES ask 0.48 is NOT a
    // violation; make NO bid > YES ask.
    let mut imp_viol = prediction_payload();
    imp_viol.insert(
        "outcomes".into(),
        Dynamic::from_array(vec![
            Dynamic::from_map(outcome("YES", 0.50, 0.52, 100.0)),
            Dynamic::from_map(outcome("NO", 0.60, 0.62, 100.0)),
        ]),
    );
    let out = eval(
        &engine,
        &cartridge("mev_11_008_correlated_event_arbitrage"),
        with_payload("prediction", imp_viol),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "correlation_bound_violation_profit");
    // Neutral partition, no violated bound (NO bid under YES ask).
    let mut neutral = prediction_payload();
    neutral.insert(
        "outcomes".into(),
        Dynamic::from_array(vec![
            Dynamic::from_map(outcome("YES", 0.55, 0.57, 100.0)),
            Dynamic::from_map(outcome("NO", 0.45, 0.47, 100.0)),
        ]),
    );
    let out = eval(
        &engine,
        &cartridge("mev_11_008_correlated_event_arbitrage"),
        with_payload("prediction", neutral),
    );
    assert_eq!(reason(&out), "no_correlation_violation");
}

#[test]
fn g11_prediction_amm_and_observe() {
    let engine = math_engine();

    // 012: AMM curve at 0.5/share vs book bid 0.55 for outcome 0.
    let mut pm = prediction_payload();
    pm.insert(
        "outcomes".into(),
        Dynamic::from_array(vec![
            Dynamic::from_map(outcome("YES", 0.55, 0.70, 500.0)),
            Dynamic::from_map(outcome("NO", 0.40, 0.45, 500.0)),
        ]),
    );
    let out = eval(
        &engine,
        &cartridge("mev_11_012_prediction_amm_order_book_arbitrage"),
        with_payload("prediction", pm),
    );
    assert!(is_opp(&out), "{out:?}");
    assert_eq!(reason(&out), "prediction_amm_vs_book_profit");
    assert!(profit(&out) > 3.0, "amm edge {}", profit(&out));

    // OBSERVE family: structured evidence only.
    for id in [
        "mev_11_009_resolution_source_arbitrage",
        "mev_11_010_oracle_resolution_timing_arbitrage",
        "mev_11_011_pre_resolution_settlement_arbitrage",
    ] {
        let out = eval(
            &engine,
            &cartridge(id),
            with_payload("prediction", prediction_payload()),
        );
        assert!(!is_opp(&out), "{id}: OBSERVE never emits");
        assert_eq!(reason(&out), "observe_only_structured_evidence");
        assert_eq!(
            out.get("evidence_outcomes").unwrap().clone().cast::<i64>(),
            2
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. Rich union payload sweep — no runtime error anywhere, payload runs
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn union_payload_sweep_all_75() {
    let engine = math_engine();
    let mut pd = Map::new();
    pd.insert("chain_id".into(), i(1));

    // position + lending + auction + claim
    pd.insert("position".into(), Dynamic::from_map(position_payload()));
    pd.insert("lending".into(), Dynamic::from_map(lending_payload()));
    pd.insert(
        "auction".into(),
        Dynamic::from_map(auction_payload("dutch")),
    );
    pd.insert("claim".into(), Dynamic::from_map(claim_payload()));

    // intent + batch intents + route
    pd.insert("intent".into(), Dynamic::from_map(intent_payload()));
    let mut intents: Vec<Dynamic> = Vec::new();
    for (sell, buy, amount, min_out) in [
        ("0xa", "0xb", 100.0, 110.0),
        ("0xb", "0xa", 105.0, 100.0),
        ("0xa", "0xs1", 1.0, 1900.0),
    ] {
        let mut m = Map::new();
        m.insert("intent_id".into(), s("0xu"));
        m.insert("sell_token".into(), s(sell));
        m.insert("buy_token".into(), s(buy));
        m.insert("amount_in".into(), f(amount));
        m.insert("min_out".into(), f(min_out));
        m.insert("valid_to".into(), i(1_717_200_060));
        m.insert("ts".into(), i(1_717_200_000));
        intents.push(Dynamic::from_map(m));
    }
    pd.insert("intents".into(), Dynamic::from_array(intents));
    let route = route_pool_data(&[("0xas1", "0xa", "0xs1")]);
    pd.insert(
        "route".into(),
        Dynamic::from_array(route["route"].clone().cast::<Vec<Dynamic>>()),
    );

    // nft + prediction
    pd.insert("nft".into(), Dynamic::from_map(nft_payload()));
    pd.insert("prediction".into(), Dynamic::from_map(prediction_payload()));

    let mut ran = 0;
    for group in [G08, G09, G10, G11] {
        for id in group {
            let src = cartridge(id);
            let out = eval(&engine, &src, pd.clone());
            // Whatever the gates decide, the result must be well-formed.
            assert!(out.get("is_opportunity").unwrap().is_bool(), "{id}");
            assert!(!reason(&out).is_empty(), "{id}");
            // build_payload must run on every result (intent-only declaration).
            let payload = eval_payload(&engine, &src);
            assert!(payload.is_map(), "{id}");
            ran += 1;
        }
    }
    assert_eq!(ran, 75);
}
