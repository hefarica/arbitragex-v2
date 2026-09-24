//! FUSILE: Implementacion propia -- Optimizacion Multi-Objetivo (NSGA-II)
//! HP-03 (2026-09-08) — nuevo operador 32; diseno fuente: OPERADOR-SEED v1/v2
//! (audits/op32-highperf-2026-09-08/) + GOAL-WORKORDERS HP-02/HP-03.
//!
//! Problema (minimizacion conjunta, todo en unidades de token1):
//!   f1(x) = -Σ_i [ r1_i·γ·x_i/(r0_i + γ·x_i) - x_i ] + gas   (−Topological Yield neto)
//!   f2(x) = CVaR_α{ L_i(x_i) : x_i > 0 },  L_i = γ·x_i/(r0_i + γ·x_i)
//!           (Decoherencia de Estado por pata activa; cola α de la distribution
//!            discreta de decoherencia entre venues — riesgo de ejecucion concentrado)
//!   f3(x) = patas_activas(x) × per_leg_ms   (proxy de latencia de inclusion)
//!   sujeto a x ∈ Ω = Π_i [0, r0_i]   (n ≤ 8 venues de liquidity_reserves)
//!
//! Algoritmo NSGA-II canonico (Deb et al. 2002; mismo patron que usan los
//! solvers CoW en production — arXiv:2510.21647, canon quant-math FINDINGS:50):
//!   fast non-dominated sort + crowding distance + torneo binario
//!   (crowded-comparison) + cruce SBX (η=15, p_c=0.9) + mutacion polinomial
//!   (η_m=20, p_m=1/n) + elitismo (R = P∪Q, llenado por frentes, truncado del
//!   ultimo frente por crowding distance descendente).
//!
//! Determinismo: SmallRng sembrado desde block_number + n + bits del vector de
//! preferencias (patron op_22) — NUNCA Date::now; mismo estado ⇒ salida
//! bit-identica (reproducible para backtesting). Presupuesto acotado por
//! construction: population ∈ [4,128], generations ∈ [1,100] ⇒ evaluaciones
//! ≤ 128·101 = 12.928 (loops `for` acotados, sin recursion). Allocs del run
//! acotadas a O(population×generations) filas de 3 f64 (CVaR sin heap: buffer
//! de pila fijo, see evaluate_objectives).
//!
//! Seleccion por vector de preferencias [yield, riesgo, latencia] (features
//! mo_weight_yield/risk/latency, defaults 0.5/0.3/0.2 del SEED operador),
//! normalizado a suma 1: argmin sobre el frente de la desutilidad ponderada
//! con objetivos min-max normalizados por columna. La seleccion SIEMPRE es un
//! miembro exacto del frente retornado (mismo orden de filas).
//!
//! Degeneracion a 1 objetivo (peso activo unico en yield) ⇒ delega EXACTO en
//! el operador subyacente op_15 (Golden Section sobre el pool primario):
//! mismo escalar, misma semantica. Riesgo/latencia puro ⇒ el optimo es la
//! asignacion nula x=0 (sin Topological Yield que declarar) ⇒ None honesto.
//!
//! R8 fail-honest: sin venues usables, >8 venues, vector de preferencias
//! invalido, o frente vacio tras filtrar no-finitos ⇒ scalar None con
//! reason_*, jamas una solucion fabricada.
//! Categoria: optimization

use super::op_15_golden_section::GoldenSectionOperator;
use super::{MarketState, OperatorOutput, TopologicalOperator};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use std::collections::HashMap;

// ── Limites del presupuesto (budget acotado por construction) ────────────────
/// Maximo de venues (pools) considerados; sobre esto la exploration es
/// infactible en hot-path ⇒ None honesto (misma convencion exacta que op_29).
const MAX_POOLS: usize = 8;
/// Poblacion por defecto / maxima (clamp de features["mo_population"]).
const DEFAULT_POPULATION: usize = 40;
const MIN_POPULATION: usize = 4;
const MAX_POPULATION: usize = 128;
/// Generaciones por defecto / maxima (clamp de features["mo_generations"]).
const DEFAULT_GENERATIONS: usize = 24;
const MAX_GENERATIONS: usize = 100;
/// Indice de distribution SBX (cruce) y de la mutacion polinomial.
const SBX_ETA: f64 = 15.0;
const SBX_PROB: f64 = 0.9;
const MUTATION_ETA: f64 = 20.0;
/// Umbral bajo el cual un peso se considera objetivo inactivo.
const WEIGHT_EPS: f64 = 1e-12;
/// Defaults del vector de preferencias [yield, riesgo, latencia] (SEED operador).
const DEFAULT_W_YIELD: f64 = 0.5;
const DEFAULT_W_RISK: f64 = 0.3;
const DEFAULT_W_LATENCY: f64 = 0.2;

/// Contexto economico inmutable del run (pools reales + parametros declarados).
struct Nsga2Context {
    /// (r0, r1) por venue, ya validados finitos y > 0.
    pools: Vec<(f64, f64)>,
    /// γ = 1 − fee (misma convencion que op_15: fee_bps/1e4 o pool_fee, default 0.003).
    gamma: f64,
    /// Costo de gas total en token1 (misma formula que op_15).
    gas: f64,
    /// α de CVaR ∈ (0,1].
    cvar_alpha: f64,
    /// Latencia declarada por pata activa (ms).
    per_leg_ms: f64,
}

/// Configuracion del algoritmo evolutivo (ya clampeada al presupuesto).
struct Nsga2Config {
    population: usize,
    generations: usize,
}

// ── Nucleo matematico puro (privado, testeable por los property tests) ──────

/// Dominancia Pareto (minimizacion): a domina b si a_j ≤ b_j ∀j y existe j
/// con a_j < b_j.
fn dominates(a: &[f64], b: &[f64]) -> bool {
    debug_assert_eq!(a.len(), b.len());
    let mut any_less = false;
    for (x, y) in a.iter().zip(b.iter()) {
        if x > y {
            return false;
        }
        if x < y {
            any_less = true;
        }
    }
    any_less
}

/// Fast non-dominated sort (Deb 2002): O(M·N²). Devuelve los frentes como
/// listas de indices al vector de objetivos. Determinista (recorrido por
/// indice). NaN ⇒ no-dominancia ⇒ cae al frente 0 (entradas finitas por
/// construction; el caso es defensivo).
fn fast_non_dominated_sort(objectives: &[Vec<f64>]) -> Vec<Vec<usize>> {
    let n = objectives.len();
    let mut dominated_sets: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut dom_count = vec![0usize; n];
    let mut fronts: Vec<Vec<usize>> = vec![Vec::new()];
    for p in 0..n {
        for q in (p + 1)..n {
            if dominates(&objectives[p], &objectives[q]) {
                dominated_sets[p].push(q);
                dom_count[q] += 1;
            } else if dominates(&objectives[q], &objectives[p]) {
                dominated_sets[q].push(p);
                dom_count[p] += 1;
            }
        }
    }
    for (p, &count) in dom_count.iter().enumerate() {
        if count == 0 {
            fronts[0].push(p);
        }
    }
    // Pelado de frentes: cada q cuyo dominante cae en el frente i baja a i+1.
    let mut i = 0;
    while !fronts[i].is_empty() {
        let mut next: Vec<usize> = Vec::new();
        for &p in &fronts[i] {
            for &q in &dominated_sets[p] {
                dom_count[q] -= 1;
                if dom_count[q] == 0 {
                    next.push(q);
                }
            }
        }
        if next.is_empty() {
            break;
        }
        fronts.push(next);
        i += 1;
    }
    fronts
}

/// Crowding distance estandar NSGA-II sobre un frente (objetivos en el orden
/// del frente). Fronteras (primer/ultimo de cada objetivo ordenado) = +∞;
/// interior = Σ_j (f_j[k+1] − f_j[k−1]) / (max_j − min_j), con span 0 ⇒
/// contribucion 0 (duplicados no dividen por cero). Frente ≤ 2 miembros ⇒
/// todos ∞ (todos son frontera). Orden por total_cmp (total, determinista).
fn crowding_distance(front: &[Vec<f64>]) -> Vec<f64> {
    let m = front.len();
    if m <= 2 {
        return vec![f64::INFINITY; m];
    }
    let n_obj = front[0].len();
    let mut dist = vec![0.0_f64; m];
    // Columnas extraidas por objetivo (iteracion sin indice de loop).
    let cols: Vec<Vec<f64>> = (0..n_obj)
        .map(|j| front.iter().map(|member| member[j]).collect())
        .collect();
    for col in &cols {
        let mut idx: Vec<usize> = (0..m).collect();
        idx.sort_by(|&a, &b| col[a].total_cmp(&col[b]));
        dist[idx[0]] = f64::INFINITY;
        dist[idx[m - 1]] = f64::INFINITY;
        let span = col[idx[m - 1]] - col[idx[0]];
        if span > 0.0 && span.is_finite() {
            for k in 1..(m - 1) {
                if dist[idx[k]].is_finite() {
                    dist[idx[k]] += (col[idx[k + 1]] - col[idx[k - 1]]) / span;
                }
            }
        }
    }
    dist
}

/// Seleccion por vector de preferencias: argmin sobre el frente de la
/// desutilidad U_i = Σ_j w_j · n_ij donde n_ij es el objetivo j del miembro i
/// min-max normalizado por columna (span 0 ⇒ 0). Empates ⇒ primer indice.
/// Siempre retorna un indice valido del frente (miembro exacto del frente).
fn select_by_preference(front: &[Vec<f64>], weights: &[f64]) -> usize {
    debug_assert!(!front.is_empty());
    let n_obj = weights.len().min(front[0].len());
    let mut lo = vec![f64::INFINITY; n_obj];
    let mut hi = vec![f64::NEG_INFINITY; n_obj];
    for member in front {
        for j in 0..n_obj {
            lo[j] = lo[j].min(member[j]);
            hi[j] = hi[j].max(member[j]);
        }
    }
    let mut best = 0usize;
    let mut best_u = f64::INFINITY;
    for (i, member) in front.iter().enumerate() {
        let mut u = 0.0;
        for j in 0..n_obj {
            let span = hi[j] - lo[j];
            let normalized = if span > 0.0 && span.is_finite() {
                (member[j] - lo[j]) / span
            } else {
                0.0
            };
            u += weights[j] * normalized;
        }
        if u < best_u {
            best_u = u;
            best = i;
        }
    }
    best
}

/// Evaluacion de los 3 objetivos para una asignacion x (minimizacion).
/// Devuelve [f1, f2, f3] finitos (denominadores ≥ r0 > 0 por bounds).
/// CVaR sin heap: buffer de pila fijo + selection del top-tail (k ≤ MAX_POOLS).
fn evaluate_objectives(x: &[f64], ctx: &Nsga2Context) -> Vec<f64> {
    debug_assert_eq!(x.len(), ctx.pools.len());
    let mut gross = 0.0_f64; // Σ (out_AMM − input)
    let mut legs = [0.0_f64; MAX_POOLS];
    let mut k = 0usize;
    for (i, &xi) in x.iter().enumerate() {
        let (r0, r1) = ctx.pools[i];
        let denom = r0 + ctx.gamma * xi; // ≥ r0 > 0 (bounds + γ > 0)
        gross += (r1 * ctx.gamma * xi) / denom - xi;
        if xi > 0.0 {
            legs[k] = (ctx.gamma * xi) / denom; // decoherencia de la pata
            k += 1;
        }
    }
    // CVaR_α sobre la distribution discreta de decoherencia por pata activa:
    // media de las ⌈α·k⌉ peores (mayores) perdidas; k = 0 ⇒ riesgo 0.
    let risk = if k == 0 {
        0.0
    } else {
        let tail = ((ctx.cvar_alpha * k as f64).ceil() as usize).clamp(1, k);
        let view = &mut legs[..k];
        for i in 0..tail {
            let mut max_i = i;
            for j in (i + 1)..view.len() {
                if view[j] > view[max_i] {
                    max_i = j;
                }
            }
            view.swap(i, max_i);
        }
        view[..tail].iter().sum::<f64>() / tail as f64
    };
    let latency = k as f64 * ctx.per_leg_ms;
    vec![-gross + ctx.gas, risk, latency]
}

/// Cruce SBX (Simulated Binary Crossover) con clipping a bounds.
/// Determinista: orden fijo de draws del RNG.
fn sbx_crossover(
    p1: &[f64],
    p2: &[f64],
    bounds: &[(f64, f64)],
    rng: &mut SmallRng,
    c1: &mut [f64],
    c2: &mut [f64],
) {
    let do_crossover = rng.gen::<f64>() <= SBX_PROB;
    for i in 0..p1.len() {
        let (lo, hi) = bounds[i];
        if do_crossover {
            let u = rng.gen::<f64>();
            let beta = if u <= 0.5 {
                (2.0 * u).powf(1.0 / (SBX_ETA + 1.0))
            } else {
                (1.0 / (2.0 * (1.0 - u))).powf(1.0 / (SBX_ETA + 1.0))
            };
            let a = 0.5 * ((1.0 + beta) * p1[i] + (1.0 - beta) * p2[i]);
            let b = 0.5 * ((1.0 - beta) * p1[i] + (1.0 + beta) * p2[i]);
            c1[i] = a.clamp(lo, hi);
            c2[i] = b.clamp(lo, hi);
        } else {
            c1[i] = p1[i];
            c2[i] = p2[i];
        }
    }
}

/// Mutacion polinomial (η_m fija) con probabilidad 1/n por gen y clipping.
fn polynomial_mutation(x: &mut [f64], bounds: &[(f64, f64)], rng: &mut SmallRng) {
    if x.is_empty() {
        return;
    }
    let pm = 1.0 / x.len() as f64;
    for i in 0..x.len() {
        if rng.gen::<f64>() <= pm {
            let (lo, hi) = bounds[i];
            let u = rng.gen::<f64>();
            let delta = if u < 0.5 {
                (2.0 * u).powf(1.0 / (MUTATION_ETA + 1.0)) - 1.0
            } else {
                1.0 - (2.0 * (1.0 - u)).powf(1.0 / (MUTATION_ETA + 1.0))
            };
            x[i] = (x[i] + delta * (hi - lo)).clamp(lo, hi);
        }
    }
}

/// Torneo binario con crowded-comparison (rank asc, luego distancia desc).
/// Empate ⇒ segundo indice (determinista: regla fija, sin random tie-break).
fn tournament(ranks: &[usize], dist: &[f64], a: usize, b: usize) -> usize {
    if ranks[a] < ranks[b] || (ranks[a] == ranks[b] && dist[a] > dist[b]) {
        a
    } else {
        b
    }
}

/// Run NSGA-II completo. Devuelve (poblacion_final, objetivos_finales,
/// indices_del_frente_0). Loops `for` acotados ⇒ siempre termina (presupuesto
/// population×(generations+1) evaluaciones, garantizado por los clamps).
fn run_nsga2(
    ctx: &Nsga2Context,
    cfg: &Nsga2Config,
    seed: u64,
) -> (Vec<Vec<f64>>, Vec<Vec<f64>>, Vec<usize>) {
    let n = ctx.pools.len();
    let pop = cfg.population;
    let mut rng = SmallRng::seed_from_u64(seed);
    let bounds: Vec<(f64, f64)> = ctx.pools.iter().map(|&(r0, _)| (0.0, r0)).collect();

    // ── Poblacion inicial estructurada + relleno aleatorio determinista ──
    // x = 0 (asignacion nula: esquina de riesgo/latencia cero), un focus lleno
    // y uno al 25% por venue (semillas de las esquinas de yield), resto uniforme.
    let mut population: Vec<Vec<f64>> = Vec::with_capacity(pop);
    if population.len() < pop {
        population.push(vec![0.0; n]); // asignacion nula
    }
    for k in 0..n {
        if population.len() >= pop {
            break;
        }
        let mut v = vec![0.0; n];
        v[k] = ctx.pools[k].0; // focus lleno
        population.push(v);
    }
    for k in 0..n {
        if population.len() >= pop {
            break;
        }
        let mut v = vec![0.0; n];
        v[k] = 0.25 * ctx.pools[k].0; // focus al 25%
        population.push(v);
    }
    while population.len() < pop {
        let v: Vec<f64> = bounds
            .iter()
            .map(|&(lo, hi)| lo + rng.gen::<f64>() * (hi - lo))
            .collect();
        population.push(v);
    }

    let mut objectives: Vec<Vec<f64>> = population
        .iter()
        .map(|x| evaluate_objectives(x, ctx))
        .collect();

    for _gen in 0..cfg.generations {
        // Rango + crowding de la poblacion actual (para el torneo).
        let fronts = fast_non_dominated_sort(&objectives);
        let mut ranks = vec![0usize; pop];
        let mut dist = vec![0.0_f64; pop];
        for (r, front) in fronts.iter().enumerate() {
            let front_objs: Vec<Vec<f64>> = front.iter().map(|&i| objectives[i].clone()).collect();
            let cd = crowding_distance(&front_objs);
            for (c, &i) in front.iter().enumerate() {
                ranks[i] = r;
                dist[i] = cd[c];
            }
        }

        // Descendencia Q por torneo + SBX + mutacion (orden de draws fijo).
        let mut offspring: Vec<Vec<f64>> = Vec::with_capacity(pop);
        for _ in 0..(pop / 2) {
            let a = rng.gen_range(0..pop);
            let b = rng.gen_range(0..pop);
            let p1 = &population[tournament(&ranks, &dist, a, b)];
            let c = rng.gen_range(0..pop);
            let d = rng.gen_range(0..pop);
            let p2 = &population[tournament(&ranks, &dist, c, d)];
            let mut c1 = vec![0.0; n];
            let mut c2 = vec![0.0; n];
            sbx_crossover(p1, p2, &bounds, &mut rng, &mut c1, &mut c2);
            polynomial_mutation(&mut c1, &bounds, &mut rng);
            polynomial_mutation(&mut c2, &bounds, &mut rng);
            offspring.push(c1);
            offspring.push(c2);
        }
        if pop % 2 == 1 {
            let a = rng.gen_range(0..pop);
            let b = rng.gen_range(0..pop);
            let mut c1 = population[tournament(&ranks, &dist, a, b)].clone();
            polynomial_mutation(&mut c1, &bounds, &mut rng);
            offspring.push(c1);
        }

        // Elitismo: R = P ∪ Q, llenado por frentes, truncado del ultimo frente
        // por crowding distance descendente (empates ⇒ indice menor, estable).
        let mut merged: Vec<Vec<f64>> = Vec::with_capacity(2 * pop);
        merged.append(&mut population);
        merged.append(&mut offspring);
        let mut merged_objs: Vec<Vec<f64>> = Vec::with_capacity(2 * pop);
        merged_objs.append(&mut objectives);
        for x in merged.iter().skip(merged_objs.len()) {
            merged_objs.push(evaluate_objectives(x, ctx));
        }
        let fronts_r = fast_non_dominated_sort(&merged_objs);
        let mut next_pop: Vec<Vec<f64>> = Vec::with_capacity(pop);
        let mut next_objs: Vec<Vec<f64>> = Vec::with_capacity(pop);
        'fill: for front in &fronts_r {
            if next_pop.len() + front.len() <= pop {
                for &i in front {
                    next_pop.push(merged[i].clone());
                    next_objs.push(merged_objs[i].clone());
                }
                continue;
            }
            let front_objs: Vec<Vec<f64>> = front.iter().map(|&i| merged_objs[i].clone()).collect();
            let cd = crowding_distance(&front_objs);
            let mut order: Vec<usize> = (0..front.len()).collect();
            order.sort_by(|&a, &b| cd[b].total_cmp(&cd[a])); // desc, estable
            for &o in &order {
                if next_pop.len() >= pop {
                    break 'fill;
                }
                let i = front[o];
                next_pop.push(merged[i].clone());
                next_objs.push(merged_objs[i].clone());
            }
            break;
        }
        population = next_pop;
        objectives = next_objs;
    }

    let fronts = fast_non_dominated_sort(&objectives);
    let front0 = fronts.into_iter().next().unwrap_or_default();
    (population, objectives, front0)
}

/// Fee en bps → fraccion (misma convencion exacta que op_15: fee_bps/1e4,
/// fallback pool_fee, default 0.003).
fn fee_fraction(state: &MarketState) -> f64 {
    state
        .features
        .get("fee_bps")
        .map(|bps| *bps / 10_000.0)
        .or_else(|| state.features.get("pool_fee").copied())
        .unwrap_or(0.003)
}

/// Lee un f64 de features validandolo finito y dentro de [min, max], con default.
fn feature_f64(
    features: &HashMap<String, f64>,
    key: &str,
    default: f64,
    min: f64,
    max: f64,
) -> f64 {
    match features.get(key) {
        Some(v) if v.is_finite() && *v >= min && *v <= max => *v,
        _ => default,
    }
}

/// Lee un usize de features (f64 finito ≥ 1) clampeado a [min, max].
/// Float→int `as` satura (Rust ≥ 1.45) ⇒ valores gigantes caen al clamp.
fn feature_usize_clamped(
    features: &HashMap<String, f64>,
    key: &str,
    default: usize,
    min: usize,
    max: usize,
) -> usize {
    match features.get(key) {
        Some(v) if v.is_finite() && *v >= 1.0 => (*v as usize).clamp(min, max),
        _ => default,
    }
}

#[derive(Default)]
pub struct MultiObjectiveOperator;

impl MultiObjectiveOperator {
    pub fn new() -> Self {
        Self
    }

    fn none_out(reason: &str) -> OperatorOutput {
        OperatorOutput {
            operator_id: 32,
            operator_name: "Optimizacion Multi-Objetivo".to_string(),
            scalar_value: None,
            vector_result: None,
            matrix_result: None,
            metadata: {
                let mut m = HashMap::new();
                m.insert("computed".to_string(), 0.0);
                m.insert(format!("reason_{reason}").to_string(), 1.0);
                m
            },
        }
    }

    /// p_ref = media de la columna 0 de price_matrix (misma convencion op_15).
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
}

impl TopologicalOperator for MultiObjectiveOperator {
    fn id(&self) -> u8 {
        32
    }

    fn name(&self) -> &'static str {
        "Optimizacion Multi-Objetivo"
    }

    fn category(&self) -> &'static str {
        "optimization"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        // ── Venues usables (finitas, positivas) ──────────────────────────────
        let pools: Vec<(f64, f64)> = state
            .liquidity_reserves
            .iter()
            .filter(|(r0, r1)| r0.is_finite() && r1.is_finite() && *r0 > 0.0 && *r1 > 0.0)
            .copied()
            .collect();
        if pools.is_empty() {
            return Self::none_out("no_usable_pools");
        }
        if pools.len() > MAX_POOLS {
            return Self::none_out("too_many_pools");
        }
        let n = pools.len();

        // ── Vector de preferencias [yield, riesgo, latencia] normalizado ────
        let wy = state.features.get("mo_weight_yield").copied();
        let wr = state.features.get("mo_weight_risk").copied();
        let wl = state.features.get("mo_weight_latency").copied();
        let raw = [
            wy.unwrap_or(DEFAULT_W_YIELD),
            wr.unwrap_or(DEFAULT_W_RISK),
            wl.unwrap_or(DEFAULT_W_LATENCY),
        ];
        // Defensivo (risk-path): presente pero invalido ⇒ fail-honest, no
        // default silencioso.
        if raw.iter().any(|w| !w.is_finite() || *w < 0.0) {
            return Self::none_out("invalid_preference_vector");
        }
        let wsum: f64 = raw.iter().sum();
        if wsum <= 0.0 {
            return Self::none_out("invalid_preference_vector");
        }
        let weights = [raw[0] / wsum, raw[1] / wsum, raw[2] / wsum];
        let active = weights.iter().filter(|&&w| w > WEIGHT_EPS).count();

        // ── Degeneracion a 1 objetivo ────────────────────────────────────────
        // Solo-yield ⇒ resultado EXACTO del operador subyacente (op_15 sobre el
        // pool primario; misma formula, mismo escalar). Solo-riesgo/latencia ⇒
        // optimo = asignacion nula x = 0 ⇒ no hay Topological Yield ⇒ None.
        if active == 1 {
            if weights[0] > WEIGHT_EPS {
                let out15 = GoldenSectionOperator::new().evaluate(state);
                let mut metadata = out15.metadata.clone();
                metadata.insert("single_objective_mode".to_string(), 1.0);
                metadata.insert("delegated_to_op".to_string(), 15.0);
                if n > 1 {
                    metadata.insert("degenerate_pools_ignored".to_string(), (n - 1) as f64);
                }
                return OperatorOutput {
                    operator_id: self.id(),
                    operator_name: self.name().to_string(),
                    scalar_value: out15.scalar_value,
                    vector_result: out15.vector_result,
                    matrix_result: None,
                    metadata,
                };
            }
            return Self::none_out("degenerate_null_trade");
        }

        // ── Contexto economico ───────────────────────────────────────────────
        let gamma = 1.0 - fee_fraction(state);
        if !gamma.is_finite() || gamma <= 0.0 {
            return Self::none_out("invalid_fee");
        }
        let price = Self::reference_price(state).unwrap_or(pools[0].1 / pools[0].0);
        if !price.is_finite() || price <= 0.0 {
            return Self::none_out("invalid_price");
        }
        // Misma formula de gas que op_15 (21.000 unidades de gas base).
        let gas = state.gas_price_gwei * 21_000.0 * 1e-9 * price;
        let cvar_alpha = feature_f64(&state.features, "mo_cvar_alpha", 0.5, 1e-9, 1.0);
        // Proxy de latencia por pata: declarada explícita mo_per_leg_latency_ms,
        // si no block_time_sec×1000 ms, si no 12 s (bloque canonico) — parametro
        // del modelo declarado, sobreescribible por configuracion.
        let per_leg_ms = match state.features.get("mo_per_leg_latency_ms") {
            Some(v) if v.is_finite() && *v > 0.0 => *v,
            _ => feature_f64(&state.features, "block_time_sec", 12.0, 1e-9, f64::INFINITY) * 1000.0,
        };

        let ctx = Nsga2Context {
            pools,
            gamma,
            gas,
            cvar_alpha,
            per_leg_ms,
        };
        let cfg = Nsga2Config {
            population: feature_usize_clamped(
                &state.features,
                "mo_population",
                DEFAULT_POPULATION,
                MIN_POPULATION,
                MAX_POPULATION,
            ),
            generations: feature_usize_clamped(
                &state.features,
                "mo_generations",
                DEFAULT_GENERATIONS,
                1,
                MAX_GENERATIONS,
            ),
        };

        // ── Semilla determinista (patron op_22: derivada del estado, nunca
        // reloj) — mismo estado ⇒ mismo run bit-identico.
        let seed = state
            .block_number
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add((n as u64).wrapping_mul(0x85EB_CA6B))
            ^ weights[0].to_bits().rotate_left(13)
            ^ weights[1].to_bits().rotate_left(29)
            ^ weights[2].to_bits().rotate_left(43);

        let (population, objectives, front0) = run_nsga2(&ctx, &cfg, seed);

        // R8: frente vacio (defensivo — con objetivos finitos no ocurre).
        if front0.is_empty() {
            return Self::none_out("empty_front");
        }

        // Soluciones del frente como pares (objetivos, fila) donde fila =
        // [x_0..x_{n-1}, net_yield, cvar_riesgo, latency_ms]. Presentacion
        // determinista: orden lexicografico por la fila completa (x primero —
        // la asignacion nula encabeza el frente; empates x por yield/riesgo/lat).
        let mut front_solutions: Vec<(Vec<f64>, Vec<f64>)> = front0
            .iter()
            .map(|&i| {
                let mut row = population[i].clone();
                row.push(-objectives[i][0]); // Topological Yield neto
                row.push(objectives[i][1]);
                row.push(objectives[i][2]);
                (objectives[i].clone(), row)
            })
            .collect();
        front_solutions.sort_by(|a, b| {
            for (x, y) in a.1.iter().zip(b.1.iter()) {
                let o = x.total_cmp(y);
                if o != std::cmp::Ordering::Equal {
                    return o;
                }
            }
            std::cmp::Ordering::Equal
        });

        // Seleccion por preferencias: SIEMPRE un miembro exacto del frente
        // (indice sobre el mismo orden presentado en matrix_result).
        let front_objs: Vec<Vec<f64>> = front_solutions.iter().map(|s| s.0.clone()).collect();
        let sel = select_by_preference(&front_objs, &weights);
        let (_, sel_row) = &front_solutions[sel];

        let net_yield = sel_row[n];
        let risk_cvar = sel_row[n + 1];
        let latency_ms = sel_row[n + 2];
        if !net_yield.is_finite() || !risk_cvar.is_finite() || !latency_ms.is_finite() {
            return Self::none_out("non_finite_objectives");
        }

        let evaluations = cfg.population * (cfg.generations + 1);
        let mut metadata = HashMap::new();
        metadata.insert("computed".to_string(), 1.0);
        metadata.insert("n_pools".to_string(), n as f64);
        metadata.insert("population".to_string(), cfg.population as f64);
        metadata.insert("generations_run".to_string(), cfg.generations as f64);
        metadata.insert("evaluations".to_string(), evaluations as f64);
        metadata.insert("front_size".to_string(), front0.len() as f64);
        metadata.insert("selected_index".to_string(), sel as f64);
        metadata.insert("weight_yield".to_string(), weights[0]);
        metadata.insert("weight_risk".to_string(), weights[1]);
        metadata.insert("weight_latency".to_string(), weights[2]);
        metadata.insert("active_objectives".to_string(), active as f64);
        metadata.insert("cvar_alpha".to_string(), cvar_alpha);
        metadata.insert("per_leg_latency_ms".to_string(), per_leg_ms);
        metadata.insert("gamma".to_string(), gamma);
        metadata.insert("gas_cost".to_string(), gas);
        metadata.insert("nsga2_seed".to_string(), seed as f64);
        metadata.insert("net_yield".to_string(), net_yield);
        metadata.insert("risk_cvar".to_string(), risk_cvar);
        metadata.insert("latency_ms".to_string(), latency_ms);

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            // Escalar principal: Topological Yield neto de la solucion elegida
            // (negativo = computado y honestamente NO rentable, como op_15).
            scalar_value: Some(net_yield),
            vector_result: Some(sel_row.clone()),
            matrix_result: Some(front_solutions.into_iter().map(|(_, row)| row).collect()),
            metadata,
        }
    }
}

// HP-03 (2026-09-08) — property tests REALES de los 6 invariantes del charter
// (mas R8). Nucleo puro testeado directamente (dominancia/sort/crowding/
// seleccion) + integracion via evaluate().
#[cfg(test)]
mod tests {
    use super::super::op_15_golden_section::GoldenSectionOperator;
    use super::*;
    use std::collections::HashMap;

    /// 3 venues con profundidades/edges distintos ⇒ trade-off real de 3 vias.
    fn three_pool_state() -> MarketState {
        let mut features = HashMap::new();
        features.insert("mo_weight_yield".to_string(), 0.5);
        features.insert("mo_weight_risk".to_string(), 0.3);
        features.insert("mo_weight_latency".to_string(), 0.2);
        features.insert("fee_bps".to_string(), 30.0);
        MarketState {
            price_matrix: vec![vec![1.01], vec![1.01], vec![1.01]],
            liquidity_reserves: vec![
                (1_000_000.0, 1_010_000.0), // edge 1%
                (2_000_000.0, 2_008_000.0), // profundo, edge 0.4%
                (500_000.0, 507_500.0),     // somero, edge 1.5%
            ],
            gas_price_gwei: 20.0,
            block_timestamp: 1_700_000_000,
            block_number: 18_000_000,
            features,
        }
    }

    /// Estado de 1 pool con edge (mismo fixture que op_15 en real_ops_tests).
    fn one_pool_state(weights: Option<[f64; 3]>) -> MarketState {
        let mut features = HashMap::new();
        if let Some(w) = weights {
            features.insert("mo_weight_yield".to_string(), w[0]);
            features.insert("mo_weight_risk".to_string(), w[1]);
            features.insert("mo_weight_latency".to_string(), w[2]);
        }
        features.insert("fee_bps".to_string(), 30.0);
        MarketState {
            price_matrix: vec![vec![1.01], vec![1.01], vec![1.01]],
            liquidity_reserves: vec![(1_000_000.0, 1_010_000.0)],
            gas_price_gwei: 20.0,
            block_timestamp: 1_700_000_000,
            block_number: 18_000_000,
            features,
        }
    }

    /// Reconstruye el vector de minimizacion [−yield, cvar, lat] de una fila
    /// del frente ([x.., net_yield, cvar, lat]).
    fn minvec(row: &[f64], n: usize) -> Vec<f64> {
        vec![-row[n], row[n + 1], row[n + 2]]
    }

    // ── (1) No-dominancia por pares de todo el frente retornado ─────────────
    #[test]
    fn pareto_front_is_pairwise_nondominated() {
        let out = MultiObjectiveOperator::new().evaluate(&three_pool_state());
        assert_eq!(out.metadata.get("computed"), Some(&1.0));
        let rows = out.matrix_result.expect("frente de Pareto presente");
        assert!(rows.len() >= 2, "frente con >=2 miembros: {rows:?}");
        for i in 0..rows.len() {
            for j in 0..rows.len() {
                if i == j {
                    continue;
                }
                let a = minvec(&rows[i], 3);
                let b = minvec(&rows[j], 3);
                assert!(
                    !dominates(&a, &b),
                    "miembro {i} domina a {j} — el frente no es Pareto: {a:?} vs {b:?}"
                );
            }
        }
    }

    // (1b) fast_non_dominated_sort vs verificacion por fuerza bruta.
    #[test]
    fn fast_non_dominated_sort_matches_bruteforce() {
        let objs = vec![
            vec![1.0, 2.0], // A
            vec![2.0, 1.0], // B
            vec![1.5, 1.5], // C
            vec![3.0, 3.0], // D (dominado por A, B y C)
            vec![1.0, 2.0], // duplicado de A
        ];
        let fronts = fast_non_dominated_sort(&objs);
        let front0: Vec<usize> = fronts[0].clone();
        // Brute force: i ∈ frente 0 sii ningun j != i domina a i.
        let expect: Vec<usize> = (0..objs.len())
            .filter(|&i| (0..objs.len()).all(|j| j == i || !dominates(&objs[j], &objs[i])))
            .collect();
        let mut got = front0.clone();
        got.sort_unstable();
        assert_eq!(got, expect, "frente 0 = {front0:?}, esperado {expect:?}");
        assert!(!front0.contains(&3));
        // D debe estar en un frente posterior (dominado).
        assert!(
            fronts.len() >= 2 && fronts[1].contains(&3),
            "D dominado cae en frente >=1: {fronts:?}"
        );
    }

    // ── (2) Crowding distance: fronteras infinitas, interior exacto ─────────
    #[test]
    fn crowding_distance_boundaries_infinite_interior_exact() {
        // Triangulo perfecto en 2D: A(0,2), B(1,1), C(2,0).
        let front = vec![vec![0.0, 2.0], vec![1.0, 1.0], vec![2.0, 0.0]];
        let d = crowding_distance(&front);
        assert!(d[0].is_infinite(), "frontera obj0 = ∞");
        assert!(d[2].is_infinite(), "frontera = ∞");
        // interior: obj0 (2−0)/2 + obj1 (2−0)/2 = 2.0
        assert!((d[1] - 2.0).abs() < 1e-12, "interior B = 2.0: {d:?}");
        // Duplicados: span 0 ⇒ contribucion 0 (sin div-by-zero).
        let dup = vec![vec![5.0, 5.0], vec![5.0, 5.0], vec![5.0, 5.0]];
        let d2 = crowding_distance(&dup);
        assert!(d2[0].is_infinite() && d2[2].is_infinite());
        assert!(
            d2[1].is_finite() && d2[1] == 0.0,
            "interior duplicado = 0: {d2:?}"
        );
        // Frente de <=2 miembros: todos son frontera.
        assert_eq!(crowding_distance(&[vec![1.0, 1.0]]), vec![f64::INFINITY]);
        assert_eq!(
            crowding_distance(&[vec![1.0, 1.0], vec![2.0, 2.0]]),
            vec![f64::INFINITY, f64::INFINITY]
        );
    }

    // ── (3) Seleccion por preferencias SIEMPRE miembro del frente ───────────
    #[test]
    fn preference_selection_returns_front_member() {
        // Frente 3D: P0 yield-max, P2 riesgo/lat-min.
        let front = vec![
            vec![-10.0, 0.50, 100.0],
            vec![-5.0, 0.20, 60.0],
            vec![-2.0, 0.05, 20.0],
        ];
        // Puro yield (minimizar −yield) ⇒ P0.
        assert_eq!(select_by_preference(&front, &[1.0, 0.0, 0.0]), 0);
        // Puro riesgo ⇒ P2; pura latencia ⇒ P2.
        assert_eq!(select_by_preference(&front, &[0.0, 1.0, 0.0]), 2);
        assert_eq!(select_by_preference(&front, &[0.0, 0.0, 1.0]), 2);
        // Mixto: argmin de desutilidad ponderada recomputado (espejo honesto).
        let w = [0.5, 0.3, 0.2];
        let lo = [-10.0, 0.05, 20.0];
        let hi = [-2.0, 0.50, 100.0];
        let u: Vec<f64> = front
            .iter()
            .map(|m| {
                (0..3)
                    .map(|j| w[j] * (m[j] - lo[j]) / (hi[j] - lo[j]))
                    .sum::<f64>()
            })
            .collect();
        let expect = u
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;
        assert_eq!(select_by_preference(&front, &w), expect);
        // En evaluate(): vector_result = fila EXACTA de matrix_result (bits).
        let out = MultiObjectiveOperator::new().evaluate(&three_pool_state());
        let rows = out.matrix_result.unwrap();
        let sel = out.vector_result.unwrap();
        assert_eq!(sel.len(), rows[0].len());
        assert!(
            rows.iter().any(|r| {
                r.len() == sel.len()
                    && r.iter()
                        .zip(sel.iter())
                        .all(|(a, b)| a.to_bits() == b.to_bits())
            }),
            "vector_result debe ser fila bit-identica del frente"
        );
    }

    // ── (4) Degeneracion 1 objetivo = resultado del operador subyacente ────
    #[test]
    fn degenerate_single_objective_equals_op15() {
        let state = one_pool_state(Some([1.0, 0.0, 0.0]));
        let out15 = GoldenSectionOperator::new().evaluate(&state);
        let out32 = MultiObjectiveOperator::new().evaluate(&state);
        // Escalar delegado EXACTO (mismo f64, bit-identico).
        assert!(
            out15.scalar_value.is_some(),
            "op_15 debe computar en este fixture"
        );
        assert_eq!(out32.scalar_value, out15.scalar_value, "delegacion exacta");
        assert_eq!(out32.vector_result, out15.vector_result);
        assert_eq!(out32.metadata.get("delegated_to_op"), Some(&15.0));
        assert_eq!(out32.metadata.get("single_objective_mode"), Some(&1.0));
        assert_eq!(out32.operator_id, 32, "identidad op_32 preservada");
        // Riesgo puro ⇒ optimo nulo ⇒ R8 None.
        let risk_only =
            MultiObjectiveOperator::new().evaluate(&one_pool_state(Some([0.0, 1.0, 0.0])));
        assert!(risk_only.scalar_value.is_none());
        assert_eq!(
            risk_only.metadata.get("reason_degenerate_null_trade"),
            Some(&1.0)
        );
    }

    // ── (5) Determinismo: mismo estado ⇒ salida bit-identica ────────────────
    #[test]
    fn same_state_produces_bit_identical_output() {
        let op = MultiObjectiveOperator::new();
        let a = op.evaluate(&three_pool_state());
        let b = op.evaluate(&three_pool_state());
        assert_eq!(
            a.scalar_value.map(f64::to_bits),
            b.scalar_value.map(f64::to_bits)
        );
        let (va, vb) = (a.vector_result.unwrap(), b.vector_result.unwrap());
        assert!(va
            .iter()
            .zip(vb.iter())
            .all(|(x, y)| x.to_bits() == y.to_bits()));
        let (ma, mb) = (a.matrix_result.unwrap(), b.matrix_result.unwrap());
        assert_eq!(ma.len(), mb.len());
        for (ra, rb) in ma.iter().zip(mb.iter()) {
            assert!(ra
                .iter()
                .zip(rb.iter())
                .all(|(x, y)| x.to_bits() == y.to_bits()));
        }
        assert_eq!(a.metadata.get("nsga2_seed"), b.metadata.get("nsga2_seed"));
        assert_eq!(
            a.metadata.get("net_yield").map(|v| v.to_bits()),
            b.metadata.get("net_yield").map(|v| v.to_bits())
        );
    }

    // ── (6) Presupuesto acotado: clamps + terminacion garantizada ───────────
    #[test]
    fn bounded_budget_always_terminates() {
        let mut state = three_pool_state();
        // Configuracion absurda: los clamps deben acotar el run.
        state.features.insert("mo_population".to_string(), 1.0e9);
        state.features.insert("mo_generations".to_string(), 1.0e9);
        let out = MultiObjectiveOperator::new().evaluate(&state); // si colgara, el test muere
        assert_eq!(out.metadata.get("computed"), Some(&1.0));
        assert_eq!(
            out.metadata.get("population"),
            Some(&128.0),
            "clamp MAX_POPULATION"
        );
        assert_eq!(
            out.metadata.get("generations_run"),
            Some(&100.0),
            "clamp MAX_GENERATIONS"
        );
        assert_eq!(out.metadata.get("evaluations"), Some(&(128.0 * 101.0)));
        assert!(out.scalar_value.unwrap().is_finite());
    }

    // ── R8 fail-honest ───────────────────────────────────────────────────────
    #[test]
    fn r8_none_paths_are_honest() {
        let op = MultiObjectiveOperator::new();
        // Sin venues usables.
        let mut empty = three_pool_state();
        empty.liquidity_reserves = Vec::new();
        let out = op.evaluate(&empty);
        assert!(out.scalar_value.is_none());
        assert_eq!(out.metadata.get("reason_no_usable_pools"), Some(&1.0));
        // >MAX_POOLS venues.
        let mut nine = three_pool_state();
        nine.liquidity_reserves = vec![(1e6, 1.01e6); 9];
        let out = op.evaluate(&nine);
        assert!(out.scalar_value.is_none());
        assert_eq!(out.metadata.get("reason_too_many_pools"), Some(&1.0));
        // Vector de preferencias invalido: suma cero / negativo / NaN.
        for w in [[0.0, 0.0, 0.0], [-1.0, 0.5, 0.5], [f64::NAN, 0.3, 0.2]] {
            let out = op.evaluate(&one_pool_state(Some(w)));
            assert!(out.scalar_value.is_none(), "pesos {w:?} ⇒ None");
            assert_eq!(
                out.metadata.get("reason_invalid_preference_vector"),
                Some(&1.0)
            );
        }
    }

    // ── Smoke NSGA con 1 venue (2 objetivos efectivos: riesgo vs yield) ──────
    #[test]
    fn single_pool_default_weights_computes_honest_front() {
        let out = MultiObjectiveOperator::new().evaluate(&one_pool_state(None));
        assert_eq!(out.metadata.get("computed"), Some(&1.0));
        assert!(out.scalar_value.unwrap().is_finite());
        assert!(!out.matrix_result.unwrap().is_empty());
    }
}
