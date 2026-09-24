# BOARD — LIVE-READINESS 100% (oleada 2026-09-22)

> Extiende `audits/live-readiness-20260921/GOAL-WORKORDERS.md` (programa umbrella).
> /goal (operador 2026-09-22): página sin alertas/advertencias, todos los checks superados.
> Plan: `5W2H-2026-09-22.md` (este directorio).
> Reglas BOARD (§9.1.1): leer ANTES de trabajar, CLAIM con dueño + LOCK, actualizar al
> cerrar con evidencia reproducible. Doctrinas: RULE 00 · R8/R10 · §32/§33 (read-only,
> $0 capital, sin broadcast/wallets/flips) · no-git-until-final-gate · solo fuentes
> gratuitas (PublicNode/CoinGecko/CMC) · NO default-off sin orden.

## Estado página 16:39Z→17:00Z (orquestador)
- 16/19 verified · 2 partial (G-PAP-1, G-DISK-1) · 1 failing (G-SIM-1).
- **FIX LANDED hoy**: FE-EDGE-DIRECT-01 — `ALLOWED_ORIGINS` unset en edge prod → ACAO ''.
  Añadido a VPS `.env` + `up -d edge` (R3). Verificado HTTP/2 200 + ACAO. Consola:
  errores opportunities/live ELIMINADOS (queda solo quote/anchor → WO-LR22.2).
- **FIX LANDED hoy**: ROLLUP-GAP-01 p20260920 — 11 buckets `__totals__` zero-fill honestos;
  coverage gate PASS → p20260921 dropeable (cron 04:17Z) → G-DISK-1 esperado verde 09-23.
- **FIX LANDED hoy (sesión previa)**: cadena recovery disco-lleno → PG retention drop →
  Redis AOF repair → api-server/edge up.

## WO-LR22.1 — SCANNER-SILENT-02: detección muerta desde 10:19Z — 🟢 REVIVE+FIX LANDED, VALIDADO (solo PR/merge [OPERADOR])
- **Dueño**: hermes-gang (ccr-glm53). CLAIM con LOCK — no editar sin citation.
- **RC confirmada (código + logs vivos, 17:30Z)**:
  - Imagen desplegada f0e94d91 SÍ contiene #624 (verificado git merge-base + image_created 09-21T05:54Z).
  - El detector activo del stream es route_scanner_worker (RU-3, ARBX_ROUTE_SCANNER_MODE=on;
    entradas dex_engine/triangular): consume `blocks.next()` (route_scanner_worker.rs:747)
    SIN `next_item_with_idle` — #624 solo cubrió loops pending-tx. Socket WS zombificado
    (TCP :443 establecido, 0 frames) → loop eternamente pendiente, sin error ni rotación.
  - Evidencia: 0 eventos log del target route_scanner en TODA la ventana retenida
    (ni route_scanner.done ni connected ni errores de reconexión) mientras rpc_health
    observaba bloques nuevos cada ~12s; XLEN congelado en 10003.
  - block_scanner.rs:213/363 — misma clase de hueco (newHeads sin watchdog).
  - Tormenta 14-16Z (DNS/Redis) no fue la asesina (muerte 10:19Z) pero dejó al proceso
    con sockets/managers degradados; Redis revivió 16:51:05 y la detección NO (sockets
    zombi persisten).
  - Watchdog SCANNER-STALL-01 (#624) no disparó: GAP REAL — newHeads consume loops
    fuera de cobertura (auditar = este WO; fix SCANNER-STALL-02).
- **Acción en curso**: (a) revive operacional recreate searcher (mismo commit, sin cambio
  de imagen) + verificación DoD; (b) PR durable rama fix/scanner-stall-02: idle-watchdog
  en TODOS los consume loops WS (route_scanner + block_scanner) + tests.
  - `XLEN arbx:opps:detected`=10003 congelado; última entrada `detected_at` **2026-09-22T10:19:00Z**.
  - Heartbeat 60s: pending_received=0, decoded=0, triangular/flashloan/liquidation *_scanned=0,
    passed_all_gates=0, price_alchemy_backoff_active=1, price_coingecko_backoff_active=1.
  - Contenedor Up 35h SIN reinicio, SIN panic en ventana retenida (ventana arranca 14:26Z — R9).
  - Red del contenedor VIVA: TCP establecidos a Redis (172.18.0.5:6379), PG (:5432) y 5+ WSS :443.
  - Tormenta DNS: 401 "Temporary failure in name resolution" 14-16Z (Redis reconnects);
    Redis BusyLoadingError 14:26Z.
  - Watchdog SCANNER-STALL-01 (#624, 60s) NO disparó con 6h de stream congelado — GAP del guardián.
- **Definition of Done**: XLEN delta > 0 sostenido + heartbeat pending_received>0/decoded>0 +
  quote anchor publicado (`arbx:quote:anchor:1` TTL 35s) + explicación de la muerte silenciosa +
  PR durable (raíz, no parche). CERO mocks/hardcodes.
- **VERIFICACIÓN ORQUESTADOR (18:22Z, independiente — cita al gang ccr-glm53)**: revive (a)
  CONFIRMADO aterrizado. `arbitragex-v2-searcher-rs-1` Up 51 min (recreado; antes 35h zombie).
  Heartbeat vivo: `block_intents_dispatched=1424`, `gate_other_rejected=3004`, `db_persisted=3004`,
  `pg_period_inserted=2900`, `pg_period_profit_pos=104` (rechazos honestos R8, 0 viables —
  consistente con funnel histórico). `TTL arbx:quote:anchor:1`=**31** (antes -2) → anchor
  PUBLICADO. `XLEN`=10004 (el stream está bajo maxlen-trim; el tráfico real se ve en los
  contadores de bloque/persistencia). NOTA DoD: `pending_received=0` es esperado — el detector
  activo es route_scanner vía `blocks.next()` (ver RC); la señal de vida correcta es
  `block_intents_dispatched>0`. PENDIENTE para cerrar el WO: item (b) PR durable
  `fix/scanner-stall-02` + su validación.
- **ITEM (b) ATERRIZADO (orquestador, 18:5xZ)**: rama local `fix/scanner-stall-02` commit
  `7a9ba856` (13:54:31 local, 1 commit quirúrgico sobre f0e94d91=main desplegado):
  `next_head_with_idle` en chain_client + cableado en los 3 sitios newHeads
  (route_scanner_worker:747, block_scanner run_block_subscription + run_head_subscription);
  env ARBX_SCANNER_IDLE_TIMEOUT_SECS default 60s (~5× margen sobre heads de 12s);
  3 tests nuevos + suite 1358 PASS + clippy/fmt limpios (según commit msg — validación
  Rust independiente despachada por el orquestador, resultado pendiente). Branch hermana
  `fix/scanner-stall-02-reloader-reconnect` (worktree perf-89-edge-fe) = base sin commits
  (trabajo futuro de reconnect hardening, sin mods huérfanos — verificado R13).
  **Estado WO: 🟢 VALIDADO (ecc:rust-reviewer independiente, PASS 19:1xZ)** — `next_head_with_idle`
  (chain_client.rs:423-432) races `stream.next()` vs timeout con deadline fresco por iteración;
  Err(Elapsed)→bail = stall, Ok(None) = fin genuino; los 3 supervisores absorben el Err con
  rotación+backoff 1s→30s; tests reales (pending-stream ejercita el path Elapsed exacto del
  socket zombi). Hallazgos: 2 LOW (duplicado byte-level de next_item_with_idle #624 — unificar
  en fn genérica con param de evento; handshake pre-loop sin bound — pre-existente) + 1 INFO
  (1358-tests no re-reproducidos por el validador: árbol compartido R11 + AppControl 4551;
  re-run cuando el gang tenga worktree caliente). **PR + merge = ORQUESTADOR** (orden del
  operador 2026-09-22: "crear el PR y hacer merge, todo esto lo harás tú, tienes todos los
  accesos"). **PR #629 ABIERTO** (orquestador): 1 commit, diff exacto = los 3 archivos
  (R13 manifestación verificada vía GET /pulls/629 + /files), mergeable=True. Merge al pasar
  CI (14 required checks) — vigilancia activa.
- **MERGED+DEPLOYED (orquestador, 20:15Z)**: PR #629 merge CI-limpio (merge commit
  `58b7e55a`), imagen searcher-rs rebuild (cached, R3-justificado: backend Rust sin
  bake de NEXT_PUBLIC), `up -d` con HEAD verificado `58b7e55a`. Boot verificado:
  `rpc_pool.ready chain_id=1 count=3 providers ["publicnode","drpc","blockpi"]`
  (failover WO-LR22.12 activo en el mismo restart), quote anchor TTL=27, workers up.
  **WO-LR22.1: 🟢 CERRADO (revive + PR durable + merge + deploy verificados).**
- **NOTA flapping gateway — RESUELTO (orden del operador 2026-09-22: "eso te toca a ti")**:
  cada pérdida de run hoy correspondía a un REPAIR_START component=Hermes del watchdog
  PANTHEON_24X7 (11:58, 12:02, ..., 14:03). RC: Test-Http TimeoutSec 15 < max observado de
  /health bajo carga (14767ms, t_b18ac793) → gateway vivo-but-busy reciclado. **FIX LANDED
  (orquestador)**: TimeoutSec 15 → 45 (3× max observado) en PANTHEON_WATCHDOG_24X7.ps1.
  Además, checkpointing 15s activo: ledger del run + snapshots del BOARD en
  `checkpoints/` (muerte de run ya no reinicia desde 0).

- **RECUPERACIÓN VERIFICADA 22:12Z (orquestador)**: searcher-rs evaluando cartridges sobre
  mempool real (logs 22:12:04Z: active_count=271, pertinent=269, razones negativas honestas
  R8 por categoría); `XLEN arbx:opps:detected` 10003→10005; PG `MAX(detected_at)`=22:12:12Z
  (en vivo, 346.190 filas últimas 2h); quote_anchor HTTP 200 vía edge. El fix #629
  (scanner-stall-02, watchdog) está en origin/main (58b7e55a). Reloj G-PAP-1 avanzando
  ("last 0.0h ago"). Queda vigilancia: si vuelve a stall-ear, el watchdog #624/#629 debe
  disparar — auditar su log si recurre.
## WO-LR22.2 — quote_anchor 503 sin ACAO (proxyPassThrough) — 🟡 PR #631 EN CI
- **Dueño**: orquestador (trabajo propio, orden 2026-09-22: sin gateway). Trabajo huérfano
  del worktree `arbx-wt-edgecors` (branch `fix/edge-passthrough-cors-lr22b`) RECUPERADO
  (R13.3): fix raíz + 3 contract tests. `proxyPassThrough` retorna vía `c.header`/`c.body`
  (no `new Response` crudo) → ACAO/ACAC presentes en éxito Y error.
- **Verificado local**: vitest 6/6 PASS (3 nuevos WO-LR22.2), tsc --noEmit limpio.
- **PR #631** (diff exacto 3 archivos, R13.1 verificado): merge auto al verde CI.
## WO-LR22.3 — Stepper 0/4 topology_vault_empty — 🟢 RESUELTO EN VIVO (22:05Z)
- **Dueño**: orquestador. Verificado 22:05Z: `GET :8787/api/readiness/steps` (vía edge,
  Origin arbx.ape-tv.net) → `"readiness_score":"4/4","all_ready":true` con topology PASS
  (4 WSS + 5 RPC activos, vault version 1786916896). El vault NUNCA estuvo vacío
  (`/var/lib/arbx/topology-vault.staging.json`: 231 HTTP + 142 WSS providers, intacto
  desde 08-16) — el 0/4 fue transitorio de la cadena de crash (api-server reiniciando),
  NO pérdida de datos. Sin fix durable necesario: el productor persiste en disco atómico
  (tmp+rename) y sobrevive a wipes de Redis/PG por diseño.
## WO-LR22.4 — G-SIM-1 evidencias stale ×4 — 🟡 EN PROGRESO (orquestador, 21:45-22:00Z)
- modules_merged, fork_suite, variance_benchmark, eth_callbundle_staging — artefactos
  reproducibles únicamente (§34.5.3). `second_signoff` = [OPERADOR]. Coordina con BOARD
  forensic-engine-2026-09-21 (no duplicar).
- **AVANCE 21:45-22:00Z (orquestador)**:
  - `modules_merged` 🟢 EVIDENCED 21:45Z — POST manual reviewer:claude-omega; SHAs F0
    verificados `git merge-base --is-ancestor` vs origin/main 58b7e55a: lazy_db 00352836,
    revm_runner 74cfbf48, validators f2241c16.
  - `fork_suite` 🟢 EVIDENCED 21:46Z — run GH 35788411760 SUCCESS:
    `FORK_SUITE_OUTCOME=PASS block=26035791 chain=1 successful_calls=2 round_trip_wei=1`;
    POST workflow `recorded_and_verified` (ruta SSH, no requiere ARBX_READINESS_EVIDENCE_URL).
  - `eth_callbundle_staging` 🟢 EVIDENCED 21:47Z — run GH 35788416360 SUCCESS:
    `ETH_CALLBUNDLE_STAGING_OUTCOME=PASS outcome=economic_gate_insufficient_funds relay=flashbots.net`
    (probe simulate-only, signer efímero, sin broadcast — §32 OK); POST `recorded_and_verified`.
  - `unit_tests`+`dep_tree` — run GH 35788443570 SUCCESS (43 tests passed); registry ya
    fresco 09-13 (<30d) ✓.
  - `variance_benchmark` 🔴 FALLÓ 2ª VEZ post-#630 (22:35Z, VPS 48c86b50) — y esta vez
    es un hallazgo ESTRUCTURAL REAL, no bug de parsing: harness parseó las 400 filas
    (fix #630 verificado OK) pero `VARIANCE_BENCH_OUTCOME=FAIL samples=0` con skips
    {dedup:338, unsupported_adapter:62}: TODA la muestra es topología duplicada o
    adaptador V3. RC: harness+encoder A.3.a soportan SOLO UniswapV2/SushiSwap
    (adapter_to_semantic → None para V3) mientras el tráfico 2-leg chain-1 es ~99.98%
    V3-containing (V2+V2 puro = 4 filas/24h). Con dedup de topología + ventana
    frescura tip-1100 (~3.66h), min_samples=100 es inalcanzable SIN soporte V3.
    Bajar min_samples = score-painting (P-04) → RECHAZADO.
    **Camino**: lift V3 en el encoder Rust — el CONTRATO ya lo soporta
    (ArbitrageExecutor.executeArbitrageFlashFunded toma routers[]+payload[] arbitrarios
    con adapters/UniswapV3Adapter + dexes/PancakeV3Adapter presentes); falta encoder:
    fee tier por leg (pool.fee() RPC cacheable), calldata exactInputSingle, y quoting
    intermedio V3 en el harness. **Además**: `jq` NO existe en VPS (script línea 134
    `jq: command not found`, evidence-payload.json quedó 0 bytes) → registry POST
    roto INDEPENDIENTEMENTE del outcome; fix = de-jq vía python3 (3.12.3 presente).
    → nuevo WO-LR22.13 (encoder V3) + fix de-jq en el mismo PR o separado.
  - `second_signoff` ⬛ [OPERADOR] — quórum-2, NO auto-firmable (08-17 = stale 36d).
- Estado ítem: 5/7 frescos-verdes (o con productor en vuelo), 1 en fix (PR #630),
  1 operator-gated.

## WO-LR22.5 — G-PAP-1 reloj 7 días — ⏸ RELOJ EN MARCHA (aclarecido 22:45Z)
- days_accumulated = edad de MIN(detected_at) en opportunities (g-pap-1.ts:116).
  Tabla truncada 09-19 21:22Z → **verde automático 2026-09-26 21:22Z** si el pipeline
  sigue produciendo (última detección 0.0h — vivo) y nadie trunca. Ventana retención
  opportunities = 60d (pg_retention.sh spec) → el purge NO roba el reloj antes de 7d.
  Preocupación lateral: opportunities ya 26GB/3d — presión de disco (ver LR22.6).

## WO-LR22.13 — G-SIM-1 variance_benchmark: fork-deploy de ejecutores + encoder V3 + de-jq — 🔴 OPEN (orquestador, PR-A ✅)
- **Dueño**: orquestador. Nace del FAIL estructural documentado en WO-LR22.4.
- **HALLAZGO CRÍTICO (23:0xZ, verificado)**: ARBITRAGE_EXECUTOR / EXECUTOR_1 /
  FLASHLOAN_EXECUTOR_1 NO TIENEN CÓDIGO ni en mainnet (publicnode+drpc eth_getCode="0x")
  ni en Sepolia — nunca se hizo broadcast (consistente con §32/§33). execute_multistep_revm
  step C hace ExecuteCall del caller al FLE: sin código, TODO dispatch revierte →
  labeled=0 ESTRUCTURAL, independiente del encoder V2/V3. La "PRE-CONDICIÓN on-chain
  approvedRouters" anterior queda obsoleta: no hay allowlist que leer.
- **Plan 4 PRs (nada baja min_samples ni cambia método — eso es pintado, P-04)**:
  - **PR-A ✅ PR #634** (fix/gsim1-benchmark-dejq, c7e36d4c, R13 verificado 1 archivo):
    payload de evidencia vía python3 (jq ausente en VPS dejaba payload 0 bytes y rompía
    el POST al registry); python3 agregado a MISSING_PREREQS fail-honest. Monitor bpmut4g10.
  - **PR-B 🔨 fork-deploy en CacheDB**: desplegar AE+FLE DENTRO del fork in-process
    (impl+proxy ERC1967 con CREATE determinista desde deployer seedeado nonce-0, roles,
    approvedRouters/approvedSelectors, token approvals — espejo de
    contracts/script/DeployMainnet.s.sol) SIN broadcast, SIN signer (§32-compliant;
    el runner ya muta solo el CacheDB in-process). El harness apunta el plan a las
    direcciones fork (override de env dentro del test), las prod quedan intactas.
    Exploración call-graph en curso (AE gates exactos, FLE.executeOperation, allowance
    del router V3, AllowanceManager requerido o no).
  - **PR-C encoder V3**: parse_dex_kind/build_round_trip_context acepta
    UniswapV3/PancakeV3 con fee tier vía pool.fee() RPC cacheable; calldata
    exactInputSingle al payload[] (contrato YA router-agnóstico).
  - **PR-D harness**: adapter_to_semantic V3 + quoting intermedio V3.
- **Criterio de éxito**: re-run benchmark → labeled ≥100 + mean_abs_drift <5% →
  POST registry evidenced (real, no pintado).

## WO-LR22.6 — G-DISK-1 verificación cron 04:17Z — 🟡 ESPERA (09-23)
- p20260921 (~31GB) dropeable; coverage gate YA PASS (ROLLUP-GAP-01 fix hoy). Verificar
  `df -h /` + blockers SIN g_disk_1. Si NO dropea → RC script, auditar output del cron.

## WO-LR22.7 — ROLLUP-GAP-01 durable — 🟡 PR #632 EN CI
- **Dueño**: orquestador (trabajo propio). `rdo_zero_fill()` en pg_retention.sh: fila
  __totals__ cero SOLO para buckets sin crudo (R8: nunca tapar crudo pendiente), tras el
  backfill y antes del coverage gate. Tests: extractor estático fail-closed + PG15
  desechable reproduciendo el incidente (bucket vacío rellenado, bucket con crudo NO,
  idempotencia, gate=0). Probe en producción con ROLLBACK: INSERT 0 0 (idempotente).
  **PR #632** (diff exacto 2 archivos): merge auto al verde.
## WO-LR22.8 — 🟢 DIAGNOSTICADO (22:30Z, sin cambio de código) — Tormenta DNS Docker (401/3h) — 🔴 OPEN (investigación)
- Correlacionar DNS-fails vs Redis restarts; reconnect-hammer del ConnectionManager;
  opciones: backoff jittered (PR) / `dns:` compose — investigar, no adivinar.

## WO-LR22.11 — ENRICHER-SILENT-01: token-enricher broken-pipe zombie desde 10:19Z — 🟢 FIX DURABLE LANDED (PR #633, 22:2xZ)
- **Dueño**: orquestador (hallazgo 20:05Z investigando el flujo de data por orden del operador).
- **RC (logs vivos)**: `enricher.consumer_read_err xreadgroup error: broken pipe` cada ~500ms
  desde 10:19Z — socket Redis muerto en la tormenta 14-16Z (mismo patrón clase-zombi que
  SCANNER-SILENT-02: retry eterno sobre conexión muerta, SIN reconexión). Contenedor
  "healthy" 38h — healthcheck NO cubre el loop de consumo (gap).
- **Fix durable = PR #633** (worktree arbx-wt-enricher, commit e658cf96, 5 archivos —
  verificado vía REST API R13): (a) `MultiplexedConnection` → `redis::aio::ConnectionManager`
  (feature connection-manager): reconecta con backoff EN EL LUGAR, PEL preservado, sin
  restart; (b) gauge `arbx_enricher_last_read_unixtime` (heartbeat en TODO XREADGROUP
  round-trip exitoso, INCLUIDO idle-timeout — se setea ANTES del early-return de reply
  vacía); (c) healthcheck compose: `arbx_enricher_up 1` AND last_read < 180s → un zombi
  (socket muerto, loop vivo) ya NO puede reportar healthy. Cargo: 78/80 tests OK locales
  (2 fallos = #[sqlx::test] DATABASE_URL, pre-existentes, CI los cubre).
- **Post-merge (monitor bc176hce8 al verde CI)**: VPS pull + rebuild --no-cache
  token-enricher + up -d; verificar gauge fresco + healthcheck + batch_read.
- **Nota**: 326 pending huérfanos del grupo paper-archiver-g0 (consumers muertos, idle ~11d)
  no bloquean el flujo (XREADGROUP `>` sigue); reclamar con XAUTOCLAIM es opcional/limpieza.
- **Backlog (review externo 09-22, NO bloqueantes — anotados, no implementados)**:
  race check-then-act en needs_resolution; overlap de spawns de logo_refresh; multicall
  200-addresses puede exceder gas; join_all TrustWallet sin rate-limit; reset redundante
  de gauges.

## WO-LR22.12 — FUNNEL-V3-RPC: 80% v3_quote_unavailable = saturación RPC único chain-1 — 🟢 CERRADO (20:34Z)
- **Dueño**: orquestador (respuesta a análisis del operador 20:0xZ).
- **Evidencia métricas Prometheus (arbx_v3_quote_total)**: rpc_ok=4.400 vs rpc_error=15.426
  + batch_call_error=5.357 (batch_call_ok=91) → ~82% de llamadas RPC reales FALLAN;
  cache_neg_hit=470K amplifica el rechazo. TRANSPORTE, no matemática (histograma
  ~80% v3_quote_unavailable estable, consistente con histórico 84% SCANNER-STALL-01).
- **RC de config**: `.env` tenía `RPC_HTTP_1` = UN solo endpoint gratuito (blockpi público)
  para TODO el quoting v3 de mainnet (miles de eth_call/min). El pool soporta CSV
  failover (`name=url,...` — rpc_failover.rs:19) pero nunca se pobló.
- **Fix LANDED (20:1xZ, sin rebuild — fuentes 100% gratuitas)**:
  `RPC_HTTP_1=publicnode=https://ethereum-rpc.publicnode.com,drpc=https://eth.drpc.org,blockpi=https://ethereum.public.blockpi.network/v1/rpc/public`
  (backup `.env.bak-rpc1-20260922`). Efectivo al recrear searcher-rs con el deploy del
  idle-watchdog (#629) — un solo restart cubre ambos.
- **ACTIVO en runtime (20:15Z)**: deploy del searcher recreó el proceso con el pool de 3
  (log `rpc_pool.ready ... count=3`). Primera medición (4 min tráfico): errores RPC
  82%→60% (rpc_ok 410 / rpc_error 621) pero **batch quote ~96% fallando** (12 ok / 276
  error). Diagnóstico de contraste: multicalls pool_sync de 100 sub-llamadas baratas
  (~3M gas) triunfan (89-241ms, status ok) → el proveedor NO rechaza multicalls per se;
  lo que excede es el GAS del quote batch (100× QuoterV2 ≈ 15M, sobre el cap del tier
  gratuito).
- **Knob aplicado (20:2xZ)**: `ARBX_V3_QUOTE_BATCH_SIZE=25` (~3.75M gas, misma clase que
  los multicalls que sí pasan) + recreate searcher-rs. Segunda medición en curso.
- **SEGUNDA MEDICIÓN (20:2xZ post-recreate, orquestador) — TRANSPORTE RESUELTO**:
  rpc_ok=1407/1423 (**98.9%**, antes 40%), **rpc_error=0, batch_call_error=0**,
  batch_call **32/32 OK (100%)**, cache_neg_hit colapsó 8701→76, cache_hit=35441
  (positive cache sirviendo). Falla de quoting v3 ELIMINADA como clase de rechazo.
  Resta: `passed_all_gates` sigue 0 — los rechazos restantes son económicos honestos
  (non_positive_profit, single_pool_no_spread) o de otra capa; el histograma del
  archiver a las 20:21Z ya mostraba v3_quote_unavailable cayendo 1896→1162 y
  non_positive_profit subiendo a 1156 (shift esperado: ahora sí se computa el quote
  y el gate económico rechaza de verdad). paper_trade_runs sigue 0.
- **INCIDENTE DISK-FULL-03 (20:2x-20:31Z, orquestador)**: disco VPS 100% (0 bytes libres)
  → Redis AOF MISCONF → pipeline detenido (heartbeat todo 0) + riesgo PG crash.
  Mitigación inmediata: `docker builder prune -af` = **8.7GB** recuperados (7.1GB libres).
  Retención manual adelantada (misma acción del cron 04:17Z): backfill rollup
  missing_remaining=0 + purgas (pool_reserves 99.957 filas). p20260921 (31GB) NO
  dropeada legítimamente: guard de ventana — solo dropeable cuando el día completo
  quede fuera de la ventana de 1 día (≥00:00Z 09-23); el cron de 04:17Z la soltará
  con coverage ya materializado. Margen: 7.1GB libres vs ~0.6GB/h crecimiento RDO →
  alcanza hasta ~07:45Z; cron dispara antes. Post-prune: MISCONF=0, heartbeat vivo
  (block_intents=1860, db_persisted=4526). G-DISK-1 (WO-LR22.6) queda VERIFICADO en
  su parte operativa: el drop de esta noche es limpio.
- **VERIFICACIÓN FINAL (20:33-34Z) — WO-LR22.12: 🟢 CERRADO**. Histograma archiver
  por minuto: `v3_quote_unavailable` **76%→~2%** (95-114 de ~4.800/min); dominante
  ahora `non_positive_profit` 74% (3526-3560) + `no_price_oracle` ~8% (352-576) +
  `NegativeNetProfit=19` (razón nueva, visible solo ahora que el quote llega). El
  funnel computa quotes reales y el gate económico rechaza CON HONESTIDAD (R8):
  `passed_all_gates=0` es realidad económica del mercado actual (sin spread cubierto
  entre los pares escaneados), NO fallo de transporte. Cadena del fix: RPC failover
  ×3 (publicnode/drpc/blockpi) + ARBX_V3_QUOTE_BATCH_SIZE=25. paper_trade_runs
  seguirá 0 hasta que exista spread real — jamás fabricar (WO-LR22.10 hereda esta
  dependencia de mercado, documentada). Residual honesto a explorar fuera de este
  WO: `no_price_oracle` (~8%) = cobertura del oráculo de precios, ver WO-LR22.4.

- **DIAGNÓSTICO COMPLETO 22:30Z (orquestador, evidencia VPS)**:
  1. La "tormenta DNS" fue una ventana continua 16:16→16:51Z (~35 min) SOLO en recon (62
     hits) y relays-client (56 hits): `Reconnecting failed: ... Temporary failure in name
     resolution`. searcher-rs: 0 hits en ventana retenida.
  2. **Causa raíz**: Redis crash-loop (journalctl dockerd: exitCode=1, restart cada ~3.5s,
     restartCount≥6, restartPolicy=unless-stopped delay=0) → el endpoint DNS `redis` no
     resolvía mientras el contenedor flapeaba. 3500 eventos "restarting container" hoy
     = la cadena de crash-loops de la tarde (resuelta por los deploys; causa raíz ya
     tratada por #587-FLIPPER y fixes del día). Sin OOM (journalctl -k limpio).
  3. **Cliente INOCENTE**: redis-rs ConnectionManager reintentó cada ~7s (backoff propio,
     NO martillo); searcher-rs usa UN solo manager compartido (main.rs:394, clones
     multiplexados). recon/relays se auto-recuperaron a las 16:51:05 (recreación de Redis;
     StartedAt 16:51:05Z, RestartCount=0, 6h estable, PONG, AOF ok, 3042 claves).
  4. **Decisión (R8/"investigar, no adivinar")**: NO se cambia ConnectionManager ni
     compose `dns:` — el comportamiento del cliente fue correcto y aditivar backoff no
     habría evitado nada (el problema era el flapping del servidor). Hardening real del
     flapping = los fixes ya landed. Vigilancia: si reaparece crash-loop de Redis,
     el diagnóstico está aquí.
## WO-LR22.9 — [OPERADOR] no autónomos — ⬛ DOCUMENTADO

## WO-LR22.10 — Check gas_burn NOT_AVAILABLE (severity high) — 🔴 OPEN (orquestador, orden operador 09-22)
- **Evidencia (captura operador)**: card "Max gas burn (rolling window)" · `gas_burn` ·
  STATE=NOT_AVAILABLE · SEVERITY=high · CURRENT=— · THRESHOLD=— · SOURCE=paper_ledger ·
  OPERATOR_REQUIRED=false · badge LIVE.
- **Evidence del sistema**: "Rolling gas-burn from paper_trade_runs needs operator thresholds
  + samples — actual-gas: ARBX_CB_MAX_GAS_BURN_USD not configured; sim-gas: operator risk
  thresholds not configured (ARBX_RISK_* env)."
- **Required action**: set ARBX_CB_MAX_GAS_BURN_USD (actual) o
  ARBX_RISK_{NAV_USD,WINDOW_SECS,MIN_SAMPLES,DD_TIERS,GAS_CAP_USD} (sim) + acumular
  paper_trade_runs.
- **Dueño**: orquestador (orden del operador: "agrega esto a tus tareas" + autorización de
  accesos 09-22). Pasos: (1) localizar consumidor del check en api-server y semántica
  exacta de cada env (unidades/default); (2) elegir umbrales alineados a los límites
  conocidos (canary capital ≤ $350, §34.5) — FAIL-HONEST, sin inventar samples;
  (3) aplicar a VPS .env + `up -d` del servicio dueño (env runtime, NO bake si es backend);
  (4) verificar check → PASS/computed en la página. PR si hay cambio de código.
- **CONFIG LANDED + VERIFICADO (orquestador, ~20:0xZ)**:
  - Consumidor: `makeGasBurnBreaker` (risk-circuit-breakers.ts:744) — DOS paths:
    (A.6) actual-gas `ARBX_CB_MAX_GAS_BURN_USD` vs `SUM(actual_gas_cost_usd)`;
    (A.5) sim-gas familia `ARBX_RISK_*` vs `sim_gas_cost_usd`. Combine = worst-wins.
  - **Hallazgo clave**: en PAPER el path actual-gas NUNCA pasa (no hay gas real grabado →
    NOT_AVAILABLE "none has actual_gas_cost_usd") y por worst-wins PINNARÍA el breaker
    en NOT_AVAILABLE aunque el sim pasara → `ARBX_CB_MAX_GAS_BURN_USD` queda
    INTENCIONALMENTE UNSET (documentado en .env del VPS).
  - Aplicado al .env VPS (env_file completo → recreate, sin rebuild): NAV=10000,
    WINDOW=86400, MIN_SAMPLES=10, TIERS=10,20,30,40, GAS_CAP=50 (doctrina: ~14% del
    canary §34.5 ≤$350; igual al default documentado .env.example).
  - Verificación post-`up -d api-server` (curl /api/v1/risk/circuit-breakers/status):
    `gas_burn_breaker | NOT_AVAILABLE | insufficient samples (0/10)` — antes
    "thresholds not configured". drawdown/revert sin cambio (0 runs, R8 honesto).
  - **Dependencia restante**: `paper_trade_runs` = 0 filas (VACÍA; daily muestra runs
    hasta 2026-09-01, luego el funnel 0-viables los detuvo). El check solo pasará a
    PASS cuando el funnel produzca paper runs reales (≥10 en 24h, sim gas < $50).
    **Hereda la cadena funnel del /goal** (no es actionable por config — jamás fabricar).
    GAS_CAP=50 es el cap protector doctrinal: si al volver el tráfico tripa PAUSED,
    es un hallazgo legítimo (gas sim diario > $50), no un falso rojo.
- A.9 quorum-2 sign-off · G-SIM-1 second_signoff · merges de PRs · flips §34 (intactos).

## Sesión / logging
- Charter del gang: `hermes-charter.md` (este directorio).
- **RUN ACTIVO**: `run_b63e6f195c024475a707d43cc96c0d62` (despachado ~19:4x local tras ventana
  estable 3×200 del gateway). Pérdidas
  acumuladas hoy: run_a08e1a3a → run_ef65ef9b → run_0b89c322 → run_7fffa0e0 (gateway reciclado por el keeper
  ~cada pocos minutos: "planned gateway stop" 13:20:46 + vida previa UNCLEANLY/SIGKILL; el
  keeper `HERMES_TARGETED_REPAIR.ps1` lo revive). El charter es idempotente (BOARD-first):
  cada re-despacho retoma desde el estado del BOARD.
- Progreso → PROGRESS.md del run + espejo local.

## ⚠️ 2026-09-22 22:35Z — DEPLOY EN CURSO (sesión ws-metrics-dead-room)
Re-run workflow Deploy-to-VPS 35777265752 (main 58b7e55a, PR #629). Otras sesiones: NO tocar VPS/docker, NO push a main, NO escrituras PG/Redis hasta aviso. Detalle: ../../DEPLOY-IN-PROGRESS-2026-09-22.md
> **UPDATE 2026-09-22T23:5XZ (OMEGA)**: PR-B LANDED as branch fix/gsim1-fork-deploy-stack commit a0d9fee1 → **PR #635** (https://github.com/hefarica/arbitragex-v2/pull/635). R13 verified: diff = 16 archivos intencionales exactos (sin submodulo OZ). 77/77 tests + clippy + fmt verdes. RC fixture resuelto: doble 0x0x (gen script anteponia prefijo a bytecode.object ya prefijado). Monitor merge bb2qt1l5t armado. Siguientes: PR-C (encoder V3) y PR-D (wiring harness).
>
> **UPDATE 2026-09-23T0X:XXZ (OMEGA)**: Hermes SECUENCIAL muerto 2× (run_da1dbba60f + run_8f703c34, ambos HTTP 503 "dual gateway queue timeout" — provider saturado). RESPAWN-2 v1.2: sin más re-despachos; la sesión orquestadora ejecuta las WOs directamente. **PR-C EN CURSO** en branch `fix/gsim1-v3-encoder` (worktree arbx-wt-gsim1dejq, stacked sobre #635/a0d9fee1): encoder layer V3 completa (RoundTripPlanError + dispatch por kind, PoolFeeProvider fail-closed, guard UnsupportedV3Venue en sim_multistep hasta PR-D), 12 literales + 3 call-sites actualizados. Cargo check en curso.

## ✅ 2026-09-23 00:2xZ — CIERRE del deploy (sesión ws-metrics-dead-room)

Run **35796315686** (auto-deploy-vps, push, HEAD `2245eeb3` = #629+#630+#634) = **SUCCESS**.
Verificación post-deploy: VPS HEAD `2245eeb325ab...` == run SHA · 24 contenedores healthy ·
arbx.ape-tv.net 200 · edge 38 opps · PG fresco (lag ~11s) · Redis XLEN 10002.
Archivo `DEPLOY-IN-PROGRESS-2026-09-22.md` retirado — sesiones pueden operar normal.
Nota: disco `/` volvió a 93% por el rebuild (builder prune pendiente, candidato a acción
del operador). Fósil `deploy-vps.yml` sigue sin `--env-file` (PR futuro de retiro/fix).
