# CB-01 — CROSS-EXAMINATION (par adversarial, otra sesión)

> **WO:** CB-01-CROSS · **kind:** cross-exam · **agente:** cross-examiner par (Gang Omniscience)
> **Fecha:** 2026-09-07 (posterior a CB-01-DESIGN/CENSO/VERIFY/VERIFY-VERIFY) · **Objeto:** el
> entregable completo de CB-01 (`CB-01-CENSO.md` + MERGE FINAL, `CB-01-MODULES.json` 44 módulos,
> `CB-01-DESIGN.md`, `CB-01-VERIFY.md` + ADDENDA §6, `CB-01-VERIFY-VERIFY.md`).
> **Método:** re-derivación INDEPENDIENTE — lo que el propio CB-01-VERIFY §0 pidió ("un par
> posterior en otra sesión re-corre §5"). Read-only total: repo local + 1 viaje ssh `arbx`
> (docker/redis/git read-only). 0 requests dominio público. 0 git.

## 1. Veredicto: SUSTANCIA CONFIRMADA — quedan gaps de repro/documentación (§4)

El censo **sobrevive la refutación en todos los ejes sustanciales**. Cada intento mío de
encontrar datos fabricados, citas falsas o regressions falló con evidencia en contra:

### a. Re-validación del JSON contra los consumidores VIVOS (la del agente no era re-ejecutable — ver gap G-1)

Script propio (inline, sin archivos nuevos): **44 módulos · tabs {detection 14, evaluation 7,
execution 4, infra 12, security 5, frontend 2} = 6/6 canónicos sin huérfanos · clases {A 3,
B 38, C 3} · 0 ids duplicados · 0 errores de shape contra `CensusModuleSchema`
(control-board.ts:126-148) y `ControlBoardModuleSchema` (ControlBoardLed.tsx:44-77) · clase A
3/3 `control_key:null` (dialecto-seguro) · clase C 3/3 `gate:` · 44/44 con `evidence.parse`
Y `evidence.vps` · 0 patrones de secretos · 0 `verified_on:null` (todo verificado o
verificado-falso CON evidencia)**. Coincide 1:1 con GATE-CB01-1 del claim-owner.

### b. Los 11 módulos "restaurados" del merge — NUNCA re-verificados por nadie (ADDENDA §6 lo admitía) — ahora verificados por mí

9/11 con cita file:line exacta contra el árbol: `route_discovery_outcomes`
(cartridge_boot.rs:377-384 gate {shadow,on,1,true} default off; :460-464 V2_SCHEMA) ·
`rd_outcome_sink` (route-discovery-outcome-sink.ts:17,43 + montaje index.ts:1925-1936) ·
`paper_executor` (index.ts:1904-1915, == 'on' case-insens default dormant) ·
`paper_auto_reconcile` (paper-mode-reconcile.ts:49-53 ≠'on' → 503 reconcile_disabled) ·
`drift_tracker` (recon/main.rs:337-339 env default vacío) · `edge_rate_limit`
(edge/index.ts:59-68 Math.max(120,env)) · `token_safety_floor`
(pre_execute_checklist.rs:463-467 default 70) · `v3_arb_cartridge` (dex_arb.rhai:160
"Default OFF. Until an operator sets ARBX_V3_ARB_MODE=on…") · `tls_hsts`
(next.config.js:162-168). + `alertmanager_webhook` (alertmanager-webhook.ts:1-6,88-92;
montaje index.ts:131,690 — OJO: escribe `audit_log` SINGULAR migración 011, no
`audit_logs` 121; el JSON lo describe correctamente) y `simulator_v2_dispatch`
(capabilities.rs:13,49-52 — ver G-4).

### c. Refutaciones del merge — las tres son REALES

1. tabs inglés canónicos: ControlBoardLed.tsx:401-408 exacto (+UNCLASSIFIED_TAB :411-414) ✓
2. dialecto true/false del PUT: `await redis.set(controlKey, on ? "true" : "false")`
   (control-board.ts:591 hoy; era :530 pre-CB-04) + parseDeclaredValue estricto + 503
   fail-safe clase A sin control_key ✓ — el riesgo de corrupción del killswitch JSON
   (killswitch.rs is_enabled fail-closed default_when_absent) era REAL y el null lo evita ✓
3. CSP: consumidor `ARBX_CSP_ENFORCE === "true"` exact-string en next.config.js:150-160
   (WO-09) ✓ Y sin build arg en compose.dev.yml:337-348 NI compose.prod.yml:443-457 (args =
   NEXT_PUBLIC_EDGE_URL/WS_URL/WALLETCONNECT/ARBX_TLS_ENABLED únicamente) → built-not-wired
   de capa deploy ✓. La refutación al hallazgo "no existe" del par era correcta.

### d. Refutaciones a TERCEROS pares — también reales

- pool_enum default OFF: pool_enumeration_worker.rs:186-205 solo `'shadow'` EXACTO spawnea
  (eq_ignore_ascii_case) ✓ → CB-05 §1 "default shadow" refutado correctamente.
- `config_reload_omni` no compila: lib.rs:195 declara `pub mod config_reload;` (sin `_omni`) ✓.
- route_discovery NO-ACTIVE pin: mod.rs:50-52 enum Off|Shadow SIN Active; `"active"`→Off
  (:68-72) ✓ — "arquitectura radar, sin modo activo por diseño" es tipográficamente cierto.

### e. VPS re-observado por mí (read-only, 2026-09-07 tarde)

`git rev-parse HEAD` VPS = **e65040f124e7405bd98755fed570696d681a948a** (exacto al claim) ·
`.env`: `ARBX_MACRO_MEV_THRESHOLD=1,1` / `EPSILON=0,01` **coma real** ·
`ARBX_GATE_THRESHOLD_ENERGY=10` y `ARBX_MIN_PRICE_LIQUIDITY_USD=10000` (muertas) ·
`ARBX_USE_SIMULATOR_V2=true` **duplicada 2×** (el censo lo notó) · `ARBX_ROUTE_SCANNER_MODE=on`
· `ARBX_ORCHESTRATOR_MODE=v2` · `ARBX_POOL_ENUM_MODE=shadow` · `grep -c ARBX_CSP_ENFORCE=0` ·
redis `arbx:killswitch` = `{"enabled":false,"reason":"VER","triggered_by":"admin",
"updated_at":"2026-09-07T12:53:29.152Z"}` **byte-exacto** · `EXISTS arbx:config:control_board`
= **0** (censo aún sin publicar — consistente con CB-04 "census_absent") ·
`EXISTS arbx:config:canonical_knobs` = 1 · contenedor sim-ctl SÍ tiene
`ARBX_USE_SIMULATOR_V2=true` (via env_file ../.env) · relays boot: `live_exec.policy
enabled=false allowed_chains=[11155111] chain_id=1 live_mode=false` con "mainnet refused" —
**default-deny y MainnetRefused VIVOS e INTACTOS**.

### f. No-regresión y charter

`git status`: los 4 archivos bajo claim (`canonical_knobs.rs`, `killswitch.rs`,
`compose.dev.yml`, `ControlBoardLed.tsx`) diff **CERO** vs HEAD (solo lectura, como declara
CB-01-DESIGN §8) · `git diff --stat 27aca289 e65040f1 -- <subárboles censados>` = **vacío**
(el claim "diff 0 contra el desplegado" es cierto) · `git log --all --since=12:00Z hoy` =
**vacío** (cero commits del gang — NO-GIT respetado) · §34.3 intocado (ver e) ·
RULE 00/R8: 0 estados fabricados en mi muestreo (~35 citas + 12 hechos VPS); los negativos
están declarados como negativos con evidencia.

### g. Sincronía de mesa

Correcta y ejemplar: refuta a pares POR NOMBRE (CB-05 §1, CB-02-RUST-APPLY §3.3) con evidencia;
adopta del par por nombre (deployed SHA, proyección kill-switch, RU-3 runtime); la cadena de
auto-corrección CSP (par yerra → claim-owner refuta → par acepta con re-verificación propia)
queda documentada en ambas direcciones. CB-02-API/CB-03/CB-04 construyeron sobre CB-01 sin
contradicciones no nombradas. No encontré trabajo ajeno duplicado sin cita.

## 2. Lo que REFUTA mi examen (nada sustancial) — veredicto §1

Ningún hallazgo mío invalida el censo. Los gaps siguientes son de higiene de repro/documentación
(agent-fixable) o decisiones que exceden CB-01 (operator-gated).

## 3. GAPS (con clasificación)

| # | Gap | Clase | Fix |
|---|---|---|---|
| G-1 | **Reproducibilidad rota**: CENSO §5:143 cita `node frontend/.cb01-validate.mjs` — el archivo **NO EXISTE** (script transitorio de la sesión, borrado). El propio VERIFY §0 pide "re-corre §5" a un par: el paso de validación NO es re-ejecutable como está escrito. | agent-fixable | Recrear el validador dentro de `audits/control-board-2026-09-07/` (versionado con el entregable) o incrustar el snippet inline en §5; re-correr. Mi re-validación (§1a) ya demuestra PASS 44/44 — el gap es de repro, no de datos. |
| G-2 | **Header stale de CENSO.md**: línea 4 dice "CB-01-MODULES.json (34 módulos, validado contra el contrato Zod de ControlBoardLed.tsx)" — el JSON mergeado tiene 44 y el contrato de validación declarado en la ADDENDA es `CensusModuleSchema` (control-board.ts). Las tablas §2 (tabs español, `sim_backend :399`) son "registro histórico" por el MERGE, pero el header induce a error en primera lectura. | agent-fixable | Un párrafo-banner al tope de CENSO.md: "el registry vigente es el JSON de 44; §2 es histórico". |
| G-3 | **Referencia colgante en CB-01-DESIGN §3**: "salvo los declarados en CB-01-CENSO §8 (uso del canal de inyección, bucket audit del edge)" — CENSO **no tiene §8** y el JSON tiene 0 módulos con `verified_on:null`. Resto del borrador de 47 del claim-owner, irresoluble para un lector de la mesa. | agent-fixable | Eliminar la cláusula o corregir la referencia (no hay excepciones: 0 DESCONOCIDO en el registry). |
| G-4 | **Comentario stale detectado (no es error del censo)**: `sim-ctl/src/capabilities.rs:50-53` dice "Compose must pass ARBX_USE_SIMULATOR_V2 to BOTH services — today it does NOT" — FALSO hoy: `env_file: ../.env` lo pasa y el contenedor sim-ctl LO TIENE (verificado por mí y por el censo). El próximo agente que lea el comentario desconfiará del censo correcto. | agent-fixable | El censo (o el dueño de sim-ctl) debe anotar la contradicción comentario↔runtime; el fix del comentario es un diff marcado de 1 línea en capabilities.rs. |
| G-5 | **Denominador de flota inconsistente**: CENSO §3 "24/24 healthy 12:46Z" vs JSON annex "24/25 (geth ausente por diseño)". Dos ventanas, dos denominadores, delta sin explicación. | agent-fixable | Una línea en el annex explicando el cambio de denominador (pre/post recreate 13:20Z). |
| G-6 | **Coma decimal ES en `.env`** (`ARBX_MACRO_MEV_THRESHOLD=1,1`, `EPSILON=0,01`): CONFIRMADO por mí (lectura directa + gates/mod.rs:139-146 `.ok().and_then(parse)` → defaults del workbook efectivos). El operador escribió valores que NO están vigentes. | **operator-gated** | Reescribir con punto decimal en VPS `.env` (+recreate searcher) — decisión y acción del operador; el censo solo lo reporta. |
| G-7 | **4/5 cadenas económicamente IDLE** (trading_config solo chain 1; `ARBX_ENABLED_CHAINS=1,10,42161,8453,137`): confirmado por dos censores + código (trading_config.rs:15-20 "treats that chain as IDLE"). | **operator-gated** | Crear filas trading_config para 10/42161/8453/137 (config de detección, NO flip §34.3 — pero es decisión del operador con capital/estrategias por cadena). |
| G-8 | **Discrepancia de doctrina compose**: flota corre `compose.prod.yml` (label, doblemente observado) pero CLAUDE.md R3 y CB-05 §5 emiten comandos contra `compose.dev.yml`. | **operator-gated** | Decidir el archivo canónico de comandos (o documentar el par dev=local/prod=VPS); afecta cada propuesta clase B futura. |
| G-9 | **El censo sigue SIN publicarse** (por diseño: CB-01 read-only; `EXISTS arbx:config:control_board`=0 verificado): el board sirve snapshot vacío honesto y CB-06 no puede browser-verificar LEDs reales hasta que el publisher §4 de CB-01-DESIGN aterrice por pipeline aprobado. Nota adjunta: coexisten DOS ledgers de auditoría (`audit_log` migración 011, usada por alertmanager-webhook, vs `audit_logs` migración 121, usada por CB-02/CB-04) — reconciliación pendiente de la mesa. | **operator-gated** | Aprobar PR de publicación (gobernanza §37/P-∅) + decidir la reconciliación de ledgers. |

## 4. Preguntas al operador

1. ¿Reescribir `ARBX_MACRO_MEV_THRESHOLD`/`EPSILON` con punto decimal (G-6) o dejar defaults del workbook y eliminar las claves muertas?
2. ¿Poblar `trading_config` para chains 10/42161/8453/137 (G-7) o declararlas IDLE-por-diseño en el board?
3. ¿Compose canónico de comandos: prod (lo que corre) vs dev (lo que cita la doctrina R3) — G-8?
4. ¿Autorizar la publicación del censo (CB-01-DESIGN §4 + GATE-CB01-5) para habilitar CB-06?

> Cross-exam read-only: repo + 1 ssh (docker inspect/logs, redis GET/EXISTS, git rev-parse,
> grep .env no-secreto). Cero mutación VPS, cero git, cero dominio público, cero capital.
