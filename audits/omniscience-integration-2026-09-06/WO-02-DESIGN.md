# WO-02 — DISEÑO: wiring de `HotPathEmitter` (hallazgo CRÍTICO N3 #2 + /goal)

- **Work-order:** WO-02 · **Tipo:** DISEÑO READ-ONLY (Oleada 3) — aplicar en Oleada 4
- **Charter:** mapear módulo → localizar punto de cableado → decidir CABLEAR vs ELIMINAR → diffs exactos → invariante + gates
- **Reglas:** cero git, cero deploy, cero VPS, cero broadcast (§32/§33/§34). Solo lectura de código + `cargo check` baseline.
- **Verificación ejecutada:** `cargo check -p searcher-rs --quiet` (desde `backend/`, target caliente) → **EXIT=0**. No se editó ningún archivo de código.

---

## 0. Decisión (resumen ejecutivo)

**CABLEAR — pero SOLO `emit_simulated`, en la etapa POST-simulación REVM y DESPUÉS del publish canónico. NO cablear `emit_detected`. NO eliminar el cable.**

| Alternativa | Veredicto | Razón principal |
|---|---|---|
| Cablear `emit_simulated` (post-REVM, post-publish) | **SÍ** | Es el ÚNICO productor posible del evento `opportunity:validated` que el frontend ya escucha (`websocket-client.ts` L146-147); única señal en wei de sims en tiempo real; satisface la restricción CROSS ("etapa POST-validación, jamás el flood crudo de detección") |
| Cablear `emit_detected` | **NO** | Duplicación sin valor: WO-01 ya entregó el feed detected vía `new_opportunity` (PG LISTEN, payload MÁS rico). Cablearlo = 3 ops Redis extra por evento sobre un flood 100%-rejected (~48K/24h) — exactamente lo que CROSS prohíbe trasladar al room WS |
| Eliminar emitter + HotStreamer + rooms | **NO** | Blast radius MAYOR que cablear (frontend, e2e tests, fixtures, edge dev-local, docs x4, runbook, paper executor Task-6, scripts VPS). Además destruiría el único canal de validación sub-100ms y el contrato `opportunity:validated` del DApp. El problema no es el cable: es que nunca se conectó |

---

## 1. (a) Mapa del módulo — API pública y estado actual

`backend/searcher-rs/src/hot_path_emitter.rs` (212 líneas, declarado en `lib.rs:125`):

| Símbolo | Firma actual | Stream/clave | Call-sites |
|---|---|---|---|
| `HotPathEmitter::new` | `(MultiplexedConnection) -> Self` | — | **0** |
| `emit_detected` | `(&self, opp: &Opportunity) -> Result<(), RedisError>` | XADD `arbx:hot:detected` (MAXLEN ~10000) + HSET `arbx:hot:opp:{id}` + EXPIRE 300 | **0** |
| `emit_simulated` | `(&self, id: &str, result: &SimulationResult)` | XADD `arbx:hot:simulated` (MAXLEN ~5000) + HSET `arbx:hot:sim:{id}` (solo passed) | **0** |
| `emit_gate_commit_from_state` | `(&self, &GateEnergyState)` | XADD `arbx:gate:commit` | **0** (stream sin consumidores en todo el repo) |
| `SimulationResult` | `{passed: bool, net_profit_wei: u128, gas_used: u64}` (Serialize/Deserialize) | — | 0 |

Defectos latentes del módulo muerto (relevantes para el wiring):

1. `SimulationResult.net_profit_wei: u128` **no puede** contener el `simulated_profit_token_in: U256` real (truncaría/panickearía al convertir).
2. `emit_simulated` XADDea solo `id/status/net_profit_wei/gas_used/timestamp_ms` — el `PaperExecutor` del api-server (`paper/executor.ts:218`) hace `skip_incomplete` de toda entrada sin `opportunity_id` + `chain_id` + `strategy_kind`. **Cablear tal cual produciría un stream no-vacío pero 100% descartado** (silencio funcional disfrazado de verde — patrón G-SIM-1).
3. Toma `MultiplexedConnection`, pero el pipeline scanner maneja `redis::aio::ConnectionManager` (`decode_and_score_tx` L1485, `publisher::publish` L10).

## 2. Consumidores del canal hot (verificados en código)

| Consumidor | Stream | Estado prod (auditoría N3 2026-09-06) | Contrato que exige |
|---|---|---|---|
| `OpportunityHotStreamer` (`api-server/src/websocket.ts:704-799`) | `arbx:hot:detected` → evento WS `opportunity:detected`; `arbx:hot:simulated` → evento WS **`opportunity:validated`** | ACTIVO (boot log "Starting poll loops"); XLEN=0 perpetuo; 62 consumers huérfanos `ws-emitter-g0` | campos string arbitrarios (`parseFields` los copia todos) — aditivo-safe |
| Frontend `HotOpportunityWebSocket` (`frontend/lib/websocket-client.ts:142-147`) | eventos WS anteriores | Escucha ambos; `onValidated` existe a nivel lib (sin consumidor UI hoy); WO-01 añadió `new_opportunity` al mismo flujo detected | `HotOpportunityEvent` con index signature — aditivo-safe |
| `PaperExecutor` (`api-server/src/paper/executor.ts`) | `arbx:hot:simulated` → `paper_trade_runs` + `arbx:hot:paper_executed` | **DORMANT** (`ARBX_PAPER_EXECUTOR_MODE` default off, `index.ts:1906`) | `id`+`status` obligatorios; `opportunity_id`+`chain_id`+`strategy_kind` para no ser `skip_incomplete`; `net_profit_wei` como fallback de gross; `gas_used` como fallback de gas |
| `paper-archiver-g0` (ledger vivo de prod) | **`arbx:opps:detected`** (NO hot) | ACTIVO | fuera de este canal — el ledger paper NO depende del hot stream |

## 3. (b) Dónde DEBE cablearse — análisis del pipeline searcher

Recorrido verificado de `decode_and_score_tx` (`scanner.rs:1482-2629`, path legacy/spine — el ÚNICO donde corre la sim REVM; el path V2 `orchestrator.on_route_intent` retorna en L1576 sin simular):

```
L2372  dispatch_encoder_gate           → EncoderOk(ctx) | NoSimulator | NoProvider | Rejected   [pre-REVM]
L2384  dispatch_orchestrator_and_classify (feature v2-simulator = DEFAULT ON)
          L2984  spawn_blocking(execute_multistep_revm)  ← LA SIM REVM
          L3021+ veredicto outcome.passed → 4 salidas post-REVM
                (wrapped_calldata_missing | net_usd_rejected | SIM_SUCCESS | failed-tail)
          salidas pre-REVM (missing_executor, spawn_blocking_failed) → None
L2418  persist ValidatedPlan (arbx:validated_plan:{id}, TTL 300)      [fail-soft]
L2585  opp_dedup (BE-3.6) — dedupe SUPRIME el candidato (return Ok)
L2603  PG insert (opportunities)
L2618  publisher::publish → arbx:opps:detected                        [CANÓNICO, fail con ?]
L2620  OPPORTUNITIES_TOTAL
```

**Punto de cableado elegido: inmediatamente después de L2618 (publish canónico OK).**

Por qué ahí y no antes:

1. **Restricción CROSS (etapa POST-validación):** emitir en detección cruda trasladaría el flood (~100% rejected) al room WS. Emitir post-publish garantiza `hot:simulated ⊆ opps:detected` (1:1 con oportunidades publicadas).
2. **Integridad FK:** `paper_trade_runs.opportunity_id` referencia `opportunities.id` — el PG insert (L2603) ocurre ANTES del publish; emitir después maximiza que la entrada resuelva a fila existente (si el insert falló, el executor ya tiene el path observable `skip_opportunity_absent` 23503).
3. **Fail-honest:** si `publish` falla (`?`), no se emite hot — el canal auxiliar nunca lleva lo que el canónico no publicó.
4. **Dedup:** los candidatos dedupeados (L2585) no emiten hot — su sim referencia una oportunidad inexistente. Volumen acotado adicionalmente.
5. **Fail-soft asimétrico** (mismo patrón que `validated_plan.persist_failed` L2435): el canónico ya éxito; un error del hot stream se loguea con event-tag y NO rompe el pipeline.

**`emit_simulated` SOLO con sims que corrieron REVM de verdad:** el veredicto se captura del `SimulationOutcome` real tras `spawn_blocking` (4 salidas post-REVM → `Some`); las salidas pre-REVM → `None` (no se fabrica un "failed" de algo que nunca se simuló — R8; esas clasificaciones ya son observables vía counters/PG).

**`status` = veredicto REVM VERBATIM** (incl. `net_usd_rejected`, que REVM pasó pero el gate económico rechazó): doctrina "NUNCA re-etiquetar" — el PaperExecutor aplica su propio gate net (gross−gas) y registra REJECTED con razón; el emitter jamás re-clasifica.

## 4. (c) Criterio de decisión — valor real del canal para el DApp hoy

- `opportunity:detected` (WS): **ya servido** por WO-01 (`new_opportunity`, payload fila PG completa). Cablear `emit_detected` añadiría solo latencia marginal a costa de duplicar el flood. → NO.
- `opportunity:validated` (WS): **sin productor alternativo**. Es la única señal con economía en wei (`net_profit_wei`, `gas_used`, `gas_price_wei`) en tiempo real; el cliente ya la escucha; los paneles readiness/goal (certificación HG) necesitan distinguir "sims corren" de "sims aprueban" — este canal provee exactamente esa clase de señal. → SÍ.
- Costo del wiring: 1 XADD por sim post-publish (+2 ops solo en passed) — despreciable vs presupuesto sub-100ms; MAXLEN ~5000 acota memoria; observer-only (§34 mode-invariant, sin ramas de modo).

---

## 5. (d) Diffs EXACTOS propuestos

### 5.1 `backend/searcher-rs/src/hot_path_emitter.rs` (modificado)

```diff
--- a/backend/searcher-rs/src/hot_path_emitter.rs
+++ b/backend/searcher-rs/src/hot_path_emitter.rs
@@ -15,7 +15,7 @@
 
-use redis::aio::MultiplexedConnection;
+use redis::aio::ConnectionManager;
 use shared_rs::contracts::Opportunity;
 use std::time::{SystemTime, UNIX_EPOCH};
 
@@ -20,13 +20,25 @@
 /// Simulation outcome passed from the REVM orchestrator.
 /// Mirrored from `prioritization_spine::round_trip_executor::SimulationOutcome`
 /// to avoid deep trait coupling in the emitter boundary.
+///
+/// WO-02 (2026-09-06): `net_profit_wei`/`gas_price_wei` are decimal STRINGS
+/// because the source `simulated_profit_token_in`/`gas_price_wei` are `U256`;
+/// a `u128` field would truncate on overflow and a coerced value violates R8.
+/// The wire contract was stringified anyway (XADD net_profit_wei.to_string()).
+/// NOTE: `net_profit_wei` carries the REVM-verdict GROSS token_in delta
+/// (`simulated_profit_token_in`); the net-of-gas decision belongs to
+/// downstream consumers (paper-executor net gate / `net_usd_viable`).
 #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
 pub struct SimulationResult {
     pub passed: bool,
-    pub net_profit_wei: u128,
+    pub net_profit_wei: String,
     pub gas_used: u64,
+    pub gas_price_wei: String,
 }
@@ -35,9 +47,9 @@
 #[derive(Clone)]
 pub struct HotPathEmitter {
-    redis: MultiplexedConnection,
+    redis: ConnectionManager,
 }
@@ -42,8 +54,8 @@
-    /// Creates a new emitter from an existing Redis multiplexed connection.
-    pub fn new(redis: MultiplexedConnection) -> Self {
+    /// Creates a new emitter from an existing Redis connection manager
+    /// (the handle type the scanner pipeline already threads).
+    pub fn new(redis: ConnectionManager) -> Self {
         Self { redis }
     }
@@ -104,12 +116,24 @@
-    /// Emits a simulation result to `arbx:hot:simulated` stream.
-    ///
-    /// Stream fields:
-    ///   - `id`: Opportunity UUID
-    ///   - `status`: "passed" or "failed"
-    ///   - `net_profit_wei`: Stringified u128 (canonical for precision)
-    ///   - `gas_used`: Gas consumed in simulation
-    ///   - `timestamp_ms`: Unix timestamp millis
-    ///
-    /// On `passed=true`, also stores full result at `arbx:hot:sim:{id}` with 300s TTL.
-    pub async fn emit_simulated(
-        &self,
-        id: &str,
-        result: &SimulationResult,
-    ) -> Result<(), redis::RedisError> {
+    /// Emits a simulation result to `arbx:hot:simulated` stream.
+    ///
+    /// WO-02 (2026-09-06): takes the full `Opportunity` so the XADD carries
+    /// the fields BOTH consumers require — `OpportunityHotStreamer`
+    /// (api-server websocket.ts → room `opportunities`, event
+    /// `opportunity:validated`) and the dormant `PaperExecutor`
+    /// (api-server paper/executor.ts), whose `parseSimulatedOpportunity`
+    /// drops entries without `id`+`status` and skips (`skip_incomplete`)
+    /// entries without `opportunity_id`+`chain_id`+`strategy_kind`.
+    ///
+    /// Stream fields:
+    ///   - `id`: Opportunity UUID (stream-message correlation)
+    ///   - `opportunity_id`: same UUID — PaperExecutor FK into opportunities.id
+    ///   - `status`: "passed" | "failed" — REVM verdict, VERBATIM (R8: the
+    ///     emitter never re-classifies; downstream gates apply their own)
+    ///   - `net_profit_wei`: decimal string (see SimulationResult)
+    ///   - `gas_used`: gas consumed by the REVM round trip
+    ///   - `gas_price_wei`: decimal string (gas price the simulator used)
+    ///   - `chain_id`, `strategy_kind`, `token_pair`: correlation fields
+    ///   - `timestamp_ms`: Unix timestamp millis
+    ///
+    /// On `passed=true`, also stores full result at `arbx:hot:sim:{id}` with 300s TTL.
+    pub async fn emit_simulated(
+        &self,
+        opp: &Opportunity,
+        result: &SimulationResult,
+    ) -> Result<(), redis::RedisError> {
         let timestamp_ms = SystemTime::now()
             .duration_since(UNIX_EPOCH)
             .unwrap()
             .as_millis() as u64;
 
         let status = if result.passed { "passed" } else { "failed" };
+        let id = opp.id.to_string();
 
         // XADD arbx:hot:simulated with approximate maxlen ~5k
         let _: () = redis::cmd("XADD")
             .arg("arbx:hot:simulated")
             .arg("MAXLEN")
             .arg("~")
             .arg(5000)
             .arg("*")
             .arg("id")
-            .arg(id)
+            .arg(&id)
             .arg("status")
             .arg(status)
             .arg("net_profit_wei")
-            .arg(result.net_profit_wei.to_string())
+            .arg(&result.net_profit_wei)
             .arg("gas_used")
             .arg(result.gas_used)
+            .arg("gas_price_wei")
+            .arg(&result.gas_price_wei)
+            .arg("opportunity_id")
+            .arg(&id)
+            .arg("chain_id")
+            .arg(opp.chain_id)
+            .arg("strategy_kind")
+            .arg(opp.strategy_kind.as_str())
+            .arg("token_pair")
+            .arg(&opp.pair_symbol)
             .arg("timestamp_ms")
             .arg(timestamp_ms)
             .query_async(&mut self.redis.clone())
             .await?;
```

(HSET/EXPIRE de `arbx:hot:sim:{id}` permanecen igual — `format!("arbx:hot:sim:{}", id)` ya compila con `id: String`.)

`emit_detected` y `emit_gate_commit_from_state` quedan INTACTOS y sin cablear (decisión §0). Defecto pre-existente observado, no tocado (§3): `serde_json::to_string(opp).unwrap_or_default()` en `emit_detected` fabricaría un payload vacío si la serialización fallara — irrelevante mientras el método siga sin call-sites.

### 5.2 `backend/searcher-rs/src/scanner.rs` (modificado — 4 hunks)

**Hunk 1 — L2384 (destructure a 5-tupla):**

```diff
-    let (fail_closed_reason, trace_hash_sentinel, sim_status_str, validated_plan) =
+    // WO-02 (2026-09-06): 5th element — the REVM-verdict record for the
+    // arbx:hot:simulated stream (None when no REVM sim ran for this candidate).
+    let (fail_closed_reason, trace_hash_sentinel, sim_status_str, validated_plan, hot_sim) =
         if let (EncoderGateOutcome::EncoderOk(ctx), Some(simulator_arc)) =
             (&gate_outcome, simulator_v2.cloned())
         {
             dispatch_orchestrator_and_classify(
                 ctx.clone(),
                 simulator_arc,
                 client.chain_id,
                 &candidate,
                 gate_token_in_decimals,
                 gate_token_in_price_usd,
                 gate_eth_price_usd,
             )
             .await
         } else {
             let (fcr, ths) = gate_outcome.to_status();
             (
                 fcr.to_string(),
                 ths.to_string(),
                 "SIM_DISABLED_FAIL_CLOSED".to_string(),
                 None,
+                None,
             )
         };
```

**Hunk 2 — L2618 (emisión post-publish, fail-soft):**

```diff
     publisher::publish(redis, &opportunity).await?;
+
+    // WO-02 (2026-09-06): XADD the REVM simulation verdict to
+    // arbx:hot:simulated — ONLY when a wrapped-flash REVM sim actually ran
+    // (post-simulation stage — N3/CROSS constraint: never the raw detection
+    // flood) and ONLY after the canonical arbx:opps:detected publish, so
+    // every stream entry references a published (and normally PG-persisted)
+    // opportunity — the api-server PaperExecutor joins on opportunities.id.
+    // FAIL-SOFT: the canonical path above already succeeded; a hot-stream
+    // Redis error is logged (observable, R8) and never fails the pipeline.
+    if let Some(sim) = hot_sim {
+        let emitter = crate::hot_path_emitter::HotPathEmitter::new(redis.clone());
+        if let Err(e) = emitter.emit_simulated(&opportunity, &sim).await {
+            warn!(
+                event = "hot_path.simulated_emit_failed",
+                opp_id = %opportunity.id,
+                error = %e,
+                "fail-soft: arbx:hot:simulated XADD failed; canonical publish already succeeded"
+            );
+        }
+    }
 
     OPPORTUNITIES_TOTAL
```

**Hunk 3 — `dispatch_orchestrator_and_classify` (doc L2883, firma L2909, 6 returns + helper):**

```diff
-    /// Returns (fail_closed_reason, trace_hash_sentinel, simulation_status)
-    /// triple consumed by the hot path.
+    /// Returns (fail_closed_reason, trace_hash_sentinel, simulation_status,
+    /// validated_plan, hot_sim_record) consumed by the hot path. The 5th
+    /// element is Some ONLY on paths where the REVM sim actually ran
+    /// (WO-02: arbx:hot:simulated producer).
@@ firma (L2909-2914):
 ) -> (
     String,
     String,
     String,
     Option<prioritization_spine::ValidatedPlan>,
+    Option<crate::hot_path_emitter::SimulationResult>,
 ) {
@@ tras el match de spawn_blocking (después de L3000, antes del bloque scoring L3002):
+    // WO-02: capture the REVM verdict ONCE, verbatim, for the
+    // arbx:hot:simulated stream. Every return BELOW this point ran a real
+    // REVM sim → Some(...); the pre-REVM returns above stay None.
+    let hot_sim = hot_sim_record(&outcome);
@@ returns pre-REVM (None — REVM nunca corrió):
     L2926 missing_executor        → ( ..., None, None )
     L2993 spawn_blocking_failed   → ( ..., None, None )
@@ returns post-REVM (Some — veredicto verbatim):
     L3034 wrapped_calldata_missing → ( ..., None, Some(hot_sim.clone()) )
     L3092 net_usd_rejected         → ( ..., None, Some(hot_sim.clone()) )
     L3113 SIM_SUCCESS              → ( ..., Some(validated_plan), Some(hot_sim.clone()) )
     L3145 failed-tail              → ( ..., None, Some(hot_sim.clone()) )
@@ helper nuevo (junto a dispatch, mismo cfg):
+/// WO-02 (2026-09-06): pure mapping REVM SimulationOutcome → the
+/// arbx:hot:simulated wire record. Verbatim verdict + stringified U256
+/// economics (R8: no truncation, no re-classification). Pure so it is
+/// unit-testable without Redis (the XADD lives in hot_path_emitter).
+#[cfg(feature = "v2-simulator")]
+fn hot_sim_record(
+    outcome: &prioritization_spine::round_trip_executor::SimulationOutcome,
+) -> crate::hot_path_emitter::SimulationResult {
+    crate::hot_path_emitter::SimulationResult {
+        passed: outcome.passed,
+        net_profit_wei: outcome.simulated_profit_token_in.to_string(),
+        gas_used: outcome.gas_used_total,
+        gas_price_wei: outcome.gas_price_wei.to_string(),
+    }
+}
```

**Hunk 4 — test unitario nuevo (mod tests, L3153+):**

```rust
+    // ── WO-02: hot-stream record maps the REVM outcome verbatim (R8) ──────
+    #[cfg(feature = "v2-simulator")]
+    #[test]
+    fn hot_sim_record_maps_outcome_verbatim() {
+        use prioritization_spine::round_trip_executor::SimulationOutcome;
+
+        // Failed outcome (SimulationOutcome::failed): zeroed economics — the
+        // honest "sim did not complete" record, never fabricated numbers.
+        let failed = SimulationOutcome::failed("revm_reverted:revert");
+        let r = hot_sim_record(&failed);
+        assert!(!r.passed);
+        assert_eq!(r.net_profit_wei, "0");
+        assert_eq!(r.gas_price_wei, "0");
+        assert_eq!(r.gas_used, 0);
+
+        // Passed outcome with a U256 profit EXCEEDING u128 — the string field
+        // must preserve full precision (the pre-WO-02 u128 field truncated).
+        let mut passed = SimulationOutcome::failed("unused");
+        passed.passed = true;
+        passed.simulated_profit_token_in =
+            ethers::types::U256::from(2u32) * ethers::types::U256::from(u128::MAX);
+        passed.gas_used_total = 424_242;
+        let r2 = hot_sim_record(&passed);
+        assert!(r2.passed);
+        assert_eq!(r2.net_profit_wei, passed.simulated_profit_token_in.to_string());
+        assert_eq!(r2.gas_used, 424_242);
+    }
```

### 5.3 `docs/redis-schema/hot-path-v2.md` (modificado — sección L27-36)

```diff
 ### arbx:hot:simulated (Stream)
 - **Purpose**: Store REVM simulation results for opportunities that passed validation
-- **Producer**: searcher-rs (post-REVM simulation, only for passed results)
+- **Producer**: searcher-rs decode_and_score_tx (WO-02, 2026-09-06) — post-REVM,
+  AFTER the canonical arbx:opps:detected publish; emitted for every sim that
+  actually RAN (status passed|failed, REVM verdict verbatim), never for
+  candidates that failed before REVM dispatch
 - **Fields**:
-  - `id`: Reference to the original opportunity
-  - `sim_result`: JSON-encoded simulation output
-  - `net_profit_wei`: Net Topological Yield in wei (after gas estimation)
-  - `gas_used`: Estimated gas consumption
-  - `trace_hash`: Hash of the execution trace for verification
+  - `id`: Opportunity UUID (stream-message correlation)
+  - `opportunity_id`: same UUID — PaperExecutor FK into opportunities.id
+  - `status`: passed | failed (REVM verdict, verbatim)
+  - `net_profit_wei`: decimal string — REVM gross token_in delta; net-of-gas is a downstream decision
+  - `gas_used`: gas consumed by the REVM round trip
+  - `gas_price_wei`: decimal string — gas price the simulator used
+  - `chain_id` / `strategy_kind` / `token_pair`: correlation fields
+  - `timestamp_ms`: Unix epoch millis
 - **MAXLEN**: ~5000
+- **Consumers**: ws-emitter-g0 (api-server OpportunityHotStreamer → WS room
+  opportunities, event opportunity:validated), paper-executor-g0
+  (dormant unless ARBX_PAPER_EXECUTOR_MODE=on)
```

### 5.4 COMPANION opcional (mismo PR, decisión del operador) — `backend/api-server/src/paper/executor.ts`

El wiring alimenta `skip_failed` a nivel `info` por entrada (L210-215) — el mismo patrón R9 ya sancionado en `paper_archiver.skip_rejected` (895 líneas/27 min). Downgrade a debug (la interfaz `ExecutorLogger` gana `debug`; el logger pino de index.ts ya lo implementa — typing estructural, sin cambios en index.ts):

```diff
 export interface ExecutorLogger {
+  debug(obj: object, msg?: string): void;
   info(obj: object, msg?: string): void;
@@ L210:
-      this.deps.logger.info(
+      this.deps.logger.debug(
         { event: "paper_executor.skip_failed", opportunity_id: sim.id, status: sim.status },
```

---

## 6. (e) Invariante de verificación (§33.1 — Redis RO, deltas documentados)

```
ANTES del deploy (baseline, VPS, Redis RO):
  XLEN arbx:hot:simulated   → 0   (perpetuo hoy: N3 lo midió 0 en 2 sondeos)
  XLEN arbx:hot:detected    → 0   (y DEBE SEGUIR 0: emit_detected no cableado por diseño)
  XLEN arbx:opps:detected   → ~10000 (MAXLEN cap; medir delta/min como baseline B)
  counter searcher round_trip_executor_started_total → anotar tasa base

DESPUÉS del deploy (ventana ≥30 min con detección viva):
  INV-1  XLEN arbx:hot:simulated delta > 0  ⟺  round_trip_executor_started_total avanza
         (solo sims REVM reales cuyo opp pasó dedup + publish)
  INV-2  XLEN arbx:opps:detected delta/min ≈ B (SIN cambio — el wiring no toca el canal canónico)
  INV-3  XLEN arbx:hot:detected SIGUE = 0
  INV-4  ∀ entry E en XREVRANGE arbx:hot:simulated + - COUNT 20:
           - SELECT 1 FROM opportunities WHERE id = E.opportunity_id → existe 1 fila (FK)
           - E.net_profit_wei y E.gas_price_wei matchean ^[0-9]+$ (nunca vacío ni NaN)
           - E.status ∈ {passed, failed} ∧ E.gas_used ∈ [0, 30000000]
  INV-5  XINFO GROUPS arbx:hot:simulated → ws-emitter-g0 con last-delivered-id ≠ 0-0
         (lag avanza; api-server logs "[HotStreamer]" sin poll errors)
  INV-6  paper_trade_runs SIN filas nuevas si todos los entries son failed
         (skip_failed en debug; ledger solo via passed ∧ net>0 — nunca re-etiquetar)
```

Si INV-1 delta=0 con counter avanzando → revisar modo (`ARBX_ORCHESTRATOR_MODE`: en `v2` el scanner retorna antes del sim gate — stream honestamente vacío) o el fail-soft `hot_path.simulated_emit_failed` en logs del searcher.

## 7. Gates

| Gate | Criterio PASS |
|---|---|
| `arbx-simulation-mandatory` | Observer-only: `passed` proviene VERBATIM de `outcome.passed` (ninguna otra fuente); ninguna capa de ejecución lee `arbx:hot:*` (relays-client NO lo consume — verificado por grep repo); el wiring no crea ni bypassa sims |
| R8 fail-honest | Fail-soft con event-tag observable; `None`=no simulado (pre-REVM) jamás se emite como failed; "0" wei = REVM zero real; sin unwrap productivos nuevos |
| RULE 00 / arbx-no-hardcode | Todos los campos del XADD provienen del `SimulationOutcome` real o del `Opportunity` publicado — cero fabricación |
| §34 mode-invariant | Sin ramas de modo: emite idéntico en v1/shadow/live; solo el terminus de capital difiere |
| §37 P-∅ | PR = 1 ID (WO-02 / N3 #2 + /goal); diffs quirúrgicos sin reformateo ajeno; companion 5.4 declarado como consecuencia directa (o split a micro-PR a criterio del operador) |
| CI | `cargo check -p searcher-rs` + `cargo clippy -p searcher-rs -- -D warnings` + `cargo fmt --check` + `cargo test -p searcher-rs hot_sim_record`; si 5.4 entra: `npx tsc --noEmit` desde `backend/api-server` |
| L4 post-deploy | INV-1..INV-6 sobre Redis RO + sondeo WS con socket.io-client real (NUNCA curl) si se quiere el end-to-end del room `opportunities` |

## 8. Riesgos (declarados)

1. **SIM_SUCCESS=0 hoy** (certificación HG: 0 passed): el stream llevará solo `status=failed` reales. Es el estado honesto del sistema; el root-cause (Capa A) está FUERA de WO-02. El DApp verá `opportunity:validated` con fallos — señal útil para el gate "sims que aprueban", no un bug del wiring.
2. **Volumen desconocido pre-deploy** (acotado por la tasa EncoderOk + dedup): monitorizar `round_trip_executor_started_total`; MAXLEN ~5000 acota memoria; el downgrade 5.4 acota el ruido del executor si el operador lo activa.
3. **Modo v2 en prod** dejaría el stream honestamente vacío (el path V2 no simula): la Oleada 4 DEBE verificar `scanner.orchestrator_mode` en logs antes de reclamar el invariante.
4. **FK 23503** si el insert PG falló pero publish OK → `skip_opportunity_absent` (path existente, observable).
5. **Dead code restante deliberado**: `emit_detected` y `emit_gate_commit_from_state` siguen sin call-sites (decisión documentada §0; no se eliminan — fuera de charter quirúrgico).
6. **62 consumers huérfanos** ws-emitter-g0 siguen creciendo (fuga por restarts): higiene = propuesta CROSS #4 (DELCONSUMER/XAUTOCLAIM), PR aparte.
7. `SimulationResult` cambia tipo de campo — sin riesgo (0 call-sites; el hash `arbx:hot:sim:{id}` no tiene lectores en prod).

## 9. Trazabilidad de evidencia (lecturas, no suposiciones)

- `backend/searcher-rs/src/hot_path_emitter.rs` (completo) · `lib.rs:124-125` · `publisher.rs` · `opportunity_emitter.rs` (path V2)
- `backend/searcher-rs/src/scanner.rs` L1482-1494 (firma), L2330-2406 (gate sim), L2418-2451 (validated_plan fail-soft), L2584-2628 (dedup→insert→publish→metrics), L2686-2796 (EncoderGateOutcome), L2895-3151 (dispatch completo, 6 returns)
- `backend/prioritization-spine/src/round_trip_executor.rs` L62-96 (SimulationOutcome: simulated_profit_token_in U256, gas_used_total u64, gas_price_wei U256)
- `backend/api-server/src/websocket.ts` L679-799 (HotStreamer) · `paper/executor.ts` (contrato de campos + skip_failed/skip_incomplete) · `index.ts` L1900-1917 (PaperExecutor dormant) · `routes/paper-trade-archiver.ts` L29 (archiver consume arbx:opps:detected, NO hot)
- `frontend/lib/websocket-client.ts` L142-147 (listeners) + WO-01-APPLY.md (listener new_opportunity ya aplicado)
- `audits/omniscience-integration-2026-09-06/03-api-ws.md` · `03-api-ws-CROSS.md` (restricción de etapa POST-validación) · `GOAL-WORKORDERS.md`
- `backend/searcher-rs/Cargo.toml` L71 (default = v2-simulator — REVM ON en prod)
- Verificación baseline: `cargo check -p searcher-rs --quiet` → EXIT=0 (backend/, target caliente). Diseño NO compilado (read-only): los diffs se validan con los gates §7 en la Oleada 4 — APPLIED_UNVERIFIED hasta entonces.

*WO-02 DESIGN — 2026-09-06. Fail-honest: cada afirmación de código lleva archivo:línea; las únicas inferencias (tasa de volumen REVM, modo orquestrador de prod) están declaradas como riesgo 2/3 con el contador/log exacto para cerrarlas.*
