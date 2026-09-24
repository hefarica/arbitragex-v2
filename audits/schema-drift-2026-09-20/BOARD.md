# SCHEMA-DRIFT-2026-09-20 — Auditoría de drift de esquema en toda la DApp

> Orden del operador (verbatim 2026-09-20): "busca en donde mas hay schema drift en la dapp. audita corrige e implementa y mejora"

## Metodología (fail-honest, RULE 00)
1. Volcar esquema DESPLEGADO real (information_schema del PG del VPS, read-only).
2. Derivar esquema ESPERADO del repo (parse determinista de `database/migrations/*.sql`).
3. Diff bidireccional: columnas esperadas-sin-desplegar (drift tipo A) y desplegadas-sin-esperar (tipo B).
4. Cross-check queries del código (searcher-rs / api-server / recon / edge) vs columnas desplegadas (tipo C: query-reference drift).
5. Fix vía PR por drift confirmado. Nada se afirma sin observación.

## Estado verificado al abrir
- Repo: 111 archivos de migración (001→121, con gaps por diseño).
- Deployed: `opportunities` y `scored_opportunities` TIENEN las columnas que el INSERT de persistence.rs usa (status/detector_id/pipeline_latency_ms/evidence_vector verificados). El INSERT masivo funciona (4.3K/min persistidos).
- `scored_opportunities` NO tiene columna `status` (verificado) — correcto según migraciones.
- **Hallazgo pre-auditor (VERIFICADO)**: db_errors intermitentes del searcher (10-22/15m, clusterizados ~:02-:05 de hora) = PG `canceling statement due to lock timeout` + `terminating connection due to administrator command` — contención de locks con TRUNCATE/retención (cron horario), NO drift de columna.
- **Hallazgo pre-compaction (IRREPRODUCIBLE — R9)**: `column "status" does not exist at character 8` no aparece en la ventana de logs PG retenida (0 apariciones en ventana completa). No se fixea un fantasma; si reaparece, capturar statement completo (log_min_error_statement).

## PARTE 2 — AUDITORÍA DE DESTINOS DE DATOS (orden del operador 2026-09-20)
> "auditoria de todos los destinos a donde viaja o se supone viajaría la información y [dónde] termina pasando o apunta para otra parte"

### Metodología
- **Censo de canales vivos**: Redis streams (XLEN + XINFO GROUPS + lag), pub/sub channels (SUBSCRIBERS), PG tablas (rowcount + MAX(ts) frescura), WS rooms/channels del api-server, endpoints del edge.
- **Censo declarado en código**: grep de XADD/XREADGROUP/PUBLISH/SUBSCRIBE en backends + `NEXT_PUBLIC_*_URL` del frontend → grafo productor→canal→consumidor esperado.
- **Diff**: (a) canales con productor SIN consumidor = datos que viajan a ninguna parte; (b) consumidores esperando canales que NADIE produce = wires muertos; (c) canales vivos sin dueño en código = herencia fantasma; (d) tablas que solo reciben y nadie lee = tumbas de datos.
- Cada veredicto con evidencia (comando + salida + archivo:línea).

### WORK ORDERS
- [x] **WO-D1** ✅ `redis_census.txt` — streams vivos (4, capados por trimming) vs muertos (opps:simulated/hot:simulated xlen=0; dlq/accounting:pending/route_discovery:tick type=none)
- [x] **WO-D2** ✅ `pg_census.txt` — 66 tablas rowcounts; ~20 a 0 rows (por diseño §34 / inanición upstream / tumbas)
- [x] **WO-D3** ✅ (por canales críticos) — grafo implícito en DRIFT-REPORT §2.1-2.3 con productor/consumidor por canal; edge/WS-rooms completo queda en D5
- [x] **WO-D4** ✅ Tabla de atascos en DRIFT-REPORT §2.2 — **selector-g0 (feed cards) lag 45 831 / pending 6 096**; rd-outcome-sink lag 5.6M/pending 10 883; scoring-archiver 133K/962
- [x] **WO-D5** ✅ `ws_rooms_census.md` + **PR #616** (`fix/ws-metrics-dead-room`, commit fb547bb6): 6/7 rooms VIVOS en ambos extremos (opportunities, convergence, telemetry, route_discovery, runtime_ack, prices — este último falso positivo descartado: handler en prices-stream.ts:104); room `metrics` = WIRE MUERTO (handler sin productor NI consumidor, AsyncAPI ya lo marcaba DORMANT) → removido handler + contrato asyncapi + réplica en harness + e2e repuntado al room vivo `telemetry`. 4/4 vitest, tsc limpio. Pendiente: CI + merge + deploy.
- [x] **WO-D6** ✅ Priorización P0-P2 en DRIFT-REPORT §2.4 (P0: productor drift_observations + selector-g0 pending)
- [x] **WO-D7** ✅ **P0 drift_observations RESUELTO vía #613** (fix/drift-no-producer-r10): degradación honesta `reason=drift_observations_no_producer` cuando la tabla tiene cero observaciones históricas (sin productor, NO coherencia genuina); 3 tests de contrato nuevos (13/13); tsc limpio. Regla permanente **R10 E2E-COMPUTE GUARD** codificada en CLAUDE.md §3 (orden del operador 2026-09-20: ningún campo como computado sin procesamiento end-to-end en todas las capas). NO se implementó productor de cero-drift (sería fabricación RULE 00); el motor real queda como WO futuro.
- [x] **WO-D8** ✅ **PR #614** (`fix/selector-g0-orphan-hygiene`, commit 20efed27): sweep de higiene selector-g0 — XAUTOCLAIM pending stale (idle>60s) al consumidor vivo + reproceso, sólo después DELCONSUMER de huérfanos (idle>5min); sweep omitido con kill-switch armado; stop() zero-loss; 6 invariantes nuevas (13/13 vitest); tsc limpio. Patrón espejo del WO-15 probado de api-server/websocket.ts. Pendiente: CI + merge + deploy + verificación L4 (`XINFO CONSUMERS` huérfanos→0, pending decreciendo, log `consumer.group_hygiene`).

## WORK ORDERS (PARTE 1)
- [x] **WO-S1** ✅ `deployed_schema.tsv` (1139 columnas VPS)
- [x] **WO-S2** ✅ `diff_schema.py` — con limitación documentada: 60 migraciones con SQL dinámico → output crudo NO confiable a nivel columna (verificado por grep que los "drifts" tipo A/B eran artefactos)
- [x] **WO-S3** ✅ `DRIFT-REPORT.md` — veredicto: NO hay drift de columnas; drift real #1 = ledger schema_migrations (termina 099, migraciones 100-121 aplicadas sin registro; dos runners coexistieron)
- [x] **WO-S4** ✅ (parcial) INSERT searcher 22 cols verificadas; endpoint /api/system/drift verificado; **drift real #2: `drift_observations` sin ESCRITOR con lector vivo (api-server:112 + frontend RegistryCoherenceStrip) → veredicto "COHERENT" fabricado en pantalla**
- [x] **WO-S5** ✅ PR #612 (run_migrations.sh registra version+sha256 por archivo aplicado, ON CONFLICT DO UPDATE). P0s restantes (productor drift_observations, selector-g0 pending) priorizados en DRIFT-REPORT §2.4
- [x] **WO-S6** ✅ `wo-s6-lock-timeout-root-cause.md` — causa raíz RESUELTA sin cambio de código: (a) el clustering histórico :02-:05 horario murió con el cron de retención horario legacy (desactivado 2026-09-04) y el FLIPPER (removido 09-17); retención actual = diario 04:17 con batching, hoy 661K deletes sin errores. (b) El único lock timeout residual (14:51:53 hoy) es DEPLOY-TRANSITORIO: coincide con run_migrations.sh del deploy #612 (mtime 14:51, deploy-lock held); fail-fast 5s por diseño. Seguimiento: 2026-09-21 04:20Z verificar db_errors post-retención diaria (ventana R9 insuficiente hoy).
- [ ] **WO-S7** Deploy VPS + L4 + R7 tras merges (cola: #607→…→#612)

## Reglas
- Read-only sobre VPS hasta tener el diff completo (§6 arbx-live-engineering).
- Cada claim: fuente (query/archivo:línea). Nada de memoria.
