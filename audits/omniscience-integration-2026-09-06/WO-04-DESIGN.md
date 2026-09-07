# WO-04 — DISEÑO: Parametrización de literales económicos (/goal §2.5)

> Work-order de DISEÑO (READ-ONLY — sin ediciones de código). Produce el diseño
> exacto que la Oleada 4 aplicará. Fecha: 2026-09-06.
> Estado: **DESIGNED** (literal-confirmed por lectura directa; cero ejecución de
> builds — verificación del diseño = las citas file:line de este documento).
>
> Lexico OMEGA: gas = fricción termodinámica · LP fee = fricción de Variedad de
> Liquidez · tip = parámetro de mercado EIP-1559 (NO es fee de pool).

---

## (a) Confirmación de literales — file:line verificados por lectura directa

| # | Literal | Ubicación exacta | Código actual | Consumidor vivo |
|---|---------|------------------|---------------|-----------------|
| 1 | tip 2 gwei | `backend/relays-client/src/bundle_builder.rs:171` | `let priority_fee = U256::from(2_000_000_000u64); // 2 gwei default` | SÍ — camino de broadcast (`submit_engine.rs:438` llama `build_and_sign`; `max_fee = base_fee * 2 + priority_fee` en L172; tx L215-216) |
| 2 | GAS_COST_USD=30 | `backend/searcher-rs/src/workers/liquidation_worker.rs:147` | `const GAS_COST_USD: f64 = 30.0;` | CONDICIONAL — worker legacy `default-off-post-phase-15` (`main.rs:1029-1041`, se activa solo con `ARBX_ENABLE_LEGACY_LIQUIDATION_WORKER=true`); usos: L733 (log boot), L954 (`estimate_liquidation_profit`) |
| 3 | GAS_COST_USD=30 (espejo) | `backend/searcher-rs/src/engines/liquidation_engine.rs:60` | `const GAS_COST_USD: f64 = 30.0;` | SÍ — camino VIVO event-driven (constructor `scanner.rs:446`; uso L229; doc L56-59 admite el riesgo de deriva: *"Must match the value in `liquidation_worker::GAS_COST_USD` … by convention"*) |
| 4 | 30 bps LP fee | `backend/api-server/src/simulation/computeSimulatedNet.ts:139` | `const LP_FEE_FRACTION_DEFAULT = 0.003;` | SÍ — usos: L239 (`lp_fees_usd`), L240 (nota `"lp-fee=30bps-proxy"`), L374 (`varCostRateFromCfg`) |

Los 4 literales del charter quedan **CONFIRMADOS** en las líneas exactas
anunciadas (±0 líneas).

### Gemelos/adyacentes detectados (fuera de charter — reporte fail-honest, NO se diseñan)

- `backend/relays-client/src/executor/gas_oracle.rs:37` — `U256::from(2_000_000_000u64)`:
  **quinta copia del tip 2 gwei, pero NO cableada** (grep en relays-client: el
  único consumo de `GasOracle` es su propia definición; `executor/mod.rs:1` solo
  declara `pub mod gas_oracle;`). No es un "literal vivo" → **fuera de Oleada
  4**. Si algún día se cablea, DEBE consumir el mismo `AppConfig` del diseño
  (abajo). No se elimina (§3 Surgical: mencionar, no borrar).
- `backend/relays-client/src/bundle_builder.rs:169` — fallback base fee 30 gwei
  (`unwrap_or(30_000_000_000u64)`): literal de gas adyacente NO incluido en el
  charter. Se reporta; no se parametriza en esta oleada.
- `backend/math-engine/src/roi_engine.rs` — verificado: NO hardcodea 30 bps;
  `lp_fees_usd` entra como parámetro exacto por ruta (L74). Sin riesgo de
  deriva con el literal 4.

---

## (b) Patrones de configuración existentes en el repo (estudiados con ejemplos)

### Patrón 1 — env var validada al boot, default constante documentada (workers searcher-rs)

`backend/searcher-rs/src/main.rs:975-978` (idéntico para triangular L834-837,
flashloan L920-923, cex-dex L1048-1051):

```rust
let liquidation_period_secs: u64 = std::env::var("LIQUIDATION_WORKER_INTERVAL_SECS")
    .ok()
    .and_then(|v| v.parse().ok())
    .unwrap_or(workers::liquidation_worker::DEFAULT_INTERVAL_SECS);
```

La constante del propio archivo documenta el override
(`liquidation_worker.rs:105`: *"operator can override via
`LIQUIDATION_WORKER_INTERVAL_SECS`"*).

### Patrón 2 — config declarativa TOML `configs/app.toml` → `AppConfig` (shared-rs)

`backend/shared-rs/src/config.rs` — `ExecutionCfg` (L121-138) YA contiene la
misma clase de parámetros con serde-default documentado:
`target_block_offset` (default 1), `max_value_eth` (default 1.0),
`priority_fee_increment_pct` (default 10.0, L136-137). El call-site del bundle
builder ya recibe dos de ellos: `submit_engine.rs:444-445`
(`self.cfg.execution.max_value_eth, self.cfg.execution.target_block_offset`).
Espejos obligatorios del TOML: `configs/schemas/app.schema.json`
(`execution` con `additionalProperties: false`, L37-52) y
`shared-ts/src/config/index.ts` (zod `execution` `.strict()`, L95-106).

### Patrón 3 — config declarativa hot-reload `trading_config` (Redis `arbx:trading_config:<chain_id>` JSON + PG + pub/sub `<1s`)

- Escritura: **ÚNICAMENTE** el PUT admin `backend/api-server/src/routes/trading-config.ts`
  (zod schema → PG → espejo Redis → PUBLISH; invariantes L9-10, L393).
- Lectura Rust: `backend/shared-rs/src/trading_config.rs` —
  `TradingConfigState` con `#[serde(default = "…")]` y doc que cita la
  migración PG + CHECK (ej. `spread_sanity_mult` L337-338, migración 048).
- Lectura TS: `backend/api-server/src/simulation/tradingConfigSnapshot.ts` —
  `parseSnapshot` con `num(o["campo"], default_doctrinal)`; doc L81-86: los
  defaults TS **espejan los serde defaults Rust** para comportamiento idéntico.
- Columnas análogas de la MISMA clase que el literal 4 (componentes de costo
  fraccionarios): `flashloan_fee_pct` (0.0009), `max_slippage_pct` (0.005),
  `failure_risk_buffer_pct` (0.001) — zod L171-173, SQL L425/500/576/607/627/655/839.
- Patrón de columna PG: `database/migrations/026_trading_config.sql` L43-44 —
  `NUMERIC(6,4) NOT NULL DEFAULT 0.0010 CHECK (… >= 0)`. Última migración del
  repo: `118_opportunities_detected_at_breakdown_idx.sql` → la nueva es **119**.

### Patrón descartado explícitamente para estos literales

- `canonical_knobs.rs` (53 knobs workbook, precedencia `ARBX_KNOB_*` > yaml >
  workbook): es la superficie de knobs de **discovery/ranking de rutas** del
  searcher. Los literales 1/4 viven en otros binarios (relays-client,
  api-server) que no consumen canonical_knobs; meterlos ahí mezclaría capas.
- Redis keys ad-hoc por worker (ej. `arbx:liquidation_cap_usd` propuesto en
  `liquidation_worker.rs:139-140`): existe solo como "follow-up" en un
  comentario; no es un patrón consumado.

---

## (c) Diseño por literal — hogar, justificación, alternativas

### Literal 1 — tip 2 gwei → `ExecutionCfg.priority_fee_gwei` (Patrón 2, TOML)

**Decisión**: campo nuevo en `ExecutionCfg` con `serde(default) = 2.0`,
enhebrado como 8º parámetro de `build_and_sign` desde el call-site existente
(`submit_engine.rs`), exactamente como ya fluyen `max_value_eth` y
`target_block_offset`.

**Justificación**: es el patrón que el MISMO struct y el MISMO call-site ya
usan para parámetros hermano (incluido `priority_fee_increment_pct`, otro knob
de priority fee). relays-client **no** consume `trading_config` (verificado
por grep: cero usos) ni canonical_knobs; su superficie de config es
AppConfig TOML + env vars puntuales. Cambio de firma con 1 solo call-site
productivo; los tests del propio `bundle_builder.rs` no llaman
`build_and_sign` completo (L444-449: *"is integration-level and deferred to M5
fork validation"*).

**Conversión**: `f64 gwei → wei u64` con `(v * 1e9).round() as u64` (cast
float→int satura en Rust ≥1.45; el schema acota v ≥ 0).

**Alternativa descartada**: env var estilo `ARBX_STAGING_GAS_PRICE_GWEI`
(`relay_flashbots.rs:596-599`) — es patrón de *staging/probe*, no del camino
productivo; además partiría la coherencia con los hermanos del ExecutionCfg.

### Literales 2+3 — GAS_COST_USD → env `LIQUIDATION_GAS_COST_USD` (Patrón 1)

**Decisión**: resolver UNA vez al boot con un helper `pub fn
resolve_gas_cost_usd()` en `liquidation_worker.rs` (patrón INTERVAL_SECS del
mismo archivo, con hardening: rechaza no-finitos/negativos con `warn!` — un
gas negativo inflaría el Topological Yield neto; checks defensivos
obligatorios en camino de riesgo). La constante pasa a `pub const
DEFAULT_GAS_COST_USD: f64 = 30.0` y el **engine BORRA su espejo privado**
(elimina el riesgo de deriva que su propio doc L56-59 confiesa) recibiendo el
valor en `LiquidationEngine::new`. Worker y engine leen el MISMO knob →
imposible diverger.

**Justificación**: es un pre-screen (doc `liquidation_worker.rs:145-146`:
*"The spine evaluator will recompute at real-time gas prices downstream"*) —
no amerita superficie hot-reload. El camino VIVO hoy es el engine (worker
legacy default-off), y ambos quedan cubiertos por el mismo knob.

**Alternativa descartada (documentada)**: campo `trading_config.liquidation_gas_cost_usd`
(Patrón 3, hot-reload) — requeriría zod + 7 sitios SQL + migración PG + espejo
TS para un pre-screen que el spine ya recomputa; costo desproporcionado para
Oleada 4. Si el operador quiere tuning en caliente, es el follow-up natural.

### Literal 4 — 30 bps → `trading_config.lp_fee_default_pct` (Patrón 3, completo)

**Decisión**: campo declarativo completo (Rust serde-default 0.003 + columna
PG migración 119 + zod PUT admin + espejo TS snapshot + consumo en
`computeSimulatedNet` en sus 3 sitios). Default idéntico en los 4 planos
(0.003 / 0.003 / 0.0030 / 0.003).

**Justificación**: es la MISMA clase que `flashloan_fee_pct` /
`max_slippage_pct` / `failure_risk_buffer_pct` (componente de costo
fraccionario del snapshot que consume `forwardSimulate`); el snapshot TS ya
dice que sus defaults espejan los serde defaults Rust. Hot-reload <1s vía
pub/sub. El campo Rust es obligatorio por el precedente del incidente
schema-drift `enabled_dex_ids` (doc `trading_config.rs:282-286`: columna en
DB+API+Redis ausente del struct Rust → serde la descartaba silenciosamente).
La nota `"lp-fee=30bps-proxy"` pasa a dinámica (`lp-fee=${bps}bps-proxy`) —
verificado: cero consumidores que matcheen el string literal (grep en
backend+frontend).

**Alternativa descartada**: env var de api-server — rompería la simetría
snapshot↔Rust y crearía una clase nueva de parámetros económicos fuera del
único hogar que ya tienen todos sus hermanos.

---

## (d) Diffs exactos por archivo (para aplicar en Oleada 4)

### D1. `backend/shared-rs/src/config.rs`

```diff
@@ ExecutionCfg (post L136-137)
     #[serde(default = "default_priority_fee_inc")]
     pub priority_fee_increment_pct: f64,
+    /// EIP-1559 `max_priority_fee_per_gas` (gwei) for the broadcast tx built
+    /// by `relays-client::bundle_builder::build_and_sign`. NOT a pool fee —
+    /// gas/tip is a gas-market parameter (ROUTES_CROWN_JEWEL rule 4 governs
+    /// pool fees, not EIP-1559 tips). Default `2.0` gwei = the value
+    /// hardcoded at bundle_builder.rs:171 until 2026-09-06 (WO-04).
+    /// Operator tunes via `configs/app.toml` `[execution]`.
+    #[serde(default = "default_priority_fee_gwei")]
+    pub priority_fee_gwei: f64,
 }
@@ defaults (post L154-156)
 fn default_priority_fee_inc() -> f64 {
     10.0
 }
+fn default_priority_fee_gwei() -> f64 {
+    2.0
+}
```

### D2. `configs/schemas/app.schema.json` (execution es `additionalProperties: false` → obligatorio)

```diff
@@ execution.properties (L48-50)
         "max_value_eth":             { "type": "number",  "minimum": 0 },
         "flashbots_submit_timeout_ms":{ "type": "integer", "minimum": 100 },
-        "priority_fee_increment_pct":{ "type": "number",  "minimum": 0 }
+        "priority_fee_increment_pct":{ "type": "number",  "minimum": 0 },
+        "priority_fee_gwei":         { "type": "number",  "minimum": 0 }
```

### D3. `shared-ts/src/config/index.ts` (zod execution `.strict()` → obligatorio)

```diff
@@ execution (L104)
     priority_fee_increment_pct: z.number().nonnegative().optional().default(10),
+    // WO-04: EIP-1559 priority tip (gwei) for bundle_builder broadcasts.
+    // Mirrors shared-rs ExecutionCfg::priority_fee_gwei (serde default 2.0)
+    // and configs/schemas/app.schema.json.
+    priority_fee_gwei: z.number().nonnegative().optional().default(2.0),
   }).strict(),
```

### D4. `backend/relays-client/src/bundle_builder.rs`

```diff
@@ firma (L97-104)
 pub async fn build_and_sign(
     opp: &Opportunity,
     plan: &ValidatedPlan,
     signer: &Signer,
     provider: &AlloyHttpProvider,
     nonce_mgr: &NonceManager,
     max_value_eth: f64,
     target_block_offset: u64,
+    priority_fee_gwei: f64,
 ) -> Result<SignedBundle, BuildError> {
@@ comentario (L155)
-    // Fee estimation. For S5 we use simple "latest base fee + 2 gwei priority".
+    // Fee estimation. "Latest base fee + operator-configured priority tip"
+    // (configs/app.toml [execution] priority_fee_gwei, default 2 gwei — WO-04).
@@ literal (L171)
-    let priority_fee = U256::from(2_000_000_000u64); // 2 gwei default
+    // gwei → wei (float→int cast saturates; schema bounds v >= 0 — WO-04).
+    let priority_fee = U256::from((priority_fee_gwei * 1e9).round() as u64);
```

### D5. `backend/relays-client/src/submit_engine.rs`

```diff
@@ call-site (L438-446)
         let bundle = match build_and_sign(
             opp,
             &validated_plan,
             signer.as_ref(),
             provider.as_ref(),
             nonce.as_ref(),
             self.cfg.execution.max_value_eth,
             self.cfg.execution.target_block_offset,
+            self.cfg.execution.priority_fee_gwei,
         )
```

### D6. `backend/searcher-rs/src/workers/liquidation_worker.rs`

```diff
@@ const (L143-147)
-/// Fixed gas cost estimate (USD) for one liquidation call. A real Aave V3
-/// liquidation consumes ~300k gas; at 20 gwei × $2400/ETH that's ~$15. The
-/// 30 estimate adds a safety margin. The spine evaluator will recompute at
-/// real-time gas prices downstream; this guard is a pre-screen only.
-const GAS_COST_USD: f64 = 30.0;
+/// Default gas cost estimate (USD) for one liquidation call. A real Aave V3
+/// liquidation consumes ~300k gas; at 20 gwei × $2400/ETH that's ~$15. The
+/// 30 estimate adds a safety margin. The spine evaluator will recompute at
+/// real-time gas prices downstream; this guard is a pre-screen only.
+/// Operator override: env `LIQUIDATION_GAS_COST_USD` (resolved once at boot —
+/// see `resolve_gas_cost_usd`; same pattern as LIQUIDATION_WORKER_INTERVAL_SECS).
+pub const DEFAULT_GAS_COST_USD: f64 = 30.0;
+
+/// Resolve the operator gas-cost knob once at boot. Non-finite or negative
+/// values fall back to the default WITH a warn (R8: a silently-ignored knob is
+/// an operational lie; a negative gas cost would inflate net Topological Yield).
+pub fn resolve_gas_cost_usd() -> f64 {
+    let raw = match std::env::var("LIQUIDATION_GAS_COST_USD") {
+        Ok(v) => v,
+        Err(_) => return DEFAULT_GAS_COST_USD,
+    };
+    match raw.parse::<f64>() {
+        Ok(v) if v.is_finite() && v >= 0.0 => v,
+        _ => {
+            warn!(
+                event = "liquidation.gas_cost_usd_invalid",
+                raw = %raw,
+                fallback = DEFAULT_GAS_COST_USD,
+                "LIQUIDATION_GAS_COST_USD is not a finite non-negative number — using default"
+            );
+            DEFAULT_GAS_COST_USD
+        }
+    }
+}
@@ struct + ctor (L689-702)
 pub struct LiquidationWorker {
     pub period: Duration,
     pub chain_id: u64,
     pub rpc_pool: Option<Arc<HttpRpcPool>>,
+    /// Operator gas-cost pre-screen (USD) — env `LIQUIDATION_GAS_COST_USD`,
+    /// default `DEFAULT_GAS_COST_USD` (30.0). WO-04.
+    pub gas_cost_usd: f64,
 }
 
 impl LiquidationWorker {
-    pub fn new(interval_secs: u64, chain_id: u64) -> Self {
+    pub fn new(interval_secs: u64, chain_id: u64, gas_cost_usd: f64) -> Self {
         Self {
             period: Duration::from_secs(interval_secs.max(1)),
             chain_id,
             rpc_pool: None,
+            gas_cost_usd,
         }
     }
@@ log de boot (L733)
-            gas_cost_usd = GAS_COST_USD,
+            gas_cost_usd = self.gas_cost_usd,
@@ uso en el loop (L954)
-                    GAS_COST_USD,
+                    self.gas_cost_usd,
```

### D7. `backend/searcher-rs/src/engines/liquidation_engine.rs`

```diff
@@ espejo (L56-60) — DELETE completo
-/// Gas cost estimate (USD) for one liquidation call. Must match the value in
-/// `liquidation_worker::GAS_COST_USD` (30.0 USD) — kept private there by
-/// convention, mirrored here so the engine can use it without modifying the
-/// worker. Both constants guard the same math invariant.
-const GAS_COST_USD: f64 = 30.0;
@@ import (L42-44)
 use crate::workers::liquidation_worker::{
-    estimate_liquidation_profit, liquidation_bonus_bps_for_asset,
+    estimate_liquidation_profit, liquidation_bonus_bps_for_asset,
+    DEFAULT_GAS_COST_USD,
 };
@@ struct + ctor (L74-83)
 pub struct LiquidationEngine {
     pub indexer: Arc<Mutex<LendingPositionIndexer>>,
     pub chain_id: u64,
+    /// Operator gas-cost pre-screen (USD) — threaded from boot via
+    /// `liquidation_worker::resolve_gas_cost_usd()` (WO-04). Replaces the
+    /// private mirrored constant (drift hazard) removed above.
+    pub gas_cost_usd: f64,
 }
 
 impl LiquidationEngine {
     /// Constructs a new `LiquidationEngine`.
-    pub fn new(indexer: Arc<Mutex<LendingPositionIndexer>>, chain_id: u64) -> Self {
-        Self { indexer, chain_id }
+    pub fn new(
+        indexer: Arc<Mutex<LendingPositionIndexer>>,
+        chain_id: u64,
+        gas_cost_usd: f64,
+    ) -> Self {
+        Self {
+            indexer,
+            chain_id,
+            gas_cost_usd,
+        }
     }
@@ uso (L229)
-            GAS_COST_USD,
+            self.gas_cost_usd,
@@ tests (L520, L540, L584): reemplazar `GAS_COST_USD` → `DEFAULT_GAS_COST_USD`
```

### D8. `backend/searcher-rs/src/scanner.rs` (constructor del camino VIVO)

```diff
@@ L446
-    let liq_engine = Arc::new(LiquidationEngine::new(liq_indexer, chain_id));
+    let liq_engine = Arc::new(LiquidationEngine::new(
+        liq_indexer,
+        chain_id,
+        crate::workers::liquidation_worker::resolve_gas_cost_usd(),
+    ));
```

### D9. `backend/searcher-rs/src/main.rs` (worker legacy, gated)

```diff
@@ L1019-1022
             let mut lw = workers::liquidation_worker::LiquidationWorker::new(
                 liquidation_period_secs,
                 liquidation_chain,
+                workers::liquidation_worker::resolve_gas_cost_usd(),
             );
```

### D10. `backend/shared-rs/src/trading_config.rs`

```diff
@@ TradingConfigState (post p_copied_max, L358)
     #[serde(default = "default_p_copied_max")]
     pub p_copied_max: f64,
+
+    /// Default LP-fee fraction applied by the api-server live simulation
+    /// (component 2) when a route's per-leg fee tiers are unknown to the
+    /// hot path. Canonical value 0.003 = the V2 30 bps tier hardcoded at
+    /// computeSimulatedNet.ts:139 until 2026-09-06 (WO-04). NOT a pool-fee
+    /// source of truth: per ROUTES_CROWN_JEWEL rule 4, on-chain per-leg
+    /// tiers are the truth — this is the operator-governed proxy default
+    /// (rows carry the "-proxy" note, R8). Backed by migration 119 column
+    /// `trading_config.lp_fee_default_pct` (CHECK 0–0.5). `serde(default)`
+    /// keeps legacy Redis configs valid (schema-drift precedent: enabled_dex_ids).
+    #[serde(default = "default_lp_fee_default_pct")]
+    pub lp_fee_default_pct: f64,
@@ defaults (post default_p_copied_max, L408-410)
+/// V2 30 bps canonical LP tier as fraction — the api-server proxy default.
+fn default_lp_fee_default_pct() -> f64 {
+    0.003
+}
```

### D11. `database/migrations/119_trading_config_lp_fee_default_pct.sql` (NUEVO)

```sql
-- WO-04 (2026-09-06): parameterize the api-server LP-fee proxy default
-- (was hardcoded 0.003 at backend/api-server/src/simulation/computeSimulatedNet.ts:139).
-- DEFAULT preserves deployed behavior exactly (rows existing and new = 0.0030).
-- Doctrine (ROUTES_CROWN_JEWEL rule 4): on-chain per-leg fee tiers remain the
-- source of truth; this column is the operator-governed proxy for routes whose
-- legs the api-server hot path cannot resolve. Bounds mirror the zod schema.
ALTER TABLE trading_config
    ADD COLUMN IF NOT EXISTS lp_fee_default_pct NUMERIC(6,4) NOT NULL DEFAULT 0.0030
    CHECK (lp_fee_default_pct >= 0 AND lp_fee_default_pct <= 0.5);
```

### D12. `backend/api-server/src/routes/trading-config.ts`

```diff
@@ zod (post L173)
     flashloan_fee_pct: z.number().min(0).default(0.0009),
+    // WO-04 (migration 119): LP-fee proxy default for the live simulation
+    // when per-leg tiers are unknown. Bounds match the DB CHECK exactly.
+    lp_fee_default_pct: z.number().min(0.0).max(0.5).default(0.003),
@@ interface DbRow (post L261 flashloan_fee_pct: string;)
+  lp_fee_default_pct: string;
@@ row→snapshot map (post L370 flashloan_fee_pct: Number(...))
+    lp_fee_default_pct: Number(row.lp_fee_default_pct),
@@ SQL: añadir `lp_fee_default_pct` junto a `flashloan_fee_pct` en TODAS las
   sentencias que lo listan — sitios detectados por grep: L425, L500, L576,
   L607 (ON CONFLICT ... = EXCLUDED.), L627, L655 (PUT body), L839.
```

### D13. `backend/api-server/src/simulation/tradingConfigSnapshot.ts`

```diff
@@ interface TradingConfigSnapshot (post L48 flashloan_fee_pct: number;)
+  /** WO-04: LP-fee proxy default (fraction). Mirrors the Rust serde default. */
+  lp_fee_default_pct: number;
@@ parseSnapshot (post L167)
     flashloan_fee_pct: num(o["flashloan_fee_pct"], 0.0009),
+    lp_fee_default_pct: num(o["lp_fee_default_pct"], 0.003),
```

### D14. `backend/api-server/src/simulation/computeSimulatedNet.ts`

```diff
@@ L138-139 — DELETE (el default doctrinal vive ahora en UN solo lugar por lado: parseSnapshot)
-/** Default V2 LP fee in fraction (30 bps). */
-const LP_FEE_FRACTION_DEFAULT = 0.003;
@@ L236-240
   // Component 2: LP fees — default 30bps V2 tier when route legs unknown.
   // The api-server doesn't have per-leg fee tiers; this is a first-order proxy.
-  const lp_fees_usd = amountInUsdVal * LP_FEE_FRACTION_DEFAULT;
-  if (lp_fees_usd > 0) notes.push("lp-fee=30bps-proxy");
+  // Operator-governed default: trading_config `lp_fee_default_pct`
+  // (WO-04, migration 119; absent ⇒ 0.003 = V2 30 bps tier).
+  const lp_fees_usd = amountInUsdVal * cfg.lp_fee_default_pct;
+  if (lp_fees_usd > 0) {
+    notes.push(`lp-fee=${Math.round(cfg.lp_fee_default_pct * 10_000)}bps-proxy`);
+  }
@@ L369-377 varCostRateFromCfg
     cfg.flashloan_fee_pct +
     cfg.max_slippage_pct +
     cfg.failure_risk_buffer_pct +
-    LP_FEE_FRACTION_DEFAULT +
+    cfg.lp_fee_default_pct +
```

### D15. `backend/api-server/src/simulation/computeSimulatedNet.test.ts` (tests de invariante)

- Caso 1 (invariante de primer deploy): snapshot SIN el campo → `lp_fees_usd
  == amount_in_usd × 0.003` y nota exacta `"lp-fee=30bps-proxy"`.
- Caso 2 (knob): snapshot con `lp_fee_default_pct: 0.001` → `lp_fees_usd` ×
  0.001, nota `"lp-fee=10bps-proxy"`, y `varCostRate` refleja el delta.
- (Opcional Rust, en `liquidation_worker.rs` tests existentes): parse de
  `resolve_gas_cost_usd` — requiere env-mutation serializada (patrón ENV_LOCK
  de `shared-rs/src/config.rs:300`); si Oleada 4 no lo incluye, declararlo.

---

## (e) Gate doctrinal — ROUTES_CROWN_JEWEL_DOCTRINE (regla 4, L53-54)

> **"Fees on-chain: Aave/Balancer/Uniswap se leen de la cadena, jamás se
> hardcodean (Aave = 5bps HOY, gobernable mañana)."**

Justificación de fuente correcta POR literal:

1. **LP fee 30 bps (literal 4) — SÍ es fee de Variedad de Liquidez → la regla
   aplica.** La fuente correcta doctrinal es el fee tier on-chain por leg (V2:
   30 bps del par; V3: tiers 100/500/3000/10000 bps — la doctrina lo modela en
   `graph_builder.rs` con aristas paralelas por (DEX×versión×fee), ref. 9).
   El hoy es un PROXY declarado (comentario L237-238: *"The api-server doesn't
   have per-leg fee tiers; this is a first-order proxy"*; nota R8 `-proxy` en
   cada fila). El diseño NO finge que la config reemplaza la cadena:
   (i) mueve el default de código-oculto a config-declarada-auditable
   (workbook `01_CONFIG` = "configuración viva"), (ii) documenta en el propio
   campo que la verdad es per-leg on-chain, (iii) el camino doctrinal completo
   (per-leg tiers desde `route_metadata`) queda marcado como follow-up. No hay
   fabricación (RULE 00): el proxy ya existía etiquetado; ahora es gobernable.

2. **Tip 2 gwei (literal 1) — NO es fee de pool → la regla 4 NO gobierna su
   fuente.** Es `max_priority_fee_per_gas`, parámetro del mercado EIP-1559.
   Fuente correcta por jerarquía: señal de mercado en vivo (p75 tip / basefee)
   → estimación operador → default documentado. El repo YA implementa la fuente
   in-vivo en `TradingConfigState::resolve_gas_price_gwei(basefee, p75_tip)`
   (`trading_config.rs:434-442`, estrategias `Percentile75` /
   `DynamicBasefeePlusTip`) para el lado evaluación; relays-client (terminus de
   broadcast) no consume trading_config — su superficie es AppConfig TOML.
   El diseño parametriza con default 2 gwei DOCUMENTADO y deja anotado como
   evolución (fuera de Oleada 4) consumir un oráculo de tip; el `base_fee`
   en vivo ya se lee del último bloque (L161-170).

3. **GAS_COST_USD 30 (literales 2+3) — NO es fee de pool → regla 4 no aplica;
   es estimación de fricción termodinámica.** Fuente correcta última:
   `gas_units × gas_price_gwei × 1e9/1e18 × base_token_price_usd` — que el repo
   YA implementa downstream en `TradingConfigState::gas_cost_usd()`
   (`trading_config.rs:457-461`, idéntico al spine `config_aware::
   estimate_gas_cost_usd`). El doc del propio worker (L145-146) declara el
   alcance: *"this guard is a pre-screen only"* — el knob parametriza el
   pre-screen; el recomputo en vivo ya existe y no se toca.

---

## Invariante de primer deploy (sin cambio de comportamiento)

- **L1**: `configs/app.toml` sin la clave → serde default 2.0 → tip 2 gwei
  idéntico byte a byte. Schema validado (ARBX_VALIDATE_SCHEMA=1) sigue verde
  (default satisface `minimum: 0`). zod shared-ts idem (`.default(2.0)`).
- **L2/L3**: env ausente → `DEFAULT_GAS_COST_USD = 30.0` en worker y engine
  (misma resolución, imposible diverger). Log de boot reporta 30.0 (antes
  imprimía la constante 30.0 — mismo valor).
- **L4**: Redis blob sin la clave → serde default 0.003 + TS `num(..., 0.003)`;
  PG migración con `DEFAULT 0.0030` → filas existentes quedan en 0.003;
  nota `"lp-fee=30bps-proxy"` idéntica en el caso default (verificado:
  `Math.round(0.003*10000) === 30`).
- **Orden de deploy L4**: migración 119 ANTES del deploy del api-server nuevo
  (la fila leída sin columna rompería el SELECT — el zod default protege el
  PUT, no el SELECT).

## Verificación para Oleada 4 (comandos exactos — NO ejecutados en este WO)

- Rust (workspace raíz): `cargo check -p shared-rs -p searcher-rs -p relays-client`
  y `cargo test -p searcher-rs liquidation` — caveat Windows AppControl 4551
  (memory: dev-profile mayormente ejecuta; si bloquea, declarar
  APPLIED_UNVERIFIED para tests, check SÍ corre).
- TS api-server: desde `backend/api-server`: `npx tsc --noEmit` (workspace
  hoisted) + `npx vitest run src/simulation/computeSimulatedNet.test.ts`.
- shared-ts: `npx tsc --noEmit` dentro de `shared-ts/`.
- Paridad de defaults (4 planos del L4): 0.003 (Rust) == 0.003 (zod) ==
  0.0030 (PG) == 0.003 (snapshot TS) — cubierta por el Caso 1 del test D15.

## Riesgos

1. Cambios de firma (D4/D6/D7): call-sites únicos productivos identificados
   (submit_engine.rs:438, main.rs:1019, scanner.rs:446); tests del engine que
   usan `GAS_COST_USD` (3 sitios) quedan migrados en D7. Riesgo bajo.
2. Doble lectura del env `LIQUIDATION_GAS_COST_USD` (main.rs + scanner.rs) en
   el mismo proceso al boot — mismo valor (env estable); sin hot-reload
   (documentado: pre-screen, spine recomputa).
3. Migración 119: tabla `trading_config` diminuta (1 fila/chain); `ADD COLUMN
   ... DEFAULT` constante = metadata-only en PG ≥ 11. Riesgo despreciable.
4. §37 P-∅ (un PR = un ID): este WO agrupa 3 sub-cambios coherentes bajo
   /goal §2.5; si el operador prefiere, L1 (tip), L2+L3 (gas usd) y L4 (lp fee)
   son PRs independiables sin conflicto de archivos salvo `trading_config.rs`
   (solo L4 lo toca).
5. Fuera de alcance declarado: `gas_oracle.rs:37` (gemelo no cableado — no
   tocar) y `bundle_builder.rs:169` (fallback base fee 30 gwei — adyacente,
   no charter).

## Estado

**DESIGNED** — diseño completo y verificado por lectura; aplicación y
verificación ejecutable corresponden a la Oleada 4.
