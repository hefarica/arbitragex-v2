# CB-02-DISENO — Control Plane Runtime clase A (diseño del WO CB-02)

> **WO:** CB-02 · **kind:** design · **agente:** ecc:code-architect (Gang Omniscience) · **fecha:** 2026-09-07
> **Este documento es el desbloqueante directo del apply Rust** (CB-02-RUST-APPLY.md §1 fue NO-OP
> válido porque este diseño no existía; su §3 es el mapa de referencia sobre el que se construye).
> **Cero código editado, cero git** (NO-GIT operador 2026-08-23). Todo diff abajo es ESPEC
> EXACTA para el apply (fase posterior), con anclas file:line verificadas HOY.

---

## 0. Estado de los inputs (fail-honest, RULE 00)

| Input requerido | Estado al redactar (2026-09-07 ~14:00Z) |
|---|---|
| `CB-01-CENSO.md` / `CB-01-MODULES.json` | **AUSENTES** — `ls audits/control-board-2026-09-07/` solo contiene GOAL-WORKORDERS.md, CB-02-RUST-APPLY.md, CB-05-PROPUESTA-B.md, CB-05-DESIGN.md. **Dependencia declarada:** este diseño usa los insumos fallback (CB-02-RUST-APPLY.md §3 + CB-05-PROPUESTA-B.md §1) MÁS un censo propio file:line (§8) y verificación read-only VPS (§13). Cuando CB-01 aterrice, reconciliar por la regla de precedencia A>B de CB-05 §9: si CB-01 clasifica A algo que aquí es B, gana CB-01 y este diseño emite addendum. |
| Contrato cliente ya construido | **VERIFICADO COMPLETO** (§9): `frontend/components/ControlBoardLed.tsx` (Zod ControlBoardModule/Snapshot, fetchControlBoard, putControlBoardToggle) · `edge/worker/src/index.ts:1586-1595` (GET/PUT `/api/v1/control-board` via adminProxy, sin cache) · `frontend/lib/admin-token.ts` (V-AT-1, cookie httpOnly). |

**Presupuesto dominio público:** 0 de 5 requests HTTP manuales (solo lectura de repo local + ssh
`arbx` read-only: psql SELECT information_schema, redis-cli GET/SCAN).

---

## 1. D1 — Conjunto EXACTO de claves Redis runtime-config clase A + convención

### 1.1 Por qué tan pocas claves NUEVAS

El operador pidió "todas las más posibles". El diseño responde con TODAS las que pueden ser
runtime-toggleables **sin violar** §34.1 (mode-invariance), §37 Nivel-1 (route-discovery
congelado), hot-path cero-allocs (lectura Arc) ni R8/LOGFLOOD-01 — y enlaza el resto (§1.4).
El PUT del contrato ya construido es **booleano** (`{id, on, reason}` — ControlBoardLed.tsx:141):
todo knob numérico/string queda fuera por contrato, no por omisión.

### 1.2 Clases de claves (convención de namespace)

| Clave | Tipo | Escrita por | Leída por | Semántica |
|---|---|---|---|---|
| `arbx:killswitch` | JSON string (EXISTE, viva en VPS: `{"enabled":false,"reason":"VER",...}` leída 2026-09-07) | api-server `/admin/killswitch` (index.ts:254-278) **y ahora también el board** (mismo writer proceso, misma clave — no se duplica superficie) | KillSwitchClient Rust TTL 1s (killswitch.rs:79-104); TS client (index.ts:56-62) | `enabled=true` = detención armada (semántina INVERTIDA vs LED "encendido" — §8 fila 1) |
| `arbx:controlboard:route_scanner` | string `"true"`\|`"false"` (**NUEVA**; formato adjudicado R2 §15 — no JSON) | SOLO el PUT del board (api-server) | RuntimeToggleClient Rust del worker (poll TTL-cacheado 1s) | `"true"` = el worker spawned ESCANEA; ausente/valor ajeno → default fail-safe = modo boot (`ARBX_ROUTE_SCANNER_MODE`) |
| `arbx:controlboard:route_scanner:hb` | JSON string + **SETEX 75s** (**NUEVA**) | worker, 1 write por bloque (~12s) | GET del board (lado VERIFICADO) | `{state:"run"\|"halted", block, ts}` — TTL 75s ≈ 6 bloques de gracia; expirada → verified_on=null (DESCONOCIDO, R8) |
| `arbx:controlboard:approved` | HASH (**NUEVA**) campo por module_id | SOLO el PUT del board | GET del board + CB-04 (drift-guard/revert) | último valor APROBADO por el operador = registro de aprobación (GOAL-WORKORDERS.md:47) |
| `arbx:config:boot_census` | JSON string (**NUEVA**) | searcher-rs al boot (main.rs, §3.2) | GET del board (lado DECLARED de clase B + defaults clase A) | snapshot de env-derivados del boot (§3.2 lista exacta) |
| `arbx:config:canonical_knobs` | JSON string (EXISTE, main.rs:373-377) | searcher boot | GET del board (fila agregada 53 knobs) | declarativo-only (canonical_knobs.rs:19-24) |
| `arbx:config:control_board` | JSON census (NUEVA, la publica CB-01 — NO CB-02) | census publisher CB-01 | GET del board: identidad/clase/declared-B (adoptado R3 §15) | ausente → snapshot vacío honesto (charter); el §8 de este doc = contrato de contenido |
| ~~`arbx:controlboard:changes`~~ | canal pub/sub — **DEFERIDO (R7 §15)** | — | — | sin subscriber hoy = especulativo (P-∅); el poll TTL 1s es la fuente de verdad; PUBLISH se añade con el subscriber Rust |

### 1.3 Convención del patrón (clon del kill-switch §3.1 de CB-02-RUST-APPLY)

1. **Poll perezoso TTL-cacheado 1s** (killswitch.rs:61,79-104): máx 1 GET/s por worker, lectura
   devuelve clon de un `RuntimeToggleState` pequeño (POD) — cero allocs en el camino caliente
   cuando el cache está fresco (el miss de 1s hace 1 GET Redis + 1 clon: presupuesto idéntico al
   kill-switch ya aceptado en el tick loop, scanner.rs:1064).
2. **Fail-safe clave-ausente = comportamiento desplegado**: `default_when_absent` se resuelve al
   valor que el env/boot determinó (`RouteScannerMode::from_env()`). Ausente/garbage/Redis caído →
   el worker sigue en su estado boot. JAMÁS se fabrica estado (R8).
3. **Pub/sub: DEFERIDO en v1 (R7 §15)** — publicar sin subscriber es especulativo (P-∅). El
   poll TTL 1s es la fuente de verdad. Cuando el subscriber Rust exista (mismo PR del wiring
   §3), se añade `PUBLISH arbx:controlboard:changes` al PUT (precedente:
   `subscribeChanges()` con fallback a TTL poll, api-server index.ts:60-62).
4. **Un solo writer por clave**: `arbx:controlboard:*` lo escribe EXCLUSIVAMENTE el endpoint PUT
   del api-server (sesión admin-token via adminProxy del edge). Ningún worker, script ni agente
   escribe (soberanía, GOAL-WORKORDERS.md:43-47). `arbx:killswitch` conserva además su writer
   legacy `/admin/killswitch` — misma clave, ambos caminos auditados (§4.4).
5. **Heartbeat ≠ toggle**: el toggle declara intención aprobada; el heartbeat da el VERIFICADO.
   LED verificado siempre del heartbeat/live-key — nunca del declarado (ControlBoardLed.tsx:176-187
   `projectLed` ya lo exige).

### 1.4 Superficies runtime YA existentes (clase A por mecanismo, ENLAZADAS — cero claves nuevas)

Verificadas vivas en el edge (edge/worker/src/index.ts): cartridges pause/resume (:1600-1601),
math operator toggle (:1602), rpcs import/reload (:1603-1604), rpc-backend (:1608-1609),
service-control start/stop + readiness (:1553-1584), paper-mode (:1606), chains admin
(:1500-1547). En Rust: chains reload spawn-eado (config_reload.rs, main.rs:434), topology
hot-reload (topology_reload.rs, main.rs:390-397). El board las muestra como filas clase A con
`control_key` del estilo `api:/api/cartridges/runtime` (§8) y las ENLAZA (telemetry_href) — no
duplica sus toggles. NOTA CB-01: `config_reload_omni.rs` (11 canales arbx:config:*) está
construido-pero-no-cableado (CB-02-RUST-APPLY §3.3) — encenderlo es decisión separada, NO de este WO.

---

## 2. D2 — Qué queda clase B (boot-time) y por qué

### 2.1 Los 53 CanonicalKnobs: TODOS quedan clase B (0 promociones a runtime)

`CanonicalKnobs::from_env()` se lee UNA vez en producción (main.rs:361; la otra llamada real es
route_discovery_worker.rs:767) y se publica como snapshot (main.rs:373-377). Ningún knob tiene
re-lectura runtime ni consumidor de reload (CB-02-RUST-APPLY §3.2 — confirmado). Promociones
rechazadas por constraint:

| Grupo de knobs | Constraint que bloquea la promoción a clase A |
|---|---|
| `max_hops`, `min_hops`, `beam_k`, `enable_{2v2,v2v3,triangular,nhop,bfm,mmbf,johnson,bounded_dfs,rich,convex_size}`, `route_capacity`, `strict_*` | **§37 Nivel-1: route-discovery CONGELADO** — cambiarlos runtime cambia el route-set (canonical_knobs.rs:71-79 documenta el precedente `beam_k`: requiere PR propio gated). Además el PUT es booleano y estos no lo son. |
| `min_net_bps`, `quote_w_*` (suma=1.0), `rank_*_weight` (suma=1.0), `min_ev_usd`, `risk_haircut_pct`, etc. | Numéricos con invariantes cruzados (`validate()` canonical_knobs.rs:400-415,473-480) — el contrato `{on:bool}` no los expresa; un toggle runtime parcial violaría la suma=1.0. |
| `execution_mode`, `selected_execution_mode`, `killswitch`, `enable_observe_only` | **DECLARATIVOS-ONLY por doctrina** (canonical_knobs.rs:19-24): la autoridad de modo es `relays-client::live_exec_policy` §34.3 y el kill-switch real. Runtime-flipearlos recrearía semántica por modo (viola §34.1). |
| `dirty_reeval_enabled`, `fe_prefilter_enabled` | GATED knobs con **0 consumidores hot-path** (canonical_knobs.rs:80-93): promoción = escribir el consumidor = PR propio, fuera de CB-02. |
| budgets (`emission/candidate/cpu_op`, `block_cadence_s`, `discovery_sla_ms`, `max_state_age_blocks`, `max_freshness_s`, …) | Boot-time por mecanismo; su consumo vive en caminos ya construidos alrededor de un snapshot inmutable. |

**Superficie en el board:** UNA fila agregada `canonical_knobs` (§8 fila 16) con
`control_key: "redis:arbx:config:canonical_knobs"` y enlace a `GET /api/v1/config/canonical-knobs`
(canonical-knobs.ts) — NO 53 LEDs (ruido); el detalle vive en la superficie existente.

### 2.2 Gates de workers y flags: clase B con cita del parse

Todos boot-time por mecanismo (verificado §13): `ARBX_ROUTE_SCANNER_MODE` (spawn,
route_scanner_worker.rs:105-132,808-810) · `ARBX_ORCHESTRATOR_MODE` / `ARBX_CARTRIDGE_MODE`
(§34.2: flags de migración — runtime-flipearlos crea semántica por modo, PROHIBIDO) ·
`ARBX_MEMPOOL_MODE` (chain_client.rs:280-302) · `ARBX_NATIVE_ENGINES` (scanner.rs:541-548 →
OrchestratorContext:562) · `ARBX_POOL_ENUM_MODE` (spawn gate scanner.rs:535) ·
`ARBX_SCORING_{ENABLED,HARD_GATE}` (scoring_pipeline.rs:48-49; boot via opportunity_emitter.rs:144,173) ·
`ARBX_GATE_MACRO_MEV_ENABLED` (gates/mod.rs:127-140) · `ARBX_ROUTE_DISCOVERY_OUTCOMES`
(cartridge_boot.rs:377-384) · `SIM_BACKEND` (sim-ctl capabilities.rs:37-52, consumer.rs:421) ·
`ARBX_SERVICE_CONTROL` + `ARBX_SCORING/PAPER_ARCHIVER_MODE` (api-server, self-report §4.3) ·
`ARBX_WS_CONSUMER_PURGE_{INTERVAL,IDLE}_MS` (websocket.ts:821-822, consts a module-load) ·
`ARBX_CSP_ENFORCE` (edge; sin censo edge en v1 → declared honeste null).

**Excepción deliberada (la única promoción A):** el gate de EJECUCIÓN por bloque del
route_scanner (RU-3) — no el algoritmo (§37 intacto: el DFS no cambia, solo se omite su
invocación). Es exactamente el caso de dolor del operador (GOAL-WORKORDERS.md:9-11) y hoy está
`on` en VPS (CB-05 §1 hallazgo, log 12:13:15Z).

---

## 3. D3 — Sitios de wiring Rust (file:line exactos — insumo directo del apply)

### 3.1 NUEVO `backend/shared-rs/src/control_board.rs` (killswitch.rs NO se toca)

Clone estructural del patrón killswitch.rs:45-130 con:

```text
pub const CONTROL_BOARD_PREFIX: &str = "arbx:controlboard:";
pub const APPROVED_HASH: &str = "arbx:controlboard:approved";

// Valor de la clave = string "true"|"false" estricto (formato adjudicado R2 §15 — el board
// es el único writer; la riqueza reason/actor/updated_at vive en audit_log + approved hash).
// El cliente cachea (bool, Instant): lectura caliente = copia de un bool, cero parse.

pub struct RuntimeToggleClient { module_id: String, mgr: ConnectionManager,
    default_when_absent: bool,
    cache: Arc<RwLock<Option<(bool, Instant)>>>, cache_ttl: Duration /* 1s */ }

impl RuntimeToggleClient {
    pub async fn from_manager(mgr: ConnectionManager, module_id: &str,
                              default_when_absent: bool) -> Self          // reusa la mgr YA pasada al spawn
    pub async fn state(&self) -> Result<bool, ...>   // GET TTL-cacheado 1s; "true"→true, "false"→false,
                                                     // ausente/valor ajeno → default_when_absent (R8/R2:
                                                     // jamás interpreta un valor que no escribió el board)
    pub async fn is_on(&self) -> bool               // fail-safe → default_when_absent (killswitch.rs:72-77)
    // SIN set() en Rust: el ÚNICO writer es el api-server (§1.3-4). El cliente Rust es read-only.
    // SIN PUBLISH/SUBSCRIBE en v1 (R7 §15): el poll TTL 1s es la fuente de verdad.
}
```

- Registro en `backend/shared-rs/src/lib.rs` (export `control_board` junto a `killswitch`).
- Tests espejo de killswitch (TTL cache, ausente→default, garbage→default) — el módulo de tests
  de canonical_knobs.rs:748+ es el precedente de co-localización.
- **`backend/shared-rs/src/killswitch.rs` queda INTACTO** (claim protegido; cero diffs).

### 3.2 `backend/searcher-rs/src/main.rs:361-384` — publish del boot census

Dentro del mismo bloque `{}` del canonical-knobs publish, después de :383, añadir (mismo patrón
SET + warn no-fatal :378-383):

```text
SET arbx:config:boot_census  {"published_at": <ISO>,
  "arbx_route_scanner_mode": "on"|"off",        // RouteScannerMode::from_env() (route_scanner_worker.rs:114)
  "arbx_orchestrator_mode":  <OrchestratorMode>, // scanner.rs:164-178
  "arbx_cartridge_mode":     <CartridgeMode>,    // cartridge_boot.rs:66
  "arbx_mempool_mode":       <MempoolMode>,      // chain_client.rs:280-302
  "arbx_native_engines":     <bool>,             // scanner.rs:545-548
  "arbx_pool_enum_mode":     <str>,              // spawn gate scanner.rs:531-539
  "arbx_scoring_enabled":    <bool>, "arbx_scoring_hard_gate": <bool>, // scoring_pipeline.rs:48-49,92-95
  "arbx_gate_macro_mev_enabled": <bool>,         // gates/mod.rs:127-140
  "arbx_route_discovery_outcomes": <bool>}       // cartridge_boot.rs:377-384
```

Solo claves con consumidor citado (RULE 00). Este census = lado DECLARED de clase B + defaults
clase A + insumo del drift-guard de env de CB-04 (diff census-vs-census entre boots).

### 3.3 `backend/searcher-rs/src/workers/route_scanner_worker.rs` — el wiring clase A

| Ancla | Cambio exacto |
|---|---|
| `:786-854` `spawn_route_scanner` | Tras resolver `mode` (:795) y ANTES del early-return `Off` (:808): `let board = RuntimeToggleClient::from_manager(<mgr derivada del redis:787>, "route_scanner", /*default_when_absent=*/ mode == RouteScannerMode::On);` y pasarla por `run_loop` (:841-852) → `run_scan_subscription` (:746-755, nuevo arg). El early-return Off SE CONSERVA (spawn sigue siendo env-gated: cero overhead cuando nunca se habilitó — scanner.rs:960). |
| `:702-721` loop per-block (select sobre `blocks.next()`, `scan_block` en :710) | Entre la recepción del bloque (:709) y `scan_block`: (1) `let on = board.is_on().await;` (2) SIEMPRE `SETEX arbx:controlboard:route_scanner:hb 75 {"state": on?"run":"halted","block":N,"ts":now}` — 1 write/bloque (~0.08 QPS); (3) `if !on { debug!(event:"route_scanner.halted_by_board", chain_id, block); counter.incr(); continue; }` (LOGFLOOD-01: per-item `debug!` + el summary `info!` que ya existe por bloque se mantiene); (4) `scan_block(...)` como hoy. |
| métricas | Counter `route_scanner_board_halt_blocks_total` + gauge `route_scanner_board_on` (espejo de `KILLSWITCH_ENABLED` killswitch.rs:101) — registro junto a los metrics del crate. |

**Cero allocs en lectura:** `is_on()` fresco de cache devuelve `bool` copiado (killswitch.rs:79-87).
El costo nuevo del camino caliente = 1 GET Redis/s (cacheado) + 1 SETEX/12s (telemetría
deliberada). La decisión de escribir hb cada bloque (y no solo al alternar) es para que el board
verifique AMBOS estados (run y halted) — un halt verificado exige hb viva con state=halted.

**Semántica de propagación esperada (CB-06 la mide):** PUT → SET inmediato; worker lo ve en
≤1s (TTL cache) + cadencia newHeads 12s → LED verificado voltea en ≤2 bloques. El badge drift
(declared≠verified) parpadea "drift" durante esa ventana y luego alinea — comportamiento
correcto del contrato, no un bug.

### 3.4 Lo que NO se toca en Rust

`canonical_knobs.rs` (53 knobs siguen boot) · `killswitch.rs` · `config_reload*.rs` /
`topology_reload.rs` (fabrics existentes) · scanner.rs (kill-switch ya cableado :83,724,1035;
native_engines sigue boot :541-548) · main.rs killswitch connect :345 · chain_supervisor.rs:64.
Verificación del apply: `cargo check`/`clippy -D warnings`/`fmt --check` de `-p searcher-rs` y
`-p shared-rs` (baseline verde del árbol, CB-02-RUST-APPLY §4).

---

## 4. D4 — API `GET/PUT /api/v1/control-board` (api-server)

NUEVO `backend/api-server/src/routes/control-board.ts` + montaje en `index.ts` junto al
service-control (~:617-624, ANTES de `mountStubs` — precedente de orden de dispatch
service-control.ts:4-6). Deps inyectadas (mismo shape que service-control.ts:35-54):
`{ redis, pool, killSwitch, requireAdminToken, adminToken, writeAudit, reqUA, logger }`.
El edge YA está construido (index.ts:1594-1595 adminProxy GET+PUT, sin cache, statuses verbatim).

### 4.1 Registry declarativo server-side (RULE 00 / arbx-no-hardcode)

`const MODULE_REGISTRY: readonly RegistryEntry[]` — cada fila: los 13 campos del contrato
(ControlBoardLed.tsx:43-68) + metadata de wiring: `reader` (cómo GET ensambla verified/declared)
y `writer` (`"board_set"` | `"killswitch_set"` | `null`). Toda entrada cita consumidor file:line
(§8). Es versionado en código: agregar una fila exige cita del consumidor (CB-05 §4.2, misma regla).

### 4.2 GET — ensamblado del snapshot (declared vs verified vs drift)

```text
1. redis null → 503 {error:"redis_unavailable"}            (fail-honest, edge lo pasa verbatim)
2. Lecturas Redis (pipeline): GET arbx:killswitch · GET arbx:controlboard:route_scanner
   · GET arbx:controlboard:route_scanner:hb · HGETALL arbx:controlboard:approved
   · GET arbx:config:boot_census · EXISTS arbx:config:canonical_knobs
3. Self-env api-server (proceso propio, lectura directa): ARBX_SERVICE_CONTROL,
   ARBX_SCORING_ARCHIVER_MODE, ARBX_PAPER_ARCHIVER_MODE
4. PG (best-effort, UNA query): SELECT DISTINCT ON (target_id) target_id, action, actor,
   after_state, created_at FROM audit_log WHERE target_kind='control_board_module'
   AND action LIKE 'control_board.%' ORDER BY target_id, created_at DESC;
   → updated_at / last_actor / last_reason (after_state->>reason). PG caído → esas 3 columnas
   null + logger.warn (el estado vivo NO se inventa ni se bloquea por metadata).
5. Por fila del registry:
   declared_on  = approved[field] ?? census_default (clase A) | census (clase B) | null (C)
   verified_on  = heartbeat.state=="run" (clase A worker) | live-key (killswitch: enabled)
                  | self-env (api-server) | census_value (clase B: "boot reportó X",
                  verified_source="boot-census@<ts>") | null → DESCONOCIDO (R8)
   verified_at  = hb.updated_at / live-key.updated_at / census.published_at
   verified_source = "redis:arbx:killswitch" | "redis:arbx:controlboard:route_scanner:hb"
                  | "env:api-server" | "boot-census" | null
   (hb expirada → verified_on=null PERO verified_at conserva el último ts conocido — el board
    muestra "última verificación <ts>, ahora DESCONOCIDO")
6. 200 {generated_at: new Date().toISOString(), modules:[...]}  — shape EXACTO
   ControlBoardSnapshotSchema (ControlBoardLed.tsx:71-76). El drift lo computa el cliente
   (driftStatus :206-219) — el endpoint NO lo pre-computa (una sola fuente de verdad).
```

### 4.3 PUT — razón OBLIGATORIA server-side, audit PRIMERO, Redis idempotente

```text
zod body {id: string(min1), on: boolean, reason: string().trim().min(3).max(500)}
  fail → 400 {error:"invalid_request", details}           // razón vacía/corta NUNCA pasa (server-side)
id ∉ registry        → 404 {error:"unknown_module"}        // sin creación de claves arbitrarias
terminus denylist §34.3 (id/control_key =~ live_exec|live_mainnet|mainnet_flip|
                         ARBX_LIVE_EXEC_*|SIM_SIGNER_ADDRESS)
                     → 403 {error:"terminus_denied_c343"} ANTES del check de clase
                        (defense-in-depth adoptado del apply paralelo R4 §15: un censo que
                         mal-etiquete A un terminus JAMÁS lo desbloquea)
module_class = "C"   → 403 {error:"module_class_c_locked"}  // §34.3 candado del operador
module_class = "B"   → 409 {error:"module_class_b_restart_required",
                             hint:"CB-05 proposal flow (diff + pipeline deploy)"}  // R4 §15
id=route_scanner AND census mode=off (worker no spawn-eado)
                     → 409 {error:"module_not_spawned",
                             hint:"ARBX_ROUTE_SCANNER_MODE=on + restart habilita el spawn (CB-05)"}
─ flujo feliz ─────────────────────────────────────────────────────────────────────
before = lectura viva (toggle key / killswitch state)           // capture for audit trail
writeAudit PRIMERO: action="control_board.toggle", actor=req.header("x-arbx-actor")??"admin",
   target_kind="control_board_module", target_id=id,
   before_state={on: before}, after_state={on, reason}          // audit_log INSERT (index.ts:362)
escritura Redis IDEMPOTENTE (SET mismo valor = no-op natural):
   SET arbx:controlboard:route_scanner "true"|"false"       // formato R2 §15
   + HSET arbx:controlboard:approved route_scanner <json {on,reason,actor,updated_at}>  // CB-04
Redis FAIL tras el audit → writeAudit compensatorio action="control_board.toggle_failed"
                      + 500 {error:"redis_write_failed", detail:"audited, NOT applied"}
                      // append-only honesto (R1 §15): JAMÁS un UPDATE de la fila ya escrita —
                      // 011:21 REVOKE UPDATE lo impide sobre audit_log y la doctrina lo prohíbe
                      // sobre cualquier ledger; la fila compensatoria es el mecanismo.
200 → re-ensamblar snapshot (§4.2) y devolverlo             // el contrato espera Snapshot (ControlBoardLed.tsx:157-161)
```

Decisiones embebidas: (a) **killswitch NO se toggles desde el board** (R5 §15: el toggle
canónico vive en `/admin/killswitch`; el board enlaza — GOAL :37-38); (b) PUT idempotente
audita IGUAL (el registro de intención del operador ES el entregable del board; el re-assert
es exactamente lo que CB-04 necesita para revertear drifts al valor aprobado);
(c) NO se agrega `/api/v1/control-board/diff` en CB-02 — el drift ya viaja en el snapshot
(declared_on/verified_on) y CB-03 lo renderiza; si CB-04 necesita el endpoint de diff, es un
parche acotado suyo sobre el mismo patrón adminProxy.

---

## 5. D5 — DDL: `121_control_board_audit_logs.sql` NO se crea (premisa refutada)

**Refutación de la premisa del WO (y del hallazgo 3 de CB-05) con evidencia:**

| Afirmación previa | Realidad verificada 2026-09-07 (repo + VPS read-only) |
|---|---|
| "la tabla NO existe hoy — verificado por CB-05" (WO) | La tabla **`audit_log` (SINGULAR)** existe: migración `database/migrations/011_audit_log.sql` (CREATE TABLE :4-16, índices :18-19, append-only REVOKE UPDATE/DELETE :21), particionada mensualmente por `019_audit_log_partitions.sql`, endurecida PII en 053/055/070. |
| CB-05 §6.1: "information_schema.tables devuelve vacío... CB-02 es dueño de crearla" | CB-05 consultó el nombre **plural** `audit_logs`. En VPS (psql SELECT read-only HOY): `audit_log` existe con columnas `id,actor,action,target_kind,target_id,before_state,after_state,ip_address,user_agent,trace_id,created_at` y **3 particiones vivas** (`audit_log_2026_05/06/default`). También existe `audit_event` (registries 066/070 — otro ledger, no aplica). |
| — | El writer ya existe y está vivo: `writeAudit()` api-server/src/index.ts:362-383 (INSERT INTO audit_log) usado por killswitch (:271), blacklist (:416,:428), circuit breakers (:457,:464), service-control (service-control.ts:193-203), rpc-backend. |

**Decisión:** el ledger del board ES `audit_log` (append-only, particionado, PII-hardened).
Crear `audit_logs` paralela duplicaría la superficie de auditoría (viola §37 P-∅: la carga de la
prueba es del cambio; y dispersaría el trail que `GET /admin/audit` index.ts:480-511 ya lee).
El contrato de fila de CB-05 §6.1 mapea 1:1 sin cambio alguno de su diseño:
`actor→actor`, `action→action` ('propuesta_pendiente'), `target→target_kind+target_id`,
`payload→after_state` (JSONB), `estado/evidencia→after_state` (JSONB). **Migración 121: NINGUNA.**
El índice existente `idx_audit_action_time (action, created_at DESC)` (011:19) cubre la query
DISTINCT ON del §4.2 (volumen de toggles = acciones admin raras); si EXPLAIN algún día muestra
otra cosa, índice puntual en PR propio.

---

## 6. D6 — Denylist §34.3 server-side + soberanía

1. **La denylist es el registry mismo + validación de clase**: `module_class !== "A"` → 403
   server-side (§4.3). Un `curl` directo al api-server con token válido tampoco puede togglear
   B/C. Clase C nunca aparece con toggle en NINGUNA capa (UI ya lo niega:
   ControlBoardLed.tsx:184,194-196 — el backend es la barrera real).
2. **Filas clase C del registry** (§8 filas 23-25): `live_exec_terminus`
   (`control_key:"relays-client:live_exec_policy"` — default-deny + `MainnetRefused`
   INTOCABLES, §34.3), `relays_submit` (consumidor = submit path relays-client), `signer_capital`
   (capital-key lockout, main.rs:333-336). Sus `description` citan la regla de autorización de
   3 puntos (§34.3). LED locked gris-candado SIEMPRE (`projectLed` :184).
3. **Patrones denylist adyacentes** (mismo espíritu que CB-05 §7): cualquier id cuyo writer
   sería firma/broadcast; claves `*KEY*/*SECRET*/*TOKEN*/*PASSWORD*/*MNEMONIC*/*PRIVATE*`
   jamás entran al registry; ids no-registry → 404 (no se crea clave Redis arbitraria por PUT).
4. **Soberanía**: único principal escritor = sesión admin-token (cookie httpOnly V-AT-1 →
   adminProxy del edge → `x-arbx-admin-token` → `requireAdminToken` shared-ts/src/middleware/index.ts:124-133
   con `safeTokenEqual`). No hay camino service-token ni anónimo al PUT. El TS internal
   `/internal/audit/auth` (requireEdgeToken) NO expone escritura de control-board.
5. **Modo del board**: el board NO puede flipesar `ARBX_ORCHESTRATOR_MODE`/`ARBX_CARTRIDGE_MODE`
   (§34.2) ni `NEXT_PUBLIC_*` (R03/R04 — territorio CB-05). El terminus §34.3 queda fuera por
   construcción.

---

## 7. D7 — Hooks para CB-04 (registro de último valor aprobado + diff + revert)

| Hook provisto por CB-02 | Cómo lo consume CB-04 |
|---|---|
| Hash `arbx:controlboard:approved` (escrito SOLO por PUT) | **Registro de aprobación**: el valor aprobado del operador por módulo. |
| Campos `declared_on`/`verified_on`/`verified_source`/`verified_at` en el snapshot | CB-03 pinta drift (driftStatus ya existe); CB-04 detecta mismatch declarado≠verificado → banner + alerta audible (GOAL :46-47). |
| Vocabulario de acciones audit reservado: `control_board.toggle` · `control_board.toggle_failed` · **reservados para CB-04:** `control_board.drift_detected` · `control_board.reverted` | CB-04 escribe sus eventos al MISMO ledger con `target_kind='control_board_module'`; la query DISTINCT ON del GET los excluye del last_reason de operador (filtro `action LIKE 'control_board.toggle%'` — ajuste fino: usar `action IN ('control_board.toggle','control_board.toggle_failed')` para las columnas de operador). |
| Boot census (`arbx:config:boot_census`) con `published_at` | Drift de ENV clase B: diff census-nuevo vs census-viejo al redeploy + contra `approved`/audit trail → alerta "cambió por fuera del board" y generación del diff (CB-05 INV-B6 cierra con propuesta aplicada). |
| Revert clase A: CB-04 re-SETea el JSON de `approved` a la clave toggle (SET+PUBLISH) + audit `control_board.reverted` | El board es la fuente de aprobación; nadie más escribe (§1.3-4). El mecanismo de reescritura es el mismo SET idempotente del PUT. |
| Endpoint diff | DEFERIDO: el drift ya viaja en el GET; si CB-04 exige endpoint propio (`GET /api/v1/control-board/diff`), es parche acotado bajo el mismo adminProxy (1 línea edge + route). |

---

## 8. Registry v1 (filas EXACTAS del snapshot — cada una con cita de consumidor)

Leyenda reader: `live`=clave Redis viva · `hb`=heartbeat · `self`=env del propio api-server ·
`census`=boot_census · `link`=fila informativa/enlazada.

| # | id | clase | control_key | reader verified | fuente del dato (file:line) |
|---|---|---|---|---|---|
| 1 | `killswitch` | **A-link** (R5 §15: NO board-toggleable — el toggle canónico vive en su página `/admin/killswitch`; GOAL :37-38 "lo ENLAZA, no lo duplica") | `redis:arbx:killswitch` | live (`enabled`; **on=ARMADO=detención** — name/description lo explicitan) | killswitch.rs:15; api index.ts:254-285 |
| 2 | `route_scanner_multihop` | A | `redis:arbx:controlboard:route_scanner` | hb | route_scanner_worker.rs:105-132,702-721; spawn gate :808 |
| 3 | `cartridges_runtime` | A-link | `api:/api/cartridges/runtime` | link (agregado 264) | edge index.ts:1600-1601 |
| 4 | `math_operator_registry` | A-link | `api:/api/math/operators` | link | edge index.ts:1602 |
| 5 | `service_control` | A-link | `api:/api/v1/admin/services` | self (`ARBX_SERVICE_CONTROL`) | service-control.ts:141-145; edge :1553-1584 |
| 6 | `chains_config_reload` | A-link | `redis-channel:arbx:config:chains:reload` | link | config_reload.rs; main.rs:434; edge :1500-1547 |
| 7 | `topology_hot_reload` | A-link | `redis-channel:arbx:topology:mutation` | link | topology_reload.rs; main.rs:390-397 |
| 8 | `rpc_registry_reload` | A-link | `api:/api/admin/rpcs/reload` | link | edge :1603-1604; rpc-registry.ts |
| 9 | `retention_settings` | A-link | `db:retention_settings` | link (no-booleano) | migración 117; archive-control.ts |
| 10 | `orchestrator_mode` | B | `env:ARBX_ORCHESTRATOR_MODE` | census | scanner.rs:164-178 (§34.2 flag) |
| 11 | `cartridge_mode` | B | `env:ARBX_CARTRIDGE_MODE` | census | cartridge_boot.rs:66 (§34.2) |
| 12 | `mempool_mode` | B | `env:ARBX_MEMPOOL_MODE` | census | chain_client.rs:280-302 |
| 13 | `native_engines` | B | `env:ARBX_NATIVE_ENGINES` | census | scanner.rs:541-548 |
| 14 | `pool_enum_mode` | B | `env:ARBX_POOL_ENUM_MODE` | census | scanner.rs:531-539 |
| 15 | `route_scanner_spawn` | B | `env:ARBX_ROUTE_SCANNER_MODE` | census | route_scanner_worker.rs:808-810 (spawn; el RUN gate es la fila 2) |
| 16 | `canonical_knobs` | B | `redis:arbx:config:canonical_knobs` | EXISTS+link | canonical_knobs.rs:47-204; main.rs:361-384; canonical-knobs.ts |
| 17 | `scoring_gate` | B | `env:ARBX_SCORING_{ENABLED,HARD_GATE}` | census | scoring_pipeline.rs:48-49,92-95; opportunity_emitter.rs:144 |
| 18 | `macro_mev_gate` | B | `env:ARBX_GATE_MACRO_MEV_ENABLED` | census | gates/mod.rs:127-140 |
| 19 | `route_discovery_outcomes` | B | `env:ARBX_ROUTE_DISCOVERY_OUTCOMES` | census | cartridge_boot.rs:377-384 |
| 20 | `route_discovery_worker` | B-info | `pin:NO-ACTIVE` | link ("arquitectura radar, sin modo activo por diseño" — GOAL :34-35) | route_discovery_worker.rs:150,767 |
| 21 | `sim_backend` | B | `env:SIM_BACKEND` | census si searcher lo ve; si no, declared null honesto | sim-ctl capabilities.rs:37-52, consumer.rs:421 |
| 22 | `ws_consumer_purge` | B | `env:ARBX_WS_CONSUMER_PURGE_{INTERVAL,IDLE}_MS` | self (api-server) | websocket.ts:821-822 |
| 23 | `archivers_scoring_paper` | B | `env:ARBX_{SCORING,PAPER}_ARCHIVER_MODE` | self | api index.ts:1958-1969 |
| 24 | `csp_enforce` | B | `env:ARBX_CSP_ENFORCE` | declared null (sin censo edge en v1) | edge; RULE 02/04 — CB-05 E3 cubre el camino |
| 25 | `live_exec_terminus` | C | `relays-client:live_exec_policy` | null | live_exec_policy.rs default-deny+MainnetRefused (§34.3) |
| 26 | `relays_submit` | C | `relays-client:submit` | null | submit path relays-client (§32 read-only) |
| 27 | `signer_capital` | C | `capital-lock` | null | main.rs:333-336 capital-key lockout |

Pendientes CB-01 (sin fila hasta tener cita): watchlist/rate-limits/pool_sync-interval/
`ARBX_KNOB_*` hot-path knobs (ya cubiertos por la fila 16), alertmanager/webhooks. La regla
CB-05 §9 (A>B) aplica al reconciliar.

---

## 9. Contrato cliente: ajuste total + UN parche acotado ordenado

El diseño **se ajusta** al contrato construido (cero cambios de tipos): snapshot shape
(ControlBoardLed.tsx:71-76), PUT `{id,on,reason}` (:141), respuesta-PUT = snapshot (:157-161),
proyecciones puras (projectLed/isToggleable/driftStatus/fmtIso). Edge sin cambios
(index.ts:1586-1595 ya documentan audit-first + idempotente + no-cache).

**Parche acotado ORDENADO (para el apply de CB-02/CB-03, archivo bajo mi claim):** en
`frontend/components/ControlBoardLed.tsx` añadir UNA función pura + su test:

```ts
// WO-CB-02 (2026-09-07)
/** ¿El toggle vive en ESTE board (namespace Redis propio) vs superficie externa enlazada? */
export function isBoardToggleable(m: {
  module_class: ControlBoardModule["module_class"];
  control_key: ControlBoardModule["control_key"] | undefined;
}): boolean {
  return (
    m.module_class === "A" &&
    (m.control_key?.startsWith("redis:arbx:controlboard:") ?? false)
  );
}
```

Motivo (refinado R5/R6 §15): las filas A-link (incluida `killswitch`, cuya clave es JSON
`KillSwitchState` — jamás `"true"/"false"` del board) son runtime-pero-externas;
`isToggleable` (:194-196) las marcaría toggleables y CB-03 pintaría un toggle que este
endpoint rechazaría (403/404). El patch es aditivo-puro (sin cambio de schema ni de
proyecciones existentes); CB-03 usa `isBoardToggleable` para el affordance y
`telemetry_href` para el link. Test: fila 2 true; filas 1,3-9 false; B/C false;
`control_key` null/false-y-ajeno false.

---

## 10. Correcciones/refutaciones a pares (evidencia propia, no ignoradas)

1. **CB-05-PROPUESTA-B.md §6.1 + hallazgo 3 ("`audit_logs` NO existe; CB-02 es dueño de crear
   la tabla")** — REFUTADO por nombre plural: `audit_log` (singular) existe (011 + 019
   particiones + 053/055/070; VPS: columnas y 3 particiones vivas; writeAudit index.ts:362).
   El flujo CB-05 queda INTACTO (su contrato mapea 1:1, §5) — solo cambia el nombre de la
   tabla destino de `audit_logs` a `audit_log`.
2. **CB-02-RUST-APPLY.md §3.2 ("knobs boot-time, sin re-lectura runtime")** — CONFIRMADO
   (grep completo de `CanonicalKnobs` y `from_env`); el diseño lo respeta (0 promociones).
3. **CB-02-RUST-APPLY.md §3.4 (gates "candidatos a reclasificar")** — DECIDIDO: solo
   route_scanner run-gate promueve a A (wiring §3.3); spawn gate y demás quedan B (§2.2).
4. **GOAL-WORKORDERS.md:18 ("patrón kill-switch/canonical_knobs")** — PRECISIÓN:
   canonical_knobs NO es un patrón de poll (es publish boot); el patrón aplicable es
   exclusivamente el kill-switch (§1.3). El board enlaza kill-switch (GOAL :37-38) sin
   duplicar su página.

---

## 11. Invariantes (INV-CB02, inviolables para el apply)

- **INV-CB02-1 (fail-safe)**: clave Redis ausente/garbage/Redis caído → comportamiento
  desplegado (default = boot env). `verified` sin señal → DESCONOCIDO (null), JAMÁS off-falso.
- **INV-CB02-2 (audit-first)**: ninguna escritura Redis de `arbx:controlboard:*` (ni
  `arbx:killswitch` via board) ocurre sin fila PREVIA en `audit_log` con razón obligatoria
  validada server-side (min 3, max 500).
- **INV-CB02-3 (denylist server-side)**: PUT clase B/C → 403 con cita de regla; ids
  desconocidos → 404; sin creación de claves arbitrarias.
- **INV-CB02-4 (un writer)**: `arbx:controlboard:*` lo escribe SOLO el PUT (sesión admin-token
  via adminProxy). Rust client read-only (sin `set()`). `approved` solo por PUT (CB-04 re-SETea
  `approved`→clave como excepción documentada con audit propio).
- **INV-CB02-5 (hot-path)**: lectura cero-alloc (bool de cache TTL 1s); telemetría acotada a
  1 SETEX/bloque; per-item `debug!` + summary `info!` (LOGFLOOD-01).
- **INV-CB02-6 (mode-invariance §34.1/§34.2)**: ningún toggle clase A altera matemática ni
  semántica por modo; orchestrator/cartridge mode jamás runtime-flipeables; terminus §34.3
  intacto (default-deny + MainnetRefused sin tocar).
- **INV-CB02-7 (§37 Nivel-1)**: el algoritmo de discovery no se modifica; el toggle run-gate
  solo omite INVOCAR el scan ya desplegado.

## 12. Gates de aprobación del diseño/apply (GATE-CB02)

- **GATE-CB02-1 (contrato)**: tests del route TS — snapshot pasa Zod
  ControlBoardSnapshotSchema; PUT sin razón → 400; razón <3 → 400; id desconocido → 404;
  clase C → 403 con cita §34.3; clase B → 403 con hint CB-05; route_scanner no-spawned → 409.
- **GATE-CB02-2 (fail-safe Rust)**: unit tests — clave ausente→default boot; garbage JSON→default;
  TTL cache respeta 1 GET/s; hb expirada→verified null en GET.
- **GATE-CB02-3 (integridad)**: `cargo check`+`clippy -D warnings`+`fmt --check` (searcher-rs,
  shared-rs) y `tsc`+`vitest` (api-server, frontend) verdes; INVIANTE XLEN `arbx:opps:detected`
  delta=0 durante todo el apply (§33.3).
- **GATE-CB02-4 (E2E CB-06, posterior)**: toggle route_scanner off en browser → hb refleja
  `halted` ≤2 bloques → LED fluor→rojo SIN restart; audit_log contiene la fila con la razón;
  drift badge alinea después de propagar.

## 13. Evidencia verificada (todo read-only, 2026-09-07)

- Repo: todas las file:line citadas (killswitch.rs completo; canonical_knobs.rs 1-746;
  route_scanner_worker.rs 95-158,683-854; scanner.rs 525-569,718-775,954-1049; main.rs 330-408;
  index.ts api-server 245-383,480-511; service-control.ts completo; websocket.ts:821-822;
  gates/mod.rs:120-145; cartridge_boot.rs:377-384; scoring_pipeline.rs:30-104;
  opportunity_emitter.rs:130-175; migrations 011/019/…/120; edge index.ts:1500-1627;
  ControlBoardLed.tsx completo; admin-token.ts completo; shared-ts middleware:124-133;
  canonical-knobs.ts completo).
- VPS (`ssh arbx`, SOLO lectura): `audit_log` + 3 particiones + columnas (psql
  information_schema); `arbx:killswitch` = `{"enabled":false,"reason":"VER",...}` (12:53Z hoy);
  `arbx:config:canonical_knobs` EXISTS=1; SCAN `arbx:controlboard*` = vacío (esperado — este
  diseño las crea); `audit_event` existe (otro ledger). Nada mutado.
- Declarado NO verificado: contenido de CB-01 (no existe); estado runtime de relays/terminus
  (fuera de alcance §32); valores secretos (no leídos).

## 14. Fuera de alcance (design-only)

Cero ejecución: no se creó archivo de código, ni migración, ni clave Redis; no se editó
`.env` local ni VPS; NO-GIT. El apply (re-despacho del Rust/TS contra §3-§4 de este documento)
es fase posterior con gates GATE-CB02-1..3 y verificación CB-06 posterior.

---

## 15. RECONCILIACIÓN con el apply TS paralelo (CB-02-API-APPLY.md — aterrizó 08:31Z, tras redactar §1-§14)

**Contexto:** `CB-02-API-APPLY.md` (ecc:typescript-reviewer, RESPAWN-2 A) aplicó la mitad
TS ANTES de que este diseño existiera (lo declara honestamente en su §0) y dejó merge-points
explícitos (su §3): *"the design may retarget them in one edit"*. Este diseño EJERCE esa
autoridad (GOAL-WORKORDERS.md:18: CB-02 = diseño + apply — el diseño decide). Sus archivos
`backend/api-server/src/routes/control-board{,.test}.ts` (untracked, 22/22 tests, tsc EXIT 0)
quedan como base VÁLIDA con los retargets adjudicados abajo. Adjudicación por divergencia:

| # | Divergencia | Decisión del diseño | Retarget / evidencia |
|---|---|---|---|
| R1 | **Tabla de auditoría:** su §5.1 ordena migración 121 creando `audit_logs` NUEVA (actor/action/target/payload); su código hace `INSERT … RETURNING id` + `SELECT DISTINCT ON (target)` + **`UPDATE audit_logs SET payload…`** | **REFUTADO — reutilizar `audit_log` existente** (§5 de este doc: 011 + particiones 019 vivas en VPS + writeAudit index.ts:362). La migración 121 NO se crea. Además su `UPDATE` es **imposible por diseño**: 011:21 `REVOKE UPDATE, DELETE ON audit_log FROM arbx_rw` — fallaría en runtime y viola append-only aunque la tabla fuera nueva. El mecanismo correcto (ya en §4.3): **fila compensatoria INSERT** `action='control_board.toggle_failed'`. | Retarget acotado en `control-board.ts`: columnas `actor,action,target_kind,target_id,before_state,after_state` (payload→after_state JSONB; target→target_kind+target_id; id UUID sin RETURNING necesario), DISTINCT ON (target_id) con `WHERE action IN ('control_board.toggle','control_board.toggle_failed') AND target_kind='control_board_module'`, y reemplazar el bloque UPDATE (:503-531) por INSERT compensatorio + 500. Sus tests: rename de tabla/columnas + assert del INSERT compensatorio. |
| R2 | **Formato del valor clase A:** su `"true"/"false"` estricto (`parseDeclaredValue` :157-163) vs mi JSON RuntimeToggleState | **ADOPTO el suyo** — string estricto: cero parse en el poll caliente del worker (comparación de string), un solo writer (board), y la riqueza (reason/actor/updated_at) vive donde corresponde: `audit_log` (ledger) + hash `approved` (CB-04). El §1.2 de este doc queda enmendado: `arbx:controlboard:route_scanner` = `"true"\|"false"`. | Cero cambios en su código; §3.1 Rust lee string (más simple que el JSON mirror). |
| R3 | **Arquitectura del GET:** census-driven (Redis `arbx:config:control_board` que CB-01 publicará; ausente → snapshot vacío sancionado por charter) con `verified_*` verbatim del census | **ADOPTO el census como fuente de IDENTIDAD/CLASE/declared-B**, PERO el diseño ORDENA: `verified_*` de clase A se computa LIVE en el GET (hb / clave viva), no verbatim del census — un census publicado-por-herramienta se stalea y el LED verificado debe ser vivo (RULE 00: LED = realidad, no foto). El §8 de este doc pasa a ser el **CONTRATO DE CONTENIDO del census** que CB-01 debe publicar (clasificación, control_key, fuentes verificadas por fila). | Retarget acotado en `buildSnapshot`: para módulos con `control_key` en namespace `arbx:controlboard:` + hb, verified = lectura live; killswitch = lectura `arbx:killswitch` (ver R5). |
| R4 | **Clase B en PUT:** su 409 `module_class_b_restart_required` vs mi 403 | **ADOPTO su 409** (ya codificado + testeado; semántica "conflicto con estado boot-time" defendible; CB-05 hint viaja en el body). §4.3 de este doc enmendado: B→409, C→403, terminus-denylist→403 ANTES del check de clase (su gate 5, defense-in-depth — ADOPTADO). | Ninguno (el suyo queda). |
| R5 | **killswitch como módulo board-toggleable (mi §8 fila 1) vs link (GOAL :37-38)** | **Decisión final: killswitch = fila A-LINK, NO board-toggleable.** El valor vivo de `arbx:killswitch` es JSON `KillSwitchState` — NO `"true"/"false"` — así que el formato R2 lo deja `declared_on:null` para siempre; y GOAL manda "el board lo ENLAZA, no lo duplica". El toggle canónico del kill-switch sigue siendo su página existente (`/admin/killswitch`, auditado). El board muestra LED (verified live de la clave) + link. | §8 fila 1 pasa a A-link con `control_key:"redis:arbx:killswitch"`; el parche §9 usa prefijo `redis:arbx:controlboard:` (ver R6). **Resultado: el conjunto board-toggleable clase A v1 = exactamente 1 módulo (`route_scanner_multihop`) + los que el census CB-01 añada con control_key en el namespace del board, siempre citando consumidor.** |
| R6 | **Parche §9 `isBoardToggleable`:** prefijo `redis:` | **Refinado a `redis:arbx:controlboard:`** (namespace propio del board): excluye killswitch (R5) y canales (`redis-channel:`) de una vez. | 1 línea en el parche ordenado. |
| R7 | **Pub/sub `arbx:controlboard:changes`** (mi §1.2 opcional) | **DEFERIDO** — su §3 tabla última fila tiene razón: publicar sin subscriber es especulativo (P-∅). El poll TTL 1s es la fuente de verdad. Se añade PUBLISH cuando exista el subscriber Rust (mismo PR del wiring §3). §1.2 enmendado. | Ninguno hoy. |
| R8 | **Pool null → 503 `db_unavailable` ANTES de todo (su gate 1)** | **ADOPTADO** — más estricto que mi §4.3 (sin auditoría no hay toggle, punto). Su crítica al swallow del writeAudit legacy (index.ts:388-390) es correcta y su INSERT-falla⇒500 es el gate duro correcto. | Ninguno. |
| R9 | **`reason` zod:** suyo `trim().min(1).max(1000)`; mío min 3 max 500 | **ADOPTO el suyo** (codificado + 22 tests; killswitch usa 500 pero 1000 no daña; la sustancia de la razón la garantiza el proceso, no el length). | Ninguno. |
| R10 | **Boot census del searcher (`arbx:config:boot_census`, mi §3.2)** | **SE MANTIENE como insumo del census CB-01** (self-report R8-honest del searcher: env leído por el proceso que lo consume), NO como requisito del GET. CB-01 decide si su publisher lo consume o releva .env/inspect externamente (como CB-05 §10). | Ninguno en TS; el wiring Rust §3.2 sigue en pie para el re-despacho. |

**Estado de las mitades tras esta adjudicación:** TS/API = APLICADA con retargets R1/R3 (los
únicos que tocan su código; ambos acotados a constantes/queries, sus 22 tests se ajustan de
nombre de tabla/columnas + 1 assert nuevo del INSERT compensatorio). Migración 121 = NO (R1).
Mount en index.ts (su §5.2) = VÁLIDO tal cual. **Rust = PENDIENTE íntegro** (§3 de este doc es
su spec: RuntimeToggleClient string-based, wiring route_scanner_worker :786-854/:702-721, hb
SETEX, boot census) — re-despachar al rust-topology-engineer. CB-01 = PENDIENTE; su census
debe satisfacer el contrato de contenido §8 + R3 (verified live para A o delegación al GET).

**Nota §37 P-∅ para el PR de fusión:** los retargets R1/R3 son parte del MISMO trabajo CB-02
(un solo PR por WO), no reformateo ajeno; el diff de `edge/worker/src/index.ts` (M,
sesión paralela) y `edge-parity.test.ts` (M) se preservan tal cual (claim ajeno respetado).
