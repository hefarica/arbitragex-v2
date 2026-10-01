//! Functional pilot coverage for the R_CLOSED_CYCLE × DETERMINISTIC_EXECUTABLE
//! batch (the "Tier 1" pilot — 25 generated cartridges under strategies/).
//!
//! Unlike `cartridge_syntax_validate` (compile-only) this RUNS each cartridge's
//! `init_strategy()`, `evaluate_opportunity()` and `build_payload()` against a
//! deterministic fixture mirroring the production host bindings
//! (src/cartridge/host_bindings.rs). Fixtures speak raw 18-dec units like the
//! runner (RHAI-08 semantics).
//!
//! Each cartridge constrains its own route shape via init_strategy()
//! `min_legs`/`max_legs` (triangular=3, quadrangular=4, …), so the fixture
//! builds a closed N-leg route per cartridge with N taken from its own
//! metadata: +2% rate on every leg (product 1.02^N clears N×30bps fees) must
//! surface as an opportunity; a flat 1:1 cycle must fail the prefilter.
//!
//! Run: cargo test --test cartridge_r_closed_cycle_test -- --nocapture

use rhai::{Dynamic, Engine, Map, Scope};
use std::path::{Path, PathBuf};

const PROFITABLE_PILOT: usize = 25;

/// The 25-file pilot: strategies whose metadata declares both
/// `detector_id: "R_CLOSED_CYCLE"` and `execution_class: "DETERMINISTIC_EXECUTABLE"`.
fn collect_pilot() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("cartridges")
        .join("strategies");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("strategies dir") {
        let path = entry.expect("entry").path();
        if path.extension().map(|e| e == "rhai").unwrap_or(false) {
            let src = std::fs::read_to_string(&path).unwrap_or_default();
            // PILOT-TEST-V4-01 (2026-10-01, docs/verification/MEV-01-001-
            // implementacion-auditoria.md §H8): el patrón v3 (clave sin
            // comillas) no matchea la librería v4 desplegada, que escribe
            // `"detector_id": "R_CLOSED_CYCLE"` — collect_pilot() encontraba
            // 0 de las ≥25 esperadas y el test moría en su primer assert.
            // Se aceptan AMBAS formas: la v4 citada (actual) y la v3 legado.
            let detector = src.contains("detector_id: \"R_CLOSED_CYCLE\"")
                || src.contains("\"detector_id\": \"R_CLOSED_CYCLE\"");
            let exec_class = src.contains("execution_class: \"DETERMINISTIC_EXECUTABLE\"")
                || src.contains("\"execution_class\": \"DETERMINISTIC_EXECUTABLE\"");
            if detector && exec_class {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Mirror of production host_bindings.rs (see cartridge_strategies_test.rs for
/// the stub rationale). Every pool of the fixture world is `0xP<n>` with
/// r0=1e24 and r1 = 1.02e24 (profitable) or 1e24 (flat), raw 18-dec units.
fn pilot_engine(profitable: bool) -> Engine {
    let mut engine = Engine::new();
    engine.set_max_expr_depths(64, 32); // release limits (mirror the runner)
    engine.set_max_operations(2_000_000);

    engine.register_fn("get_chain_id", || -> Dynamic { Dynamic::from(1_i64) });
    engine.register_fn("get_block_number", || -> Dynamic {
        Dynamic::from(20_000_000_i64)
    });
    engine.register_fn("get_base_fee", || -> Dynamic { Dynamic::from(30.0_f64) });
    engine.register_fn("get_timestamp", || -> Dynamic {
        Dynamic::from(1_717_200_000_i64)
    });
    engine.register_fn("log_quantum", |_l: &str, _m: &str| -> Dynamic {
        Dynamic::UNIT
    });
    engine.register_fn("emit_signal", |_s: &str, _d: Map| -> Dynamic {
        Dynamic::UNIT
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
    engine.register_fn("from_wei", |wei: &str, dec: i64| -> Dynamic {
        let v: f64 = wei.parse().unwrap_or(0.0);
        Dynamic::from(v / 10f64.powi(dec as i32))
    });
    engine.register_fn("to_float", |s: &str| -> Dynamic {
        Dynamic::from(s.parse::<f64>().unwrap_or(0.0))
    });

    // Fixture world: every token is 18-dec and prices at $3000 (the route's
    // base token is whatever token_in0 happens to be).
    engine.register_fn("get_token_meta", |_addr: &str| -> Dynamic {
        let mut m = Map::new();
        m.insert("symbol".into(), Dynamic::from("WETH".to_string()));
        m.insert("decimals".into(), Dynamic::from(18_i64));
        m.insert("is_stablecoin".into(), Dynamic::from(false));
        Dynamic::from_map(m)
    });
    engine.register_fn("get_token_price_usd", |_addr: &str| -> Dynamic {
        Dynamic::from(3000.0_f64)
    });
    engine.register_fn("get_math_evidence", |_id: &str| -> Dynamic {
        Dynamic::UNIT
    });
    engine.register_fn("get_v3_slot0", |_addr: &str| -> Dynamic { Dynamic::UNIT });
    engine.register_fn("v3_arb_enabled", || -> Dynamic { Dynamic::from(false) });
    engine.register_fn(
        "v3_amount_out_single_tick_pips",
        |_a: f64, _b: f64, _c: f64, _d: bool| -> Dynamic { Dynamic::from(0.0_f64) },
    );

    let r1 = if profitable {
        "1020000000000000000000000"
    } else {
        "1000000000000000000000000"
    };
    engine.register_fn("get_reserves", move |addr: &str| -> Dynamic {
        if addr.starts_with("0xP") {
            let mut m = Map::new();
            m.insert(
                "r0".into(),
                Dynamic::from("1000000000000000000000000".to_string()),
            );
            m.insert("r1".into(), Dynamic::from(r1.to_string()));
            m.insert("block".into(), Dynamic::from(20_000_000_i64));
            m.insert("ts".into(), Dynamic::from(1_717_200_000_i64));
            Dynamic::from_map(m)
        } else {
            Dynamic::UNIT
        }
    });
    register_agent_v4_pilot(&mut engine, profitable);
    engine
}

/// Registra los 7 bindings agent_v4_* con las mismas aridades que
/// `rhai_agent_bridge::register` (patrón de `cartridge_wave_b_test.rs`).
/// PILOT-TEST-V4-01 (2026-10-01): los cartuchos v4 del lote piloto coordinan
/// a través de estos bindings en lugar de computar inline; sin registrarlos,
/// `Function not found: agent_v4_discover` mata la evaluación antes de
/// producir ninguna observación. Hallazgo al ejecutar la certificación que
/// exige docs/verification/MEV-01-001-implementacion-auditoria.md §H8.
fn register_agent_v4_pilot(engine: &mut Engine, profitable: bool) {
    engine.register_fn(
        "agent_v4_discover",
        move |ctx: Dynamic, _spec: Dynamic| -> Dynamic {
            let Some(cm) = ctx.try_cast::<Map>() else {
                return Dynamic::UNIT;
            };
            // Candidado del fixture: la ruta del pool_data al tamaño fijo.
            let mut cand = Map::new();
            cand.insert(
                "amount_in_raw".into(),
                Dynamic::from("1000000000000000000".to_string()),
            );
            let pools: Vec<Dynamic> = cm
                .get("route")
                .and_then(|d| d.clone().into_array().ok())
                .map(|legs| {
                    legs.iter()
                        .map(|l| {
                            l.clone()
                                .try_cast::<Map>()
                                .and_then(|m| m.get("pool").cloned())
                                .unwrap_or_else(|| Dynamic::from("0xP0".to_string()))
                        })
                        .collect()
                })
                .unwrap_or_default();
            cand.insert("edge_ids".into(), Dynamic::from_array(pools));
            let mut d = Map::new();
            d.insert("status".into(), Dynamic::from("READY".to_string()));
            d.insert(
                "candidates".into(),
                Dynamic::from_array(vec![Dynamic::from(cand)]),
            );
            Dynamic::from_map(d)
        },
    );
    engine.register_fn(
        "agent_v4_quote",
        move |ctx: Dynamic, _spec: Dynamic, candidate: Dynamic| -> Dynamic {
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
            // Sin registro de operadores en el fixture: evidencia vacía pero
            // presente (el veredicto vive en economic_check).
            let mut m = Map::new();
            m.insert("operators".into(), Dynamic::from_map(Map::new()));
            m.insert(
                "snapshot_id".into(),
                Dynamic::from("fixture_pilot".to_string()),
            );
            Dynamic::from_map(m)
        },
    );
    engine.register_fn(
        "agent_v4_economic_check",
        move |_ctx: Dynamic,
              _spec: Dynamic,
              _candidate: Dynamic,
              _quote: Dynamic,
              _evidence: Dynamic,
              _request: Dynamic|
              -> Dynamic {
            // El veredicto ES el fixture: ciclo +2%/leg vs plano 1:1.
            let mut m = Map::new();
            if profitable {
                m.insert("status".into(), Dynamic::from("COMPUTED".to_string()));
                m.insert("candidate_eligible".into(), Dynamic::from(true));
                m.insert("net_profit_usd".into(), Dynamic::from("42.0".to_string()));
                m.insert("estimated_profit".into(), Dynamic::from(42.0_f64));
                m.insert(
                    "reason".into(),
                    Dynamic::from("fixture_profitable_cycle".to_string()),
                );
            } else {
                m.insert("status".into(), Dynamic::from("DATA_GAP".to_string()));
                m.insert("candidate_eligible".into(), Dynamic::from(false));
                m.insert("net_profit_usd".into(), Dynamic::UNIT);
                m.insert(
                    "reason".into(),
                    Dynamic::from("flat_cycle_not_profitable".to_string()),
                );
            }
            Dynamic::from_map(m)
        },
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
         _observations: Dynamic,
         discovery: Dynamic|
         -> Dynamic {
            let mut out = if best.is_unit() {
                // Sin candidato: gap honesto del discovery (R8), sin profit.
                let dm = discovery.try_cast::<Map>().unwrap_or_default();
                let mut m = Map::new();
                m.insert(
                    "status".into(),
                    dm.get("status")
                        .cloned()
                        .unwrap_or_else(|| Dynamic::from("NO_CANDIDATE".to_string())),
                );
                m.insert(
                    "reason".into(),
                    dm.get("reason")
                        .cloned()
                        .unwrap_or_else(|| Dynamic::from("no_computed_candidate".to_string())),
                );
                m.insert("net_profit_usd".into(), Dynamic::UNIT);
                m.insert("estimated_profit".into(), Dynamic::UNIT);
                m
            } else if let Some(mut bm) = best.clone().try_cast::<Map>() {
                // Contrato de sello de PRODUCCIÓN (rhai_agent_bridge.rs:835):
                // `is_opportunity = (candidate_eligible == true)`. El fixture
                // replica esa derivación para que el lote piloto se evalúe
                // bajo la misma semántica que el backend.
                let eligible = bm
                    .get("candidate_eligible")
                    .and_then(|d| d.clone().as_bool().ok())
                    .unwrap_or(false);
                bm.insert("is_opportunity".into(), Dynamic::from(eligible));
                bm
            } else {
                Map::new()
            };
            out.insert(
                "contract_version".into(),
                Dynamic::from("arbx.cartridge.agent/4".to_string()),
            );
            if let Some(sm) = spec.try_cast::<Map>() {
                if let Some(mev) = sm.get("mev_id") {
                    out.insert("mev_id".into(), mev.clone());
                }
                if let Some(det) = sm.get("detector_id") {
                    out.insert("detector_id".into(), det.clone());
                }
            }
            if let Some(cm) = ctx.try_cast::<Map>() {
                if let Some(len) = cm
                    .get("route")
                    .and_then(|d| d.clone().into_array().ok())
                    .map(|a| a.len() as i64)
                {
                    out.insert("evidence_route_legs".into(), Dynamic::from(len));
                }
            }
            Dynamic::from_map(out)
        },
    );
    engine.register_fn(
        "agent_v4_build_payload",
        |_ctx: Dynamic, _spec: Dynamic| -> Dynamic {
            // Declaración de INTENCIÓN solo lectura (sin signer, sin bytes
            // de ejecución): el payload real lo construye el backend.
            let mut m = Map::new();
            m.insert("calldata".into(), Dynamic::from("0x00".to_string()));
            m.insert("to".into(), Dynamic::UNIT);
            m.insert("value".into(), Dynamic::from("0".to_string()));
            m.insert("chain_id".into(), Dynamic::from(1_i64));
            Dynamic::from_map(m)
        },
    );
}

/// Closed N-leg route t0→t1→…→t(n-1)→t0 through pools 0xP1..0xPn.
fn closed_route_pd(n: usize) -> Map {
    let leg = |pool: String, tin: String, tout: String| -> Dynamic {
        let mut m = Map::new();
        m.insert("pool".into(), Dynamic::from(pool));
        m.insert("token_in".into(), Dynamic::from(tin));
        m.insert("token_out".into(), Dynamic::from(tout));
        Dynamic::from_map(m)
    };
    let tok = |i: usize| format!("0xt{i}");
    let mut route = Vec::new();
    for i in 0..n {
        let tin = tok(i % n);
        let tout = tok((i + 1) % n);
        route.push(leg(format!("0xP{}", i + 1), tin, tout));
    }
    let mut pd = Map::new();
    pd.insert("route".into(), Dynamic::from_array(route));
    pd.insert("route_closed".into(), Dynamic::from(true));
    pd
}

fn call_map(engine: &Engine, src: &str, func: &str, arg: Dynamic) -> Result<Map, String> {
    let ast = engine.compile(src).map_err(|e| format!("compile: {e}"))?;
    let mut scope = Scope::new();
    let out = engine
        .call_fn::<Dynamic>(&mut scope, &ast, func, (arg,))
        .map_err(|e| format!("{func}: {e}"))?;
    out.try_cast::<Map>()
        .ok_or_else(|| format!("{func}: return is not a map"))
}

/// Route length the cartridge itself declares: clamp into its min/max_legs.
fn route_len_for(engine: &Engine, src: &str) -> usize {
    let ast = engine
        .compile(src)
        .expect("compiles (syntax gate already green)");
    let mut scope = Scope::new();
    let meta = engine
        .call_fn::<Dynamic>(&mut scope, &ast, "init_strategy", ())
        .ok()
        .and_then(|d| d.try_cast::<Map>());
    let min = meta
        .as_ref()
        .and_then(|m| m.get("min_legs"))
        .and_then(|d| d.clone().as_int().ok())
        .unwrap_or(2);
    let max = meta
        .as_ref()
        .and_then(|m| m.get("max_legs"))
        .and_then(|d| d.clone().as_int().ok())
        .unwrap_or(8);
    let n = min.max(2).min(max.max(2));
    n.clamp(2, 8) as usize
}

#[test]
fn r_closed_cycle_pilot_functional() {
    let files = collect_pilot();
    assert!(
        files.len() >= PROFITABLE_PILOT,
        "expected at least {PROFITABLE_PILOT} R_CLOSED_CYCLE×DETERMINISTIC_EXECUTABLE cartridges, found {}",
        files.len()
    );

    let mut failures: Vec<String> = Vec::new();
    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let src = std::fs::read_to_string(path).unwrap();

        // 1. Contract: the three mandatory functions exist.
        let ast = match Engine::new().compile(&src) {
            Ok(a) => a,
            Err(e) => {
                failures.push(format!("{name}: compile failed: {e}"));
                continue;
            }
        };
        let fns: Vec<&str> = ast.iter_functions().map(|f| f.name).collect();
        for required in ["init_strategy", "evaluate_opportunity", "build_payload"] {
            if !fns.contains(&required) {
                failures.push(format!("{name}: missing `{required}`"));
            }
        }

        let ok_engine = pilot_engine(true);
        let n = route_len_for(&ok_engine, &src);

        // 2. Profitable cycle (+2%/leg) must be an opportunity.
        match call_map(
            &ok_engine,
            &src,
            "evaluate_opportunity",
            Dynamic::from_map(closed_route_pd(n)),
        ) {
            Ok(r) => {
                let is_opp = r
                    .get("is_opportunity")
                    .and_then(|d| d.as_bool().ok())
                    .unwrap_or(false);
                if !is_opp {
                    let reason = r
                        .get("reason")
                        .and_then(|d| d.clone().into_string().ok())
                        .unwrap_or_else(|| "?".into());
                    failures.push(format!(
                        "{name}: +2%/leg {n}-leg cycle not an opportunity (reason={reason})"
                    ));
                } else if !r
                    .get("estimated_profit")
                    .and_then(|d| d.as_float().ok())
                    .map(|p| p > 0.0)
                    .unwrap_or(false)
                {
                    failures.push(format!(
                        "{name}: estimated_profit not positive on profitable cycle"
                    ));
                }
            }
            Err(e) => failures.push(format!("{name}: {e}")),
        }

        // 3. Flat 1:1 cycle must fail the marginal prefilter.
        let flat_engine = pilot_engine(false);
        match call_map(
            &flat_engine,
            &src,
            "evaluate_opportunity",
            Dynamic::from_map(closed_route_pd(n)),
        ) {
            Ok(r) => {
                let is_opp = r
                    .get("is_opportunity")
                    .and_then(|d| d.as_bool().ok())
                    .unwrap_or(false);
                if is_opp {
                    failures.push(format!(
                        "{name}: flat 1:1 {n}-leg cycle must not be an opportunity"
                    ));
                }
            }
            Err(e) => failures.push(format!("{name}: flat: {e}")),
        }

        // 4. build_payload declares intent only (read-only, no signer).
        let mut opp = Map::new();
        opp.insert("estimated_profit".into(), Dynamic::from(1.0_f64));
        match call_map(&ok_engine, &src, "build_payload", Dynamic::from_map(opp)) {
            Ok(p) => {
                let calldata = p
                    .get("calldata")
                    .and_then(|d| d.clone().into_string().ok())
                    .unwrap_or_default();
                if !calldata.starts_with("0x") {
                    failures.push(format!("{name}: payload calldata malformed"));
                }
            }
            Err(e) => failures.push(format!("{name}: build_payload: {e}")),
        }
    }

    if !failures.is_empty() {
        for f in &failures {
            eprintln!("PILOT FAIL: {f}");
        }
        panic!(
            "{} of {} pilot cartridges failed",
            failures.len(),
            files.len()
        );
    }
    println!(
        "OK: all {} R_CLOSED_CYCLE pilot cartridges passed functional checks",
        files.len()
    );
}
