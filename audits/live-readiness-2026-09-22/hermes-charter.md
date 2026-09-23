# CHARTER GANG — LIVE-READINESS 100% VERDE (2026-09-22, oleada 2)

## /GOAL (operador, verbatim-intent)
https://arbx.ape-tv.net/live-readiness debe quedar SIN alertas, SIN advertencias y con TODOS
los checks superados (los ítems [OPERADOR] quedan documentados, no forzados). Loop
éxito-o-éxito: detectar gaps → confirmar → corregir → re-verificar, hasta cerrar.

## BOARD (eje central — leer ANTES de trabajar, actualizar AL cerrar con evidencia)
- `C:\Users\HFRC\Desktop\arbitragex-v2-main (17)\audits\live-readiness-2026-09-22\GOAL-WORKORDERS.md`
- Plan 5W2H (mismo directorio): `5W2H-2026-09-22.md`
- BOARD umbrella (NO duplicar, delega): `audits\live-readiness-20260921\GOAL-WORKORDERS.md`
- Ledger de experiencia: `C:\Users\HFRC\.claude\skills\arbitragex-omniscience\LEARNINGS.md` — leer antes; anexar lecciones al cierre.

## WORK ORDERS (prioridad)
1. **WO-LR22.1 (P0) SCANNER-SILENT-02** — pipeline de detección muerto desde 2026-09-22T10:19Z.
   Evidencia (verificada por orquestador):
   - `XLEN arbx:opps:detected` = 10003 congelado; última entrada detected_at 10:19:00Z (triangular, spot_product_le_one).
   - Heartbeat 60s todo en 0: pending_received=0, decoded=0, *_scanned=0, passed_all_gates=0,
     price_alchemy_backoff_active=1, price_coingecko_backoff_active=1.
   - Contenedor `arbitragex-v2-searcher-rs-1` Up 35h SIN reinicio; SIN panic en ventana retenida
     (ventana arranca 14:26Z — regla R9: la ausencia de eventos de arranque es artefacto de rotación).
   - Red del contenedor VIVA: TCP establecidos a Redis 172.18.0.5:6379, PG 172.18.0.8:5432, y 5+ WSS :443.
   - Tormenta DNS: 401× "Temporary failure in name resolution" 14-16Z (redis::ConnectionManager
     reconnects); Redis BusyLoadingError 14:26Z (restart AOF). Binance WS worker reconecta OK.
   - Watchdog SCANNER-STALL-01 (#624, 60s) NO disparó con ~6h de stream congelado → GAP del guardián (auditar).
   - Backoffs price permanentes: alchemy key 429 crónico (api.g.alchemy.com/prices), coingecko 400+429.
     Regla de casa: SOLO fuentes gratuitas — PublicNode + CoinGecko + CoinMarketCap; NO proponer pagar nada.
   Código clave: `backend/searcher-rs/src/scanner.rs` (run_chain:799, run_subscription:1346,
   subscribe_pending:1459, subscribe_pending_filtered_txs:1414), `src/workers/route_scanner_worker.rs` (RU-3).
   DoD: XLEN delta>0 sostenido + heartbeat pending_received>0 + `arbx:quote:anchor:1` publicado
   (TTL 35s) + RC de la muerte silenciosa documentada + PR durable en rama (NO push/merge sin operador).
2. **WO-LR22.2** — edge `proxyPassThrough` no adjunta CORS en respuestas de error → el 503 honesto
   `quote_anchor_not_published` se ve como error CORS en consola. Fix: ACAO/ACAC en éxito Y error
   (edge/worker/src/index.ts ~1368 + middleware CORS ~313-328). Contract test. PR en rama.
3. **WO-LR22.3** — Stepper 0/4 `topology_vault_empty` (REGRESIÓN: 09-21 era 4/4). Localizar el
   productor del snapshot del Topology Vault (endpoint /api/readiness/steps, api-server), por qué
   no re-publica tras la cadena de crash (Redis wipe / PG panic), re-publicar con datos REALES
   (RPC_WS_1 etc. ya están en .env del VPS). Cero invención (RULE 00).
4. **WO-LR22.8** — tormenta DNS Docker: correlacionar 401 fallos vs restarts Redis; si es
   reconnect-hammer del ConnectionManager, proponer backoff jittered (PR en rama) — investigar, no adivinar.
5. **WO-LR22.4** — G-SIM-1 evidencias stale (modules_merged, fork_suite, variance_benchmark,
   eth_callbundle_staging): SOLO artefactos reproducibles. Requiere pipeline vivo (tras LR22.1).
   Coordinar con BOARD `audits/forensic-engine-2026-09-21/` (no duplicar).
6. **WO-LR22.7** — ROLLUP-GAP-01 durable: parchear pg_retention.sh (zero-fill `__totals__` para
   días con horas iniciales vacías; hoy se aplicó manual ver `audits/live-readiness-2026-09-22/`).
7. WO-LR22.6 es verificación post-cron 04:17Z (no actuar antes).

## VERIFICACIÓN (comandos honestos, VPS via ssh alias `arbx`)
- Heartbeat: `curl -s http://127.0.0.1:8080/api/v1/scanner/heartbeat` (desde VPS)
- Stream: `docker exec arbitragex-v2-redis-1 redis-cli XLEN arbx:opps:detected` (delta > 0)
- Anchor: `docker exec arbitragex-v2-redis-1 redis-cli TTL arbx:quote:anchor:1` (>0)
- Página: https://arbx.ape-tv.net/live-readiness (sin alertas/warnings)
- Deploy (si cambio de imagen es necesario — SIEMPRE): `docker compose --env-file .env -f
  docker/compose.prod.yml build --no-cache <svc>` + `up -d <svc>` (compose.prod.yml es el
  ACTIVO — verificado por label; NUNCA compose.dev.yml). Post-deploy: `git rev-parse HEAD` veraz.
- Logs: ventana rotada — SIEMPRE comparar State.StartedAt vs primera línea retenida (R9).

## LÍMITES INVIOLABLES (§32/§33 + doctrina de casa)
- Read-only/shadow sobre capital: SIN wallets, SIN private keys, SIN broadcast on-chain, SIN
  flips a live, capital expuesto = 0. `PRIVATE_KEY` vacío siempre.
- Postgres/Redis/GitHub: lectura; escrituras de datos SÍ permitidas solo si son el fix real
  (p.ej. re-publish de snapshot con datos reales). NUNCA TRUNCATE/DROP sin nota BOARD explícita
  con justificación y evidencia.
- RULE 00 (cero mocks/hardcodes), R8 (fail-honest), R10, no default-off sin orden.
- NO git push / NO PR / NO merge sin operador (no-git-until-final-gate). Preparar ramas + diff
  y reportar en BOARD. NO commitear en branches ajenas (§36: verificar `git branch --show-current`).
- NO tocar `codex/*` branches ni worktrees ajenos; NO borrar nada cuyo origen no conozcas.
- Si una ruta exige violar esto → DETENERSE y reportar el bloqueo en BOARD.

## GANG (§9.1: mesa redonda, todos observan a todos)
- Composición sugerida: agente-Rust-pipeline (LR22.1) + agente-edge-TS (LR22.2) +
  agente-readiness (LR22.3) + agente-SRE (LR22.8) + validadores (Rust reviewer + TS reviewer).
- Sincronía: cada agente cita a sus pares en el BOARD; refutar con nombre; nadie edita el WO
  de otro sin CLAIM previo (dueño + LOCK).
- Respawn-2 (límite v1.2): agente caído (429/timeout) → el resto SIGUE; 2 reemplazos con mayor
  capacidad partiendo la tarea; si 429 SISTÉMICO (≥6 muertes y >2× éxitos) → oleadas de ~4,
  presupuesto global, SUSPENDER y reanudar con resumeFromRunId.
- Verificación en árbol ajeno (R11-R13): cierre de ciclo = `git status --porcelain | grep -v '^??'`
  en cada worktree propio; PR manifestado = SU DIFF, no el POST.

## ENTREGA
- BOARD actualizado por WO (estado + evidencia reproducible + artefactos).
- PROGRESS.md del run con hitos. Lecciones nuevas → LEARNINGS.md (1-3 líneas, [GEN]/[ARBX]/[CCR]).
- Reporte final: qué quedó verde en la página, qué queda [OPERADOR], qué PRs esperan merge.
