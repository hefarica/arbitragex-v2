# WO-FUNNEL-01 — FIX (2026-09-17, fixer gang ronda 1)

> Gap asignado: "Scanner Pipeline Funnel en /operations muestra 0/0/0/0/0 en 'last 60s'
> mientras los logs del searcher muestran evaluaciones disparadas por tx_hash y el panel
> reporta PG INSERTED 2500 — el contador del heartbeat no está cableado a la ruta activa
> (riesgo de falso 'pipeline quieto', anti-patrón R9)".
> Fuente del gap: `BROWSE-Operador-de-mesa...md` §4.1 (gap 1). Estado: **FIX APLICADO y
> verificado localmente** (cero git/deploy/VPS-mutación).

## 0. Veredicto ejecutivo

El funnel 0/0/0/0/0 tenía **dos defectos independientes y ambos reales**:

- **Defecto A (bug de bucket, el load-bearing)**: TODA la salida del funnel
  (gate_* / passed_all_gates / db_persisted / db_errors) se incrementaba en el bucket
  legacy chain-0 (`counters()`), mientras el heartbeat drena `chain_counters(primary_chain)`
  (=1 en VPS). Los incrementos vivían pero eran invisibles para el heartbeat.
- **Defecto B (ruta activa sin contador de intake)**: la ruta de detección ACTIVA hoy
  (RouteScannerWorker RU-3, per-block, `DetectionSource::NewBlock`) no incrementa NINGÚN
  contador de intake — `pending_received`/`decoded_ok` miden exclusivamente el stream
  mempool WS, que hoy entrega 0.

Evidencia VPS read-only (07:48–07:52Z, `ssh arbx docker logs`, R9 aplicado — ventana
retenida arranca 07:36Z vs StartedAt 06:29Z, rotada):

- `scanner.heartbeat`: pending_received=0, decoded_ok=0, enriched=0, passed=0,
  db_persisted=0 **con pg_period_inserted=2520/2538/1679 y redis_stream_delta 0..3**.
- `intent_received` últimos 5m: **625/625 con source_event=new_block** (0 de mempool).
- `route_scanner.done`: 500 cycles_found / ~400 dispatched por bloque (~12 s);
  `canonical_dispatch` budget=25/block → 25×25=625 = coincide EXACTO con intent_received.
- `route_decoder.done` (path mempool→orchestrator): **0 en 60m** → el stream mempool
  no entrega nada (ver §5 residual 1).
- Env del contenedor (docker inspect, read-only): `ARBX_MEMPOOL_MODE=auto`,
  `ARBX_ORCHESTRATOR_MODE=v2`, `ARBX_CARTRIDGE_MODE=active`,
  `ARBX_MEMPOOL_ALLOWLIST=` (vacío) + `RPC_WS_1` alchemy+publicnode → auto resuelve a
  **Firehose** en mainnet.

## 1. Cadena causal exacta (con file:line)

```
route_scanner_worker.rs (RU-3, cada newHeads)
  └─ :651 dispatched += 1  (intents a on_route_intent :636 y spawn_cartridge_eval :647)
       └─ orchestrator.spawn_cartridge_eval (orchestrator.rs:261) o on_route_intent (:292)
            └─ cartridge_boot.rs:964 active_evaluate_and_emit  (log :983 active_eval_enter)
                 └─ orchestrator process_candidate → OpportunityEmitter
                      ├─ emit_rejected  (opportunity_emitter.rs:450)  → gate_* al bucket chain-0  ← DEFECTO A
                      ├─ emit_accepted  (:313/:369)                  → passed_all_gates chain-0 ← DEFECTO A
                      └─ try_insert_pg_with_route (:699/:703)         → db_persisted/db_errors chain-0 ← DEFECTO A
                           └─ PG INSERT ~2500/min (lo que el panel SÍ veía) + Redis XADD
heartbeat_worker.rs:236-250 drena chain_counters(primary_chain=1) → lee TODO 0
```

El intake (Defecto B): `pending_received` solo se incrementa en `process_pending`
(scanner.rs:1397) / `process_pending_tx` (:1468) — rutas que hoy no disparan
(route_decoder.done=0). `decoded_ok` solo en scanner.rs:1561 (mismo path muerto) y :1620
(legacy). Precedente repo del fix: el comentario N-01 en scanner.rs:1557-1559 documenta
exactamente esta clase de gap ("Without this the legacy counter stays 0 … making the
pipeline look dead") — este fix es la continuación del mismo patrón hacia la ruta RU-3.

## 2. Fix aplicado (6 archivos, todos marcados `// WO-FUNNEL-01 (2026-09-17)`)

### Defecto A — bucket per-chain en el emitter (bug fix puro)
- `backend/searcher-rs/src/opportunity_emitter.rs`
  - :313, :369 `counters().passed_all_gates` → `chain_counters(opportunity.chain_id)`
  - :450 `let c = counters()` → `chain_counters(opportunity.chain_id)` (taxonomía gate_*)
  - :699/:703 `counters().db_persisted|db_errors` → `chain_counters(opp.chain_id)`
  - import `counters` → `chain_counters` + doc-header :14-15 actualizado.
  - La elección del bucket por `opp.chain_id` (no por un campo del emitter) es deliberada:
  el emitter es único y sirve a todas las chains; la fila ya lleva su chain.

### Defecto B — contador nuevo para el intake activo (aditivo, sin re-etiquetar)
- `backend/searcher-rs/src/counters.rs` — campo nuevo `block_intents_dispatched`
  (doc: cuenta DISPATCHES, no evaluaciones; 1 ciclo que spawnea N cartridge evals = +1).
- `backend/searcher-rs/src/workers/route_scanner_worker.rs` — incremento junto a
  `dispatched += 1` (:651), cubre ambos forks (canonical + cartridge).
- `backend/searcher-rs/src/workers/heartbeat_worker.rs` — drain + campo en struct
  HeartbeatSnapshot (`#[serde(default)]` como el resto, forward-compat) + campo de log
  `block_intents_dispatched` + persist Redis (key existente, sin cambio de schema rotador:
  api-server index.ts:790-819 pasa el snapshot VERBATIM).
- `frontend/lib/operations-schemas.ts` — `block_intents_dispatched: optional` (ausencia =
  "contador no existía" en searchers pre-fix; el fixture prod_20260824 sigue válido).
- `frontend/app/operations/components/PipelineFunnelCard.tsx` — fila nueva
  "0. Block-scan intents (RU-3)" (primera del funnel) + hint de stage 1 re-escrito
  ("0 with block-scan > 0 = mempool stream quiet, NOT pipeline idle") + descripción
  del card dual-source.

**Decisión de diseño documentada**: NO re-etiqueté `pending_received`/`decoded_ok` para
incluir ciclos de bloque — eso cambiaría silenciosamente la semántica de métricas que
Grafana/paneles ya consumen (counters.rs:177-179 declara esa estabilidad de nombre como
contrato). Contador nuevo + fila nueva es aditivo, honesto y reversible. Alternativa
descartada por esa razón.

Con ambos fixes, el funnel post-deploy esperado en VPS: `[0] block_intents ≈ 2.100/min ·
[1] pending 0 · [2] decoded 0 · [3] enriched 0 · [4] passed ~0 · [5] persisted ≈ 2.500/min
· rejects ≈ 2.500/min` — coherente (intake ≥ salida) y con la taxonomía de rechazo viva
(gate_other_rejected absorberá razones como `v3_quote_unavailable` vía :463).

## 3. Verificación (RULE: verificar siempre; todo verde)

- `cargo check -p searcher-rs` → exit 0 (dev profile, 20.4s).
- `cargo test -p searcher-rs --lib counters` → 7/7 ok.
- `cargo test -p searcher-rs --lib route_scanner` → 24/24 ok.
- `cargo test -p searcher-rs --lib emitter` → 13/13 ok.
- `cargo fmt -p searcher-rs -- --check` → limpio (gate WO-06 fmt).
- `npx tsc --noEmit` (frontend) → 0 errores.
- `npx vitest run lib/__tests__/api-json-vs-zod-contract.test.ts` → 27/27 ok
  (fixture prod_20260824 sin el campo nuevo sigue pasando — optional).
- `npx vitest run app/operations/components/__tests__/route-funnel.test.ts` → 5/5 ok.
- Cero git (status verificado: sin commit/push/PR), cero deploy, cero mutación VPS
  (solo `docker logs`/`docker inspect` read-only), 0/5 requests HTTP públicos.

## 4. Conflictos con otros WO (claims de archivo)

- Archivos tocados NO reclamados por otro WO como superficie de edición. Citas read-only
  previas que apuntan a opportunity_emitter.rs:367 (WO-02d-FIX-G2, board:264-265) siguen
  byte-válidas — mis edits no tocan :333-367 (el :369 original se desplazó por el propio
  edit del emitter, no existía cita cruzada a esa línea exacta).
- `backend/searcher-rs/src/route_intent.rs` (diff huérfano PANCAKE-ROUTER-01, ERRATA-WO-02a):
  **NO tocado**.

## 5. Residuales documentados (NO corregidos aquí — gated / fuera de charter)

1. **Stream mempool aparentemente muerto en VPS** (INFERRED, no diagnosticado a fondo):
   `route_decoder.done=0` en 60m pese a Firehose alchemy configurado. `pending_received=0`
   es honesto PARA LO QUE MIDE, pero implica ceguera de detección mempool (JIT/backrun
   pre-confirmación). Clase WSSUB-05 (chain_client.rs:106-123 documenta el patrón).
   Boot-logs rotados (R9) impiden ver `scanner.mempool_mode_selected`/errores de
   subscribe del arranque 06:29Z. WO de diagnóstico aparte.
2. **block_scanner.rs (MempoolMode::Block) tiene el mismo gap de clase** (intents NewBlock
   sin contador, block_scanner.rs:433-437). NO cableado aquí: modo no activo en VPS
   (ARBX_MEMPOOL_MODE=auto) — candidato documentado, no especulativo aplicarlo.
3. **Stage 3 "Enriched" sigue midiendo solo el path legacy** (enriched_v2/v3, scanner.rs
   :1952-1958): 0 es honesto para su semántica documentada; el trabajo de quotes de la
   ruta cartridge se observa en las razones de rechazo (v3_quote_unavailable 76.8%).
   Re-etiquetarlo sería violar la estabilidad de contrato (§2 decisión de diseño).
4. **Latente pre-existente**: TS schema exige `pg_period_inserted: nonnegative()`
   (operations-schemas.ts:65) pero Rust emite -1 sentinel cuando PG falla
   (heartbeat_worker.rs:217-226) → Zod rechazaría el snapshot (fail-loud, aceptable R8,
   pero el hint del error confundiría). Documentado, no tocado.
5. El deploy del fix es operator-gated (NO-GIT vigente). Verificación post-deploy
   propuesta: `docker logs ... | grep scanner.heartbeat | tail -1` → block_intents_dispatched
   > 0 y gate_other_rejected/db_persisted ≈ pg_period_inserted.

## 6. Síntesis para la mesa

- **Confirma** BROWSE §4.1 y su hipótesis ("el contador del heartbeat no está cableado a
  la ruta activa") con la cadena file:line completa; **niega el matiz** "o muestrea otra
  etapa del decoder": no muestrea otra etapa, escribe en otro bucket (chain-0) Y la etapa
  de intake activa no tiene contador.
- **Resuelve la tensión BROWSE §5 vs WO-02a** ("hft_mempool_listener MUERTOS pero ruta
  mempool→cartuchos viva"): lo que dispara `cartridge.active_eval_enter` NO es mempool —
  es el RouteScannerWorker RU-3 per-block (625/625 intents source=new_block). El listener
  HFT sigue muerto Y el stream mempool está silencioso; la vida del pipeline viene del
  escaneo por bloque. WO-02a queda consistente.
- Para WO-06 (gates): el fix es verificable con el comando del §5.5 tras deploy.
