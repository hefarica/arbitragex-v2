# CB-01 — Reporte de diseño + censo (rust-topology-engineer · Gang Omniscience · 2026-09-07)

**WO:** CB-01 · **kind:** design (censo read-only) · **Estado:** COMPLETO (merge final de 2 censores).
**Entregables:** `CB-01-CENSO.md` (censo humano: base ecc:rust-reviewer + MERGE FINAL) ·
`CB-01-MODULES.json` (registry máquina mergeado: **44 módulos**, shape `CensusModuleSchema` +
`tab` + campos completos `ControlBoardModuleSchema`) · este reporte.
**Ventana de verificación VPS:** 2026-09-07T12:46Z–13:55Z (ssh `arbx` SOLO lectura, dos agentes;
la flota fue redeployada 13:20:06Z — deploy `e65040f1` PR #555 — DURANTE los censos; todo
`verified_at` es posterior a ese boot).

## 0. Colisión de mesa resuelta (registro)

Dos censores paralelos aterrizaron los mismos archivos: **ecc:rust-reviewer** (CB-01-VERIFY,
12:46-13:40Z, 34 módulos) y este agente (claim-owner, 13:20-13:55Z, 47 módulos). Resolución
documentada en `CB-01-CENSO.md` §MERGE FINAL: se adopta la base del par íntegra (su linaje
deployed-SHA es superior), con **3 correcciones** (tabs español→inglés canónico de
`ControlBoardLed.tsx:401-408`; `control_key` de claves dialecto JSON/SET → null — riesgo de
corrupción bajo el PUT aplicado de CB-02; refutación parcial de su declaración
«ARBX_CSP_ENFORCE no existe») y **11 módulos restaurados** míos con evidencia propia. Resultado:
**44 módulos** validados (A=3, B=38, C=3). Ambas contribuciones quedan citadas en el JSON
(`census_meta.merge_record`).

## 1. Qué se entregó

Censo exhaustivo de **44 módulos/switches reales** — cada uno con nombre, qué hace, fuente de
verdad del estado HOY (file:line), clase A (runtime-pollable) / B (boot-time env) / C
(operator-gated §34.3 LOCKED) y estado verificado en VPS con evidencia concretada:

| Pestaña | Módulos | Contenido destacado |
|---|---|---|
| detection | 14 | orchestrator v2 · cartridges active · **route_scanner RU-3 ON y produciendo (500 ciclos/bloque, 371 despachados)** · route_discovery **shadow ON + PIN sin-Active** · outcomes emitter + sink ON · mempool auto · native engines on · pool_enum shadow · V3 off · 3 legacy off · enabled_chains 5 |
| evaluation | 7 | 53 knobs ARBX_KNOB_* (snapshot Redis vivo, beam_k=4) · **trading_config A runtime (chain 1 ON capital $1000; 4 cadenas IDLE)** · scoring hard-gate dormido · **gate macro-MEV con coma decimal (1,1/0,01 → defaults, CONFIRMADO)** · SIM_BACKEND anvil · SimulatorV2 dispatch · **watchlist A VACÍA (SCARD=0)** |
| execution | 4 | **terminus C LOCKED (barrera ARMADA, MainnetRefused)** · capital-key lockout C sellado · **paper_mode terminus C (chain 1 paper desde 08-29)** · executor paper dormido · reconcile off |
| infra | 12 | pool_sync 12s · rpc_health 15s · heartbeat 60s · WS purge defaults · **edge RL 600/min** · **drift_tracker dormido** · **alertmanager webhook montado** · archivers on · opps_bridge off · retention crontab activo · **OmniReloadCoordinator NO COMPILA (built-not-wired)** · token-enricher oracles active |
| security | 5 | **kill-switch A NO accionado (12:53:29Z, proyección invertida: verde = detección activa)** · capital lockout C · **CSP enforce OFF + built-not-wired al deploy** · sybil ASN off · token-safety floor 70 |
| frontend | 2 | EDGE/WS URLs horneadas (dominio público / bare-IP) · TLS/HSTS off |

Clases: **A=3** (kill_switch, trading_config_gate, aave_watchlist — las tres con `control_key: null`
por el gap de dialecto §0.1) · **B=38** · **C=3** (live_exec_policy, paper_mode_terminus,
capital_key_lockout). Los reloads pub/sub cableados (chains/topology) viven en `annex.always_on`.

## 2. Hallazgos centrales (novedad sobre la semilla del operador)

1. **HALLAZGO-CB01-1 — Gap de dialecto clase A** (CB-01-CENSO §MERGE, JSON `projections`): la
   superficie runtime REAL son 5 mecanismos (killswitch/papermode/trading_config con cache 1s TTL;
   chains/topology pub/sub), pero **NINGUNO habla el `true`/`false` que el PUT de CB-02 escribe**
   (control-board.ts:530). Si el censo nombrara `redis:arbx:killswitch` como control_key, un toggle
   corrompería el JSON y el fail-closed (killswitch.rs:72-77) haltearía todo el sistema. **Decisión
   del censo (merge):** las claves JSON/SET se publican con `control_key: null` (el PUT actual las
   rechaza en fail-safe, control-board.ts:476-481) y CB-02-DISENO debe definir el adaptador
   por-clave. El censo NO inventa claves que nadie pollaea (RULE 00).
2. **HALLAZGO-CB01-2 — Semilla del operador confirmada íntegra** y extendida: RU-3 `on` **y
   produciendo** (runtime `route_scanner.done` 500 ciclos/bloque, 371 despachados — evidencia del
   par, adoptada), radar `shadow` **con el pin honesto verificado en el tipo** (no existe Active;
   `"active"`→Off, mod.rs:68-70), `SIM_BACKEND=anvil`, kill-switch no-accionado (12:53:29Z razón
   VER), `ARBX_NATIVE_ENGINES` on-por-ausencia, purge defaults 15m/30m, terminus C LOCKED,
   `ARBX_SERVICE_CONTROL=on`, retention = crontab host activo, edge RL 600/min, alertmanager
   webhook montado.
3. **HALLAZGO-CB01-3 — Multichain desbalanceado:** `ARBX_ENABLED_CHAINS=1,10,42161,8453,137` pero
   solo chain 1 tiene `trading_config` (capital_usd 1000) y `papermode` — las otras 4 corren
   detección y quedan **IDLE explícito** en evaluación (trading_config.rs:17-20). Confirmado por
   ambos censores de forma independiente.
4. **HALLAZGO-CB01-4 — Construido-pero-no-cableado (la categoría del /goal):** (a)
   **OmniReloadCoordinator** — ni siquiera declarado como módulo (lib.rs:195 declara `config_reload`
   sin `_omni`) → NO COMPILO en el binario (refinación del par sobre CB-02-RUST-APPLY §3.3,
   confirmada por mí); (b) **ARBX_CSP_ENFORCE** — consumidor existe (next.config.js:150-160 WO-09)
   pero sin build arg en NINGÚN compose y ausente de `.env` → inalcanzable en el deploy actual
   (built-not-wired de capa deploy); (c) knob `beam_k` publicado (beam_k=4) sin consumidor hot-path
   (DFS congelado §37 Nivel-1) + `dirty_reeval`/`fe_prefilter` OFF idénticos a propósito.
5. **HALLAZGO-CB01-5 — Dormidos honestos** (existe el switch, hoy OFF): scoring_hard_gate,
   paper_executor, opps_bridge (legacy; reemplazado por rd_outcome_sink ON), drift_tracker,
   V3 arb cartridge, auto_reconcile, CSP enforce, TLS/HSTS, sybil ASN.
6. **HALLAZGO-CB01-6 — Locale-coma real:** `ARBX_MACRO_MEV_THRESHOLD=1,1` / `EPSILON=0,01`
   (coma ES) → parse f64 falla → **defaults del workbook efectivos** (gates/mod.rs:139-144).
   CONFIRMADO por segunda lectura (HYPOTHESIS del par → CONFIRMED). El operador debe reescribir
   con punto decimal si quiere esos valores.
7. **HALLAZGO-CB01-7 — Claves .env muertas (ruido RULE 00):** `ARBX_GATE_THRESHOLD_ENERGY`,
   `ARBX_MIN_PRICE_LIQUIDITY_USD` (solo passthrough compose) — 0 consumidores. Y
   `ARBX_USE_SIMULATOR_V2` es **inert en searcher** (main.rs:465 `let _ =` «no runtime flip yet»)
   aunque con consumidor REAL en sim-ctl (capabilities.rs:13,103).

## 3. Contrato del registry (shape EXACTA)

- **Fuente:** `CensusModuleSchema` de `backend/api-server/src/routes/control-board.ts:112-134`
  (el GET/PUT de CB-02 ya aplicado consume `arbx:config:control_board`) + campo `tab` de
  `frontend/components/ControlBoardLed.tsx:58` con los 6 valores canónicos `CONTROL_BOARD_TAB_VALUES`
  (:401-408). Zod `z.object` hace strip de claves desconocidas por defecto → `_meta` y `tab` viajan
  seguros para ambas puntas.
- **Convención `control_key`** (alineada con los fixtures de CB-03, ControlBoard.test.tsx:65,91,101):
  `env:VAR` (B) · `redis:arbx:clave` (A) · `gate:entidad` (C) · `null` = sin switch nombrable.
- **`declared_on` para B/C** = estado efectivo del env (lo que el operador declaró al desplegar);
  para A = `null` (el board lee el vivo desde la clave; control-board.ts:359).
- **`verified_on: null` = DESCONOCIDO** — en este censo ningún módulo quedó DESCONOCIDO salvo los
  declarados en CB-01-CENSO §8 (uso del canal de inyección, bucket audit del edge); los estados
  "vacío/dormido/off" son verificados con evidencia, jamás asumidos.

## 4. Diseño de publicación (diff EXACTO propuesto — NO aplicado, read-only)

El charter de CB-02 ya definió `arbx:config:control_board` como la clave del censo y espera un
"CB-01 census publisher". Este WO es read-only sobre el VPS → la publicación es un diff de apply
para CB-02-apply (o su merge point), NO ejecutado aquí. Diseño mínimo (mismo patrón que
`arbx:config:canonical_knobs` ← searcher boot):

1. Copiar el artifact versionado: `audits/control-board-2026-09-07/CB-01-MODULES.json` →
   `backend/api-server/config/control-board-census.json` (solo el objeto `{modules:[…]}`; `_meta`
   se puede conservar — Zod lo ignora).
2. En `backend/api-server/src/index.ts`, tras conectar Redis (sitio de montaje del control-board,
   junto a `mountControlBoard`):

```ts
// WO CB-01 (2026-09-07) — census publisher: publica el censo versionado a la clave que
// GET/PUT /api/v1/control-board consume (CONTROL_BOARD_CENSUS_REDIS_KEY). Fail-loud en log,
// nunca fabricado: si la lectura falla, la clave no se toca y GET sirve snapshot vacío honesto.
import { readFileSync } from "node:fs";
import { CONTROL_BOARD_CENSUS_REDIS_KEY } from "./routes/control-board.js";
// …tras `const redis = …` válido y antes de app.listen:
try {
  const census = JSON.parse(
    readFileSync("/app/repo/backend/api-server/config/control-board-census.json", "utf8"),
  );
  if (!Array.isArray(census.modules) || census.modules.length === 0) {
    throw new Error("census artifact sin modules — no se publica nada vacío sin causa");
  }
  await redis.set(CONTROL_BOARD_CENSUS_REDIS_KEY, JSON.stringify(census));
  logger.info({ event: "control_board.census_published", modules: census.modules.length });
} catch (e) {
  logger.error({ event: "control_board.census_publish_failed", err: (e as Error).message });
}
```

   (La ruta `/app/repo/...` existe por el mount `..:/repo:ro` de api-server, compose.dev.yml:298-302.
   Alternativa sin mount: bake del JSON en la imagen — decisión del apply.)
3. **Caducidad:** el censo es un snapshot clasificatorio — su re-publicación por boot es idempotente
   (SET completo). Los estados `verified_*` del censo son la línea-base de CB-04 (drift-guard);
   el board NO los reescribe en runtime: la frescura de clase A la da el GET en vivo
   (control-board.ts:333-348), y la de clase B la re-verifica CB-04 contra env/logs.

## 5. Invariantes (INV-CB01, inviolables)

- **INV-CB01-1 — El censo es el único clasificador.** PUT de CB-02 niega módulos fuera del censo
  (404, control-board.ts:438-441). Nadie togglea lo no clasificado.
- **INV-CB01-2 — DESCONOCIDO ≠ off.** `verified_on: null` se pinta ámbar (projectLed,
  ControlBoardLed.tsx:180-187). El censo jamás publica un `false` sin evidencia.
- **INV-CB01-3 — Prohibido nombrar `control_key` con dialecto incompatible.** Ninguna clave cuyo
  formato real ≠ string `"true"/"false"` puede ser `control_key` de clase A bajo el código actual
  (riesgo killswitch: corrupción JSON → fail-closed halt). Esas entradas van `null` + la clave real
  en `description` (el PUT actual 503 en fail-safe). Un adaptador JSON exige PR CB-02-DISENO con
  test de round-trip del formato del CONSUMIDOR (p.ej. `KillSwitchState` parsea tras escribir).
- **INV-CB01-4 — §34.3 candado absoluto.** `terminus_live_exec`/`capital_key_lockout`/`trade_mode_paper`
  clase C; ids y claves respetan la denylist terminus de control-board.ts:91-96. `default-deny` y
  `MainnetRefused` INTOCADOS.
- **INV-CB01-5 — Sin consumidor no hay entrada switch.** Toda entrada cita file:line del consumidor
  real (RULE 00); los casos construido-pero-no-cableado se censan como tales (off verificado por
  código), jamás como switches disponibles.
- **INV-CB01-6 — `tab` ∈ los 6 valores canónicos** de ControlBoardLed.tsx:401-408 (validado).
- **INV-CB01-7 — Cero secretos.** El censo transporta solo nombres de claves no-secretas y estados;
  valores `*KEY*/*SECRET*/*TOKEN*/*PASSWORD*` jamás leídos ni publicados.

## 6. Gates de este diseño

- **GATE-CB01-1 (corrido sobre el merge, PASS):** `CB-01-MODULES.json` parsea y pasa las
  constraints de `CensusModuleSchema` + `ControlBoardModuleSchema` + enum `tab` canónico:
  `SCHEMA-OK` — 44 módulos (merge: base 34 del par + 11 restaurados - 1 sustituido), tabs
  {detection 14, evaluation 7, execution 4, infra 12, security 5, frontend 2}, 0 duplicados,
  longitudes dentro de límites.
- **GATE-CB01-2 (corrido, PASS):** todo `verified_source` cita evidencia real con timestamp
  (log/env/docker/redis/crontab) — trazable en CB-01-CENSO §1-§6.
- **GATE-CB01-3 (corrido, PASS):** 0 secretos en censo/JSON (grep de patrones prohibidos = vacío).
- **GATE-CB01-4 (corrido, PASS):** alineación con pares — schema control-board.ts:112-134, tabs
  ControlBoardLed.tsx:401-430, id `route_scanner_multihop` = fixture CB-03.
- **GATE-CB01-5 (para el apply que publique):** tras `SET arbx:config:control_board`,
  `GET /api/v1/control-board` sirve 44 módulos; LEDs correctos por pestaña en browser (CB-06);
  PUT sobre clase C → 403; PUT sobre clase B → 409; PUT sobre clase A sin control_key → 503
  fail-safe. Ningún LED verde sin evidencia (muestreo CB-06).

## 7. Sincronía de mesa — construcción, colisión y diferencias

- **Construye sobre** CB-02-RUST-APPLY §3 (mapa poll/boot citado, no re-derivado) y CB-05-PROPUESTA-B
  §1 (clase B verificada 12:40Z — confirmada 1:1 y extendida; su fila «pool_enum default shadow»
  queda REFUTADA: default off, solo 'shadow' exacto spawnea).
- **Colisión resuelta con ecc:rust-reviewer (CB-01-VERIFY)**: ambos censamos en paralelo y ambos
  escribimos los entregables CB-01. Su base se adopta íntegra; 3 correcciones + 11 restauraciones
  mías (detalle en CB-01-CENSO §MERGE FINAL y `census_meta.merge_record`). Sus hallazgos únicos
  ADOPTADOS: deployed_sha e65040f1, proyección invertida del kill-switch, runtime `route_scanner.done`
  (RU-3 produciendo), `let _ =` inert de ARBX_USE_SIMULATOR_V2 en searcher, omni-no-compila, ruido
  ARBX_GATE_THRESHOLD_ENERGY/MIN_PRICE_LIQUIDITY_USD, oracles del token-enricher, sybil ASN,
  locale-coma. Su error REFUTADO con evidencia: «ARBX_CSP_ENFORCE no existe» (consumidor en
  next.config.js:150-160; su conclusión práctica —inalcanzable en deploy— se conserva).
- **CB-02-DISENO (aterrizó durante este WO)**: declaró `arbx:config:control_board` publicada por
  CB-01 y pidió reconciliación A>B cuando CB-01 aterrizara — este censo ES esa reconciliación.
  Su §8 (censo propio) y este registry difieren en detalles menores; la regla de precedencia la
  resuelve SU addendum. HALLAZGO-CB01-1 (gap de dialecto) es input directo de su adaptador.
  Su D5 ya corrigió a CB-05: la tabla es `audit_log` (SINGULAR, migración 011) — CB-05 consultó
  el plural; el CB-02-API-APPLY posterior creó 121 según el contrato real.
- **CB-02-API-APPLY (aterrizó)**: `control-board.ts` ya corre; la clave del censo NADIE la publica
  aún → GET sirve snapshot vacío honesto. El diff §4 de este diseño sigue siendo la propuesta de
  publicación (ajustar al import/estructura que CB-02-APPLY prefiera — p.ej. su archivo monta desde
  index.ts; la ruta /app/repo existe por el mount `..:/repo:ro`).
- **CB-03 (ControlBoardLed/page/tests)**: `tab` con SUS 6 valores canónicos (el motivo de la
  corrección 1 del merge); su tabla MAX_POTENTIAL_DEFAULTS decía «CB-01-CENSO no existe aún;
  reconciliar» — este censo ES la reconciliación (todo coincide; su fila killswitch «no censado en
  CB-05» queda cubierta con observación propia doble).
- **Contradicciones NINGUNA restante**; la única ampliación temporal: CB-05 verificó pre-redeploy
  (12:40Z) y los censores post-redeploy (13:20Z+), sin drift en lo solapante (`.env` mtime 12:12:58Z).

## 8. Verificación de esta sesión

- **GATE-CB01-1 re-corrido sobre el MERGE (PASS):** `CB-01-MODULES.json` parsea y pasa las
  constraints de `CensusModuleSchema` (control-board.ts:112-134) + `ControlBoardModuleSchema`
  completa (ControlBoardLed.tsx:43-68, incl. updated_at/last_reason/last_actor) + enum `tab`
  canónico: 44 módulos, tabs {detection 14, evaluation 7, execution 4, infra 12, security 5,
  frontend 2}, clases {A 3, B 38, C 3}, 0 duplicados, 0 claves faltantes.
- **GATE-CB01-3 re-corrido (PASS):** 0 secretos (scan de patrones hex/API-key/JWT = 0 hits en los
  3 archivos CB-01).
- VPS SOLO lectura: `docker ps/inspect/logs`, `redis-cli GET/EXISTS/SCARD/SCAN/TTL`,
  `grep` no-secreto `.env`, `crontab -l`, `git rev-parse`. **Cero mutación** (§32/§33).
- **Cero código de producción editado** (charter: kind design). Los archivos bajo claim
  (`canonical_knobs.rs`, `killswitch.rs`, `compose.dev.yml`, `ControlBoardLed.tsx`) solo se LEYERON
  (ControlBoardLed.tsx creció por CB-03 en paralelo; autoría CB-03 en su header).
- Presupuesto dominio público: **0 de 5** requests HTTP manuales (todo ssh read-only + repo local).
- NO-GIT respetado: cero commit/push/PR.

## 9. Fuera de alcance / next steps para la mesa

1. **CB-02-DISENO** debe resolver HALLAZGO-CB01-1 (adaptadores por-clase-A) — este censo es su insumo directo.
2. La publicación §4 es un diff propuesto; su apply + GATE-CB01-5 browser-verify es CB-02-apply/CB-06.
3. Re-verificación periódica de `verified_*` (frescen): ownership CB-04 (drift-guard) — el censo es
   la línea-base declarada, no un feed vivo.
