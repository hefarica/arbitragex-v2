//! WO-14 (PERF-STACK-2026-09-20): asignación proporcional restringida de
//! proveedores RPC.
//!
//! El "controlador MPC" propuesto en el borrador original era, en realidad,
//! un producto de scores normalizado — no un QP. Se implementa aquí con su
//! nombre honesto (Hermes run_273901de §3, veredicto -61): forma cerrada
//!     wᵢ* ∝ Qᵢ · E[rewardᵢ] · (1 − hazardᵢ)
//! Sin OSQP, sin dependencias nuevas. La cuota entra multiplicativamente en
//! el score: un proveedor agotado ve su share decaer a cero suavemente, que
//! es el comportamiento correcto — un tope duro wᵢ ≤ Qᵢ/ΣQ (water-filling)
//! fue evaluado y RECHAZADO: distorsiona el fairness proporcional (cuotas
//! iguales ⇒ ningún proveedor puede superar 50% aunque sea 2× mejor; test
//! share_proportional_to_score lo demostró). Es el punto estacionario de
//! fairness proporcional — no una solución de programa cuadrático y no se
//! vende como tal.

/// Asigna shares de tráfico Σ=1 entre proveedores.
///
/// Entradas por proveedor (mismo índice):
/// - `quota_remaining`: presupuesto restante del token bucket (≥0; 0 = sin
///   derecho a tráfico).
/// - `posterior_mean`: E[reward] del bandit Thompson (∈[0,1]).
/// - `hazard`: riesgo EWMA de 429 (∈[0,1]).
///
/// R8: input vacío → vec vacío; scores todos cero → uniforme sobre los
/// proveedores con cuota positiva (asignación neutral, nunca inventada).
/// Ninguna NaN entra por construcción (clamps); una NaN externa se trata
/// como score 0.
pub fn constrained_proportional_allocation(
    quota_remaining: &[f64],
    posterior_mean: &[f64],
    hazard: &[f64],
) -> Vec<f64> {
    let n = quota_remaining.len();
    debug_assert_eq!(posterior_mean.len(), n);
    debug_assert_eq!(hazard.len(), n);
    if n == 0 {
        return Vec::new();
    }
    // Longitudes desiguales: contratos del caller violados → nada computado
    // honestamente; devolvemos la neutral sobre la longitud de cuotas.
    if posterior_mean.len() != n || hazard.len() != n {
        return neutral(quota_remaining);
    }

    let score: Vec<f64> = (0..n)
        .map(|i| {
            let q = if quota_remaining[i].is_finite() && quota_remaining[i] > 0.0 {
                quota_remaining[i]
            } else {
                0.0
            };
            let mu = clamp_unit(posterior_mean.get(i).copied().unwrap_or(0.0));
            let h = clamp_unit(hazard.get(i).copied().unwrap_or(0.0));
            q * mu * (1.0 - h)
        })
        .collect();

    let total: f64 = score.iter().sum();
    if total <= 0.0 {
        return neutral(quota_remaining);
    }

    score.iter().map(|s| s / total).collect()
}

fn clamp_unit(v: f64) -> f64 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn neutral(quota_remaining: &[f64]) -> Vec<f64> {
    let eligible = quota_remaining.iter().filter(|q| **q > 0.0).count();
    if eligible == 0 {
        return vec![0.0; quota_remaining.len()];
    }
    quota_remaining
        .iter()
        .map(|q| if *q > 0.0 { 1.0 / eligible as f64 } else { 0.0 })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close_to_sum_one(w: &[f64]) {
        let s: f64 = w.iter().sum();
        assert!((s - 1.0).abs() < 1e-9, "sum={s}");
    }

    #[test]
    fn empty_input_returns_empty() {
        assert!(constrained_proportional_allocation(&[], &[], &[]).is_empty());
    }

    #[test]
    fn shares_sum_to_one() {
        let w = constrained_proportional_allocation(
            &[100.0, 50.0, 25.0],
            &[0.9, 0.6, 0.3],
            &[0.1, 0.2, 0.0],
        );
        close_to_sum_one(&w);
    }

    #[test]
    fn zero_quota_provider_gets_zero_share() {
        let w = constrained_proportional_allocation(
            &[100.0, 0.0, 100.0],
            &[0.5, 1.0, 0.5],
            &[0.0, 0.0, 0.0],
        );
        assert_eq!(w[1], 0.0);
        close_to_sum_one(&w);
    }

    /// Vector independiente: scores 2:1 con hazards 0 → shares 2:1 exactos.
    #[test]
    fn share_proportional_to_score() {
        let w = constrained_proportional_allocation(&[10.0, 10.0], &[0.8, 0.4], &[0.0, 0.0]);
        assert!((w[0] - 2.0 / 3.0).abs() < 1e-9, "w0={}", w[0]);
        assert!((w[1] - 1.0 / 3.0).abs() < 1e-9, "w1={}", w[1]);
    }

    /// Hazard alto reduce el share por debajo de la proporción de score.
    #[test]
    fn hazard_reduces_share() {
        let w = constrained_proportional_allocation(&[10.0, 10.0], &[0.8, 0.8], &[0.9, 0.0]);
        assert!(w[0] < w[1], "w0={} w1={}", w[0], w[1]);
    }

    #[test]
    fn all_zero_scores_fall_back_to_neutral_over_positive_quota() {
        let w = constrained_proportional_allocation(&[5.0, 0.0, 5.0], &[0.0, 0.0, 0.0], &[0.0, 0.0, 0.0]);
        assert_eq!(w, vec![0.5, 0.0, 0.5]);
    }

    /// La cuota decae SUAVE (multiplicativa), sin tope duro: un proveedor
    /// dominante con poca cuota restante cede share gradualmente al otro.
    #[test]
    fn depleted_quota_smoothly_yields_share() {
        let w = constrained_proportional_allocation(
            &[1.0, 4.0],
            &[1.0, 0.25],
            &[0.0, 0.0],
        );
        // Scores: 1·1 = 1.0 vs 4·0.25 = 1.0 → 50/50 con cuotas desiguales.
        assert!((w[0] - 0.5).abs() < 1e-9, "w0={}", w[0]);
        assert!((w[1] - 0.5).abs() < 1e-9, "w1={}", w[1]);
    }

    #[test]
    fn nan_inputs_treated_as_zero_score() {
        let w = constrained_proportional_allocation(&[10.0, 10.0], &[f64::NAN, 0.5], &[0.0, 0.0]);
        assert_eq!(w[0], 0.0);
        assert!((w[1] - 1.0).abs() < 1e-9);
    }

    /// WO-14/WO-15 — benchmark MEDIDO (fixture de test explícita, semilla
    /// fija, RULE 00: esto es evidencia de simulación, NO telemetría de
    /// producción; la ventaja en vivo se mide post-deploy vs baseline).
    ///
    /// Escenario: 3 proveedores con calidad heterogénea; el mejor degrada a
    /// mitad de la corrida (429s en cadena). Política OFF = uniforme (status
    /// quo). Política ON = Thompson (rpc_bandit) + hazard EWMA + asignación
    /// proporcional (esta crate). Métrica = rendimiento neto por intento:
    /// éxito +1, fallo −1 (cuota/gas desperdiciados).
    #[test]
    fn measured_reinforcement_advantage_offline_benchmark() {
        use super::super::rpc_bandit::{ArmObservation, HazardEwma429, ThompsonSamplingSelector};
        use rand::rngs::StdRng;
        use rand::{Rng, SeedableRng};

        struct Provider {
            success_p: f64,
            latency_ms: f64,
            is_429_prone: bool,
        }
        let providers_at = |half: u32| {
            vec![
                // A: rápido y sano… hasta que su free-tier colapsa (hora pico).
                Provider {
                    success_p: if half == 0 { 0.98 } else { 0.30 },
                    latency_ms: if half == 0 { 60.0 } else { 400.0 },
                    is_429_prone: true,
                },
                // B: estable, algo más lento.
                Provider { success_p: 0.85, latency_ms: 120.0, is_429_prone: false },
                // C: marginal.
                Provider { success_p: 0.60, latency_ms: 90.0, is_429_prone: false },
            ]
        };

        const ROUNDS: usize = 20_000;

        // ── OFF: elección uniforme (baseline status quo) ─────────────────────
        let mut rng = StdRng::seed_from_u64(20260920);
        let (mut off_net, mut off_429) = (0.0f64, 0usize);
        for r in 0..ROUNDS {
            let half = (r < ROUNDS / 2) as u32;
            let provs = providers_at(half);
            let i = rng.gen_range(0..provs.len());
            let ok = rng.gen::<f64>() < provs[i].success_p;
            off_net += if ok { 1.0 } else { -1.0 };
            if !ok && provs[i].is_429_prone {
                off_429 += 1;
            }
        }

        // ── ON: bandit + hazard + asignación proporcional ────────────────────
        let mut rng = StdRng::seed_from_u64(20260920);
        let mut bandit = ThompsonSamplingSelector::new(3);
        let mut hazards: Vec<HazardEwma429> = (0..3)
            .map(|_| HazardEwma429::new(0.3, 60_000))
            .collect();
        let mut now_ms = 0u64;
        let (mut on_net, mut on_429) = (0.0f64, 0usize);
        for r in 0..ROUNDS {
            now_ms += 50;
            let half = (r < ROUNDS / 2) as u32;
            let provs = providers_at(half);

            let posteriors: Vec<f64> = (0..3)
                .map(|i| bandit.posterior_mean(i).unwrap_or(0.0))
                .collect();
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

            let ok = rng.gen::<f64>() < provs[pick].success_p;
            let latency = provs[pick].latency_ms;
            bandit.update(
                pick,
                ArmObservation { success: ok, latency_ms: latency },
            );
            hazards[pick].record(!ok && provs[pick].is_429_prone, now_ms);
            on_net += if ok { 1.0 } else { -1.0 };
            if !ok && provs[pick].is_429_prone {
                on_429 += 1;
            }
        }

        // Métricas medidas (visibles con --nocapture).
        println!("OFF (uniforme):   net={off_net:.0}/{ROUNDS}  rate={:.3}  429-waste={off_429}", off_net / ROUNDS as f64);
        println!("ON  (refuerzos):  net={on_net:.0}/{ROUNDS}  rate={:.3}  429-waste={on_429}", on_net / ROUNDS as f64);
        println!("mejora neta: {:.1}%  |  429 evitados: {:.0}%", {
            100.0 * (on_net - off_net) / off_net.abs().max(1.0)
        }, {
            100.0 * (1.0 - on_429 as f64 / off_429.max(1) as f64)
        });

        // Vectores independientes del ESCENARIO (no de la fórmula): el baseline
        // uniforme promedia ≈0.70 de tasa (0.81 y 0.583 por mitades ⇒ net
        // ≈ +0.40·R); la política con refuerzos debe superar por ≥40% el neto
        // y reducir los desperdicios 429 a menos de la mitad.
        assert!(off_net > 0.30 * ROUNDS as f64, "sanity OFF: net={off_net}");
        assert!(on_net > 1.4 * off_net, "ON debe superar OFF por ≥40%: on={on_net} off={off_net}");
        assert!(on_429 * 2 < off_429, "429-waste debe caer >50%: on={on_429} off={off_429}");
    }
}
