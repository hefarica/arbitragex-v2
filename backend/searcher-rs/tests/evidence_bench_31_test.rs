//! WO-15 benchmark medido: costo y cobertura de la evidencia de los ops 1-31
//! (matriz 264×31) por estrategia. Fixture determinista (RULE 00): MarketState
//! sintético con series y reservas, semilla fija, NO telemetría de producción.
//!
//! Responde la pregunta del operador "¿cuánto mejoran los primeros 31 ops las
//! estrategias?" con dos números medidos:
//! (a) cobertura: cuántos ops declarados computan por estrategia (ON) vs 0 (OFF);
//! (b) costo: µs por evaluación de evidencia completa (declared set) vs vacío.

use math_engine::MarketState;
use std::time::Instant;

fn rich_state() -> MarketState {
    let mut features = std::collections::HashMap::new();
    features.insert("mempool_arrivals_per_block".to_string(), 0.5);
    features.insert("block_time_sec".to_string(), 12.0);
    features.insert("block_time_variance_sec2".to_string(), 4.0);
    features.insert("fee_bps".to_string(), 30.0);
    MarketState {
        // 7 snapshots × 3 activos: suficiente para espectrales/game/control.
        price_matrix: vec![
            vec![100.0, 200.0, 50.0],
            vec![101.0, 202.0, 49.0],
            vec![102.0, 204.0, 51.0],
            vec![103.0, 206.0, 48.0],
            vec![104.0, 208.0, 52.0],
            vec![105.0, 210.0, 50.0],
            vec![106.0, 212.0, 51.0],
        ],
        liquidity_reserves: vec![(1_000_000.0, 1_050_000.0), (500_000.0, 500_000.0)],
        gas_price_gwei: 20.0,
        block_timestamp: 1_700_000_000,
        block_number: 18_000_000,
        features,
    }
}

#[test]
fn measured_evidence_cost_and_coverage_for_declared_ops() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("cartridges/strategies");
    let mut declared_sets: Vec<(String, Vec<u8>)> = Vec::new();
    for entry in std::fs::read_dir(&dir)
        .expect("cartridges/strategies legible")
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("mev_") || !name.ends_with(".rhai") {
            continue;
        }
        let src = std::fs::read_to_string(entry.path()).expect("cartucho legible");
        let mut ops: Vec<u8> = Vec::new();
        for pat in [
            "primary_operators: [",
            "secondary_operators: [",
            "\"primary_operators\": [",
            "\"secondary_operators\": [",
        ] {
            if let Some(i) = src.find(pat) {
                let rest = &src[i + pat.len()..];
                if let Some(j) = rest.find(']') {
                    ops.extend(
                        rest[..j]
                            .split(',')
                            .filter_map(|t| t.trim().parse::<u8>().ok()),
                    );
                }
            }
        }
        ops.sort_unstable();
        ops.dedup();
        declared_sets.push((name, ops));
    }
    declared_sets.sort();
    assert_eq!(declared_sets.len(), 264, "canon 264 cartuchos");

    let registry = math_engine::OperatorRegistry::new();
    let state = rich_state();

    // Calentamiento + medición: N pasadas del declared-set por cartucho.
    const PASSES: usize = 200;
    let (mut total_computed, mut total_declared) = (0usize, 0usize);
    let start = Instant::now();
    for _ in 0..PASSES {
        for (_, ops) in &declared_sets {
            for id in ops {
                if let Some(out) = registry.dispatch(*id, &state) {
                    if out.scalar_value.is_some() {
                        total_computed += 1;
                    }
                    total_declared += 1;
                }
            }
        }
    }
    let on_us = start.elapsed().as_micros() as f64 / (PASSES * declared_sets.len()) as f64;

    // Baseline OFF: dispatch de conjunto vacío (lo que cuesta no evaluar nada).
    let start = Instant::now();
    for _ in 0..PASSES {
        for _ in &declared_sets {
            for _ in std::iter::empty::<u8>() {
                let _ = std::iter::empty::<u8>().next();
            }
        }
    }
    let off_us = start.elapsed().as_micros() as f64 / (PASSES * declared_sets.len()) as f64;

    let avg_declared = declared_sets.iter().map(|(_, o)| o.len()).sum::<usize>() as f64
        / declared_sets.len() as f64;
    let compute_rate = total_computed as f64 / total_declared.max(1) as f64;

    println!(
        "=== Evidencia ops 1-31 por estrategia (264 cartuchos, {} pasadas) ===",
        PASSES
    );
    println!("ops declarados por estrategia: avg {avg_declared:.1}");
    println!(
        "tasa de cómputo sobre estado rico: {:.1}% ({}/{})",
        100.0 * compute_rate,
        total_computed,
        total_declared
    );
    println!("costo ON  (declared set completo): {on_us:.1} µs/estrategia");
    println!("costo OFF (conjunto vacío):        {off_us:.2} µs/estrategia");

    // Vectores independientes: el estado rico satisface los prerrequisitos de
    // todos los ops (mismo fixture del smoke all_31), así que la mayoría debe
    // computar; el costo debe ser acotado (evidencia NO puede dominar el ciclo).
    assert!(
        compute_rate > 0.5,
        "con estado rico la mayoría de ops deben computar: {compute_rate:.2}"
    );
    assert!(avg_declared >= 3.0, "avg declarados: {avg_declared}");
    assert!(
        on_us < 5_000.0,
        "evidencia debe costar <5ms/estrategia: {on_us:.0}µs"
    );
}
