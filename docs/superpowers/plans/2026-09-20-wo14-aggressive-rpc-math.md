# WO-14 — Aggressive RPC Selection Math + WO-15 Ops Matemáticos (Plan)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Materializar la capa agresiva de selección/limitación de proveedores RPC (Thompson Sampling + hazard EWMA + asignación proporcional restringida) sobre `rpc_failover.rs` SIN colisionar con WO-13, y evaluar/implementar los operadores matemáticos nuevos (`op_33`+`) como capa aditiva al registry 264×31.

**Architecture:** Tres componentes sin dependencias nuevas en hot-path: (1) `ThompsonSamplingSelector` — bandit Beta(α,β) con reward `1/(1+latency_ms/100)` que reemplaza el weighted-random del pick; (2) `HazardEwma429` — estimador exponencial-suavizado del riesgo de 429 por proveedor (reemplaza el LSTM propuesto: misma señal, costo O(1), sin libtorch); (3) `constrained_proportional_allocation` — el "MPC" honesto: producto de scores (cuota restante × score de latencia × (1−error)) normalizado con tope por presupuesto del token bucket de WO-13. Nash equilibrium se RECHAZA como componente runtime (los proveedores no son jugadores con utilidades conocidas; la asignación proporcional ya converge a un punto fijo de fairness proporcional — ver Analysis).

**Tech Stack:** Rust (shared-rs crate, `rand`/`rand_distr` ya en workspace), Prometheus metrics existentes, math-engine operators (patrón op_32).

## Veredicto por técnica (análisis + validación Hermes run_273901de4b384d7a94ca0326e31cbe67)

| Técnica propuesta | Veredicto | Razón |
|---|---|---|
| Thompson Sampling (Beta) | **ADOPTAR** | O(1) por request, exploración/explotación óptima asintótica para bandits estocásticos, sin deps nuevas. Domina sobre EWMA sticky-pick cuando las condiciones de los proveedores son no-estacionarias (free-tier congestionado en horas pico). |
| "MPC/QP" | **ADAPTAR** → asignación proporcional restringida | El solver propuesto ES un producto de scores normalizado con proyección de presupuesto — NO es QP. Lo implementamos con su nombre honesto. OSQP en hot-path viola cero-dependencias-obesas. |
| LSTM (tch/libtorch) | **RECHAZAR** | Dependencia C++ ~GB, inferencia por request en hot-path sub-ms, requiere training data que no existe. Sustituto: hazard EWMA (misma señal de predicción de 429, costo O(1), actualización online). |
| Nash equilibrium | **RECHAZAR** como runtime | Best-response iteration sobre utilidades inventadas no tiene garantía de convergencia al equilibrio real (los proveedores no declaran utilidades). La asignación proporcional produce fairness proporcional (Kelly/justice) sin fingir teoría de juegos. |
| Weighted voting 0.4/0.3/0.2/0.1 | **ADAPTAR** | La combinación lineal fija es arbitraria; Thompson + presupuesto ya integran exploración y constraint. El voting queda como composición: Thompson para pick, allocation para share telemetría. |

**R8/RULE 00:** Los números de la tabla comparativa del operador (3× throughput, 89% predicción, p95 45ms) son ASPIRACIONALES. Este plan NO los promete; los medirá post-deploy (`arbx_rpc_provider_requests_total`, p95 por provider, tasa 429 evitados vs baseline 09-20).

## Global Constraints

- Cero deps nuevas en searcher-rs/shared-rs hot-path (`rand_distr` debe verificarse en el lock; si no existe, implementar Gamma sampling vía Marsaglia-Tsang local, ~30 líneas).
- SIN tocar `rpc_failover.rs` hasta que WO-13 (PR de -89) mergee — este plan construye la capa SUPERIOR. Branch nueva `perf/rpc-aggressive-selection` desde origin/main DESPUÉS del merge de WO-13.
- §37 freeze Nivel 1: route-discovery/route_scanner_worker intocados.
- Mode-invariant (§34.1): la selección de proveedor no cambia matemática económica.
- RULE 00: fixtures explícitos en tests, jamás en producto. R8: `None` = no computado.
- Ops nuevos (`op_33+`): registro ADITIVO versionado — NO re-generar la matriz 264×31 existente (certificación exact-264 intacta); los ops nuevos viven en una extensión del registry con versión `matrix_v2` y las 264 estrategias los referencian solo tras derivación explícita (lección WO-CONFIG-DERIVE-01: derivar del registry, no lista manual).
- Commits: `Co-Authored-By: Claude Code <noreply@anthropic.com>`; PR footer `🤖 Generated with [Claude Code](https://github.com/claude-code)`. §36: verificar `git branch --show-current`. Merge authority: -61.

---

### Task 1: `ThompsonSamplingSelector` (shared-rs, archivo nuevo)

**Files:**
- Create: `backend/shared-rs/src/rpc_bandit.rs`
- Modify: `backend/shared-rs/src/lib.rs` (añadir `pub mod rpc_bandit;`)

**Interfaces:**
- Produces: `pub struct ThompsonSamplingSelector`, `pub struct ArmObservation { pub success: bool, pub latency_ms: f64 }`, `pub fn new(provider_names: &[&str]) -> Self`, `pub fn select(&mut self, rng: &mut impl Rng) -> usize`, `pub fn update(&mut self, idx: usize, obs: ArmObservation)`, `pub fn alpha(&self, idx: usize) -> f64`, `pub fn beta(&self, idx: usize) -> f64`.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    #[test]
    fn select_returns_valid_index_on_cold_start() {
        let mut s = ThompsonSamplingSelector::new(&["alchemy", "drpc", "publicnode"]);
        let mut rng = StdRng::seed_from_u64(42);
        let i = s.select(&mut rng);
        assert!(i < 3);
    }

    #[test]
    fn update_converges_to_best_provider() {
        // alchemy siempre exitoso y rápido; publicnode siempre 429.
        let mut s = ThompsonSamplingSelector::new(&["alchemy", "publicnode"]);
        let mut rng = StdRng::seed_from_u64(7);
        let mut alchemy_picks = 0usize;
        for _ in 0..2000 {
            let i = s.select(&mut rng);
            if i == 0 { alchemy_picks += 1; }
            s.update(i, if i == 0 { ArmObservation { success: true, latency_ms: 80.0 } }
                     else { ArmObservation { success: false, latency_ms: 900.0 } });
        }
        // Tras aprender, alchemy debe dominar (>80% de picks en el tramo final).
        assert!(alchemy_picks > 1600, "alchemy_picks={alchemy_picks}");
    }

    #[test]
    fn reward_is_latency_sensitive() {
        let mut s = ThompsonSamplingSelector::new(&["fast", "slow"]);
        for _ in 0..500 {
            s.update(0, ArmObservation { success: true, latency_ms: 40.0 });
            s.update(1, ArmObservation { success: true, latency_ms: 800.0 });
        }
        // α de fast crece más por unidad de éxito (reward 1/(1+lat/100)).
        assert!(s.alpha(0) > s.alpha(1));
    }
}
```

- [ ] **Step 2: Run to verify they fail** — `cargo test -p shared-rs rpc_bandit` → FAIL (módulo inexistente).
- [ ] **Step 3: Implement** (Theta de cada brazo = Beta(α,β); sample Gamma α,β → θ=γ₁/(γ₁+γ₂)):

```rust
//! Thompson Sampling para selección de proveedor RPC (WO-14).
//! Cada proveedor es un brazo con posterior Beta(α,β); el reward por
//! observación exitosa es 1/(1+latency_ms/100) — éxito rápido empuja α
//! más que éxito lento. El fallo (429/5xx/timeout) empuja β. La selección
//! samplea θᵢ ~ Beta(αᵢ,βᵢ) y toma argmax: exploración automática al
//! inicio, explotación cuando las posteriors se separan.

use rand::Rng;

pub struct ArmObservation {
    pub success: bool,
    pub latency_ms: f64,
}

pub struct ThompsonSamplingSelector {
    alpha: Vec<f64>,
    beta: Vec<f64>,
}

impl ThompsonSamplingSelector {
    pub fn new(provider_names: &[&str]) -> Self {
        let n = provider_names.len();
        // Prior Beta(1,1) = uniforme: sin datos, todos igual de creíbles (R8).
        Self { alpha: vec![1.0; n], beta: vec![1.0; n] }
    }

    pub fn select<R: Rng>(&mut self, rng: &mut R) -> usize {
        let mut best = 0usize;
        let mut best_theta = 0.0f64;
        for i in 0..self.alpha.len() {
            let theta = sample_beta(rng, self.alpha[i], self.beta[i]);
            if theta > best_theta { best_theta = theta; best = i; }
        }
        best
    }

    pub fn update(&mut self, idx: usize, obs: ArmObservation) {
        if obs.success {
            self.alpha[idx] += 1.0 / (1.0 + obs.latency_ms / 100.0);
        } else {
            self.beta[idx] += 1.0;
        }
    }

    pub fn alpha(&self, idx: usize) -> f64 { self.alpha[idx] }
    pub fn beta(&self, idx: usize) -> f64 { self.beta[idx] }
}

/// θ ~ Beta(α,β) vía γ₁~Gamma(α), γ₂~Gamma(β), θ = γ₁/(γ₁+γ₂).
/// Marsaglia-Tsang para α,β ≥ 1; para α<1 usa boost α+1 con potencia α⁻¹.
fn sample_beta<R: Rng>(rng: &mut R, alpha: f64, beta: f64) -> f64 {
    let g1 = sample_gamma(rng, alpha);
    let g2 = sample_gamma(rng, beta);
    if g1 + g2 <= 0.0 { return 0.5; } // degenerado: honesto neutro
    g1 / (g1 + g2)
}

fn sample_gamma<R: Rng>(rng: &mut R, shape: f64) -> f64 {
    if shape < 1.0 {
        // Boost: Gamma(shape) = Gamma(shape+1) * U^(1/shape)
        let g = sample_gamma(rng, shape + 1.0);
        let u: f64 = rng.gen::<f64>().max(f64::MIN_POSITIVE);
        return g * u.powf(1.0 / shape);
    }
    // Marsaglia-Tsang (shape >= 1)
    let d = shape - 1.0 / 3.0;
    let c = (9.0 * d).sqrt();
    loop {
        let x: f64 = rng.gen::<f64>() * 2.0 - 1.0; // no exacto: usar normal estándar
        // NOTA DE IMPLEMENTACIÓN: sustituir por sample normal (Box-Muller inline
        // o rand_distr::StandardNormal si está en el lock). Ver Global Constraints.
        let v = 1.0 + c * x;
        if v <= 0.0 { continue; }
        let v = v * v * v;
        let u: f64 = rng.gen::<f64>();
        if u < 1.0 - 0.0331 * x * x * x * x { return d * v; }
        if (u.ln() + 0.5 * x * x + d * (1.0 - v + v.ln())) < 0.0 { return d * v; }
    }
}
```

- [ ] **Step 4: Resolver el sampler normal** — verificar `rand_distr` en `Cargo.lock` (workspace): si existe, usar `rand_distr::Distribution::<f64>::sample(&rand_distr::StandardNormal, rng)` y eliminar el placeholder del loop; si no, Box-Muller inline (2 gen() + ln + sqrt, ~6 líneas). El test de convergencia (Step 1) es el gate de corrección del sampler.
- [ ] **Step 5: Tests PASS** — `cargo test -p shared-rs rpc_bandit` → 3/3.
- [ ] **Step 6: Commit** — `git add backend/shared-rs/src/rpc_bandit.rs backend/shared-rs/src/lib.rs && git commit -m "feat(shared-rs): ThompsonSamplingSelector for RPC provider selection (WO-14)"`

### Task 2: `HazardEwma429` (mismo archivo)

**Files:**
- Modify: `backend/shared-rs/src/rpc_bandit.rs`

**Interfaces:**
- Produces: `pub struct HazardEwma429 { ... }` con `pub fn new(alpha: f64) -> Self` (α = factor de suavizado, default 0.3), `pub fn record(&mut self, hit_limit: bool, now_ms: u64)`, `pub fn hazard(&self) -> f64` (∈[0,1]), `pub fn should_pre_throttle(&self, threshold: f64) -> bool`.

- [ ] **Step 1: Failing tests** — (a) hazard arranca en 0 y sube hacia 1 con racha de 429s; (b) decae con éxitos; (c) `should_pre_throttle(0.7)` false tras éxitos sostenidos, true tras 5+ 429s recientes; (d) α=0 nunca actualiza (invariante input inválido → clamp o assert doc).
- [ ] **Step 2: Verify FAIL** — `cargo test -p shared-rs hazard` → compile error.
- [ ] **Step 3: Implement** — EWMA dual: `h ← α·1 + (1−α)·h` en 429, `h ← α·0 + (1−α)·h` en éxito, con recencia por ventana (si `now_ms - last_ms > window_ms`, primero decaer por factor explícito). Es el sustituto O(1) del LSTM: predicción = suavizado exponencial del indicador, no secuencia.
- [ ] **Step 4: Tests PASS** — `cargo test -p shared-rs hazard` → 4/4.
- [ ] **Step 5: Commit** — `feat(shared-rs): HazardEwma429 429-risk estimator (LSTM replacement, WO-14)`

### Task 3: Asignación proporcional restringida + integración en `rpc_failover` (POST WO-13 merge)

**Files:**
- Create: `backend/shared-rs/src/rpc_allocation.rs`
- Modify: `backend/shared-rs/src/rpc_failover.rs` (SOLO después del merge de WO-13; el pick pasa por `ThompsonSamplingSelector` cuando el estado del provider es `Open`, el token bucket de WO-13 define el tope, y `should_pre_throttle` rebaja el share del provider)

**Interfaces:**
- Consumes: `ThompsonSamplingSelector`, `HazardEwma429`, token-bucket budget de WO-13.
- Produces: `pub fn constrained_proportional_allocation(budget_remaining: &[f64], latency_ms_ewma: &[f64], error_rate: &[f64], hazard: &[f64]) -> Vec<f64>` — share Σ=1, uᵢ ≤ budgetᵢ/budget_total.

- [ ] **Step 1: Failing tests** — (a) suma de shares = 1 ± 1e-9; (b) provider con budget 0 recibe share 0; (c) provider congestionado (hazard alto) recibe menos que su proporción de score; (d) input vacío → vec vacío (R8, no panic).
- [ ] **Step 2: Implement** — scoreᵢ = (budgetᵢ)·(1/(1+latᵢ/100))·(1−errᵢ)·(1−hazardᵢ); normalizar; re-proyectar sobre el simplex con el tope de presupuesto (water-filling de 3 líneas). Este es el "MPC honesto": punto estacionario de fairness proporcional, no QP.
- [ ] **Step 3: Integración en rpc_failover** — el `sticky_pick` existente se conserva como fast-path (provider sano + bucket con tokens → sin sampleo); Thompson entra solo en la decisión multi-provider. Documentar en el módulo por qué no hay Nash (ver Analysis) ni LSTM.
- [ ] **Step 4: Tests + clippy** — `cargo clippy -p shared-rs --locked --all-targets -- -D warnings` → 0; suite completa.
- [ ] **Step 5: Commit** — `feat(shared-rs): constrained proportional allocation + bandit/hazard wiring (WO-14)`

### Task 4: Métricas + dashboard

**Files:**
- Modify: `backend/shared-rs/src/metrics.rs` (o donde vivan `arbx_rpc_provider_*`)
- Modify: `monitoring/grafana/dashboards/rpc-failover.json`

- [ ] **Step 1:** Contadores `arbx_rpc_bandit_selected_total{provider}` + gauges `arbx_rpc_bandit_arm{provider,alpha,beta}` (etiquetas de α/β como labels solo si el cardinbal de providers es bajo — si no, histograma de θ) + gauge `arbx_rpc_provider_hazard{provider}`.
- [ ] **Step 2:** Panel "Rate limits & bandit" en el dashboard: hazard por provider, share allocation, 429-rate. `promtool` check si aplica al dashboard JSON (no aplica; aplica a rules si se añaden).
- [ ] **Step 3:** Commit — `feat(monitoring): bandit/hazard metrics + rate-limit panel (WO-14)`

### Task 5 (WO-15): Ops matemáticos `op_33..op_35` en math-engine — OBLIGATORIO (orden del operador 2026-09-20)

> **Orden del operador (verbatim):** "garantiza que se aplican a las estrategias, haz pruebas y demuestra con hechos cuales son las ventajas e integralas e implmentalas perfectamente end to end, integralas en donde estan las otras ops y garantiza que cuando se active el toogle, estas serán refuerzos para hacer arbitrages mas exitosos".

Consecuencias duras de la orden:
1. Los ops se integran DONDE están los otros ops (`backend/math-engine/src/operators/`, mismo trait/registro que op_32) — no un módulo aparte.
2. Se APLICAN a las estrategias: binding real en el pipeline (call-site donde un op actúa como refuerzo de estrategia), no solo registro pasivo.
3. TOGGLE: un op apagado = la estrategia funciona como hoy (sin refuerzo); un op encendido = refuerzo activo. Reutilizar el mecanismo de toggle existente de los 32 ops (auditarlo en Step 0).
4. VENTAJAS DEMOSTRADAS CON HECHOS: benchmark medido pre/post (misma data, seed fija, op OFF vs ON) — metric: para op_33 convergencia al mejor proveedor/ruta vs round-robin; para op_34 detección temprana de hazard (429 evitados en replay de logs); para op_35 asignación proporcional vs uniforme ( Throughput útil / p95). Sin número medido = no se reporta la ventaja (R8).

**Files (preliminar — confirmar con auditoría Step 0):**
- Create: `backend/math-engine/src/operators/op_33_thompson_sampling/` (+ op_34_hazard_ewma, op_35_adaptive_allocation) siguiendo EXACTAMENTE el patrón de `op_32`.
- Modify: registry donde op_32 está registrado (misma tabla/enum/manifest, sin tocar las 8.184 relaciones 264×31 existentes).
- Modify: call-site del pipeline donde los ops actúan como refuerzo de estrategia + wiring del toggle.

**Step 0 — Auditoría del registry (bloqueante, sin código antes):**
Mapa exacto (agente Explore despachado 2026-09-20): (a) estructura completa de op_32; (b) punto(s) de registro (todos los grep-hits de "op_32"); (c) mecanismo de toggle (endpoint, persistencia, semántica OFF); (d) call-site real estrategia↔op; (e) patrón de test con vectores independientes. Si el registry no admite extensión aditiva SIN regenerar la matriz exact-264 → DETENERSE y reportar (no hacks, §37).

- [ ] **Pasos por op:** TDD (vectores independientes con seed — jamás test que re-compute la propia fórmula; orden operador 2026-09-17), OPERATOR.json, SKILL.md, registro junto a op_32, binding a estrategias por el call-site real, toggle verificado (OFF = idéntico a hoy, test de paridad; ON = refuerzo aplicado, test de efecto), benchmark pre/post con números medidos, suite completa, commit por op.

### Task 6: PR + BOARD + verificación post-deploy

- [ ] PR `feat(shared-rs): WO-14 aggressive RPC selection math (Thompson+hazard+allocation)` — CI 34 checks, merge authority -61, después del PR de WO-13.
- [ ] BOARD `audits/perf-stack-2026-09-20/GOAL-WORKORDERS.md`: estado WO-14/WO-15.
- [ ] Post-deploy gates (evidencia, no promesas): (1) `arbx_rpc_bandit_selected_total` crece y discrimina providers; (2) tasa de 429 por provider post-deploy < baseline 09-20 (medido, cifra real); (3) p95 quote latency por provider reportado; (4) si WO-15 procedió: math-engine arranca con 35 ops y las 264 estrategias intactas (censo exact-264 sin cambio).

## Rollback

Revert del merge (§37 Parte 5). Todo es aditivo: el fast-path sticky-pick queda intacto; quitar el bandit restaura la selección previa sin migración.
