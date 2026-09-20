# RPC Rate Budget (WO-13 / PERF-STACK extra) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Client-side per-provider rate budgeting (token bucket) en `HttpRpcPool` para PREVENIR 429s (no solo reaccionar), con métricas Prometheus por proveedor y paneles Grafana.

**Architecture:** El pool multi-vendor ya existe (`backend/shared-rs/src/rpc_failover.rs`: failover + circuit breaker 429 ARBX-R-0003 + EWMA + load-probe). Este plan añade una capa de presupuesto ANTES del 429: un `TokenBucket` por entrada, gateado en `pick()`/`with_retry()`, configurable por env (no-hardcode). Sin tocar `v3_quote_provider.rs` (territorio #600/a8).

**Tech Stack:** Rust (std Mutex + atomics, sin deps nuevas), prometheus crate (patrón Lazy+REGISTRY existente), Grafana schemaVersion 39 JSON.

## Global Constraints

- RULE 00 / no-hardcode: presupuesto SOLO desde env `RPC_HTTP_RATE_BUDGETS` (CSV `name=calls_per_min`). Env vacío/ausente = sin presupuesto (comportamiento actual, backward compatible).
- §36 / coordinación: archivos permitidos = `backend/shared-rs/src/rate_budget.rs` (nuevo), `backend/shared-rs/src/rpc_failover.rs`, `backend/shared-rs/src/metrics.rs`, `backend/shared-rs/src/lib.rs` (solo `pub mod`), `monitoring/grafana/dashboards/rpc-failover.json`. CERO touch en `v3_quote_provider.rs`, `amm_math.rs`, `dex_engine.rs`, `state_projector.rs` (#600/a8).
- Correctura de diseño (validada por -61): la rotación de v3 quotes SOLO aplica entre endpoints JSON-RPC. CoinGecko/CMC/Etherscan/DexScreener NO entran (no pueden responder `quoteExactInputSingle`). Endpoints free-tier extra (Publicnode) = cambio de `.env` VPS, zero código.
- Requisito -61: tests de presupuesto REAL (prevención, no solo reacción al 429) + las métricas nuevas no rompen exporters existentes (registro en el mismo REGISTRY, nombres `arbx_rpc_*` libres).
- Requisito a8 (dueño #600, verificado contra main): (1) el bucket mide POR INTENTO dentro del pool (with_retry re-ejecuta el closure tras failover — un quote lógico = N intentos/tokens); (2) rechazo de bucket ≠ 429 del provider: NUNCA pasa por `report_failure`/`classify_cause` (triplearía el breaker por un throttle que el provider no impuso) — se expone como variante `PoolError::BudgetExhausted` que el caller trata como "sin presupuesto, no negativo-cachear"; (3) `requests_total` cuenta INTENTOS de pool (sus labels rpc/rpc_ok son por elemento de batch, no sumar).
- Branch: `perf/rpc-rate-budget` desde `origin/main`, worktree perf-89-edge-fe. Commit footer `Co-Authored-By: Claude Code <noreply@anthropic.com>`; PR footer `🤖 Generated with [Claude Code](https://github.com/claude/code)`.

---

### Task 1: TokenBucket module (`rate_budget.rs`)

**Files:**
- Create: `backend/shared-rs/src/rate_budget.rs`
- Modify: `backend/shared-rs/src/lib.rs` (añadir `pub mod rate_budget;`)
- Test: inline `mod tests` en el mismo archivo

**Interfaces:**
- Produces: `pub struct TokenBucket`; `TokenBucket::new(per_minute: u32) -> Self`; `pub fn try_acquire(&self) -> bool` (consume 1 token, false si no hay); `pub fn tokens_remaining(&self) -> u64` (enteros, no consume); `fn refill_to_cap(tokens_milli: u64, elapsed_ms: u64, cap_milli: u64, per_minute: u32) -> u64` (pure, pub(crate)); `#[cfg(test)] fn fast_forward(&self, d: Duration)`.

- [x] **Step 1.1: Escribir el módulo con tests inline**

```rust
//! rate_budget — client-side per-provider token bucket (WO-13 / PERF-STACK).
//!
//! El circuit breaker 429 de `rpc_failover` (ARBX-R-0003) REACCIONA al 429;
//! este módulo lo PREVIENE: cada proveedor con presupuesto configurado en
//! `RPC_HTTP_RATE_BUDGETS` (CSV `name=calls_per_min`) consume 1 token por
//! intento de request. Sin env = sin presupuesto (backward compatible).
//!
//! Burst capacity = 1 minuto completo de presupuesto (per_minute tokens),
//! refill continuo. Mutex breve (nunca cruzando await): pick() es síncrono
//! y la contención por acquire es despreciable frente al RTT del RPC.

use std::sync::Mutex;
use std::time::{Duration, Instant};

const MILLI: u64 = 1_000;

/// Pure refill math — tokens (milli) tras `elapsed_ms`, capped at capacity.
/// refill_milli/min = per_minute * 1000 → per elapsed ms: per_minute * elapsed / 60.
pub(crate) fn refill_to_cap(tokens_milli: u64, elapsed_ms: u64, cap_milli: u64, per_minute: u32) -> u64 {
    let refill = (per_minute as u64).saturating_mul(elapsed_ms) / 60;
    tokens_milli.saturating_add(refill).min(cap_milli)
}

pub struct TokenBucket {
    cap_milli: u64,
    per_minute: u32,
    /// (last_refill_instant, tokens_milli)
    inner: Mutex<(Instant, u64)>,
}

impl TokenBucket {
    /// Burst = un minuto completo del presupuesto. `per_minute` >= 1.
    pub fn new(per_minute: u32) -> Self {
        assert!(per_minute >= 1, "rate budget per_minute must be >= 1");
        let cap = per_minute as u64 * MILLI;
        Self {
            cap_milli: cap,
            per_minute,
            inner: Mutex::new((Instant::now(), cap)),
        }
    }

    /// Consume 1 token si hay; false si el bucket está agotado (el caller
    /// debe tratar al proveedor como temporalmente no elegible).
    pub fn try_acquire(&self) -> bool {
        let mut g = self.inner.lock().expect("rate_budget lock");
        let now = Instant::now();
        let elapsed_ms = now.duration_since(g.0).as_millis() as u64;
        g.1 = refill_to_cap(g.1, elapsed_ms, self.cap_milli, self.per_minute);
        if g.1 < MILLI {
            g.0 = now;
            return false;
        }
        g.1 -= MILLI;
        g.0 = now;
        true
    }

    /// Tokens enteros disponibles (refill lazy incluido). NO consume.
    pub fn tokens_remaining(&self) -> u64 {
        let mut g = self.inner.lock().expect("rate_budget lock");
        let now = Instant::now();
        let elapsed_ms = now.duration_since(g.0).as_millis() as u64;
        g.1 = refill_to_cap(g.1, elapsed_ms, self.cap_milli, self.per_minute);
        g.0 = now;
        g.1 / MILLI
    }

    /// Test-only: retrocede el reloj interno `d` para simular refill
    /// transcurrido SIN dormir (determinista en CI).
    #[cfg(test)]
    pub(crate) fn fast_forward(&self, d: Duration) {
        let mut g = self.inner.lock().expect("rate_budget lock");
        g.0 = g
            .0
            .checked_sub(d)
            .expect("fast_forward beyond process start");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refill_math_linear_and_capped() {
        // 60/min → 1000 milli/min → 16.66 milli/ms (integer: 60*ms/60 = ms).
        // 500 ms elapsed → +500 milli.
        assert_eq!(refill_to_cap(0, 500, 60_000, 60), 500);
        // Cap: nunca excede la capacidad.
        assert_eq!(refill_to_cap(59_999, 60_000, 60_000, 60), 60_000);
        // Un minuto completo a 60/min repone exactamente 60 tokens.
        assert_eq!(refill_to_cap(0, 60_000, 60_000, 60), 60_000);
        // 300/min → 5 milli/ms → 1000 ms repone 5000 milli (5 tokens).
        assert_eq!(refill_to_cap(0, 1_000, 300_000, 300), 5_000);
    }

    #[test]
    fn acquire_drains_capacity_then_rejects() {
        let b = TokenBucket::new(5);
        for _ in 0..5 {
            assert!(b.try_acquire());
        }
        assert!(!b.try_acquire(), "bucket exhausted must reject");
        assert_eq!(b.tokens_remaining(), 0);
    }

    #[test]
    fn budget_prevention_real_refill_restores_token() {
        // PREVENCIÓN (requisito -61): tras agotar, el refill del presupuesto
        // devuelve tokens SIN necesidad de un 429 que "despierte" nada.
        let b = TokenBucket::new(60); // 1 token/min → 1000ms por token
        for _ in 0..60 {
            assert!(b.try_acquire());
        }
        assert!(!b.try_acquire());
        b.fast_forward(Duration::from_millis(1_050));
        assert!(b.try_acquire(), "refilled token must be acquirable");
        assert_eq!(b.tokens_remaining(), 0);
    }

    #[test]
    fn burst_capacity_equals_one_minute() {
        let b = TokenBucket::new(300);
        assert_eq!(b.tokens_remaining(), 300);
    }
}
```

En `backend/shared-rs/src/lib.rs`, junto a los demás `pub mod`:
```rust
pub mod rate_budget;
```

- [x] **Step 1.2: Verificar**

Run: `cargo test -p shared-rs rate_budget` — Expected: 4 PASS.
Run: `cargo fmt -p shared-rs` + `cargo clippy -p shared-rs -- -D warnings` — Expected: limpio.

- [x] **Step 1.3: Commit** — `feat(shared-rs): per-provider token bucket for RPC rate budgeting (WO-13)`

---

### Task 2: Métricas Prometheus

**Files:**
- Modify: `backend/shared-rs/src/metrics.rs` (bloque RPC, tras `RPC_POOL_DRIFT_DETECTED_TOTAL`, línea ~261)

**Interfaces:**
- Produces: `RPC_PROVIDER_REQUESTS_TOTAL: IntCounterVec` labels `[provider, kind, outcome]` (outcome: `success|error`); `RPC_PROVIDER_BUDGET_TOKENS: IntGaugeVec` labels `[provider, kind]`; `RPC_PROVIDER_BUDGET_THROTTLED_TOTAL: IntCounterVec` labels `[provider, kind]`.

- [x] **Step 2.1: Añadir las 3 métricas con el patrón existente (Lazy + REGISTRY.register)**

```rust
pub static RPC_PROVIDER_REQUESTS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_rpc_provider_requests_total",
            "RPC provider request attempts through the pool hot path by outcome (health probes excluded)"
        ),
        &["provider", "kind", "outcome"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

pub static RPC_PROVIDER_BUDGET_TOKENS: Lazy<prometheus::IntGaugeVec> = Lazy::new(|| {
    let g = prometheus::IntGaugeVec::new(
        prometheus::opts!(
            "arbx_rpc_provider_budget_tokens",
            "Client-side rate-budget tokens remaining per provider (only providers with RPC_HTTP_RATE_BUDGETS configured)"
        ),
        &["provider", "kind"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(g.clone())).expect("register");
    g
});

pub static RPC_PROVIDER_BUDGET_THROTTLED_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_rpc_provider_budget_throttled_total",
            "Times a provider was skipped (pick or backup) because its client-side rate budget was exhausted"
        ),
        &["provider", "kind"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});
```

- [x] **Step 2.2: Verificar export no roto** — `cargo test -p shared-rs` completo (los tests de métricas existentes validan el REGISTRY; un nombre duplicado paniquea en register → cualquier test que toque la métrica lo detecta).

- [x] **Step 2.3: Commit** — `feat(shared-rs): request/budget Prometheus metrics for the RPC pool (WO-13)`

---

### Task 3: Integración en `rpc_failover.rs` (env, HttpEntry, pick, with_retry)

**Files:**
- Modify: `backend/shared-rs/src/rpc_failover.rs`

**Interfaces:**
- Consumes: Task 1 `TokenBucket`, Task 2 métricas.
- Produces: `HttpEntry.budget: Option<Arc<TokenBucket>>`; `parse_budgets(csv: &str) -> HashMap<String, u32>`; helper `fn budget_has_token(e: &HttpEntry) -> bool` (NO consume); env `RPC_HTTP_RATE_BUDGETS`.

- [x] **Step 3.1: Pre-verificación de callers y matches exhaustivos**

Run: `grep -rn "\.pick()\|PoolError::" backend/ --include="*.rs"` — anotar (a) callers de `pick()` y (b) TODO match exhaustivo sobre `PoolError` fuera de shared-rs. `pick()` queda NO-consumidor de tokens (solo consulta); el consumo (`try_acquire`) ocurre EXCLUSIVAMENTE en `with_retry` por intento ejecutado (a8 confirmó: TODO el tráfico del quoter funnela por with_retry — un quote lógico = N intentos = N tokens). Si un match exhaustivo ajeno (p.ej. searcher-rs) no tuviera wildcard `_`, coordinar con su dueño ANTES de pushear la variante nueva.

- [x] **Step 3.2: Parse de budgets + campo en HttpEntry**

En `rpc_failover.rs` (cerca de `parse_csv`):

```rust
/// Parse `RPC_HTTP_RATE_BUDGETS` CSV (`name=30,other=60`, calls/min).
/// Entradas malformadas se omiten con warn (una typo no desarma los
/// presupuestos restantes — mismo criterio que parse_csv, MC-RPC-1).
fn parse_budgets(csv: &str) -> std::collections::HashMap<String, u32> {
    let mut out = std::collections::HashMap::new();
    for tok in csv.split(',') {
        let tok = tok.trim();
        if tok.is_empty() { continue; }
        let Some((name, rpm)) = tok.split_once('=') else {
            warn!(event = "rpc_pool.budget_malformed", token = %tok, "expected name=calls_per_min");
            continue;
        };
        match rpm.trim().parse::<u32>() {
            Ok(n) if n >= 1 => { out.insert(name.trim().to_string(), n); }
            _ => warn!(event = "rpc_pool.budget_invalid_rpm", token = %tok, "calls_per_min must be an integer >= 1"),
        }
    }
    out
}
```

En `HttpEntry` (struct, línea ~179) añadir:
```rust
/// WO-13: client-side rate budget. None = sin presupuesto configurado
/// (comportamiento actual, ilimitado).
pub budget: Option<Arc<crate::rate_budget::TokenBucket>>,
```

En `from_csv`, leer env una vez antes del loop de entradas y construir el bucket por nombre:
```rust
let budgets = parse_budgets(&std::env::var("RPC_HTTP_RATE_BUDGETS").unwrap_or_default());
```
y en la construcción de `HttpEntry` (línea ~324):
```rust
budget: budgets.get(name.as_str()).map(|rpm| {
    info!(
        event = "rpc_pool.rate_budget_set",
        chain_id, name = name.as_str(), calls_per_min = rpm,
        "client-side rate budget active for provider"
    );
    Arc::new(crate::rate_budget::TokenBucket::new(*rpm))
}),
```
NOTA: la métrica `RPC_PROVIDER_BUDGET_TOKENS` se inicializa en el mismo bucle final de gauges (~357) solo para entries con budget:
```rust
if let Some(b) = &e.budget {
    crate::metrics::RPC_PROVIDER_BUDGET_TOKENS
        .with_label_values(&[e.name.as_str(), "http"])
        .set(b.tokens_remaining() as i64);
}
```

- [x] **Step 3.3: Gate en `pick()` (consulta, no consumo)**

Helper junto a `is_rate_limit_sticky`:
```rust
/// WO-13: budget consultivo — true si la entrada no tiene presupuesto o
/// le queda >= 1 token. NO consume (el consumo ocurre en with_retry).
fn budget_has_token(e: &HttpEntry) -> bool {
    e.budget.as_ref().map(|b| b.tokens_remaining() >= 1).unwrap_or(true)
}
```
En `pick()`, dentro del `for e in &self.entries`, primera condición del match (junto a `ProviderState::Open => continue`): tratar entrada sin budget como no elegible en CUALQUIER estado:
```rust
if !budget_has_token(e) {
    crate::metrics::RPC_PROVIDER_BUDGET_THROTTLED_TOTAL
        .with_label_values(&[e.name.as_str(), "http"])
        .inc();
    budget_blocked = true;
    continue;
}
```
(`let mut budget_blocked = false;` antes del loop.)

**Variante nueva en `PoolError`** (punto 2/3 de a8 — un rechazo de presupuesto NO es un 429 del provider: no lo clasifica classify_cause, no tripea el breaker, y el caller puede distinguirlo para NO negativo-cachear):
```rust
/// WO-13: todos los proveedores elegibles están bloqueados por el
/// presupuesto client-side (RPC_HTTP_RATE_BUDGETS). NO es una falla del
/// provider — el breaker NO se toca y el caller no debe negative-cachear.
BudgetExhausted(u64),
```
Semántica de `pick()` al fallar: si `budget_blocked` → `Err(PoolError::BudgetExhausted(self.chain_id))`, si no → `Err(AllUnhealthy)` como hoy. Display: `"rpc pool {0}: rate budget exhausted for all providers"`.
```

- [x] **Step 3.4: Consumo + requests_total en `with_retry()`**

En el Try 1 (tras `let first = self.pick()?;`):
```rust
if let Some(b) = &first.budget {
    if !b.try_acquire() {
        // Carrera pick→acquire: el token se agotó entre la consulta y el
        // consumo. No es failure del proveedor — pasar al backup sin
        // tocar el circuit breaker.
        crate::metrics::RPC_PROVIDER_BUDGET_THROTTLED_TOTAL
            .with_label_values(&[first.name.as_str(), "http"])
            .inc();
    } else {
        crate::metrics::RPC_PROVIDER_BUDGET_TOKENS
            .with_label_values(&[first.name.as_str(), "http"])
            .set(b.tokens_remaining() as i64);
    }
}
```
ESTRUCTURA: si el acquire falla, saltar directamente a la fase de backup (reestructurar el match en un `if acquired { match op(...) {...} } else { /* log */ }` — el intento 1 NO se ejecuta y NUNCA se llama `report_failure` por un rechazo de bucket: no toca breaker, no entra a classify_cause, no cuenta en errors_total). Si además no hay backup elegible y `first` seguía con presupuesto agotado → `return Err(PoolError::BudgetExhausted(self.chain_id))`. En el Ok del op: `RPC_PROVIDER_REQUESTS_TOTAL.with_label_values(&[first.name.as_str(), "http", "success"]).inc();` En el Err: `..."error"].inc();`. Igual en el Try 2 (backup), que además filtra `budget_has_token(e)` en el `find`:
```rust
let backup = self.entries.iter().find(|e| {
    !Arc::ptr_eq(e, &first)
        && !matches!(e.snapshot_state(), ProviderState::Open)
        && budget_has_token(e)
});
```
y consume su token igual que Try 1 (con retry del `find` NO — un solo backup como hoy).

- [x] **Step 3.5: Tests de pool con presupuesto REAL (requisito -61)**

En `mod tests`, extender `dummy_entry` con budget `None` por defecto (campo nuevo) y añadir:

```rust
fn dummy_entry_budgeted(name: &str, per_minute: u32) -> Arc<HttpEntry> {
    let mut e = dummy_entry(name);
    Arc::get_mut(&mut e).expect("sole ref").budget =
        Some(Arc::new(crate::rate_budget::TokenBucket::new(per_minute)));
    e
}

#[tokio::test]
async fn pick_skips_budget_exhausted_provider() {
    let pool = HttpRpcPool {
        chain_id: 1,
        entries: vec![dummy_entry("a"), dummy_entry_budgeted("b", 1)],
    };
    // Agota el presupuesto de b (1 token).
    assert!(pool.entries[1].budget.as_ref().unwrap().try_acquire());
    // pick debe elegir a (presupuesto agotado = no elegible, prevención real:
    // el 429 NUNCA llega a dispararse porque el request nunca sale).
    let picked = pool.pick().unwrap();
    assert_eq!(picked.name, "a");
}

#[tokio::test]
async fn pick_all_budgets_exhausted_returns_budget_exhausted() {
    let pool = HttpRpcPool {
        chain_id: 1,
        entries: vec![dummy_entry_budgeted("a", 1), dummy_entry_budgeted("b", 1)],
    };
    for e in &pool.entries { assert!(e.budget.as_ref().unwrap().try_acquire()); }
    let err = pool.pick().unwrap_err();
    // Variante distinta (a8): el caller la distingue de AllUnhealthy para NO
    // negativo-cachear — es un throttle local, no un fallo del provider.
    assert!(matches!(err, PoolError::BudgetExhausted(1)));
}

#[tokio::test]
async fn parse_budgets_valid_and_malformed() {
    let m = parse_budgets("alchemy=30, publicnode=60 ,bad, x=0, y=abc");
    assert_eq!(m.get("alchemy"), Some(&30));
    assert_eq!(m.get("publicnode"), Some(&60));
    assert_eq!(m.len(), 2, "malformed tokens are skipped, rest survives");
}

#[test]
fn budget_none_means_unlimited_backward_compatible() {
    // Entradas sin budget (env ausente) nunca son throttled: dummy_entry
    // ya construye budget: None y el resto de la suite existente lo ejercita.
    let e = dummy_entry("a");
    assert!(budget_has_token(&e));
}

#[tokio::test]
async fn budget_refill_makes_provider_eligible_again() {
    let pool = HttpRpcPool {
        chain_id: 1,
        entries: vec![dummy_entry_budgeted("a", 2)],
    };
    let b = pool.entries[0].budget.clone().unwrap();
    assert!(b.try_acquire());
    assert!(b.try_acquire());
    assert!(!pool.pick().is_ok(), "agotado → AllUnhealthy");
    b.fast_forward(std::time::Duration::from_millis(31_000)); // 2/min → 1 token/30s
    assert!(pool.pick().is_ok(), "refill restaura elegibilidad sin 429");
}
```
NOTA: `fast_forward` es `#[cfg(test)]` en rate_budget → llamarlo desde tests de rpc_failover requiere que ambos módulos estén en el mismo crate (sí: shared-rs). Si `dummy_entry` usa `Arc::new` directo, ajustar `dummy_entry_budgeted` para construir el HttpEntry completo (copiar el cuerpo de dummy_entry añadiendo el campo).

- [x] **Step 3.6: Verificar** — `cargo test -p shared-rs` (suite completa: ~25 existentes + 5 nuevos PASS), `cargo fmt`, `cargo clippy -p shared-rs -- -D warnings`.

- [x] **Step 3.7: Commit** — `feat(shared-rs): gate RPC pool picks by client-side rate budget (WO-13)`

---

### Task 4: Paneles Grafana

**Files:**
- Modify: `monitoring/grafana/dashboards/rpc-failover.json` (añadir 5 paneles con ids siguientes al máximo existente; `version: 2`)

- [x] **Step 4.1: Añadir paneles** (mismo datasource `arbx-prometheus`, gridPos continuación):

1. **"Requests / sec by provider & outcome"** (timeseries, stacked): `sum by (provider, outcome) (rate(arbx_rpc_provider_requests_total[1m]))`
2. **"Availability % by provider (5m)"** (gauge/timeseries): `100 * sum by (provider) (rate(arbx_rpc_provider_requests_total{outcome="success"}[5m])) / clamp_min(sum by (provider) (rate(arbx_rpc_provider_requests_total[5m])), 0.000001)`
3. **"Rate-budget tokens remaining"** (timeseries): `arbx_rpc_provider_budget_tokens`
4. **"Budget throttled events / 5m"** (timeseries): `sum by (provider) (increase(arbx_rpc_provider_budget_throttled_total[5m]))`
5. **"429-class errors / sec"** (timeseries): `sum by (provider) (rate(arbx_rpc_provider_errors_total{cause="rate_limit"}[1m]))`

- [x] **Step 4.2: Validar JSON** — `python -c "import json;json.load(open('monitoring/grafana/dashboards/rpc-failover.json',encoding='utf-8'))"` → sin error. Si existe CI/promtool de dashboards, correrlo.

- [x] **Step 4.3: Commit** — `feat(monitoring): RPC rate-budget panels in failover dashboard (WO-13)`

---

### Task 5: Entrega

- [x] **Step 5.1:** `cargo test -p shared-rs && cargo clippy -p shared-rs -- -D warnings && cargo fmt --check -p shared-rs` — todo verde.
- [x] **Step 5.2:** Push `perf/rpc-rate-budget` a origin + PR (base main, body con desviación de diseño documentada: CoinGecko/CMC/Etherscan/DexScreener excluidos como fuentes de quote — solo rotación JSON-RPC; budget por env; env VPS ejemplo `RPC_HTTP_1="alchemy=...,publicnode=https://ethereum-rpc.publicnode.com" RPC_HTTP_RATE_BUDGETS="publicnode=60"`). Footer 🤖.
- [ ] **Step 5.3:** Avisar a -61 (review línea-a-línea + cola de merges tras #606) y veredicto Hermes.

## Self-Review

1. **Spec coverage:** rotación (via pool existente + budgets) ✓, tests integración ✓ (Task 3.5), métricas por proveedor ✓ (Task 2), dashboard rate limits ✓ (Task 4). El "40x" del doc original NO se promete (era cifra fabricada sobre fuentes inválidas). Publicnode end-free-tier = .env (documentado en PR, no código).
2. **Placeholders:** los pasos de código están completos; el restructuring exacto del Try-1 en 3.4 se describe con la estructura `if acquired`.
3. **Tipos:** `TokenBucket::new(u32)`, `try_acquire() -> bool`, `tokens_remaining() -> u64`, `budget: Option<Arc<TokenBucket>>` consistentes en Tasks 1/3/5.
