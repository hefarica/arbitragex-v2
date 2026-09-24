# BOARD — Motor Forense End-to-End (FORENSIC-ENGINE-2026-09-21)

> /goal (operador, 2026-09-21): "INTEGRA ESTE MOTOR TAL CUAL Y ADAPTATE PARA QUE
> NUESTRA HERRAMIENTA LE SAQUE EL 100% DE ESTE MOTOR DESARROLLADO" — motor de
> arbitraje end-to-end (ARBITRAGEX_FORENSIC_UPDATE).
>
> Reglas del BOARD (§9.1.1 gang omniscience): todo agente lo lee ANTES de trabajar,
> CLAIM-ea su WO con dueño, y lo actualiza al terminar. Nadie reporta solo al
> orquestador. CLAIM con LOCK (lección 2026-09-20: scoping por archivo NO basta).
>
> Doctrinas activas: RULE 00 (cero mocks) · R8/R10 (fail-honest) · §32/§33
> (read-only, sin broadcast/capital) · §34 mode-invariant · no-git-until-final-gate.
> Paquete: create-only, 25 archivos, baseline 538cd191, hashes verificados.
> Auditoría estricta: PRODUCTION NOT CERTIFIED (22 hallazgos: 20 abiertos,
> 1 parche preparado, 1 regla implementada).

## Estado de verificación local (evidencia reproducible)

| Suite | Resultado | Artefacto |
|---|---|---|
| Python `run_tests.py` | **139/139 PASS** | (fix WinError 32 en test_outbox.py: contextlib.closing — solo test-infra) |
| Node `native_cards_runtime.cjs` vs CJS compilado | **22/22 PASS** | `%TEMP%\fisrc_cjs\real-card-integrity.js` |
| Node `adapter_runtime.cjs` vs CJS compilado | **22/22 PASS** | `%TEMP%\fisrc_cjs_adapter\forensic.js` |
| Overlay integrate.py --apply | 25/25 creados, 0 config changes | production_wired=false (correcto) |

## Work Orders

### WO-FE0 — Instalación + verificación del paquete ✅ DONE (orquestador, 2026-09-21)
- Aplicado TAL CUAL (dry-run → apply). Hashes verificados contra OVERLAY_MANIFEST.
- 3 suites verdes localmente (arriba). Única modificación: test_outbox.py handle
  Windows (contextlib.closing) — lógica de motor intacta.
- Gate: DONE.

### WO-FE1 — PriceBus export contract `arbx.pricebus.export.v1` 🟡 LOCAL GATES DONE by orquestador (2026-09-21 11:52Z; validación Hermes en cola — slot ocupado por WO-LR0.1 re-despachado)
- Contrato NUEVO que NO existe en la rama PriceBus inspeccionada (0761c944).
- Construir el exportador en price_bus (Rust): snapshot_hash (ARBX-CJSON-1),
  policy_hash, prices[chain:address] con asset_key, price_usd (string decimal),
  verdict, observed_at_ms, valid_until_ms, source_references, input_hashes.
- Stale_anchor/frozen NUNCA promovidos a verified.
- Gate: test Rust con vector externo (§12.4 — jamás test que re-compute su fórmula)
  + validación Hermes durable run.
- Archivos: rama worktree-price-canonical-cex (3 ahead/2 behind main).
- **Evidencia gates locales (worktree price-canonical-cex)**:
  - `price_bus.rs`: refactor `resolve_fused` (fuente única de fusión: hot path y
    export consumen la MISMA resolución; now_ns inyectado = determinista) +
    `export_policy_hash` + `export_v1(now_ms, asset_keys)` + `export_record`.
    Verdicts via `Verdict::as_str()`: ok/stale_binance aceptados por el consumer
    estricto; stale_anchor queda SIN verificar; frozen/no_live_price explícitos
    (price "0", valid_until 0). `sha2.workspace = true` en Cargo.toml.
  - Vector EXTERNO §12.4: `tests/gen_export_vector.py` (referencia independiente
    Python `tools/forensic_integrity/integrity.py digest`) genera
    `tests/fixtures/pricebus_export_v1_vector.json`
    (snapshot_hash=7fab07b0b2fe0aa3510aceaca69e1b01d1f4fefc6f504f9a3996c4658bd2be7f);
    `tests/pricebus_export_v1_vector.rs` reconstruye el bus desde el fixture y
    compara la salida del exporter CONTRA el fixture (jamás re-computa hashes).
    **PASS 1/1** — bytes Rust == bytes Python en el primer intento.
  - `cargo test -p shared-rs`: **274 passed; 0 failed** (unit price_bus 18/18:
    hash+asset_keys+provenance, tamper detection, stale_anchor nunca verificado,
    frozen/no_source explícitos, + suite completa).
  - `cargo fmt -p shared-rs` aplicado · `cargo clippy -p shared-rs --all-targets`
    → **0 warnings**.
  - PENDIENTE: validación Hermes durable run (despachar cuando libere el slot).

### WO-FE2 — Parche vwap_usd (UNIDAD: QUOTE/base sin reconversión USD) ✅ DONE (orquestador, 2026-09-21)
- Dry-run MATCH (blob 32e4ebce… verificado 2×) → `--apply` EXIT=0. Backup:
  `price_bus.rs.before-PC9` en worktree price-canonical-cex (rama feat/price-pc347-wiring, HEAD e2b95652).
- Gates post-apply: `cargo fmt -p shared-rs` (test nuevo del parche necesitaba formato) ·
  `cargo test -p shared-rs vwap_usd_applies_the_quote_currency_anchor` → **1 passed; 0 failed** (30.5s compile, target frío) ·
  `cargo clippy -p shared-rs --all-targets` → **0 warnings**.
- Diff: `backend/shared-rs/src/price_bus.rs | 30 insertions(+), 1 deletion(-)` (solo en el worktree, sin commit — gate final del operador).
- Nota: el script del parche truena al final en consola cp1252 (print `→`) — cosmético;
  fijar `PYTHONIOENCODING=utf-8`. Blob-check corre ANTES del print, así que el apply es válido.
- Gate: DONE. Siguiente en cadena: WO-FE1 (mismo archivo — serializado tras este cierre).

### WO-FE3 — Frontera Sources→PriceBus (IDs/bloques/roundId/updateId) ✅ DONE (orquestador, 2026-09-21 12:30Z)
- Preservar IDs de fuente, block_number, roundId/updateId en la torre de precios.
- PC3/PC7 del paquete ya completos como referencia de patrón.
- **Implementado (worktree price-canonical-cex, sin commit — gate operador)**:
  - `price_bus.rs`: structs extendidos `Anchor{round_id: u64}` (Chainlink roundId),
    `BookTicker{update_id: u64}` (Binance `u`), `Depth5{last_update_id: u64}`
    (Binance `lastUpdateId`). Convención 0 = no disponible (mismo patrón que
    event_ms:0). `anchor_input_hash` += `"round_id": str` (uint80 > 2^53 → string
    decimal, ARBX-CJSON-1); hash ticker += `"update_id": str`.
  - `price_worker.rs`: `eth_call_latest_answer` ahora decodifica word[0] (roundId,
    hex chars 32..64) → `(round_id, raw, updated_at)`; Anchor productor en :1198
    recibe round_id real. **block_number NO disponible vía eth_call** —
    fail-honest documentado en el doc-comment: procedencia anchor = roundId +
    timestamps, no altura de bloque.
  - `binance_ws.rs`: parse_book_ticker lee `u`; parse_depth5 lee `lastUpdateId`;
    aserciones nuevas en tests (update_id=400900217, last_update_id=42 con payloads
    reales de Binance que ya los traían).
  - Vector §12.4 regenerado: `gen_export_vector.py` extendido (round_ids uint80
    >2^53 ejercitando la regla string) → snapshot_hash nuevo
    `f2c281733bdb3a2ee8d6317854bc160c166210e8b72bc85002bccccd45e1853a`;
    test vector actualizado (puebla round_id/update_id desde fixture INPUTS).
- **Gates (evidencia reproducible)**:
  - `cargo test -p shared-rs`: **274 passed; 0 failed** (259 unit + 13 oracle +
    1 vector + 1 doc). Vector EXTERNO §12.4 PASS 1/1 — bytes Rust == bytes Python
    AL PRIMER INTENTO con los IDs nuevos.
  - `cargo test -p searcher-rs` (suite completa, todos los binarios):
    **2799 passed; 0 failed; 5+ ignored** (incluye binance_ws 9/9 con las
    aserciones de procedencia nuevas).
  - `cargo fmt -p shared-rs` aplicado · `cargo clippy -p shared-rs -p searcher-rs
    --all-targets` → **0 warnings**.
  - Incidente durante gates: disco C: a 0.00GB (os error 112) — purgado SOLO
    caché incremental regenerable de ambos target/ (22.4GB) → 22.1GB libres;
    clippy re-PASS. Remedio = memoria local-target-disk-full-link-failures.
- **Reconciliación merge-time (decisión documentada, NO ejecutada)**: el worktree
  tiene `binance_ws.rs` (bus-wired, feed directo al PriceBus); main tiene
  `binance_stream_worker.rs` (escribe tier price_oracle Redis). Recomendación:
  al mergear la rama PriceBus, mantener AMBAS fronteras separadas —
  binance_stream_worker alimenta el snapshot Redis (api-server/PricesSnapshot),
  binance_ws alimenta el PriceBus canónico (fusión/export forense); NO duplicar
  parsers: portar el parseo de IDs FE3 al otro worker si aún no los captura.
  Decisión final en el WO de merge (§6.2 PLAN-9FRONTERAS).
- Gate: DONE.

### WO-FE4 — Frontera PriceBus→engines (lectura atómica única) 🔴 OPEN
- Consumidores leen SOLO del PriceBus canónico; sin fuente alterna en consumers.

### WO-FE5 — Frontera Cotización→economía (per-hop real por protocolo) 🔴 OPEN
- Cantidades per-hop via adaptador de protocolo (V2/V3/StableSwap/CEX depth).
- Re-quote tras cambio de size; V3 = ticks/liquidity/fee en bloque fijo;
  StableSwap ≠ x*y=k; profundidad CEX real (no bid).
- quote_router.py + math_reference.py del paquete = referencia.

### WO-FE6 — Frontera Matemática→decisión (evidence + applicability mask) 🔴 OPEN
- Evidence ligada a evento, IDs de operador, máscara de aplicabilidad;
  sin zero-arrays. 182 diferencias JSON vs Rhai + 183 estrategias con inputs
  sin cobertura (auditoría) = mapa de trabajo.

### WO-FE7 — Frontera Productores→packet (todos los aplicables corren) 🔴 OPEN
- runRealCardPipeline ya lo garantiza (verificado WO-FE0) — falta el WIRING:
  productores reales del host registrados en el manifest por strategy_kind.

### WO-FE8 — Frontera PG/Redis (outbox transaccional) 🔴 OPEN
- opportunity + outbox event en LA MISMA transacción PostgreSQL; dedup/ACK;
  retries acotados. outbox.py (SQLite) = referencia de semántica, NO producción.

### WO-FE9 — Frontera API→wire→store→card (presentRealField + atributos) 🔴 OPEN
- Campo versionado nuevo en wire + Zod; NUNCA sobrescribir simulated_*;
  store preserva packet completo + revision compare;
  cards renderizan via presentRealField con data-real-field/data-field-state/
  data-field-reason. probe_browser.mjs = patrón de verificación pasiva.

### WO-FE10 — Frontera operator toggles (request_id + applied_revision) 🟡 CLAIMED by orquestador (2026-09-21 12:57Z)
- Toda acción de toggle responde con request_id + applied_revision (loop
  requested→persisted→applied→reflected; F13 de AUDITORIA_ESTRICTA).
- Implementación en worktree price-canonical-cex (archivos disjuntos de FE1-FE3:
  operator_toggles.rs + routes/operator.ts). Rust gates diferidos hasta que la
  validación WO-FE1 libere el target/ del worktree.

### WO-FE11 — Grafo de arquitectura pre-acción (§13) ✅ DONE (Hermes run_cabae51f59e7446f8f69ccdb2697510f, 2026-09-21)
- Entregable: `audits/forensic-engine-2026-09-21/PLAN-9FRONTERAS.md` (17.4KB, escrito).
- Grafo consultado read-only (graph.db 321MB, 21.745 nodos): price_bus=0 nodos/0 edges
  en main, price_oracle=285 edges, overlay forense=0 edges (production_wired=false
  confirmado estructuralmente). Grafo STALE (SHA 399c18d6 vs HEAD 8a5a5e05) — regenerar
  tras merge. Mapeo manual grep/read como respaldo (fail-honest).
- Veredicto FE2: dry-run del parche vwap MATCH (blob 32e4ebce… = git hash-object del
  price_bus.rs del worktree price-canonical-cex, EXIT=0). NO aplicado (READ-ONLY).
- Orden FE2→FE1→merge→FE4→FE5→FE7→FE8→FE9; FE10 y FE6-contracto paralelizables.
  File-claims por WO en PLAN-9FRONTERAS.md §3. Cruzamiento F01-F22 en §5
  (fuera de alcance: F01, F02, F18, F20, F22 + censo Rhai de F03-F05).
- Gate: DONE (análisis; sin edits de código, sin git, sin deploy).

## Paralelo (no bloquea este BOARD)
- Stream A (Binance WS F1-F5, plan quiet-launching-thacker):
  **F1 ✅** (worker + ChangeDetector + price_oracle merge + métricas; cargo test verde
  1343+1332+244 passed / 0 failed, clippy 0 warnings).
  **F2 ✅** (2026-09-21): PricesSnapshot + REST `/api/v1/prices/live` extendidos con
  `cex: {BASE → {price, ts_ms, quote, source}}` desde `arbx:cex_prices:<chain>`;
  espejo `usePricesStream.ts` (PricesEvent.cex opcional + PricesState.cex, R10).
  Verificación: api-server tsc 0 + vitest 6/6; frontend tsc 0 + vitest 11/11.
  F3 (cards join CEX) pendiente.

## Convenciones
- Claim: editar este archivo, `Estado: 🔴 OPEN → 🟡 CLAIMED by <agente>`.
- Cierre: evidencia reproducible (salida de test, hash, path) — aserciones ≠ facts.
- Rust total serial en árbol principal (target compartido); TS paralelo OK.
- Respawns: presupuesto 8, oleadas ≤4, tripwire muertes≥6 y >2× éxitos ⇒ suspender.
