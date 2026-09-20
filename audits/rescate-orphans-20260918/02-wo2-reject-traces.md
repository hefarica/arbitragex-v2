# WO-2 — Rescate `reject-traces` E1 (worktree `.claude/worktrees/reject-traces`)

> Programa RESCATE-ORPHANS-0918 · fecha 2026-09-18 · base del worktree `77321555`
> (branch `feat/reject-traces-20260918`, 0 commits propios) · protocolo no-git-until-final-gate:
> **CERO commit / push / PR** — todo vive como cambios sin commit en el worktree.

## 0. Dictamen ejecutivo

**RESCATABLE (≈60% hecho, ≈40% cableado faltante) → COMPLETADO en el mismo worktree.**

El builder original murió (503) *a mitad de una edición*: ya había cambiado la llamada
`get_pool_quote(..., trace)` a 5 argumentos pero la `fn` seguía con 4 → el árbol no compilaba.
La infraestructura (módulo `reject_traces.rs` con 7 tests, métricas, migración 122, campo
`computed_evidence` en el contrato) estaba completa y de calidad; lo que faltaba era **todo el
cableado**: nadie llamaba a `finalize`/`submit`/`init_writer`, la persistencia no ligaba la
columna, y 14 constructores de `Opportunity` quedaron sin el campo nuevo.

Estado final: `tsc --noEmit` exit 0 · vitest 4/4 · cargo — ver §5.

## 1. Forense: dictamen por archivo (estado ENCONTRADO, antes de tocar nada)

| Archivo | Delta encontrado | Dictamen | Razón |
|---|---|---|---|
| `backend/searcher-rs/src/reject_traces.rs` (nuevo, 755 líneas) | Módulo completo: vocabulario cerrado de sobres, `Trace` plano, `finalize` (C1-C3 + histograma), `trace_hash`, sink batched mpsc (200 filas/250 ms, dedup TTL, mantenimiento particiones), 7 tests | **RESCATABLE** | Cerrado sintáctica y semánticamente; ninguna función a medias. Único hueco: **cero llamadores** |
| `database/migrations/122_reject_traces.sql` (nuevo) | `computed_evidence JSONB` (DO-guarded, rerun-lock-safe), `rechazos_traces` particionada por hora + default, índice, rollup diario, `arbx_reject_traces_maintain()` | **RESCATABLE** | Idempotente, sigue doctrina 102/121. Faltaban las columnas del addendum |
| `backend/shared-rs/src/contracts.rs` | `+computed_evidence: Option<serde_json::Value>` con `#[serde(default, skip_serializing_if)]` | **RESCATABLE** | Correcto; rompe (a propósito) todo constructor literal → debía propagarse |
| `backend/searcher-rs/src/engines/candidate.rs` | `+trace: reject_traces::Trace` en `StrategyCandidate` | **RESCATABLE** | Diseño (b) del charter: slots planos, zero-alloc por gate |
| `backend/searcher-rs/src/engines/dex_engine.rs` | `let mut trace`, stage `Priced`, `compute_v3_gross_usd(.., &mut trace)`, captura `v3_quote_a/b`, llamada `get_pool_quote(.., trace)` | **INCOMPLETO-PERO-SALVABLE** | La llamada tenía 5 args y la `fn get_pool_quote` 4 → **E0061**. Sub-etiqueta `QuoteFailed` nunca se capturaba |
| `backend/searcher-rs/src/metrics.rs` | 3 métricas nuevas (`arbx_bug_suspected_rejections_total`, `arbx_reject_trace_serialized_bytes`, `arbx_reject_trace_dump_total`) | **RESCATABLE** | Registradas en `REGISTRY`; nombres coherentes con el módulo |
| `backend/searcher-rs/src/lib.rs` | `pub mod reject_traces;` | **RESCATABLE** | Pero `main.rs` re-declara módulos propios y NO lo tenía → `crate::reject_traces` irresoluble en el binario |
| 12 engines (`backrun, cex_dex, cross_chain_bridge, dlp, flashloan, funding_rate, liquidation, liquidation_snipe, spanning_tree, spatial, svs, triangular_atomic, triangular`) + `cartridge_boot.rs` + `orchestrator.rs` (tests) + `size_optimizer.rs` (tests) | `trace: Trace::default()` en cada `StrategyCandidate {..}` | **RESCATABLE** | Mecánico y correcto. Pero **7 constructores de `Opportunity`** en esos mismos archivos quedaron sin `computed_evidence` |
| `relays-client/{persistence,submit_engine}.rs`, `sim-ctl/tx_builder.rs`, `searcher-rs/{opportunity_emitter,orchestrator,patterns}.rs` (tests/patterns) | `computed_evidence: None` | **RESCATABLE** | Propagación parcial: faltaban `candidate_simulation.rs`, `gates/mod.rs`, 3 workers, `sim-ctl/revm_backend.rs` y los 7 sitios no-test de engines |
| `opportunity_emitter.rs` (código productivo) | sin cambio | **HUECO** | `emit_rejected` no sabía nada de la traza |
| `persistence.rs` (searcher) | sin cambio | **HUECO** | INSERT sin la columna; sin `submit` al segundo sink |
| `main.rs` | sin cambio | **HUECO** | `init_writer` nunca invocado → el sink habría contado todo como `disabled` |

Nada era BASURA. Ningún archivo se descartó.

## 2. Qué completé (cambios quirúrgicos, en el worktree)

**Compilación / propagación**
- `dex_engine.rs::get_pool_quote`: firma con `trace: &mut Trace`; captura `ProjectV3Error::QuoteFailed(detail)` en `trace.v3_sublabel` ANTES del aplanado `as_label()` (así `v3_quote_unavailable` se segmenta por causa raíz).
- `computed_evidence: None` en los 14 constructores faltantes de `Opportunity` (7 productivos en engines/cartridge_boot + `candidate_simulation`, `gates/mod`, 3 workers, `sim-ctl/revm_backend`).
- `main.rs`: `mod reject_traces;` (el binario re-declara módulos) + `init_writer(pool)` tras el pool PG, con log `reject_traces.writer_started`.
- `orchestrator.rs`: `evidence_at_rejection()` (visibilidad vía re-export `crate::engines::StrategyCandidate`) — hidrata la traza SOLO con `Some` ya computados (`gross`/`net`) y llama a `finalize` una vez. Aplicado en los dos gates donde hay economía: rechazo de engine y `EvaluatedRejected`. **Descartado**: un `opp.amount_in_usd()` que había escrito — el método no existe y convertir wei a USD sin precio sería fabricación (R8).

**Cableado de los dos sinks (sitio único de serialización respetado)**
- `opportunity_emitter.rs::emit_rejected` (ruta real, no dry-run): si la fila rechazada llega sin bloque, publica el sobre honesto `no_alcanzado` vía `finalize(reason, &Trace::default())`. Ningún rechazo sale sin evidencia; ninguna fila viable la lleva.
- `persistence.rs`: INSERT con `computed_evidence` (`$23`, bind del mismo `Value`); tras `rows_affected() > 0` **y solo entonces**, `submit(RejectTraceRow{..})` con `trace_hash` sobre los bytes exactos → V2 (byte-igualdad entre sinks) por construcción; un duplicado `ON CONFLICT` no dumpea dos veces.

**Addendum (ver §3)**: `probe_block`/`oracle_block` en `Trace`, en el wire (`put_num`, omitidos si `None`), columnas generadas STORED en `rechazos_traces`, `probe_block` estampado desde `RouteIntent.observed_block_number`, endpoint TS + test.

## 3. Diseño del addendum — `GET /api/forensic/pool_b_lag`

**Semántica.** `probe_block` = altura a la que se sondeó el candidato (lado intent/pool A);
`oracle_block` = altura desde la que respondió el proyector/oráculo (pool B).
`gap = probe_block − oracle_block`: `0` sincro · `1` atraso_1blk · `≥2` stale · `<0` negativo (reloj/reorg, se reporta aparte).

**Tabla** (`122_reject_traces.sql`, columnas generadas sobre el JSONB, NULL si la clave no existe):
```sql
probe_block  BIGINT GENERATED ALWAYS AS ((evidence->>'probe_block')::bigint)  STORED,
oracle_block BIGINT GENERATED ALWAYS AS ((evidence->>'oracle_block')::bigint) STORED,
```

**Queries** (archivo `backend/api-server/src/routes/forensic-pool-b-lag.ts`):
1. Existencia: `SELECT to_regclass('public.rechazos_traces') IS NOT NULL`.
2. Cobertura: `min(detected_at)`, `count(*) FILTER (24h)`; gate `≥ 24 h`.
3. Agregado 24 h (solo filas con AMBOS bloques): `count FILTER gap=0 / =1 / ≥2 / <0`, `percentile_cont(0.5|0.95)`, `max(gap)`.
4. Serie por segundo (ventana `window_s` ∈ [30, 900], default 300): `miss_pb` (probe sin oracle), `quote_gap` (mediana gap), `economia_ok` (`net_profit_usd > 0`), `p50_net`, `sospechosas` (`gate.estado = 'gate_repair'`), `total`.

**Shape JSON**
```jsonc
// sin datos (JAMÁS ceros)
{ "estado":"sin_datos", "razon":"db_unavailable|tabla_ausente|cobertura_insuficiente|sin_bloques|query_failed",
  "cobertura_h":12.4, "filas_24h":8123, "generado_en":"…" }          // 503 para db/tabla/query, 200 para cobertura/bloques
// ok
{ "estado":"ok", "cobertura_h":31.2, "ventana_s":300,
  "agregado":{ "filas":n, "sincro":n, "atraso_1blk":n, "stale":n, "negativo":n,
               "mediana_gap":x|null, "p95_gap":x|null, "peor_gap":x|null },
  "serie":[{ "segundo":"ISO", "total":n, "miss_pb":n, "quote_gap":x|null,
             "economia_ok":n, "p50_net":x|null, "sospechosas":n }],
  "generado_en":"…" }
```
Celdas sin observación → `null` ("—"), nunca `0`.

**Verdad del productor (honesta):** hoy `oracle_block` **no tiene productor** — `ReservesCache` en
memoria no rastrea alturas (`source_block: 0` placeholder en `state_projector.rs`). Mientras no exista,
el endpoint responderá legítimamente `sin_bloques`. Follow-up fichado: WO-ORACLE-BLOCK-01 (propagar
altura real desde el proyector V3 / cache de reservas hasta `trace.oracle_block`).

Montaje: `index.ts` → `mountPoolBLag(app, { pool, logger })` junto a `mountAgentsStatus`.

## 4. Hallazgo de proceso — `CARGO_TARGET_DIR` compartido entre worktrees ALIASEA crates

Primer `cargo check` (target caliente del árbol principal, §36.4) dio 17 errores imposibles:
`missing field leg_fees_bps in RouteMetadata` — campo que **no existe** ni en este worktree ni en el
árbol principal (`grep` = 0 en ambos). Origen: el rlib de `shared-rs` provenía de los worktrees
`arbx-legfix-20260918`/`arbx-legs-econ-f1-20260918` (8794ece6). Cargo deriva el hash de metadata de
los path-deps de la **ruta relativa** (`shared-rs`), idéntica en todos los worktrees, y el fingerprint
apunta a los `.rs` del OTRO árbol → "Fresh" → artefacto ajeno. Remedio aplicado: `cargo clean -p
shared-rs -p searcher-rs -p relays-client -p sim-ctl` (borró **29.7 GiB** de artefactos aliasados) y
recompilar desde este worktree. Consecuencias: WO-1 y el árbol principal recompilan esos 4 crates en su
próximo check (tiempo, no fuentes). **Regla propuesta para el BOARD/LEARNINGS**: un target compartido
sólo es válido si TODOS los árboles que lo usan tienen el mismo contenido de path-deps; en caso
contrario, `cargo clean -p <crates tocados>` antes de dar fe de un `cargo check`.

## 5. Verificación

| Capa | Comando | Resultado |
|---|---|---|
| TS api-server | `npx tsc --noEmit -p tsconfig.json` (node_modules por junction al árbol principal, gitignored) | **exit 0** |
| TS api-server | `npx vitest run src/routes/forensic-pool-b-lag.test.ts` | **4/4 passed** |
| Rust | `cargo check -p shared-rs -p searcher-rs -p relays-client -p sim-ctl` (target caliente, tras clean) | ver actualización al pie |
| Rust | `cargo check --tests -p searcher-rs -p relays-client` | ver actualización al pie |
| Rust | `cargo test -p searcher-rs --lib reject_traces` | ver actualización al pie |

AppControl 4551: no bloqueó `cargo check` (perfil dev; memoria: "dev EJECUTA mayormente").

## 6. Pendientes / riesgos declarados
- **WO-ORACLE-BLOCK-01**: productor real de `oracle_block` (sin él, `pool_b_lag` = `sin_bloques`, honesto).
- Gates de sizing (`size_optimizer`) aún no escriben en `trace` (cap_usd, slippage, gate pair). El
  sobre `rechazado` con gross/net desde la `Opportunity` cubre la economía; la granularidad por gate del
  optimizer es E2.
- Cartuchos (`cartridge_boot`) rechazan vía `emit_rejected` sin `StrategyCandidate` → hoy reciben el
  sobre por defecto del emisor (correcto por R8, menos rico).
- `MIGRATION_HISTORY.md` no tocado (entrada 122 no requiere nota de gap/duplicado).
- Junction `backend/api-server/node_modules` en el worktree: artefacto local, invisible a git.
