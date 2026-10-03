//! FUSILE: Implementacion propia -- Newton-Raphson
//! Halla la raíz de f(x)=0: el tamaño de break-even donde el Topological Yield
//! neto se anula sobre la curva CPMM (slippage/Decoherencia embebido en la
//! curva, como dictamina op_21: "algebraically reduces to"):
//!   gross_yield(x) = r1·γ·x / (r0 + γ·x) − x
//!   f(x)   = gross_yield(x) − gas − break_even_target   (target = 0 ⇒ break-even)
//!   f'(x)  = r1·γ·r0 / (r0 + γ·x)² − 1                  (derivada analítica)
//!   x_{k+1} = x_k − f(x_k) / f'(x_k)
//!
//! Sembrado robusto: x₀ = ½·(gas+target)/(γ·p_pool − 1) = ½·x_lin, donde x_lin
//! es la estimación lineal del break-even. Como f es estrictamente cóncva
//! (f''<0), f(x) ≤ tangente en 0 = f_lin(x), y por tanto x* ≥ x_lin > x₀: la
//! semilla queda estrictamente bajo la raíz con f(x₀) < 0 ⇒ Newton converge
//! monótonamente (x₀ < … < x_{k} ≤ x*) sin overshoot, y f' > f'(x*) > 0 en todo
//! paso (nunca se anula salvo x*→x_peak, caso legítimamente no admitible).
//! gas = gas_price_gwei · 21000 · 1e-9 · p_ref.
//! Categoria: numerical
//!
//! R8 fail-honest: sin reservas, r0≤0, r1≤0, γ≤0, sin edge (x_peak≤0), raíz no
//! rentable (f(x_peak)≤0), |f'|<1e-12 (divergencia), no-convergencia en 50 iters,
//! o raíz fuera de (0, r0] ⇒ scalar_value None. FEATURES-DEFAULTS-01: sin
//! NINGUNA clave de fee ⇒ `reason_fee_unavailable` (nunca 30 bps inventados).
//! `break_even_target` AUSENTE ⇒ se resuelve por la DEFINICIÓN del modelo
//! (`:6`: `target = 0 ⇒ break-even`) y se computa EQUILIBRIO, declarando la
//! resolución con el flag numérico `metadata.break_even_target_defined = 1.0`.
//! No es un default hardcodeado: un `0.003` de fee era un DATO DE MERCADO
//! inventado; un `target = 0` es la condición que define el equilibrio, y sin
//! ella el operador no mediría nada. Un `break_even_target` PRESENTE pero no
//! finito (`NaN`/`inf`) es un DATO INVÁLIDO, no una ausencia ⇒
//! `reason_break_even_target_unavailable`; nunca se degrada a la definición.

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

#[derive(Default)]
pub struct NewtonOperator;

impl NewtonOperator {
    pub fn new() -> Self {
        Self
    }

    /// p_ref = media de la columna 0 de price_matrix (consenso cross-venue).
    fn reference_price(state: &MarketState) -> Option<f64> {
        let col: Vec<f64> = state
            .price_matrix
            .iter()
            .filter_map(|row| row.first().copied())
            .filter(|p| p.is_finite() && *p > 0.0)
            .collect();
        if col.is_empty() {
            return None;
        }
        Some(col.iter().sum::<f64>() / col.len() as f64)
    }

    /// Fee del pool como FRACCION: `fee_bps`/1e4 o `pool_fee` directa.
    ///
    /// `None` si NINGUNA esta presente — un fee desconocido no se asume (antes
    /// 0.003 por convencion V2). `gamma = 1 - fee` mueve la raiz de break-even,
    /// asi que el numero inventado movia el resultado publicado
    /// (FEATURES-DEFAULTS-01, R8).
    fn fee_fraction(state: &MarketState) -> Option<f64> {
        state
            .features
            .get("fee_bps")
            .map(|bps| *bps / 10_000.0)
            .or_else(|| state.features.get("pool_fee").copied())
    }
}

impl TopologicalOperator for NewtonOperator {
    fn id(&self) -> u8 {
        21
    }

    fn name(&self) -> &'static str {
        "Newton-Raphson"
    }

    fn category(&self) -> &'static str {
        "numerical"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let none_out = |reason: &str| OperatorOutput {
            operator_id: 21,
            operator_name: "Newton-Raphson".to_string(),
            scalar_value: None,
            vector_result: None,
            matrix_result: None,
            metadata: {
                let mut m = HashMap::new();
                m.insert("computed".to_string(), 0.0);
                m.insert(format!("reason_{reason}").to_string(), 1.0);
                m
            },
        };

        if state.liquidity_reserves.is_empty() {
            return none_out("no_reserves");
        }
        let (r0, r1) = state.liquidity_reserves[0];
        if !r0.is_finite() || !r1.is_finite() || r0 <= 0.0 || r1 <= 0.0 {
            return none_out("degenerate_pool");
        }

        let Some(fee) = Self::fee_fraction(state) else {
            return none_out("fee_unavailable");
        };
        let gamma = 1.0 - fee;
        if !gamma.is_finite() || gamma <= 0.0 {
            return none_out("invalid_fee");
        }

        let price = Self::reference_price(state).unwrap_or(r1 / r0);
        if !price.is_finite() || price <= 0.0 {
            return none_out("invalid_price");
        }
        // MATH-03 fix (2026-09-24): same dimensional correction as op_15 —
        // measure everything in token0-numerary (price·(out − x)), gas at
        // swap-real units (150k default, features-overridable). 21000 was a
        // plain ETH transfer.
        let gas_units = state
            .features
            .get("gas_units")
            .copied()
            .filter(|v| *v > 0.0 && v.is_finite())
            .unwrap_or(150_000.0);
        let gas = state.gas_price_gwei * gas_units * 1e-9 * price;
        // FEATURES-DEFAULTS-01 distinguía dos cosas que hasta ahora iban juntas,
        // y esa distinción es la que decide qué se hace con el target AUSENTE:
        //
        //   · `break_even_target` es el HURDLE economico de la ecuacion — el
        //     yield neto a superar. Un hurdle CONFIGURADO se mide y se respeta.
        //   · Su AUSENCIA no es un dato de mercado que falte: es la condición de
        //     EQUILIBRIO del modelo (`:6`, `target = 0 ⇒ break-even`). Sin ella
        //     el operador no publica "un umbral que nadie fijó", publica el
        //     tamaño de equilibrio — que es justo lo que su nombre declara.
        //
        // Se separa de un default hardcodeado por PROVENANCE, no por valor: el
        // `0.003` de fee era un dato de mercado inventado y por eso pasó a
        // `reason_fee_unavailable`; `target = 0` no se inventa, se DECLARA, y
        // viaja como `declared_definition` para que un consumidor pueda
        // distinguirlo de una medicion. Un `0.0` PRESENTE sigue siendo un CERO
        // ACREDITADO (hurdle declarado en cero): `measured`, no definición.
        let (break_even_target, target_source) = match state.features.get("break_even_target") {
            Some(v) if v.is_finite() => (*v, "measured"),
            Some(_) => return none_out("break_even_target_unavailable"),
            None => (0.0, "declared_definition"),
        };

        // f(x)  = p·(r1·γ·x/(r0+γ·x) − x) − gas − break_even_target   [token0 numerary]
        // f'(x) = p·r1·γ·r0/(r0+γ·x)² − p
        let f = |x: f64| -> f64 {
            let denom = r0 + gamma * x;
            if denom <= 0.0 {
                return f64::INFINITY;
            }
            price * ((r1 * gamma * x) / denom - x) - gas - break_even_target
        };
        let df = |x: f64| -> f64 {
            let denom = r0 + gamma * x;
            if denom <= 0.0 {
                return f64::INFINITY;
            }
            price * ((r1 * gamma * r0) / (denom * denom) - 1.0)
        };

        let p_pool = r1 / r0;

        // (1) Edge: x_peak = (sqrt(r1·γ·r0) − r0)/γ > 0  ⇔  γ·p_pool > 1.
        let x_peak = ((r1 * gamma * r0).sqrt() - r0) / gamma;
        if !x_peak.is_finite() || x_peak <= 0.0 {
            return none_out("no_edge");
        }
        // (2) Rentabilidad en el pico: debe existir x donde f > 0.
        if f(x_peak) <= 0.0 {
            return none_out("no_profitable_root");
        }
        // (3) f(0) = −(gas+target) < 0; si no, la raíz está en el origen o antes.
        let offset = gas + break_even_target;
        if offset.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
            return none_out("root_at_origin");
        }

        // Semilla x₀ = ½·x_lin < x* (demostrado vía concavidad). x_lin = offset/(γ·p_pool−1).
        let mut x = 0.5 * offset / (gamma * p_pool - 1.0);
        if !x.is_finite() || x <= 0.0 {
            x = 0.5 * x_peak; // fallback conservador
        }

        let tol = 1e-10 * r0;
        let deriv_floor = 1e-12_f64;
        let n_max = 50_usize;
        let mut iters = 0_usize;
        let mut converged = false;
        let mut f_at_root = f(x);
        for _ in 0..n_max {
            iters += 1;
            let fp = df(x);
            if !fp.is_finite() || fp.abs() < deriv_floor {
                return none_out("deriv_singular");
            }
            let fv = f(x);
            let step = fv / fp;
            let x_next = x - step;
            // x_next debe permanecer ≥ 0 (convergencia monótona desde abajo).
            if !x_next.is_finite() || x_next < 0.0 {
                return none_out("divergence");
            }
            x = x_next;
            f_at_root = f(x);
            if step.abs() < tol {
                converged = true;
                break;
            }
        }

        if !converged {
            return none_out("non_converged");
        }
        // Raíz admisible: estrictamente positiva y dentro del bracket (0, r0].
        if !x.is_finite() || x <= 0.0 || x > r0 {
            return none_out("root_out_of_range");
        }

        let mut metadata = HashMap::new();
        metadata.insert("computed".to_string(), 1.0);
        metadata.insert("break_even_size".to_string(), x);
        metadata.insert("residual_f".to_string(), f_at_root);
        metadata.insert("iterations".to_string(), iters as f64);
        metadata.insert("x_peak".to_string(), x_peak);
        metadata.insert("gas_cost".to_string(), gas);
        metadata.insert("break_even_target".to_string(), break_even_target);
        // Flag NUMERICO (el metadata es `HashMap<String, f64>`): 1.0 = el hurdle
        // se resolvio por la DEFINICION de equilibrio (ausente); 0.0 = se midio
        // un hurdle presente (incluido un 0.0 acreditado). El recibo del
        // adaptador refleja la definicion en `defined_inputs`.
        metadata.insert(
            "break_even_target_defined".to_string(),
            if target_source == "measured" {
                0.0
            } else {
                1.0
            },
        );
        metadata.insert("gamma".to_string(), gamma);
        metadata.insert("reference_price".to_string(), price);
        metadata.insert("r0".to_string(), r0);
        metadata.insert("r1".to_string(), r1);

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: Some(x),
            vector_result: Some(vec![x, f_at_root, x_peak]),
            matrix_result: None,
            metadata,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Clave canonica del par del fixture (formato FEATURES-01b):
    /// pair_keys.len() == price_matrix.len(), mismo par en cada fila.
    const PAR_FIXTURE: &str =
        "0xaaaa0000000000000000000000000000000000aa|0xbbbb0000000000000000000000000000000000bb";

    /// Pool primario con edge (r1/r0 = 1.05 ⇒ γ·p_pool > 1, hay x_peak) y
    /// referencia cross-venue 1.01.
    fn state_with(feats: &[(&str, f64)]) -> MarketState {
        MarketState {
            price_matrix: vec![vec![1.01], vec![1.01]],
            pair_keys: vec![PAR_FIXTURE.to_string(); 2],
            liquidity_reserves: vec![(1_000_000.0, 1_050_000.0)],
            gas_price_gwei: 20.0,
            block_timestamp: 1_700_000_000,
            block_number: 18_000_000,
            features: feats.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        }
    }

    /// FEATURES-DEFAULTS-01: sin fee medido ⇒ hueco declarado (antes 30 bps de
    /// convencion, que movia la raiz publicada).
    #[test]
    fn fee_absent_declares_the_gap() {
        let op = NewtonOperator::new();
        let out = op.evaluate(&state_with(&[("break_even_target", 0.0)]));
        assert!(out.scalar_value.is_none());
        assert_eq!(out.metadata.get("computed"), Some(&0.0));
        assert_eq!(out.metadata.get("reason_fee_unavailable"), Some(&1.0));
    }

    /// FEATURES-DEFAULTS-01 separó dos ausencias que antes se trataban igual, y
    /// sólo una de ellas es un hueco:
    ///
    ///   · sin `fee_bps`/`pool_fee` NO hay operador: la comisión es un dato de
    ///     mercado y su ausencia es un hueco (`fee_absent_declares_the_gap`).
    ///   · sin `break_even_target` SÍ hay operador: `target = 0` es la DEFINICIÓN
    ///     de equilibrio (`:6`), no un hurdle inventado. Se computa y se declara
    ///     la definición aplicada.
    ///
    /// La diferencia no es de valor sino de PROVENANCE: `0.003` era un dato de
    /// mercado fabricado; `0` es la condición que define lo que este operador
    /// mide. Verificado por equivalencia: ausente ≡ `0.0` presente.
    #[test]
    fn break_even_target_absent_resolves_by_definition_not_by_default() {
        let op = NewtonOperator::new();
        let absent = op.evaluate(&state_with(&[("fee_bps", 30.0)]));
        assert_eq!(absent.metadata.get("computed"), Some(&1.0));
        assert_eq!(
            absent.metadata.get("break_even_target_defined"),
            Some(&1.0),
            "la resolución debe viajar declarada, no inferirse del valor"
        );
        assert_eq!(absent.metadata.get("break_even_target"), Some(&0.0));

        let declared_zero = op.evaluate(&state_with(&[
            ("fee_bps", 30.0),
            ("break_even_target", 0.0),
        ]));
        assert_eq!(declared_zero.metadata.get("computed"), Some(&1.0));
        assert_eq!(
            declared_zero.metadata.get("break_even_target_defined"),
            Some(&0.0),
            "un 0.0 PRESENTE es un cero ACREDITADO: medido, no definido"
        );

        let x_absent = absent.scalar_value.expect("equilibrio por definición");
        let x_zero = declared_zero.scalar_value.expect("cero acreditado");
        assert!(
            (x_absent - x_zero).abs() < 1e-12,
            "definición y cero acreditado deben dar el MISMO tamaño de \
             equilibrio: {x_absent} vs {x_zero}"
        );
    }

    /// Un `break_even_target` PRESENTE pero no finito es un dato invalido, no una
    /// ausencia — se declara el mismo hueco sin publicar raiz. Y NO se degrada a
    /// la definición: un valor ilegible no se convierte en un cero silencioso.
    #[test]
    fn non_finite_break_even_target_is_rejected_and_not_degraded_to_definition() {
        let op = NewtonOperator::new();
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let out = op.evaluate(&state_with(&[
                ("fee_bps", 30.0),
                ("break_even_target", bad),
            ]));
            assert!(
                out.scalar_value.is_none(),
                "un target no finito ({bad}) no puede publicar una raiz"
            );
            assert_eq!(out.metadata.get("computed"), Some(&0.0));
            assert_eq!(
                out.metadata.get("reason_break_even_target_unavailable"),
                Some(&1.0)
            );
            assert!(
                !out.metadata.contains_key("break_even_target_defined"),
                "un dato invalido no se resuelve por definición: debe quedar como hueco"
            );
        }
    }

    /// Guarda REAL de no-convergencia. El sembrado monótono (`x₀ = ½·x_lin`,
    /// demostrado por concavidad en `:10-15`) cubre el régimen bien condicionado,
    /// así que esta salida sólo se alcanza cuando el propio supuesto del sembrado
    /// se rompe: con `γ·p_pool = 1.00697` (fee 30 bps, `r1/r0 = 1.01`) y un hurdle
    /// de 20 en un pool que sólo rinde `f(x_peak) = 20.7` en el pico, la semilla
    /// queda fuera del bracket (`x₀ = 1434.7`) donde `df(x₀) = −0.0703`, y el paso
    /// `f(x₀)/df(x₀) ≈ 1450` cruza a negativo: `x_next = −14.8`.
    ///
    /// Es una guarda defensiva de la que el operador SALE sin publicar nada — no
    /// un resultado económico. Se verifica la no-convergencia declarada y que no
    /// se publique una raíz. Con el mismo pool y un hurdle de 10 el sistema
    /// converge en 4 iteraciones (`x = 147.0151548373465`, residual 1.4e-13), así
    /// que el caso no es un rechazo ciego. La salida del estado mal condicionado
    /// es `divergence` (`:193-195`), no el `non_converged` de `:204-206`: la
    /// guarda que se dispara es la del paso que cruza a negativo.
    #[test]
    fn ill_conditioned_system_declares_divergence_instead_of_a_root() {
        let mut state = state_with(&[("fee_bps", 30.0), ("break_even_target", 20.0)]);
        state.price_matrix = vec![vec![10.0]]; // p_ref = 10 (dislocación cross-venue)
        state.liquidity_reserves = vec![(1_000_000.0, 1_010_000.0)];

        let out = NewtonOperator::new().evaluate(&state);
        assert!(
            out.scalar_value.is_none(),
            "no debe publicar raíz si Newton no converge: {:?}",
            out.scalar_value
        );
        assert_eq!(out.metadata.get("computed"), Some(&0.0));
        assert_eq!(
            out.metadata.get("reason_divergence"),
            Some(&1.0),
            "la razón debe ser la no-convergencia real, no un genérico"
        );

        // El mismo pool con un hurdle alcanzable converge: la guarda no es ciega.
        let mut reachable = state.clone();
        reachable
            .features
            .insert("break_even_target".to_string(), 10.0);
        let ok = NewtonOperator::new().evaluate(&reachable);
        let x = ok.scalar_value.expect("hurdle alcanzable ⇒ raíz publicada");
        assert!(
            (x - 147.015_154_837_346_5).abs() < 1e-9,
            "raíz esperada 147.0151548373465, fue {x}"
        );
    }

    /// Camino bueno intacto: fee Y target medidos ⇒ computa como antes. El 0.0
    /// PRESENTE es un CERO ACREDITADO (hurdle declarado en cero), no una
    /// ausencia, y por eso SI computa.
    #[test]
    fn accredited_zero_target_still_computes() {
        let op = NewtonOperator::new();
        let out = op.evaluate(&state_with(&[
            ("fee_bps", 30.0),
            ("break_even_target", 0.0),
        ]));
        assert_eq!(out.metadata.get("computed"), Some(&1.0));
        let x = out.scalar_value.expect("raiz de break-even publicada");
        assert!(x.is_finite() && x > 0.0);
        assert!((out.metadata.get("gamma").unwrap() - 0.997).abs() < 1e-12);
        assert!((out.metadata.get("break_even_size").unwrap() - x).abs() < 1e-12);
    }

    /// El hurdle es real: subirlo desplaza el break-even hacia arriba (prueba de
    /// que `break_even_target` se usa de verdad y no era decorativo).
    #[test]
    fn larger_target_moves_the_break_even_up() {
        let op = NewtonOperator::new();
        let base = op
            .evaluate(&state_with(&[
                ("fee_bps", 30.0),
                ("break_even_target", 0.0),
            ]))
            .scalar_value
            .expect("raiz con hurdle 0");
        let raised = op
            .evaluate(&state_with(&[
                ("fee_bps", 30.0),
                ("break_even_target", 10.0),
            ]))
            .scalar_value
            .expect("raiz con hurdle 10");
        assert!(
            raised > base,
            "hurdle mayor ⇒ break-even mayor (base={base}, raised={raised})"
        );
    }

    /// Mismo fee por las dos unidades ⇒ mismo resultado (precedencia preservada).
    #[test]
    fn fee_units_are_equivalent_and_bps_wins() {
        let op = NewtonOperator::new();
        let by_bps = op.evaluate(&state_with(&[
            ("fee_bps", 30.0),
            ("break_even_target", 0.0),
        ]));
        let by_fraction = op.evaluate(&state_with(&[
            ("pool_fee", 0.003),
            ("break_even_target", 0.0),
        ]));
        assert!((by_bps.scalar_value.unwrap() - by_fraction.scalar_value.unwrap()).abs() < 1e-9);
        let both = op.evaluate(&state_with(&[
            ("fee_bps", 100.0),
            ("pool_fee", 0.003),
            ("break_even_target", 0.0),
        ]));
        assert!((both.metadata.get("gamma").unwrap() - 0.99).abs() < 1e-12);
    }
}
