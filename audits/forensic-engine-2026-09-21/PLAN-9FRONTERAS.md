# PLAN-9FRONTERAS — Mapa de riesgo estructural pre-acción (WO-FE11)

> Ciclo FORENSIC-ENGINE-2026-09-21 · Entregable WO-FE11 (§13 grafo de arquitectura pre-acción).
> Regla: READ-ONLY sobre código. Cada claim cita file:line o salida de comando verificable.
> Método: grep/read directo + grafo code-review-graph (`.code-review-graph/graph.db`, 321MB,
> 21.745 nodos / 210.399 edges, build full 2026-09-19T17:45, SHA 399c18d6, rama fix/runmigrations-psqlvar-stdin).

## 0. Estado real del terreno (topología verificada)

| Elemento | Estado | Evidencia |
|---|---|---|
| Árbol main | HEAD `8a5a5e05` (fix/ws-metrics-dead-room) + WIP NO commiteado `wip(searcher-rs): BE-3.2 Binance WS bookTicker` | `git log --oneline -3` |
| Rama PriceBus | worktree `.claude/worktrees/price-canonical-cex`, rama `feat/price-pc347-wiring`, HEAD `e2b95652` (merge de origin/main), **39 ahead / 0 behind** de main → fast-forward posible | `git rev-list --left-right --count main...feat/price-pc347-wiring` → `0 39` |
| `price_bus.rs` en main | **NO EXISTE** (0 resultados search_files; grafo: 0 nodos) | grep + grafo |
| `price_bus.rs` en rama | 866 líneas, blob `32e4ebce35e4c449fcaa870faf9a3d6a46f08fde` idéntico en HEAD, `ef859a38` y `0761c944` | `git rev-parse` ×3 + `git hash-object` |
| Fuente canónica de precios en main HOY | `price_oracle.rs` (Redis hash `arbx:token_prices:<chain>`): 285 edges entrantes en el grafo (IMPORTS_FROM/CALLS/IMPLEMENTS/TESTED_BY) | SQL: `SELECT COUNT(*) FROM edges WHERE target_qualified LIKE '%price_oracle%'` → 285 |
| Overlay forense (WO-FE0) | 25 archivos create-only bajo `tools/forensic_integrity/**` + `shared-ts/src/real-card-integrity.ts`; **0 edges entrantes en grafo, 0 call-sites en backend/frontend** → production_wired=false confirmado estructuralmente | OVERLAY_MANIFEST.json + grafo (0 edges a real-card/forensic) |
| Grafo | ÚTIL pero STALE: construido sobre SHA `399c18d6` (2026-09-19), main hoy `8a5a5e05`; no contiene nodos de la rama PriceBus ni del overlay (creados después). `operator_toggles.rs` da 0 nodos pese a existir en disco (111 líneas, último commit `c139b29b` 2026-09-19) — gap de indexación, reportado fail-honest | tabla `metadata` del graph.db |
| Doble worker Binance | main tiene `backend/searcher-rs/src/workers/binance_stream_worker.rs` (WIP no commiteado, untracked); la rama tiene `binance_ws.rs` (commiteado). Dos implementaciones paralelas del mismo feed → riesgo de colisión al mergear | `git status --short` + `ls` worktree |

## 1. Mapa de riesgo de los 9 enlaces (INTEGRACION.md vs código real)

### L1 — Fuentes → PriceBus · RIESGO ALTO · WO-FE3
- Anclas: en toda la rama, el ÚNICO productor de anclas es `price_worker.rs:1188` (`bus.update_anchor(...)`); `binance_ws.rs` NO actualiza anclas (grep `update_anchor` en worktree: solo price_worker.rs:1188). INTEGRACION exige PC3/PC7 completos: pendiente.
- Profundidad: `binance_ws.rs:318` hace `bus.update_depth(&pair, d)` preservando SOLO `event_ms` (líneas 85, 91, 125: `data.get("E")`). NO se conservan `last_update_id` (Binance depth payload `u`/`U`), ni roundId/updateId de Chainlink, ni block_number → hallazgo F17 (procedencia insuficiente) vigente.
- Reconexión no rejuvenece: por verificar en tests de la rama (PC pendiente); hoy no hay test que fije event_ms tras reconnect.
- Colisión estructural: main avanza `binance_stream_worker.rs` (WIP) mientras la rama avanza `binance_ws.rs`. FE3 debe elegir/reconciliar UNA implementación.

### L2 — PriceBus → motores · RIESGO ALTO · WO-FE1 + WO-FE4
- Contrato `arbx.pricebus.export.v1`: grep `pricebus.export|arbx.pricebus` en TODO el backend de la rama → **0 ocurrencias**. El contrato NO existe: confirmado el supuesto del BOARD (WO-FE1).
- Lectores en main: `scanner.rs:2192` lee `RedisCachedPriceOracle::snapshot_from_redis` (price_oracle, NO bus). El wiring bus-como-tier-0 vive solo en la rama (diff rama: scanner.rs +140 líneas, price_worker.rs +128).
- Grafo main: 0 edges hacia `price_bus.*` — ningún consumidor. Consumidores reales actuales cuelgan todos de price_oracle (285 edges). Migrar lectores (FE4) = tocar la dependencia más citada del backend de precios: alto blast radius.
- `prices-stream.ts` (API) sirve snapshots desde el hash Redis de price_oracle (`prices-stream.ts:7,34,72`) → bajo doctrina de fuente única (FE4) este stream es un consumidor a migrar o exentar explícitamente.

### L3 — Cotización → economía · RIESGO ALTO · WO-FE5
- V2/V3: `size_optimizer.rs:32` usa `v2_amount_out` / `v3_amount_out_single_tick`; etiquetas fail-honest ya existen (`size_optimizer.rs:133` `v3_quote_unavailable`).
- StableSwap: `cartridge_boot.rs:534-535` mapea `v2|constantproduct|constant_product` → `constant_product`; `detector_policy.rs:307` declara criterio StableSwap (Newton, invariante D/A) SOLO como texto de política. Hallazgo F06 confirmado: cotización CPMM donde se declara StableSwap.
- CEX: no existe adaptador de profundidad CEX en size_optimizer (grep depth/quote: 0 rutas CEX); `vwap_usd` del bus (rama) es el único primitivo VWAP y hoy devuelve QUOTE/base sin reconversión (ver §4).
- Re-quote tras cambio de size: `size_optimizer.rs:755` documenta la condición Sancho (amount_out final consumido por el profit) — parcial; no hay re-quote genérico por salto tras resize.

### L4 — Matemática → decisión · RIESGO MEDIO-ALTO · WO-FE6
- `math_evidence.rs:115` `declared_combo_snapshot` produce un OBJETO por estrategia (`"source": "declared_combo"`, línea 142).
- `priors_cache.rs:187` `section_iv_fold` exige `serde_json::Value::Array` o devuelve None → **F09 confirmado en código**: el fold no interpreta el contrato objeto del snapshot. Matices: los ÚNICOS call-sites de `section_iv_fold` hoy son tests (priors_cache.rs:218-248) — el desajuste está latente, no rompiendo producción ahora.
- `math_evidence.rs:443` test legitima `all zeros` para estado degenerado → F10 (ausencia vs cero) requiere estados diferenciados en el contrato nuevo.
- Censo estático (del paquete): 182 divergencias JSON↔Rhai (F04), 183 estrategias con inputs sin cobertura (F05), IDs de operador del mapa ≠ registro real de 32 (F03; registro: `math_evidence.rs:19` `OperatorRegistry`, dispatch por `id as u8` línea 104).

### L5 — Productores → packet · RIESGO MEDIO · WO-FE7
- `runRealCardPipeline` existe y está verificado (WO-FE0: 22/22 tests nativos): `shared-ts/src/real-card-integrity.ts:127` (pipeline), `:229` (`presentRealField` → `{text,state,reason}`), `:103` (`fieldFailure`).
- Grafo: 0 edges entrantes a real-card-integrity en main → NADIE lo llama. El wiring (productores reales del host por strategy_kind — 269 kinds según `reports/requirements_269.json`) es exactly lo pendiente. Punto de ensamblaje natural: `opportunity_emitter.rs` (docstring línea 8: "persistence::insert_opportunity and publisher::publish in future phases").

### L6 — PG/Redis (outbox transaccional) · RIESGO ALTO · WO-FE8
- Orden actual real: dedup → Gate-C scoring → PG insert (`opportunity_emitter.rs:420` `try_insert_pg_with_route`) → XADD no-fatal (`opportunity_emitter.rs:647,660`; "non-fatal" explícito en 409/647).
- **F11 confirmado**: si el INSERT de PG falla, el XADD igual puede publicarse (XADD no-fatal tras PG-fail) → evento en Redis sin fila en PG.
- `persistence.rs`: 0 menciones de outbox; dedup por `ON CONFLICT (id) DO NOTHING` (persistence.rs:196).
- `publisher.rs:156-157`: XADD a `arbx:opps:detected` con MAXLEN ~10000 via comando crudo; sin ACK ni recibos por evento → F12 (XLEN/counters no prueban entrega).

### L7 — API → wire · RIESGO MEDIO · WO-FE9(a)
- `opportunities-live.ts:548-559` construye `simulated_*` desde filas de sim (forward/inverse); `api-contracts.ts:250-256` esquemas Zod `simulated_*` — NO existe campo versionado para packet/estado-por-campo. Regla "no sobrescribir simulated_*" hoy es trivialmente cierta (no hay campo real rival) pero sin schema que lo garantice al añadir el nuevo.
- Fixture de cierre (mismo ID conserva valor/unidad/estado): no existe test de ese tipo en `opportunities-live.test.ts` hoy.

### L8 — wire → store → card · RIESGO MEDIO · WO-FE9(b)
- `omni-store.ts` (520 líneas): merge-upsert por `id` (líneas 365-391); grep `revision|last_seen|confirmations` → 0 hits. NO compara revisiones: re-detección puede mezclar contextos (riesgo INTEGRACION fila wire→store).
- `types.ts:312-318`: solo bloque `simulated_*`; sin packet. types.ts es el nodo de mayor riesgo del frontend en el grafo (risk 0.85, 80 callers) — cambiarlo toca todo el DAG de tipos.
- `OpportunityTradeCard.tsx:154-183`: render directo de `simulated_*`; grep `presentRealField|data-real-field|data-field-state|data-field-reason` → 0. La card y el omni-store figuran `untested` en el risk_index del grafo.

### L9 — operador → backend → UI (toggles) · RIESGO MEDIO · WO-FE10
- `operator_toggles.rs` (111 líneas): `is_disabled(id)` + poll loop Redis (líneas 39, 63); grep `request_id|applied_revision|revision` → 0.
- `routes/operator.ts:77` (`POST /preferences`, rol steward) y `:154` (`POST /feature-overrides`, rol sovereign): grep request_id/revision → 0. F13 confirmado: sin闭环 requested→persisted→applied→reflected.

## 2. Orden de dependencias WO-FE1..FE10

```
FASE 0 (rama, serializadas entre sí por archivo):
  WO-FE2 (parche vwap) ──► WO-FE1 (export contract)   [ambos tocan price_bus.rs: SERIALIZAR]
  WO-FE3 (sources→bus IDs)                            [toca binance_ws.rs/price_worker.rs de la rama;
                                                       borde con FE1 en tipos source_references]
  ── MERGE rama → main (fast-forward posible: 0 behind) ──
FASE 1 (main):
  WO-FE4 (bus→engines, lectura única)   [desbloquea FE5; requiere merge]
  WO-FE10 (toggles)                     [INDEPENDIENTE — puede correr en paralelo desde ya]
FASE 2:
  WO-FE5 (per-hop quote)  [requiere FE4: precios del bus; desbloquea FE6 con inputs reales]
  WO-FE6 (evidence+mask)  [contrato objeto→fold independiente; valor completo tras FE5]
FASE 3:
  WO-FE7 (productores→packet)  [requiere FE5/FE6 para productores con datos reales]
  WO-FE8 (outbox PG)            [SERIALIZAR tras FE7: ambos tocan opportunity_emitter.rs]
FASE 4:
  WO-FE9 (wire+store+card)     [requiere FE7 (packet existe) y FE8 (revision)]
```

Cadenas duras (no paralelizables): FE2→FE1→merge→FE4→FE5→FE7→FE8→FE9.
Paralelismo seguro máximo: {FE3} ∥ {FE2→FE1} (fase 0, archivos disjuntos salvo borde de tipos); {FE10} en cualquier momento; {FE6-contracto} ∥ {FE5}.

Riesgo de merge a gestionar ANTES de FASE 1: (a) WIP no commiteado en main (binance_stream_worker.rs) vs binance_ws.rs de la rama — reconciliar; (b) grafo stale — regenerar `update` tras merge.

## 3. File-claims propuestos por WO (adjudicación sin colisiones)

| WO | Archivos a tocar | Notas de colisión |
|---|---|---|
| FE2 | `[WT] backend/shared-rs/src/price_bus.rs` | EXCLUYENTE con FE1: FE2 PRIMERO (blob-check ya verde) |
| FE1 | `[WT] backend/shared-rs/src/price_bus.rs`, `[WT] backend/shared-rs/src/lib.rs` (+ tests export) | Tras FE2. Borde de tipos con FE3 (source_references) |
| FE3 | `[WT] backend/searcher-rs/src/workers/binance_ws.rs`, `[WT] backend/searcher-rs/src/workers/price_worker.rs` | No tocar price_bus.rs (es de FE1); decidir binance_ws vs binance_stream_worker (main) |
| FE4 | `[main] backend/searcher-rs/src/scanner.rs`, `[main] backend/searcher-rs/src/workers/price_worker.rs`, `[main] backend/api-server/src/prices-stream.ts` (decisión: migrar o exentar) | Tras merge; scanner.rs es zona compartida — LOCK amplio |
| FE5 | `[main] backend/searcher-rs/src/size_optimizer.rs`, `cartridge_boot.rs`, `state_projector.rs` (+ adaptador CEX depth nuevo) | Disjunto de FE4 salvo state_projector (coordinar) |
| FE6 | `[main] backend/searcher-rs/src/math_evidence.rs`, `priors_cache.rs` | Disjunto |
| FE7 | `[main] backend/searcher-rs/src/opportunity_emitter.rs`, host-adapters nuevos (import de `shared-ts/src/real-card-integrity.ts`) | real-card-integrity.ts es create-only del paquete: preferir adapter nuevo, no editar |
| FE8 | `[main] backend/searcher-rs/src/persistence.rs`, `opportunity_emitter.rs`, `publisher.rs` + migración SQL | SERIALIZAR tras FE7 (emitter compartido) |
| FE9 | `[main] shared-ts/src/api-contracts.ts`, `backend/api-server/src/routes/opportunities-live.ts`, `frontend/lib/store/types.ts`, `frontend/lib/store/omni-store.ts`, `frontend/components/OpportunityTradeCard.tsx` (+ tests) | TS paralelo OK; types.ts risk 0.85/80 callers — LOCK estrecho |
| FE10 | `[main] backend/searcher-rs/src/operator_toggles.rs`, `backend/api-server/src/routes/operator.ts` | Disjunto de todo — puede claim inmediato |

## 4. Veredicto de factibilidad WO-FE2 (parche vwap_usd) — **APLICABLE**

Dry-run ejecutado (check-only, sin --apply), 2026-09-21:

```
$ python %TEMP%\arbx_forensic_engine\ARBITRAGEX_FORENSIC_UPDATE\patches\fix_pricebus_vwap.py \
    --repo "<ws>/.claude/worktrees/price-canonical-cex"
Verified exact PriceBus blob: 32e4ebce35e4c449fcaa870faf9a3d6a46f08fde
Plan: VWAP quote→USD conversion + one regression test. Rust compilation still required.
EXIT=0
```

- Blob git actual del archivo en HEAD de la rama = `32e4ebce35e4c449fcaa870faf9a3d6a46f08fde` — **MATCH exacto** con `EXPECTED_BLOB` del parche (fix_pricebus_vwap.py:13). Igual en `0761c944` y `ef859a38`.
- Bug verificado en fuente: `price_bus.rs:486-487` — `let size_quote = size_usd / quote_usd; vwap_for_quote(depth, side, size_quote)` devuelve QUOTE/base sin `× quote_usd`. El parche añade la reconversión + test `vwap_usd_applies_the_quote_currency_anchor` (ancla $0.80, VWAP esperado $8/base).
- El parche es idempotente-seguro (aborta si anchors no únicos o blob drift) y crea backup `.before-PC9`. NO aplicado (READ-ONLY este run).
- Gate post-apply (obligatorio): `cargo fmt --check && cargo test -p shared-rs vwap_usd_applies_the_quote_currency_anchor` + clippy. Nota: target compartido → Rust serial en árbol principal (convención BOARD).

## 5. Cruzamiento con AUDITORIA_ESTRICTA.md (22 hallazgos)

Dentro de WO-FE1..FE10:

| Hallazgo | Título corto | WO destino |
|---|---|---|
| F15 (PARCHE_PREPARADO) | vwap_usd QUOTE/base | **FE2** (dry-run match, §4) |
| F14 | PriceBus no cableado en fuentes | FE3+FE4 |
| F17 | Procedencia roundId/updateId/bloque | FE3 (+FE1: source_references/input_hashes) |
| F16 | Frescura depth + latch divergencia | FE3/FE1 (bus interno + anclas) |
| F06 | StableSwap usa CPMM | FE5 |
| F07 | MarketState pierde unidades/coverage | FE5/FE6 |
| F08 | Evidencia no ligada a oportunidad | FE6 |
| F09 | Objeto→array incompatible en fold | FE6 (priors_cache.rs:187 vs math_evidence.rs:115) |
| F10 | Ausencia confundida con cero | FE6 |
| F03/F04/F05 | IDs operadores / 182 divergencias / 183 sin cobertura | FE6 (parcial: censo y corrección de cartuchos Rhai excede el wiring) |
| F11 | Redis sin INSERT PG | FE8 (emitter: XADD non-fatal, línea 647) |
| F12 | Contadores/XLEN ≠ entrega | FE8 (recibos por evento) |
| F13 | Toggles sin confirmación de aplicación | FE10 |
| F21 | No forzar 3-legs en incompatibles | FE5/FE6 (máscara aplicabilidad) |
| F19 (REGLA_IMPLEMENTADA) | Defaults del mapa | cerrado por el paquete (solo referencia) |

**Fuera de alcance de WO-FE1..FE10** (requieren acto del orquestador, no wiring):
- F01 — "el mapa no es certificación de producción": dictamen de governance, no un WO de código.
- F02 — 269 identidades ≠ 269 motores: dimensión de universo/producto.
- F18 — SLAs del mapa se contradicen: negociación de contrato con el proveedor.
- F20 — dominio vivo no certificado en esta ejecución: campaña de producción post-wiring.
- F22 — completitud de campos no garantiza ganancia: doctrina económica permanente.
- (F03/F04/F05 parcial: el censo léxico Rhai/JSON y la reparación de 183 constructores excede FE6-wiring.)

## 6. Riesgos estructurales transversales

1. **Dualismo de fuentes de precio**: price_oracle (main, 285 edges) vs price_bus (rama, 0 edges). Mientras FE4 no cierre, conviven dos verdades. Toda herramienta que hoy lee `arbx:token_prices:*` sigue en el mundo viejo.
2. **Merge con WIP sucio**: main tiene trabajo no commiteado (BE-3.2 binance_stream_worker.rs) que colisiona conceptualmente con binance_ws.rs de la rama. Resolver ANTES del merge de FASE 1.
3. **Grafo stale**: built SHA 399c18d6 (2026-09-19) vs HEAD 8a5a5e05; sin nodos de rama ni overlay; operator_toggles.rs sin indexar. Regenerar (`update`, no rebuild full) tras el merge y antes de FE4.
4. **Blast radius de types.ts** (risk 0.85, 80 callers) y de price_oracle→scanner: los dos cambios de mayor fan-out del plan están en FE9 y FE4 — asignar a workers senior con LOCK de archivo amplio.
5. **XADD no-fatal es comportamiento deseado hoy** (emitter lo documenta como decisión): FE8 debe invertir la semántca con outbox transaccional sin romper los watchdogs que ya observan ese orden (persistence.rs:193 comment H2 FREEZE-01).

— Generado por run Hermes WO-FE11. Evidencia: comandos citados inline; grafo consultado read-only (mode=ro).
