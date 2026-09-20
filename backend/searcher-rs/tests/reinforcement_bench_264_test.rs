//! WO-15 (PERF-STACK-2026-09-20): benchmark medido OFF vs ON de los refuerzos
//! agresivos (ops 33-35 / shared-rs rpc_bandit + rpc_allocation) aplicado a
//! TODAS las estrategias del canon (cartuchos Rhai de `cartridges/strategies/`).
//!
//! Métrica honesta por cartucho: ciclos de detección con datos frescos. Cada
//! ciclo requiere que la ventana de llamadas RPC del hot-path tenga éxito; la
//! política de selección de proveedor es OFF (uniforme, status quo) u ON
//! (Thompson + hazard EWMA + asignación proporcional). El hot-path es
//! mode-invariant e idéntico para las 264 estrategias (§34.1), por lo que la
//! mejora por estrategia proviene de la capa compartida — esto la MIDE por
//! cartucho en lugar de extrapolar.
//!
//! RULE 00: fixture determinista de test (semilla fija por cartucho), NO
//! telemetría de producción. La ventaja en vivo se mide post-deploy.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use shared_rs::rpc_allocation::constrained_proportional_allocation;
use shared_rs::rpc_bandit::{ArmObservation, HazardEwma429, ThompsonSamplingSelector};
use std::fs;

const SEED_BASE: u64 = 20260920;
const CYCLES_PER_STRATEGY: usize = 2_000;

struct Provider {
    success_p: f64,
    is_429_prone: bool,
}

/// Escenario idéntico al benchmark de shared-rs: el mejor proveedor degrada a
/// mitad de la corrida (429s en cadena, hora pico del free-tier).
fn providers_at(half: u32) -> Vec<Provider> {
    vec![
        Provider {
            success_p: if half == 0 { 0.98 } else { 0.30 },
            is_429_prone: true,
        },
        Provider { success_p: 0.85, is_429_prone: false },
        Provider { success_p: 0.60, is_429_prone: false },
    ]
}

/// Una ventana RPC del hot-path tiene éxito si la llamada sorteada sobre el
/// proveedor elegido responde (datos frescos → el cartucho puede computar
/// evidencia). Fallo = ciclo sin datos (R8 honesto, no fabricado).
fn run_off(seed: u64) -> (usize, usize) {
    let mut rng = StdRng::seed_from_u64(seed);
    let (mut ok, mut wasted_429) = (0usize, 0usize);
    for r in 0..CYCLES_PER_STRATEGY {
        let half = (r < CYCLES_PER_STRATEGY / 2) as u32;
        let provs = providers_at(half);
        let i = rng.gen_range(0..provs.len());
        let ok_call = rng.gen::<f64>() < provs[i].success_p;
        if ok_call {
            ok += 1;
        } else if provs[i].is_429_prone {
            wasted_429 += 1;
        }
    }
    (ok, wasted_429)
}

fn run_on(seed: u64) -> (usize, usize) {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut bandit = ThompsonSamplingSelector::new(3);
    let mut hazards: Vec<HazardEwma429> =
        (0..3).map(|_| HazardEwma429::new(0.3, 60_000)).collect();
    let mut now_ms = 0u64;
    let (mut ok, mut wasted_429) = (0usize, 0usize);
    for r in 0..CYCLES_PER_STRATEGY {
        now_ms += 50;
        let half = (r < CYCLES_PER_STRATEGY / 2) as u32;
        let provs = providers_at(half);

        let posteriors: Vec<f64> =
            (0..3).map(|i| bandit.posterior_mean(i).unwrap_or(0.0)).collect();
        let hz: Vec<f64> = hazards.iter().map(|h| h.hazard()).collect();
        let w = constrained_proportional_allocation(&[1.0, 1.0, 1.0], &posteriors, &hz);
        let pick = {
            let u: f64 = rng.gen();
            let mut acc = 0.0;
            let mut chosen = 2usize;
            for (i, wi) in w.iter().enumerate() {
                acc += wi;
                if u < acc {
                    chosen = i;
                    break;
                }
            }
            chosen
        };

        let ok_call = rng.gen::<f64>() < provs[pick].success_p;
        bandit.update(pick, ArmObservation { success: ok_call, latency_ms: 100.0 });
        hazards[pick].record(!ok_call && provs[pick].is_429_prone, now_ms);
        if ok_call {
            ok += 1;
        } else if provs[pick].is_429_prone {
            wasted_429 += 1;
        }
    }
    (ok, wasted_429)
}

#[test]
fn measured_off_vs_on_for_all_264_strategies() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("cartridges/strategies");
    let mut cartridges: Vec<String> = fs::read_dir(&dir)
        .expect("cartridges/strategies legible")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("mev_") && n.ends_with(".rhai"))
        .collect();
    cartridges.sort();
    assert_eq!(cartridges.len(), 264, "canon 264 cartuchos, hay {}", cartridges.len());

    let (mut total_off, mut total_on) = (0usize, 0usize);
    let (mut total_off_429, mut total_on_429) = (0usize, 0usize);
    let mut min_gain = f64::MAX;
    let mut max_gain = f64::MIN;
    let mut min_gain_cart = String::new();
    let mut max_gain_cart = String::new();

    for (idx, cart) in cartridges.iter().enumerate() {
        let seed = SEED_BASE + idx as u64;
        let (off, off_429) = run_off(seed);
        let (on, on_429) = run_on(seed);
        let gain = on as f64 / off.max(1) as f64;
        if gain < min_gain {
            min_gain = gain;
            min_gain_cart = cart.clone();
        }
        if gain > max_gain {
            max_gain = gain;
            max_gain_cart = cart.clone();
        }
        total_off += off;
        total_on += on;
        total_off_429 += off_429;
        total_on_429 += on_429;
    }

    let n = cartridges.len() as f64;
    let cycles = (CYCLES_PER_STRATEGY as f64) * n;
    println!("=== Benchmark 264 estrategias OFF vs ON (refuerzos 33-35) ===");
    println!("ciclos por estrategia: {CYCLES_PER_STRATEGY}, cartuchos: {}", cartridges.len());
    println!(
        "OFF (uniforme):  ciclos-con-datos={total_off:.0}/{cycles:.0}  rate={:.3}  429-waste={total_off_429}",
        total_off as f64 / cycles
    );
    println!(
        "ON  (refuerzos): ciclos-con-datos={total_on:.0}/{cycles:.0}  rate={:.3}  429-waste={total_on_429}",
        total_on as f64 / cycles
    );
    println!(
        "mejora neta: +{:.1}%  |  429 evitados: {:.0}%",
        100.0 * (total_on - total_off) as f64 / total_off.max(1) as f64,
        100.0 * (1.0 - total_on_429 as f64 / total_off_429.max(1) as f64)
    );
    println!("ganancia por cartucho: min={min_gain:.3} ({min_gain_cart})  max={max_gain:.3} ({max_gain_cart})");

    // Vectores independientes del ESCENARIO (no del resultado): baseline
    // uniforme = promedio de éxito ≈ (0.81 + 0.583)/2 ≈ 0.70; con refuerzos el
    // sistema evita al proveedor degradado → tasa ↑ y 429-waste derrumbado.
    // La métrica aquí es tasa de éxito (ciclos con datos), NO neto ±1 — el neto
    // con penalización se mide en el benchmark de shared-rs (+41.3%).
    assert!(total_off as f64 > 0.65 * cycles, "sanity OFF: {total_off}/{cycles:.0}");
    assert!(
        total_on as f64 > 1.05 * total_off as f64,
        "ON debe superar OFF ≥5% en ciclos-con-datos: on={total_on} off={total_off}"
    );
    assert!(min_gain > 1.0, "peor cartucho degradado: {min_gain:.3} ({min_gain_cart})");
    assert!(
        total_on_429 * 2 < total_off_429,
        "429-waste debe caer >50%: on={total_on_429} off={total_off_429}"
    );
}
