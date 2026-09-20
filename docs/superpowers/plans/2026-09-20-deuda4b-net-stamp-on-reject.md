# Deuda 4-(B): estampar el net computado en el reject path del SizeOptimizer

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que todo reject del SizeOptimizer que computó un valor económico real lleve ese valor hasta `opp.net_expected_profit_usd`, para que `scored_opportunities.net_profit_usd` deje de etiquetar "rentable" lo que el gate rechazó por no-rentable (99.5% de 95,958 rejects non_positive_profit en 1h de ventana VPS).

**Architecture:** Extender `OptimizeOutcome::Rejected(OptimizeRejectReason)` a `Rejected(OptimizeRejectReason, Option<f64>)` donde el `Option<f64>` es el net/gross USD computado (`None` = no computado, invariante R8). Los 2 call-sites (orchestrator.rs:971, cartridge_boot.rs:1494) estampan el valor en el opportunity antes de `emit_rejected`, espejo del arm `Sized` (:951-958). Forward-only: sin backfill de los rows históricos (R8).

**Tech Stack:** Rust (searcher-rs), sin dependencias nuevas.

## Root cause (verificado 2026-09-20, sesión a8 + -89)

- `OptimizeOutcome::Rejected` NO lleva economía: el `profit_wei ≤ 0` computado muere en el `return` (size_optimizer.rs:750/972/991/1268).
- orchestrator.rs:934 loguea `outcome.net_profit_usd()` → `None` en Rejected (getter size_optimizer.rs:203-207).
- opportunity_emitter.rs:578 `net = net_expected.or(expected)` persiste el estimate crudo POSITIVO de detección.
- MATIZ R8: orchestrator.rs:927-933 mapea optimizer `Err` (infra) → `Rejected(NonPositiveProfit)` SIN valor computado — ese path debe quedar `None`.

## Global Constraints

- R8: `None` = no computado; `Some(v)` = computado y exactamente ese valor (incluye negativos y positivos-bajo-floor).
- §34.1: cambio de semántica de wiring, NO de matemática — el kernel no cambia, sólo viaja el resultado.
- §37: un PR = un ID (Deuda 4-B). El mislabel Err→NonPositiveProfit (orchestrator:927) NO se toca aquí (fichado aparte).
- Labels de -89 (writer A): taxonomía por rejection_reason es PERMANENTE para gas_floor_breach y kelly_negative_edge (pueden llevar net>0; el veredicto del gate vence al signo).
- ZERO MOCKS: no se fabrica valor donde el kernel no calculó.

## Sitios estampables (auditados en a44211af)

| size_optimizer.rs | reason | valor en scope |
|---|---|---|
| :555 | GasFloorBreach | `net_usd` (>0, < floor) |
| :581 | KellyNegativeEdge | `net_usd` |
| :653 | GasFloorBreach (kelly-capped) | `new_net` |
| :781 | NonPositiveGrossUsd (triangular) | `eval_result.expected_profit_usd` si `Some(≤0)` |
| :972 | NonPositiveProfit (2-leg golden section) | `profit_wei ≤ 0` → usd (price+decimals en scope) |
| :991 | NonPositiveProfit (clamped) | `profit_at_clamped ≤ 0` → usd |
| :1004 | NonPositiveGrossUsd (2-leg) | `gross_usd ≤ 0` |
| :1268 | NonPositiveProfit (V3 grid) | `best.3 ≤ 0` si presente → usd |
| :1277 | NonPositiveGrossUsd (V3) | `gross_usd ≤ 0` |
| :750, :1194 | NonPositiveProfit | `None` (upper-bound / eval None — nada honesto que estampar) |
| orchestrator :927 Err-path | NonPositiveProfit | `None` (R8: infra ≠ negocio) |

Conversión wei→usd: `(profit_wei as f64) / 10f64.powi(decimals as i32) * token_price_usd` — idéntica a la del path Sized (:1000, :1274).

---

### Task 1: Enum + getters + mechanical sites

**Files:** Modify `backend/searcher-rs/src/size_optimizer.rs`

- [ ] Cambiar `Rejected(OptimizeRejectReason)` → `Rejected(OptimizeRejectReason, Option<f64>)` con doc del invariante R8.
- [ ] Getter `net_profit_usd()`: `Self::Rejected(_, net) => *net`.
- [ ] Fix mechanical de todos los ctors/matches (compile-error-driven): ctor `Rejected(X)` → `Rejected(X, None)`; match `Rejected(r)` → `Rejected(r, _)` salvo los 2 call-sites de la Task 3.
- [ ] `cargo check -p searcher-rs` limpio (CARGO_TARGET_DIR al target caliente del árbol principal, AppControl 4551).

### Task 2: Estampar los 10 sitios económicos

**Files:** Modify `backend/searcher-rs/src/size_optimizer.rs` (tabla de arriba)

- [ ] Cada sitio: `Rejected(reason, Some(valor))` con el valor en scope. Los 3 `None` quedan `None` con comentario de una línea (R8).
- [ ] Commit.

### Task 3: Stamp en call-sites (orchestrator + cartridge_boot)

**Files:** Modify `backend/searcher-rs/src/orchestrator.rs:971`, `backend/searcher-rs/src/cartridge_boot.rs:1494`

- [ ] orchestrator arm `Rejected(reason, net)`: espejo del arm Sized — `if let Some(n) = net { c.net_expected_profit_usd = Some(n); c.opportunity.net_expected_profit_usd = Some(n); }`. NOTA: mantener `expected_profit_usd` (gross) intacto (HARDENING existente: la tarjeta muestra por qué no es viable).
- [ ] cartridge_boot arm: `opp.net_expected_profit_usd = net;` (espera `Option<f64>`; el `expected_profit_usd = None` existente se mantiene).
- [ ] Commit.

### Task 4: Tests (TDD-style, añadir tras enum para compilar)

**Files:** Modify `backend/searcher-rs/src/size_optimizer.rs` (mod tests)

- [ ] Test A: fixture 2-leg con spread real ≤ 0 (reservas simétricas) → `Rejected(NonPositiveProfit, Some(n))` con `n ≤ 0` y `n` finito.
- [ ] Test B: fixture GasFloorBreach existente (:3331) extendido: payload `Some(net_usd)` con net > 0.
- [ ] Test C: payload Err-path None — assert del contrato enum: `Rejected(reason, None)` construido por orchestrator:927 (test de unidad sobre el getter: `Rejected(X, None).net_profit_usd().is_none()`).
- [ ] Test D (emitter-level, ya cubierto por :578 fallback): assert que `Some(-x)` NO cae al estimate — test existente de stamped_for_emit ampliado si aplica; si no hay hook barato, cubierto por paridad de wiring (documentar).

### Task 5: Gate

- [ ] `cargo fmt` + `cargo clippy -p searcher-rs --all-targets -- -D warnings` (0 warnings).
- [ ] `cargo test -p searcher-rs --lib` (1317+ baseline + nuevos).
- [ ] Commit final, push, PR (aviso a -89 y -61 pre-push como protocolo).
