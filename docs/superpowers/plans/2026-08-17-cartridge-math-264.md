# Cartridge Math 264 — Plan de Implementación (Universo de Rutas)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que cada uno de los 264 cartuchos produzca oportunidades con economía real (profit/ROI/riesgo computados en ms), siguiendo el catálogo maestro ArbitrageX (264 estrategias × 31 operadores, 60 familias detectoras).

**Architecture:** El universo de rutas dinámico (pool_cycles) alimenta ImpactIndex; los cartuchos ya tienen el contrato Runner completo (audit: 264/264) pero solo 36 tienen matemática — este plan conecta el manifest matemático del Excel (Detector ID + operadores + ecuación) al pipeline existente, por oleadas de familia.

**Tech Stack:** Rust (searcher-rs, math-engine), PostgreSQL (migraciones), Rhai (cartuchos), vitest/cargo test.

## Global Constraints

- REGLA 00: cero mocks/datos sintéticos; dato faltante = skip + razón.
- REGLA 01: NO-TOCAR `route_discovery/guarantees.rs`, `pmiCalculator.ts`, kill-switch, audit store.
- Modos del Excel son ley: 160 SHADOW / 104 PAPER; `shadow_only` forzado NO se toca.
- Migraciones: siguiente número = **107**; CREATE INDEX sobre tabla poblada solo CONCURRENTLY (lint CI).
- Catálogo canónico: `ArbitrageX_264_Cartridge_Math_Architecture (2).xlsx` hoja `02_CARTRIDGE_MATH_MAP` (264×28, estado READY_FOR_CARTRIDGE_MIGRATION).
- Baseline auditado: contrato Runner 264/264 OK; math real solo G01 29/36, G02 7/17, G03–G11 0/N.

---

### Task 1: pool_cycles — universo dinámico (RU-1, rompe impacted_cycles=0)

**Files:**
- Create: `database/migrations/107_pool_cycles.sql`
- Create: `backend/searcher-rs/src/route_discovery/cycle_enumerator.rs`
- Modify: `backend/searcher-rs/src/impact_index.rs` (source swap + CycleSpec registry)
- Test: `backend/searcher-rs/src/route_discovery/cycle_enumerator.rs` (#[cfg(test)])

**Interfaces:**
- Produces: `pub async fn enumerate_and_persist(pool: &sqlx::PgPool, chain_id: u64) -> anyhow::Result<usize>` — count de ciclos upserteados.
- Produces: `pub fn load_pool_to_cycles(rows: &[(String, String, String)]) -> HashMap<Address, Vec<CycleId>>` en impact_index (pool_path → cycle ids).

- [ ] **Step 1: migración 107**

```sql
CREATE TABLE IF NOT EXISTS pool_cycles (
    cycle_id     BIGSERIAL PRIMARY KEY,
    chain_id     INT NOT NULL,
    token_path   TEXT[] NOT NULL,
    pool_path    TEXT[] NOT NULL,
    direction    SMALLINT NOT NULL DEFAULT 1,
    active       BOOLEAN NOT NULL DEFAULT TRUE,
    discovered_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (chain_id, token_path, pool_path, direction)
);
```

- [ ] **Step 2: test del enumerador (grafos sintéticos) — falla primero**

Test: grafo 4 tokens/6 pools (WETH-USDC, USDC-DAI, DAI-WETH, WETH-PEPE, PEPE-USDC, WETH-USDT) ⇒ enumera triángulos exactos {WETH,USDC,DAI}, {WETH,PEPE,USDC} en ambas direcciones, dedup por rotación.

- [ ] **Step 3: enumerador** — reutiliza `unique_route_finder::find_unique_routes` sobre el pool set de PG; upsert `ON CONFLICT DO NOTHING`; cap `max_cycles_active=5000` con `capped=true` honesto; telemetría `cycle_enum.done`.

- [ ] **Step 4: swap en ImpactIndex** — boot: `SELECT token_path, pool_path FROM pool_cycles WHERE chain_id=$1 AND active` → `pool_to_cycles`; si tabla vacía ⇒ seed `MVP_CYCLES` (cold-boot, comportamiento actual). Registry `Vec<CycleSpec>` con getter `cycle_spec(id) -> Option<&CycleSpec>`; consumidor (triangular_worker) lee el spec, no la constante.

- [ ] **Step 5: test regresión** — "evento sobre pool de un ciclo descubierto ⇒ impacted_cycles>0" (el bug estructural 23/23=0).

- [ ] **Step 6**: `cargo check + clippy` local; commit `fix/omega-ru-1-pool-cycles`; CI; deploy searcher+api-server; verificación viva 24h: `v2.impact.resolved` con `impacted_cycles>0` (RU-G2).

### Task 2: Math Manifest — el Excel 264×28 como datos machine-readable

**Files:**
- Create: `scripts/gen_math_manifest.py` (lee el XLSX → JSON; NO commitea el XLSX)
- Create: `backend/searcher-rs/cartridges/manifests/math_map.json` (264 entries)
- Test: `backend/searcher-rs/src/cartridge/manifest_test.rs`

**Interfaces:**
- Produces: `math_map.json[i] = {mev_id, detector_id, primary_ops: ["op_27","op_21",...], equation, data_bindings, frontend_toggle, mode}`.

- [ ] **Step 1**: generador openpyxl → JSON (campos [0],[14],[18],[17],[20],[5] + modo del `01_MEV_MATRIX` col 23).
- [ ] **Step 2**: test de contrato: 264 entries, mev_id único, detector_id ∈ 60 familias, ops ⊆ op_01..op_31, mode ∈ {SHADOW, PAPER} con conteos 160/104 exactos.
- [ ] **Step 3**: commit + CI (el test corre en `cargo test --lib`).

### Task 3: Mapeo de categorías — las 11 familias Excel a evaluación real

**Files:**
- Modify: `backend/searcher-rs/src/cartridge_boot.rs:1145` (match de labels)

**Interfaces:** cada categoría → StrategyLabel EXISTENTE (sin colapso C.5: identidad preservada en RoutePlan.strategy_kind).

```rust
let label = match category.as_str() {
    "dex_arb" | "dex_arb_v2v2" => StrategyLabel::DexArbV2V2,
    "dex_arb_v2v3" => StrategyLabel::DexArbV2V3,
    "dex_arb_v3v2" => StrategyLabel::DexArbV3V2,
    "dex_arb_v3v3" => StrategyLabel::DexArbV3V3,
    "triangular_arb" => StrategyLabel::TriangularArb,
    "flashloan_arb" => StrategyLabel::FlashloanArb,
    "liquidation" => StrategyLabel::Liquidation,
    "spanning_tree_arb" => StrategyLabel::SpanningTreeArb,
    "cross_chain_arb" | "cross_domain_engine" => StrategyLabel::CrossChainArb,
    "liquidation_snipe" | "credit_liquidation_engine" => StrategyLabel::LiquidationSnipe,
    // G01/G02 spot-DEX: la variante granular vive en RoutePlan.strategy_kind
    "route_graph_engine" | "amm_curve_engine" => StrategyLabel::DexArbV2V2,
    // G03/G04: evaluación por evento de estado — hoy medible como triangular cuando
    // la forma es ciclo cerrado; el perfil propio llega en RU-4 full.
    "state_event_engine" | "parity_redemption_engine" if is_closed_cycle => StrategyLabel::TriangularArb,
    // Resto (derivatives, cex_external, intents, nft, prediction): SIN motor
    // honesto aún → siguen rechazados con su razón (fail-honest), entran por oleada.
    _ => { /* rama actual de reject con razón */ }
};
```

- [ ] Test: los 11 categories mapeados producen label correcto; los 4 sin motor siguen rechazando CON razón (no silencio).
- [ ] Verificación viva: `cartridge.unmapped_strategy_label` count → de 224/24h a solo G05/G07/G09/G10/G11.

### Task 4: Oleada G01 — 36 cartuchos con math real vía manifest

**Files:**
- Modify: 36 `cartridges/strategies/mev_01_*.rhai` (los 7 sin math completan; los 29 con math se alinean al manifest)
- Anchor: `evaluate_opportunity(pool_data)` ya recibe legs/reserves (host_bindings)

**Template por cartucho** (familia R_CLOSED_CYCLE, detector dominante de G01):
1. Lee `detector_id`+ops del manifest (via init_strategy metadata ya declarado).
2. `evaluate_opportunity`: verifica forma (ciclo cerrado, legs 2–8), reservas frescas; sin dato ⇒ `no_opp("missing_reserves")`.
3. Sizing: golden-section (op_15, ya en repo) sobre el producto de `cpmm_out` — max `Q_R(x)-x-C_R(x)`.
4. Emite payload con profit/ROI computados (build_payload).

- [ ] Test: un cartucho representativo con vector canónico (reservas sintéticas EN TEST, jamás runtime) ⇒ profit esperado exacto.
- [ ] Verificación viva: primeras filas con `expected_profit_usd NOT NULL` (RU-G3) → tarjetas evaluadas con Execute (RU-A ya en main).

### Task 5+: Oleadas restantes (plantilla Task 4)

Orden por dependencia de datos (no por número): G02 (17, curvas — tipado ProtocolType) → G03+G04 (62, evento/paridad — aristas sintéticas) → G08 (25, health-factor ya existe) → G05+G06+G07 (74 PAPER — anclas externas/derivados) → G09–G11 (50, fase tardía). Cada oleada: manifest check + tests + 7 días midiendo (RU-G7).

---

## Self-Review
- Cobertura: RU-1 (Task 1) ✓, manifest (Task 2) ✓, mapping (Task 3 = RU-4-lite) ✓, oleadas (Tasks 4-5 = RU-5) ✓. V3 tick-math (RU-2) y hops 4–8 (RU-3) quedan como plan propio posterior — anotado.
- Tipos: `enumerate_and_persist`, `load_pool_to_cycles`, `CycleSpec` consistentes entre Tasks.
- Sin placeholders: los pasos sin código completo referencian anchors exactos ya verificados en repo.

## Execution Handoff
**Plan completo y guardado.** Opciones: (1) Subagent-driven (recomendado — contexto fresco por tarea + code review entre tareas), (2) Inline (executing-plans).
