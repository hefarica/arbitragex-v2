# CB-01 — Censo exhaustivo de módulos/switches reales de procesamiento

> **WO:** CB-01 · **kind:** censo (read-only, cero git) · **agente:** ecc:rust-reviewer (Gang Omniscience)
> **Fecha:** 2026-09-07 · **Entregable máquina:** `CB-01-MODULES.json` (34 módulos, validado contra el contrato Zod de `ControlBoardLed.tsx`)
> **Verificación adversarial:** ver `CB-01-VERIFY.md` (mismo agente, método y límites declarados ahí).

## 0. Contexto de ejecución (fail-honest)

- **Estado de los inputs al iniciar:** ni CB-01 ni CB-02-DISENO existían (confirmado: solo
  `GOAL-WORKORDERS.md` + 3 reportes de pares — CB-02-RUST-APPLY §1, CB-05-DESIGN §Hallazgo-1
  lo declaran). Este censo se derivó de **fuentes primarias** (repo + VPS read-only), no de
  entregables previos.
- **Código desplegado:** VPS corre `main @ e65040f1` (merge PR #555, 2026-09-07 07:51-05:00 =
  12:51Z). El árbol local está en `feat/hops-live-01 @ 27aca289` (padre exacto del merge):
  `git diff --stat HEAD e65040f1 -- <archivos censados>` = **CERO diff** → todos los file:line
  de este censo son válidos contra el código desplegado.
- **Evento drift EN VIVO durante el censo:** la flota completa (24 contenedores) fue recreada
  a las **2026-09-07T13:20:06Z** (deploy de e65040f1) entre mis sondas 12:46-13:06Z y
  13:23-13:40Z. Los valores `.env` no cambiaron (mtime `.env` = 12:12:58Z, el flip RU-3 de
  CB-05); el boot 13:20Z confirmó los mismos estados. Esto es exactamente el drift que CB-04
  debe detectar — quedó registrado aquí como evidencia del caso real.
- **Stack corriendo:** `docker/compose.prod.yml` (label `com.docker.compose.project.config_files`
  del api-server). NOTA para la mesa: CLAUDE.md R3 y CB-05 §5 citan `compose.dev.yml` como
  archivo canónico de comandos — discrepancia real, ver VERIFY §Corrección-2.
- **VPS:** SOLO LECTURA (docker ps/inspect/logs, redis-cli GET/SCARD/EXISTS/scan, grep .env
  no-secreto, crontab -l, git rev-parse). Nada mutado. Secretos NO leídos.

## 1. Taxonomía (board GOAL-WORKORDERS.md:17)

| Clase | Definición | Criterio mecánico verificado |
|---|---|---|
| **A** runtime-pollable | el worker polla una clave Redis por tick/TTL | existe loop de lectura periódica en el consumidor (TTL cache / per-tick) |
| **B** boot-time env | leído UNA vez al arrancar (`std::env::var` / `process.env` en boot-path) | grep del consumidor: lectura única, sin re-read ni suscripción |
| **C** operator-gated §34.3 | LOCKED sin toggle: capital/broadcast/live-flip | gate físico (`live_exec_policy::assert_broadcast_allowed`, panic de boot) — jamás UI |

Regla de precedencia (CB-05 §9): si una var es A y B a la vez, gana A. Hallazgo estructural:
**la superficie A viva hoy es mínima** — `arbx:killswitch` (TTL 1s + pub/sub), `arbx:trading_config:<chain>` (TTL 1s + pub/sub), `arbx:aave_v3_watchlist:<chain>` (SMEMBERS por tick), `arbx:papermode:<chain>` (TTL 1s, pero clasificada C por ser terminus). Todo lo demás es B. Cuáles knobs B pasan a A es decisión de CB-02-DISENO (insumo: mapa de CB-02 §3).

## 2. Censo por pestaña (34 módulos — detalle completo en CB-01-MODULES.json)

### Tab `deteccion` (11)
| id | clase | control_key | estado verificado HOY | fuente |
|---|---|---|---|---|
| orchestrator_mode | B | `ARBX_ORCHESTRATOR_MODE` | **v2** (env+container; boot `worker_orchestrator.boot` 13:20:17Z) | scanner.rs:164-178 |
| cartridge_mode | B | `ARBX_CARTRIDGE_MODE` | **active** (≈shadow hoy por diseño; boot `cartridge.loaded` ×N) | cartridge_boot.rs:59-79 |
| route_scanner_multihop | B | `ARBX_ROUTE_SCANNER_MODE` | **on** (RU-3 encendido hoy 12:13Z; boot `route_scanner.mode mode=on dispatch_path=orchestrator` 13:20:25Z + runtime `route_scanner.done` ×136) | route_scanner_worker.rs:104-125,796-807; scanner.rs:960-973 |
| route_discovery_radar | B | `ARBX_ROUTE_DISCOVERY_MODE` | **shadow** (runtime `route_discovery.tick` vivo; SIN modo activo por diseño del enum) | route_discovery/mod.rs:46-78 |
| pool_enumeration | B | `ARBX_POOL_ENUM_MODE` | **shadow** (boot `poolenum.enabled` 13:20:25Z interval 1h top_n 500) | pool_enumeration_worker.rs:45,186-205 |
| native_engines | B | `ARBX_NATIVE_ENGINES` | **on** (ausente → default-on; runtime dex_arb_v3v3 observado) | scanner.rs:541-549 |
| mempool_mode | B | `ARBX_MEMPOOL_MODE` | **auto** (env; sin línea block_mode — consistente) | chain_client.rs:280-302 |
| legacy_triangular_worker | B | `ARBX_ENABLE_LEGACY_TRIANGULAR_WORKER` | **off** (boot `enabled=false default-off-post-phase-15` 13:20:17Z) | main.rs:253-258 |
| legacy_flashloan_worker | B | `ARBX_ENABLE_LEGACY_FLASHLOAN_WORKER` | **off** (idem) | main.rs:260-265 |
| legacy_liquidation_worker | B | `ARBX_ENABLE_LEGACY_LIQUIDATION_WORKER` | **off** (idem) | main.rs:267-272 |
| enabled_chains | B | `ARBX_ENABLED_CHAINS` | **1,10,42161,8453,137** (env+container) | shared-rs/src/config.rs:248-255; main.rs:752-760 |

### Tab `evaluacion` (7)
| id | clase | control_key | estado verificado HOY | fuente |
|---|---|---|---|---|
| canonical_knobs | B | `ARBX_KNOB_*` (53) | **defaults del workbook** (0 knobs en .env; boot `config.canonical_knobs` beam_k=4…; Redis key EXISTS) | canonical_knobs.rs:46+,214,345; main.rs:361 |
| trading_config_gate | **A** | `redis:arbx:trading_config:<chain>` | **enabled=true chain 1** (capital $1000; 5 estrategias). Cadenas 10/42161/8453/137 SIN clave → **IDLE** | trading_config.rs:22-36; trading-config.ts:9-10,67 |
| scoring_hard_gate | B | `ARBX_SCORING_HARD_GATE` | **false (dormido)** — clave ausente → advisory-only | scoring_pipeline.rs:92-93,278-288 |
| gate_macro_mev | B | `ARBX_GATE_MACRO_MEV_ENABLED` | **on** (=1); OJO: THRESHOLD/EPSILON con coma decimal (`1,1`/`0,01`) — HYPOTHESIS: caen al default por locale | gates/mod.rs:127-140 |
| sim_backend | B | `SIM_BACKEND` | **anvil** (revm exigiría REDIS_URL, sin fall-through) | sim-ctl/src/main.rs:655-666,399 |
| simulator_v2_ready | B | `ARBX_SIMULATOR_V2_READY` | **true** (gates relays+api-server; `ARBX_USE_SIMULATOR_V2` en searcher es INERT — se descarta) | g-sim-1.ts, wallet-sim-runtime.ts, live_exec_policy.rs:7; main.rs:443-449 |
| aave_watchlist | **A** | `redis:arbx:aave_v3_watchlist:<chain>` | **vacía** (SCARD=0 → skip honesto por tick) | liquidation_worker.rs:21,59-71 |

### Tab `terminus` (2 — C LOCKED §34.3, LED candado SIN toggle)
| id | clase | gate | estado verificado HOY | fuente |
|---|---|---|---|---|
| live_exec_policy | **C** | default-deny + `MainnetRefused` | **DENEGADO** (`ARBX_LIVE_EXEC_ENABLED=False` ≠ 'true' exacto; boot relays `live_exec.policy enabled=false allowed_chains=[11155111] chain_id=1 live_mode=false` 13:20:06Z) | live_exec_policy.rs:12,53-54,74-83 |
| paper_mode_terminus | **C** | terminus §34.3 (orden del operador) | **PAPER activo chain 1** (Redis `arbx:papermode:1 enabled:true` desde 2026-08-29; `ARBX_TRADE_MODE=paper`) | paper_mode.rs:23-36; index.ts:1293-1301 |

### Tab `infra` (9)
| id | clase | control_key | estado verificado HOY | fuente |
|---|---|---|---|---|
| service_control | B | `ARBX_SERVICE_CONTROL` | **on** (≠on→501) | service-control.ts:141-145 |
| scoring_archiver | B | `ARBX_SCORING_ARCHIVER_MODE` | **on** (boot `scoring_archiver.started` 13:20:08Z) | index.ts:1958-1961 |
| paper_archiver | B | `ARBX_PAPER_ARCHIVER_MODE` | **on** (boot `paper_archiver.started` + capital_seed 1000) | index.ts (~1958-1969, patrón par) |
| opps_bridge_archiver | B | `ARBX_OPPS_BRIDGE_MODE` | **off** | opportunities-bridge-archiver.ts:79 |
| ws_consumer_purge | B | `ARBX_WS_CONSUMER_PURGE_INTERVAL_MS\|_IDLE_MS` | **defaults 15/30min** (claves ausentes) | websocket.ts:821-822 |
| pool_sync | B | `POOL_SYNC_*` (8 knobs) | **defaults** (worker activo vía orchestrator multichain) | pool_sync_worker.rs; main.rs:801-808 |
| retention | B | `ARBX_RETENTION_ARCHIVE_DIR` | **default /app/archives + cron 04:17 diario pg_retention.sh + prune dom 05:23** (crontab verificado) | archive-control.ts:76 |
| config_reload_omni | B | (ninguno) | **BUILT-NOT-WIRED: no compila** (0 referencias externas; ni `mod` declarado) | config_reload_omni.rs:1-18; grep=0 |
| token_enricher_oracles | B | `ARBX_DEXSCREENER_ORACLE`\|`ARBX_GECKOTERMINAL_ORACLE` | **active ambos** | dexscreener.rs:76; geckoterminal_tier.rs:66 |

### Tab `seguridad` (4)
| id | clase | control_key | estado verificado HOY | fuente |
|---|---|---|---|---|
| kill_switch | **A** | `redis:arbx:killswitch` | **DISARMADO = detección ACTIVA** (`enabled:false` reason VER admin 12:53:29Z HOY). PROYECCIÓN invertida documentada para CB-02/03 | killswitch.rs:15-16,54-77; index.ts:254 |
| capital_key_lockout | **C** | invariante boot (FASE D) | **verificado** (boot `searcher.capital_lock` 13:20:06.607Z) | main.rs:309-336 |
| csp_headers | B | código next.config.js (NO env) | **DESCONOCIDO runtime** (Report-Only por código; clave `ARBX_CSP_ENFORCE` de la semilla NO EXISTE) | next.config.js:19-26; pr-1-csp.ts:15 |
| sybil_asn_denylist | B | `SYBIL_ASN_DENYLIST` (edge) | **off** (ausente en .env, grep -c=0) | edge/worker/src/index.ts |

### Tab `frontend` (1)
| id | clase | control_key | estado verificado HOY | fuente |
|---|---|---|---|---|
| frontend_edge_urls | B | `NEXT_PUBLIC_EDGE_URL`\|`NEXT_PUBLIC_WS_URL` | `https://edge-arbx.ape-tv.net` / `http://195.201.235.70` (build-time RULE 03; frontend+edge recreados 13:20:06Z por el deploy) | compose.dev.yml:337-348 |

## 3. Anexos (documentados en CB-01-MODULES.json → `annex`)

1. **Siempre-encendidos SIN switch** (LED informativo si CB-03 los quiere, no son
   controlables): price_worker, heartbeat, cex_dex, topology_reload, chain-config-reload,
   rpc_health/gas_oracle, block_scanner, latency series, alerts stack (prom+am+rules),
   math-engine (servicio, compose.prod.yml:185).
2. **Claves .env SIN consumidor (ruido RULE 00):** `ARBX_GATE_THRESHOLD_ENERGY`,
   `ARBX_MIN_PRICE_LIQUIDITY_USD` (solo passthrough compose.prod.yml:611). **Inert:**
   `ARBX_USE_SIMULATOR_V2` (parseado y descartado, main.rs:445-449). **Semilla inexistente:**
   `ARBX_CSP_ENFORCE` (0 hits repo + 0 en .env).
3. **Flota:** 24/24 healthy a las 12:46Z; recreate completo 13:20:06Z (deploy e65040f1);
   api `/health` 200 interno.

## 4. Hallazgos que el operador debe ver (resumen)

1. **RU-3 ya está ON y produciendo**: `route_scanner.done` con 500 ciclos/bloque, 371
   despachados (129 shadow-forced h6-8) — el objetivo del /goal está cumplido en runtime.
2. **4 de 5 cadenas económicamente IDLE**: `trading_config` solo existe para chain 1
   (trading_config.rs:17-20: sin fila → IDLE). Detección de 10/42161/8453/137 no tiene
   config económica — candidato a corrección operativa vía el futuro board.
3. **Kill-switch disarmado HOY 12:53:29Z** (reason "VER", admin) — evento pre-board que
   justifica el drift-guard CB-04.
4. **Dos claves de la semilla no existen tal cual** (`ARBX_CSP_ENFORCE`) o están muertas
   (`ARBX_GATE_THRESHOLD_ENERGY`) — el board debe mostrar la realidad, no la semilla.
5. **Locale-coma** en `ARBX_MACRO_MEV_THRESHOLD=1,1` / `EPSILON=0,01`: HYPOTHESIS (no
   verificado en runtime) de que caen al default del workbook por fallo de parse f64.

## 5. Reproducibilidad (comandos read-only de este censo)

```bash
# VPS (ssh arbx, SOLO LECTURA)
docker ps --format "{{.Names}}|{{.Status}}"
grep -E "^(ARBX_|SIM_|NEXT_PUBLIC_)" /opt/arbitragex-v2/.env | grep -viE "KEY|SECRET|TOKEN|PASSWORD"
docker inspect arbitragex-v2-searcher-rs-1 --format '{{range .Config.Env}}{{println .}}{{end}}' | grep ^ARBX_
docker exec arbitragex-v2-redis-1 redis-cli GET arbx:killswitch
docker exec arbitragex-v2-redis-1 redis-cli GET arbx:papermode:1
docker exec arbitragex-v2-redis-1 redis-cli GET arbx:trading_config:1
docker exec arbitragex-v2-redis-1 redis-cli SCARD arbx:aave_v3_watchlist:1
docker logs arbitragex-v2-searcher-rs-1 2>&1 | grep -m1 "route_scanner.mode"
docker logs arbitragex-v2-searcher-rs-1 2>&1 | grep -m3 "scanner.legacy_worker_state"
docker logs arbitragex-v2-relays-client-1 2>&1 | grep -m1 "live_exec.policy"
crontab -l | grep -E "retention|prune"
# Repo (local)
grep -rn "env::var(\"ARBX_" backend/searcher-rs/src/ --include="*.rs"
node frontend/.cb01-validate.mjs   # validación Zod de CB-01-MODULES.json (ver VERIFY §d)
```

> **Sin git:** cero commit/push/PR/deploy. Este archivo + CB-01-MODULES.json + CB-01-VERIFY.md

---

# MERGE FINAL (2026-09-07T13:55Z) — rust-topology-engineer, claim-owner CB-01

Dos censores paralelos convergieron en los mismos entregables (ecc:rust-reviewer CB-01-VERIFY 12:46-13:40Z
≈ 34 módulos; rust-topology-engineer 13:20-13:55Z ≈ 47 módulos). El registry final
`CB-01-MODULES.json` = **base del par (adoptada íntegra) + 3 correcciones + 11 módulos restaurados
del claim-owner = 44 módulos** (A=3, B=38, C=3). Todo lo no mencionado abajo queda TAL CUAL la
versión del par (verificada por ambos agentes de forma independiente).

## Correcciones sobre la base del par (con evidencia)

1. **Tabs español→inglés** (rompe-contrato): `ControlBoardLed.tsx:401-408` fija
   `CONTROL_BOARD_TAB_VALUES = [detection, evaluation, execution, infra, security, frontend]`;
   las tabs `deteccion/evaluacion/terminus/seguridad` del par mandaban TODOS los módulos al bucket
   «Sin clasificar» del board ya aplicado. Mapeo aplicado 1:1 en el JSON.
2. **control_key de claves dialecto JSON/SET → null** (riesgo de corrupción): el PUT aplicado de
   CB-02 escribe exactamente `"true"/"false"` (control-board.ts:530 + parseDeclaredValue :189-193).
   `redis:arbx:killswitch` (JSON `KillSwitchState`), `redis:arbx:trading_config:<chain>` (JSON) y
   `redis:arbx:aave_v3_watchlist:<chain>` (SET) quedan `null` + clave real en description: el PUT
   actual los rechaza en fail-safe (503, control-board.ts:476-481) y el toggle existente es el
   endpoint admin de cada uno (killswitch: /admin/killswitch; trading-config:
   /admin/trading-config/:chain_id; watchlist: operador directo en Redis). Escribir el dialecto
   true/false sobre `arbx:killswitch` rompería el parse y el fail-closed (killswitch.rs:72-77)
   haltearía todo el sistema. La proyección invertida del par (verde = detección activa) se CONSERVA.
3. **ARBX_CSP_ENFORCE refutado-complejo**: el par declaró «ESA CLAVE NO EXISTE (ni repo ni .env)» —
   **el consumidor SÍ existe**: `frontend/next.config.js:150-160` (WO-09 2026-09-06, exact-string
   `"true"`, fail-closed, header enforcing duplicado byte a byte, evaluado en `next build`). Lo que
   el pair VIÓ BIEN (y se conserva como `built_not_wired: true` de capa deploy): la clave está
   ausente de `.env` Y sin build arg en `compose.dev.yml:337-348` y `compose.prod.yml:443-449` → no
   puede llegar al builder hoy. Módulo renombrado `csp_enforce` con ambos hechos.

## Confirmaciones del claim-owner sobre hallazgos del par (segunda lectura independiente)

- **pool_enum default OFF** (solo `'shadow'` exacto spawnea — pool_enumeration_worker.rs:186-205):
  CONFIRMADO; refuta además la fila de CB-05 §1 que decía «default shadow».
- **config_reload_omni NO COMPILO**: lib.rs:195 declara `config_reload` (sin `_omni`) → ni módulo
  es. CONFIRMADO — refina CB-02-RUST-APPLY §3.3 («no spawn-eado» → «no declarado»).
- **Coma decimal** `ARBX_MACRO_MEV_THRESHOLD=1,1` / `EPSILON=0,01`: CONFIRMADO por lectura directa
  (.env 13:5xZ) + semántica determinística (gates/mod.rs:139-144 `.ok().and_then(parse)` → defaults
  del workbook efectivos). Sube de HYPOTHESIS a CONFIRMED.
- **deployed_sha e65040f1** (PR #555): CONFIRMADO (`git rev-parse` VPS) — explica el redeploy
  13:20:06Z observado en vivo por ambos agentes.

## Módulos restaurados del claim-owner (11, con evidencia propia; ver JSON)

`route_discovery_outcomes` (on=shadow) · `rd_outcome_sink` (ON, log 13:20:08Z — el feed shadow
real) · `paper_executor` (dormido, log 13:20:08Z) · `paper_auto_reconcile` (off) · `drift_tracker`
(dormido, log 13:20:17Z) · `edge_rate_limit` (600/min — semilla «rate limits») ·
`alertmanager_webhook` (montado incondicional — semilla «alertmanager/webhooks») ·
`token_safety_floor` (70 default) · `v3_arb_cartridge` (off por defecto, dex_arb.rhai:160) ·
`tls_hsts` (off) · `simulator_v2_dispatch` (refinamiento: `ARBX_USE_SIMULATOR_V2` inert en searcher
main.rs:465 `let _ =` — hallazgo del par — PERO con consumidor REAL en sim-ctl capabilities.rs:13,103;
sustituye al módulo `simulator_v2_ready` del par, cuya info vive ahora en la description).

## Vigencia de este documento

Las tablas §2-§4 de arriba son la versión del par (34 módulos) y quedan como registro histórico de
SU censo; el registry máquina de referencia es `CB-01-MODULES.json` (merge 44 módulos). Las
diferencias entre ambos están exhaustivamente enumeradas en esta sección — ninguna otra.

> son la entrega completa de CB-01 (+ verificación), bajo claim exclusivo del agente.
