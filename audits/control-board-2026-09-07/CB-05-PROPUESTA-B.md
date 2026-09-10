# CB-05 — Diseño del flujo Clase B (boot-time env): «Proponer cambio» → propuesta auditable, JAMÁS auto-aplicada

> **WO:** CB-05 · **kind:** design · **agente:** devops-platform (Gang Omniscience) · **fecha:** 2026-09-07
> **Inputs declarados:** CB-01-INVENTARIO + CB-02-DISENO.
> **Estado real de los inputs (fail-honest, RULE 00):** al momento de redactar este diseño
> (2026-09-07 ~12:40Z) NI `CB-01-INVENTARIO` NI `CB-02-DISENO` existen todavía en
> `audits/control-board-2026-09-07/` (verificado: solo `GOAL-WORKORDERS.md` está presente —
> agentes paralelos del gang aún no escriben sus entregables). **Mitigación:** todo el
> inventario-clase-B usado aquí fue derivado de fuentes primarias del repo (file:line citado)
> y verificado read-only contra el VPS HOY (evidencia con timestamp). Cuando CB-01/CB-02
> aterricen, este diseño debe reconciliarse contra ellos (sección §10).

---

## 0. Resumen ejecutivo

Clase B = switches que el proceso lee **una sola vez al arrancar** (`std::env::var(...)` en
Rust boot-path; `process.env[...]` en TS boot-path). No existen en Redis, no se pueden
pollear: cambiarlos exige **editar `.env` en el VPS + recrear (o rebuild) el contenedor**.
El flujo CB-05 convierte esa operación manual invisible (la que dejó a RU-3 apagado meses,
GOAL-WORKORDERS.md:9-11) en un **acto auditable de tres pasos**:

1. El board `/control` muestra el estado verificado del módulo B + botón **«Proponer cambio»**.
2. El backend genera el **diff exacto** (líneas `.env`, servicios afectados, comando compose
   R3 canónico, rebuild `--no-cache` si toca `NEXT_PUBLIC_*`, restart requerido, verificación
   post-apply, rollback) y lo persiste como **`propuesta_pendiente` en `audit_logs`**.
3. El **operador** aplica esa propuesta por el pipeline de deploy existente. El board jamás
   escribe `.env`, jamás ejecuta `docker compose`, jamás reinicia nada.

**Cero clase C por diseño:** ninguna variable del terminus de capital/broadcast §34.3 es
proponible (denylist server-side, §7). Este WO es **solo diseño** — cero ejecución.

---

## 1. Definición operativa de Clase B (inventario derivado, verificado 2026-09-07)

Criterio: la variable se lee en boot-path (una vez) y no existe mecanismo runtime-poll.
Fuente de verdad del estado HOY: presencia en `/opt/arbitragex-v2/.env` + `docker inspect
<svc>` env del contenedor vivo + log de boot (señal de verificación).

| Variable | Servicio | Valores válidos (parse real, file:line) | Default fail-safe | Estado VPS 2026-09-07 | Señal de verificación post-restart |
|---|---|---|---|---|---|
| `ARBX_ROUTE_SCANNER_MODE` | searcher-rs | `on`\|`off` (garbage→off) — `route_scanner_worker.rs:105-132` | `off` | **`on`** (.env:136, container, log 12:13:15Z) | log `route_scanner.mode` mode+dispatch_path (`route_scanner_worker.rs:800-812`) |
| `ARBX_ORCHESTRATOR_MODE` | searcher-rs | `v2`\|`shadow`\|`off` (default→`v1`) — `scanner.rs:164-178` | `v1` | `v2` (.env:24, container) | log `worker_orchestrator.boot` (visto 12:13:07Z) |
| `ARBX_CARTRIDGE_MODE` | searcher-rs | parse en `cartridge_boot.rs:66` | — | `active` (.env:5, container) | log boot cartridges |
| `ARBX_MEMPOOL_MODE` | searcher-rs | `disabled|off|none` / `filtered|alchemy` / `firehose|all|raw` / `block|blocks|newheads|backrun` / `auto` (unknown→auto+warn) — `chain_client.rs:280-302` | `auto` | `auto` (.env:20, container) | warn `chain_client.mempool_mode_unknown` si valor inválido |
| `ARBX_NATIVE_ENGINES` | searcher-rs | `off` = off; **cualquier otra cosa (incl. ausente) = on** — `scanner.rs:543-549` | `on` | ausente → `on` | fan-out native en logs scanner |
| `ARBX_GATE_MACRO_MEV_ENABLED` | searcher-rs | `1|true|yes|on` (bool) — `gates/mod.rs:127-140` | `true` | `1` (.env:10, container) | log hits gate |
| `ARBX_SCORING_ENABLED` / `ARBX_SCORING_HARD_GATE` | searcher-rs | bool `env_bool` (`1|true|yes|on`) — `scoring_pipeline.rs:49,92-95,280-288` | `true` / **`false`** | HARD_GATE **ausente → false (dormido)** | scoring_pipeline emite advisory vs blocking |
| `ARBX_ROUTE_SCANNER_ANCHORS`, `ARBX_ROUTE_SCANNER_MAX_ROUTES_PER_BLOCK`, `ARBX_ROUTE_SCANNER_MAX_SCAN_MS` | searcher-rs | anchors lista / usize / u64 — `route_scanner_worker.rs:229-245` | defaults internos | ausentes | config leída al spawn |
| `ARBX_ENABLE_LEGACY_{TRIANGULAR,FLASHLOAN,LIQUIDATION}_WORKER` | searcher-rs | `true` exacto — `main.rs:253-273` | `false` (off) | ausentes → off | (debug-only; no fomentar) |
| `ARBX_SERVICE_CONTROL` | api-server | `on` ≠ on → 501 — `api-server/src/routes/service-control.ts:141-145` | `off` (501) | `on` (.env:40, container) | GET service-control resuelve ≠ 501 |
| `ARBX_SCORING_ARCHIVER_MODE` / `ARBX_PAPER_ARCHIVER_MODE` | api-server | `on` (case-insens) — `api-server/src/index.ts:1958-1969` | `off` (dormant) | ambas `on` (.env:39, .env:25) | log archiver boot vs "dormant" |
| `NEXT_PUBLIC_EDGE_URL`, `NEXT_PUBLIC_WS_URL` | **frontend (build-time, RULE 03)** | URL — `docker/compose.dev.yml:337-348` | `http://localhost:8787` / `:3000` | `https://edge-arbx.ape-tv.net` (.env:128) / `http://195.201.235.70` (.env:129) | `curl -I http://127.0.0.1:5173/opportunities` → CSP sin `localhost` (RULE 04) |
| `ARBX_POOL_ENUM_MODE` | searcher-rs | (enum pool-enumeration) | `shadow` | `shadow` (.env:29) | worker pool_enum boot |

**Nota RULE 00:** la columna «Estado VPS» contiene solo lo observado por ssh read-only el
2026-09-07 (~12:40Z): `grep` selectivo de claves no-secretas en `.env`, `docker inspect`
env del contenedor searcher-rs, y `docker logs`. Nada fue mutado. Los valores de claves
secretas (tokens/passwords) NO fueron leídos ni se reproducen aquí.

**Hallazgo clave del día:** `ARBX_ROUTE_SCANNER_MODE=on` YA está aplicado (`.env:136` +
container + log `route_scanner.mode mode=on dispatch_path=orchestrator` a las
2026-09-07T12:13:15Z, tras recrear el searcher a las 12:13:00Z mientras el resto de la
flota lleva 6h). Es decir: el flujo manual «editar .env → recrear servicio» acaba de
ejecutarse a mano HOY — exactamente el acto que CB-05 vuelve auditable.

---

## 2. Invariantes del flujo (INV-B, inviolables)

- **INV-B1 — Solo texto, nunca acción.** El único efecto tangible de «Proponer cambio» es
  INSERT de una fila `propuesta_pendiente` en `audit_logs` (PG). No existe endpoint de
  apply, no existe `docker exec`, no existe escritura de `.env`. El que aplica es el
  operador, por fuera del board, con el pipeline existente (`scripts/deploy.sh` /
  compose directo).
- **INV-B2 — Diff contra estado vivo, o nada.** El diff se calcula contra el valor REAL
  leído del inventario (CB-01: `.env` + `docker inspect`). Si la lectura falla o el valor
  actual es `DESCONOCIDO` → **la propuesta no se genera** (RULE 00/R8: jamás un diff
  contra un valor inventado).
- **INV-B3 — Denylist §34.3 server-side.** Variables del terminus capital/broadcast y
  secretos son rechazadas por el backend (no solo ocultas en UI): ver §7. `default-deny`
  y `MainnetRefused` de `live_exec_policy.rs` quedan intactos por construcción — este
  flujo ni los nombra como proponible.
- **INV-B4 — `NEXT_PUBLIC_*` ⇒ rebuild obligatorio.** Toda propuesta que toque una var
  `NEXT_PUBLIC_*` debe incluir el comando RULE 03 completo (`build --no-cache` + `up -d`,
  ambos con `--env-file .env`) y la verificación RULE 04 (CSP sin `localhost`). Un simple
  recreate es **insuficiente y el generador lo marca como error** (CLAUDE.md §3 R03/R04:
  las `NEXT_PUBLIC_*` se hornean en `next build`; `docker compose restart` no las aplica).
- **INV-B5 — Auditoría antes que cambio.** Sin fila `propuesta_pendiente` (con quién,
  cuándo, razón obligatoria, diff, rollback) no existe cambio aprobable. La fila se escribe
  ANTES de que el operador toque el VPS.
- **INV-B6 — Cierre de drift.** Tras el apply manual, el operador (o el verificador CB-06)
  marca la propuesta `aplicada` adjuntando evidencia (StartedAt del contenedor + log de
  boot esperado). CB-04 cierra el drift solo con esa evidencia — un cambio de env por
  fuera de una propuesta queda como drift abierto/alerta (ese es su trabajo, no el mío).
- **INV-B7 — Estado ≠ off-falso.** Módulo B sin señal de verificación (sin log de boot en
  la ventana retenida, R9) se muestra `DESCONOCIDO`, nunca «apagado».

---

## 3. Máquina de estados de la propuesta

```
[board /control, tab Clase B]
        │ operador pulsa «Proponer cambio» (badge B = requiere-restart)
        ▼
   (1) FORM: variable (del registry §4), nuevo valor (enum o input validado),
       razón (OBLIGATORIA, texto libre)
        │ POST /api/v1/control-board/proposals   (x-arbx-admin-token)
        ▼
   (2) BACKEND: lee valor vivo (inventario CB-01) → valida enum/denylist
        │ OK                                     │ FAIL → 4xx con razón (no genera nada)
        ▼                                          (valor inválido / denylist §34.3 /
   (3) INSERT audit_logs                                / lectura vivo fallida INV-B2)
       action='propuesta_pendiente'
       payload = diff exacto + comandos + verificación + rollback + razón
        │
        ▼
   propuesta_pendiente ── operador aprueba (checkbox + razón) ──▶ aprobada
        │                                                            │
        │ operador descarta (razón)                                  │ operador aplica POR FUERA
        ▼                                                            │ (pipeline deploy, manual)
     descartada                                                      ▼
                                              aplicada ◀── marca + evidencia (StartedAt + log boot)
                                                 │
                                                 ▼
                                        CB-04 drift-guard cierra
```

- `pendiente → descartada`: reversible solo creando propuesta nueva (append-only).
- `aplicada` exige evidencia; sin ella el LED del módulo sigue mostrando el estado viejo
  verificado (INV-B7) y el drift queda abierto.
- **No existe estado `auto-aplicada`.** No hay transición board→VPS en ningún punto.

---

## 4. El generador de diff EXACTO (server-side)

### 4.1 Por qué server-side
El diff debe generarse en el backend (api-server, junto al endpoint de CB-02) para que el
«valor viejo» salga del inventario vivo, no de una copia stale del navegador (INV-B2). El
frontend solo renderiza el bloque resultante.

### 4.2 Registry de variables clase B (fuente del dropdown)
Tabla declarativa **en el backend** (config versionada, NO en el frontend — arbx-no-hardcode):
cada entrada declara `key`, `servicio(s)`, `tipo` (enum|bool|int|uint|url|list|str),
`valores_validos` (derivados del parse real citado en §1), `rebuild_required`,
`signal_verification` (log/evento esperado post-restart), `default_fail_safe`, y flag
`denylist_c343`. El registry solo contiene claves **observadas en el repo** (RULE 00);
agregar una clave nueva exige cita file:line del consumidor — sin consumidor, no es
switch, es ruido.

### 4.3 Reglas de generación del diff
1. **Valor viejo:** del inventario vivo. Variable ausente ⇒ línea `old = "(ausente —
   default fail-safe <D>)"`, y el diff lo expresa como inserción pura (`+CLAVE=valor`).
2. **Valor nuevo:** enum → solo opciones del registry; bool → `true|false` (canonicaliza
   `1/yes/on`→`true` según el parse real del consumidor, p.ej. `env_bool`
   `scoring_pipeline.rs:280-288`); numérico → rango del consumidor; URL/string → input del
   operador + validación de formato. **El generador jamás sugiere valores por su cuenta**
   (no-hardcode: el operador elige; el sistema valida).
3. **No-op:** nuevo == viejo ⇒ rechazo («el estado vivo ya es ese; no hay diff»).
4. **Formato del diff:** unified diff textual contra `/opt/arbitragex-v2/.env`, líneas
   exactas `CLAVE=valor` (sin espacios, sin comillas a menos que el consumidor las
   espere). Secrets NUNCA en el diff: si `old` provino de una clave clasificada secreta,
   el diff muestra `old=<redacted, sólo presencia>` (Ghost Protocol; y de todos modos el
   denylist §7 impide proponerlas).
5. **Servicios afectados:** del registry (1..n). Cada servicio aporta su comando §5.
6. **Idempotencia/duplicados:** solo una `propuesta_pendiente` abierta por `key` — una
   segunda propuesta de la misma clave mientras otra pende ⇒ 409 con link a la abierta.

### 4.4 Placeholders `process.env.*` (§32.5)
En cualquier código/scaffold que este diseño genere después (CB-03 UI, endpoint CB-02),
los valores de operador van como `process.env.*` en archivos versionados; los literales
viven solo en `.env` del VPS (gitignored, «legitimate VPS runtime state» —
`scripts/deploy.sh:27-28`). El diff propuesto ES el contenido futuro de `.env`: ahí sí va
el literal `CLAVE=valor`, porque ese archivo es precisamente el hogar de los literales.

---

## 5. Matriz de comandos compose EXACTOS (R3 canónico, siempre `--env-file .env`)

Caso A — **servicio backend (env runtime, sin `NEXT_PUBLIC_`)**: recreate basta (el env se
interpola al crear el contenedor; la imagen no cambia):

```bash
cd /opt/arbitragex-v2
docker compose --env-file .env -f docker/compose.dev.yml up -d <servicio>
```

Caso B — **toca `NEXT_PUBLIC_*` (frontend)**: RULE 03 — rebuild obligatorio + up, ambos
con `--env-file` (RULE 04: sin `--env-file`, la interpolación cae al fallback localhost):

```bash
cd /opt/arbitragex-v2
docker compose --env-file .env -f docker/compose.dev.yml build --no-cache frontend
docker compose --env-file .env -f docker/compose.dev.yml up -d frontend
```

- El servicio `edge` también recibe `NEXT_PUBLIC_*` como build args
  (`docker/compose.dev.yml:337-345`): si la propuesta declara que el edge las consume para
  su build, el comando incluye `edge` en la lista de rebuild. El registry decide por
  variable, no por intuición.
- **Prohibido en la propuesta** (el generador jamás los emite): `docker compose restart`
  para efectos de env (R03), `up` sin `--env-file` (R04), `docker compose build` a secas (R3).
- La propuesta lista los comandos en **orden de ejecución** y con `restart_required: true`:
  el recreate ES el restart (un `up -d` que no recrea porque nada cambió ⇒ la propuesta
  advierte verificar `State.StartedAt` nuevo como evidencia).

---

## 6. Formato de la propuesta: bloque auditable `propuesta_pendiente`

### 6.1 Fila en `audit_logs` (contrato para CB-02)

**Dependencia declarada:** la tabla `audit_logs` NO existe hoy — verificado read-only el
2026-09-07 (`information_schema.tables` en PG `arbitragex` devuelve vacío) ni hay
migración que la cree (`database/migrations/` 094–120, ninguna la menciona). CB-02
(GOAL-WORKORDERS.md:18) es el dueño de crearla. CB-05 **define el contrato** de la fila
que escribe; la migración concreta queda en CB-02 (o en el apply-fase de CB-05 si CB-02
declina), nunca en este WO de diseño:

```sql
-- contrato (la tabla la crea CB-02; columnas mínimas que CB-05 requiere):
INSERT INTO audit_logs (actor, action, target, payload)
VALUES (
  '<operador autenticado por x-arbx-admin-token>',
  'propuesta_pendiente',
  'control-board/class-b',
  '<JSON §6.2>'
);
```

### 6.2 Payload JSON (esquema del bloque auditable)

```jsonc
{
  "id": "CB-B-<seq>",                      // secuencial, asignado por el backend
  "wo": "CB-05", "created_at": "<ISO-8601 UTC>",
  "razon": "<texto libre OBLIGATORIO del operador>",
  "variables": [{
    "key": "ARBX_...",
    "old": "<valor vivo leído | '(ausente — default fail-safe X)'>",
    "old_source": "inventario CB-01: .env:<n> + docker inspect <svc> @ <ts lectura>",
    "new": "<valor elegido por el operador>"
  }],
  "servicios": ["<svc>"],
  "rebuild_required": false,               // true solo si toca NEXT_PUBLIC_*
  "restart_required": true,
  "diff": "<unified diff exacto §4.3>",     // bloque textual §6.3
  "comandos": ["<compose R3 en orden>"],
  "verificacion": {
    "signal": "<log/evento esperado post-restart>",
    "ejemplo": "docker logs <svc> 2>&1 | grep -m1 'route_scanner.mode'"
  },
  "rollback": { "diff": "<diff inverso>", "comandos": ["<mismos compose R3>"] },
  "estado": "pendiente",                    // pendiente|aprobada|aplicada|descartada
  "evidencia_apply": null                   // al aplicar: StartedAt + línea de log
}
```

### 6.3 El bloque textual que el operador ve (y copia al pipeline)

````
# PROPUESTA CB-B-0042 — encender ARBX_SCORING_HARD_GATE (searcher-rs)
estado: pendiente · propuesto: 2026-09-07T13:00Z · por: <operador> · WO: CB-05
razón: «<la razón obligatoria del operador>»

## Diff .env (contra estado vivo leído 2026-09-07T12:55Z — .env + docker inspect)
--- /opt/arbitragex-v2/.env   (vivo, verificado)
+++ /opt/arbitragex-v2/.env   (propuesto CB-B-0042)
+ARBX_SCORING_HARD_GATE=true

## Servicios
- searcher-rs · restart requerido: SÍ · rebuild requerido: NO (env runtime, Caso A)

## Comandos (R3, en orden)
cd /opt/arbitragex-v2
docker compose --env-file .env -f docker/compose.dev.yml up -d searcher-rs

## Verificación post-apply (evidencia para cerrar)
docker inspect arbitragex-v2-searcher-rs-1 --format '{{.State.StartedAt}}'  # debe ser > hora de apply
docker logs arbitragex-v2-searcher-rs-1 2>&1 | head -50                     # boot limpio

## Rollback (diff inverso + mismos comandos)
--- /opt/arbitragex-v2/.env   (propuesto CB-B-0042)
+++ /opt/arbitragex-v2/.env   (volver a vivo)
-ARBX_SCORING_HARD_GATE=true
cd /opt/arbitragex-v2 && docker compose --env-file .env -f docker/compose.dev.yml up -d searcher-rs
````

---

## 7. Denylist §34.3 / secretos — lo que este flujo NUNCA propone (server-side)

| Clave/patrón | Razón de exclusión |
|---|---|
| `ARBX_LIVE_EXEC_ENABLED`, `ARBX_LIVE_EXEC_CHAINS` | Terminus capital/broadcast §34.3 — `live_exec_policy.rs:12,53-54` default-deny; flip = operador-only con gates, jamás propuesta de board |
| `SIM_SIGNER_ADDRESS` y cualquier `*KEY*/*SECRET*/*TOKEN*/*PASSWORD*/*MNEMONIC*/*PRIVATE*` | Secretos §33.2 + RULE 02; el diff jamás transporta secretos |
| Cualquier var cuyo consumidor sea firma/broadcast (`relays-client` submit path) | §32 modo permanente audit/scaffold/shadow/read-only |
| Claves sin consumidor verificado en el repo | RULE 00 — no es switch, es ruido |

El rechazo es del BACKEND (403 con la cita de la regla), no solo UI: un `curl` directo al
endpoint tampoco puede crear la propuesta. La denylist es **append-only y documentada**;
quitar una entrada exige orden explícita del operador con los 3 puntos de §34.3 satisfechos
(fuera del alcance de este board por diseño — «NUNCA genera toggle de clase C»).

---

## 8. Tres ejemplos REALES completos (del inventario verificado HOY)

### E1 — `ARBX_ROUTE_SCANNER_MODE` off→on — el caso RU-3 (canónico)

**Contexto:** el switch que originó el `/goal` (GOAL-WORKORDERS.md:9-11): construido y
anclado, jamás encendido porque la clave estaba ausente del `.env` — el default fail-safe
`off` (`route_scanner_worker.rs:105-115`) lo mantenía dormido sin error alguno.

**Estado HOY (verificado read-only 2026-09-07 ~12:40Z):** ya está `on` — `.env:136`
`ARBX_ROUTE_SCANNER_MODE=on`, env del contenedor searcher-rs, y log de boot
`{"event":"route_scanner.mode","chain_id":1,"mode":"on","dispatch_path":"orchestrator"}`
a las 2026-09-07T12:13:15Z (container StartedAt 12:13:00Z). Es decir, **el flujo manual ya
ocurrió hoy** — abajo va la propuesta tal como el board la habría generado esta mañana
(estado previo documentado en GOAL-WORKORDERS.md:10: «`ARBX_ROUTE_SCANNER_MODE` ausente»):

````
# PROPUESTA CB-B-E1 — encender route_scanner multihop RU-3 (searcher-rs)
estado: (histórico — aplicado manualmente 2026-09-07T12:13Z antes de existir el flujo)
razón: «feed sin hops 2-6: scanner RU-3 construido pero jamás encendido (RU-3)»

## Diff .env
--- /opt/arbitragex-v2/.env   (vivo: clave AUSENTE — default fail-safe off)
+++ /opt/arbitragex-v2/.env   (propuesto)
+ARBX_ROUTE_SCANNER_MODE=on

## Servicios: searcher-rs · restart: SÍ · rebuild: NO (Caso A)
## Comandos (R3):
cd /opt/arbitragex-v2
docker compose --env-file .env -f docker/compose.dev.yml up -d searcher-rs

## Verificación (señal real: route_scanner_worker.rs:800-812):
docker logs arbitragex-v2-searcher-rs-1 2>&1 | grep -m1 'route_scanner.mode'
# esperado: "mode":"on" y dispatch_path="orchestrator" (v1-only daría cartridge_shadow/telemetry_only)
# skips honestos si faltan prerrequisitos: route_scanner.skipped reason=no_impact_index|no_ws_endpoints

## Rollback: quitar la línea (o poner off) + mismo up -d searcher-rs
````

**Invariantes ejercitados:** INV-B2 (old ausente declarado), INV-B5 (auditoría previa),
señal de verificación con skips honestos R8.

### E2 — `ARBX_SCORING_HARD_GATE` (ausente/false)→true — knob dormido verificado HOY

**Contexto:** el scoring es hoy advisory-only: con hard-gate off «NEVER blocks»
(`scoring_pipeline.rs:12`); `env_bool` default `false` (`scoring_pipeline.rs:95,284-288`;
acepta `1|true|yes|on`). **Verificado HOY:** la clave NO está en el `.env` del VPS
(grep selectivo 2026-09-07) ⇒ estado efectivo `false` = el gate existe pero no bloquea.

````
# PROPUESTA CB-B-E2 — activar hard-gate de scoring Bayesian (searcher-rs)
estado: pendiente (ejemplo de diseño)
razón: «scoring advisory desde always; endurecer a bloqueo por posterior»

## Diff .env
--- /opt/arbitragex-v2/.env   (vivo: clave AUSENTE — default false, advisory-only)
+++ /opt/arbitragex-v2/.env   (propuesto)
+ARBX_SCORING_HARD_GATE=true

## Servicios: searcher-rs · restart: SÍ · rebuild: NO (Caso A)
## Comandos (R3):
cd /opt/arbitragex-v2
docker compose --env-file .env -f docker/compose.dev.yml up -d searcher-rs

## Verificación:
# 1) StartedAt nuevo; 2) boot limpio sin panic; 3) en emisiones: rejects con score-reason
#    bayesian (antes pasaban advisory). Señal derivada del parse scoring_pipeline.rs:149-159:
#    hard_gate=true ⇒ !accepted NO emite.
## Rollback: quitar línea + mismo up -d searcher-rs (vuelve a advisory-only)
````

**Nota de efecto (para la aprobación del operador):** endurecer el gate puede reducir
emisiones aceptadas (más rejects honestos por prior plano en estrategias sin calibrar —
`FLAT_PRIOR` `scoring_pipeline.rs:44-47`). La propuesta debe mostrar ese trade-off porque
el board es el registro de aprobación, no un botón de sí.

### E3 — cambio de `NEXT_PUBLIC_WS_URL` — el camino con rebuild (RULE 03/04)

**Contexto:** único caso que obliga a rebuild: `NEXT_PUBLIC_*` se hornea en `next build`
(CLAUDE.md §3 R03). **Verificado HOY:** `.env:129` `NEXT_PUBLIC_WS_URL=http://195.201.235.70`
(bare IP). El **nuevo valor lo escribe el operador en el form** (variable tipo URL — no
enum; el generador NO propone valores, §4.3-2: sin inventar datos, RULE 00). El diff
exacto abajo usa `<URL_WS_NUEVA>` como marcador del input validado del operador:

````
# PROPUESTA CB-B-E3 — mover NEXT_PUBLIC_WS_URL a <URL_WS_NUEVA> (frontend + edge)
estado: pendiente (ejemplo de diseño; new-value = input del operador, validado como URL)
razón: «<razón del operador>»

## Diff .env
--- /opt/arbitragex-v2/.env   (vivo: NEXT_PUBLIC_WS_URL=http://195.201.235.70 — .env:129, leído 2026-09-07)
+++ /opt/arbitragex-v2/.env   (propuesto)
-NEXT_PUBLIC_WS_URL=http://195.201.235.70
+NEXT_PUBLIC_WS_URL=<URL_WS_NUEVA>

## Servicios: frontend (rebuild SÍ) · edge (rebuild SÍ — recibe NEXT_PUBLIC_* como build args,
##             docker/compose.dev.yml:337-345) · restart: SÍ (implícito en up -d)

## Comandos (R3 + RULE 03 — rebuild --no-cache OBLIGATORIO, jamás restart):
cd /opt/arbitragex-v2
docker compose --env-file .env -f docker/compose.dev.yml build --no-cache frontend
docker compose --env-file .env -f docker/compose.dev.yml up -d frontend
# (si el registry declara consumo por build del edge, anteponer el mismo par build/up para edge)

## Verificación (RULE 04):
curl -I http://127.0.0.1:5173/opportunities    # CSP/asset URLs deben reflejar <URL_WS_NUEVA>;
                                               # si contiene localhost → LA REGLA FUE VIOLADA (R04)
# + en browser: WS conecta a la nueva URL (RULE 02: WS va DIRECTO a api-server, nunca por edge)

## Rollback: restaurar la línea vieja + mismos comandos build --no-cache + up -d
````

**Invariantes ejercitadas:** INV-B4 (rebuild obligatorio + CSP check), §4.3-2 (valor nuevo
= operador, no sugerido).

---

## 9. Integración con los demás WOs (bordes de responsabilidad)

| WO | Borde con CB-05 |
|---|---|
| CB-01 | CB-05 consume el inventario como fuente del «valor vivo» y del registry §4.2. Si CB-01 clasifica una var como A (runtime-pollable) que aquí figurara B, **gana CB-01** (A > B: si ya es polleable, el board debe togglearla en runtime, no proponer restart). |
| CB-02 | CB-05 requiere: tabla `audit_logs` (§6.1) y monta su endpoint bajo el mismo `/api/v1/control-board` con `x-arbx-admin-token` + razón obligatoria. Contrato declarado aquí; implementación en CB-02/apply. |
| CB-03 | El botón «Proponer cambio» + badge «B · requiere-restart» + sección «Propuestas pendientes» viven en `/control`. Un LED B JAMÁS es clicable como toggle — el click abre la propuesta. |
| CB-04 | El drift-guard ve cambios de env no cubiertos por propuesta `aplicada` ⇒ alerta. La propuesta `aplicada` + evidencia es el único cierre legítimo de un drift clase B. |
| CB-06 | Browser-verifica: proponer (fila aparece), descartar (estado cambia), y E2E del ejemplo E2 real con aprobación del operador — el board NO aplica, el verificador observa al operador aplicar y el LED cambiar vía inventario. |

---

## 10. Qué se verificó para este diseño (evidencia, todo read-only)

- Repo: parses reales de cada var clase B citada (§1, file:line); `.env.example` como
  template canónico; `docker/compose.dev.yml` servicios y build-args NEXT_PUBLIC_*
  (líneas 337-348); `scripts/deploy.sh` (lock, .env export línea ~76, verificación
  de rama/main R-0004); sin tabla `audit_logs` en `database/migrations/` (094–120).
- VPS (ssh `arbx`, SOLO lectura, ~12:40Z 2026-09-07): presencia/valores de knobs no-secretos
  en `/opt/arbitragex-v2/.env` (grep selectivo §1); `docker inspect` env del searcher-rs
  vivo (coincide 1:1 con `.env` — sin drift hoy en las claves relevadas); `StartedAt`
  searcher 12:13:00Z vs flota 6h; log `route_scanner.mode mode=on` 12:13:15Z;
  `information_schema.tables` sin `audit_logs`; `docker ps` flota 25/25 healthy.
- **No verificado / declarado:** valores secretos (no leídos por diseño); estado pre-12:13Z
  del `.env` (el diff E1-old cita GOAL-WORKORDERS.md:10 como fuente documental, no
  observación propia); contenido de CB-01/CB-02 (aún no existen — §encabezado).

## 11. Fuera de alcance (design-only)

Cero ejecución: no se creó tabla, no se modificó código de producción, no se editó `.env`
local ni VPS, no se propuso ningún PR (NO-GIT). Los comandos de §5/§8 son SALIDA del
generador diseñado, no comandos ejecutados. Aplicar este diseño (endpoint + UI + migración
audit_logs) es trabajo de apply-fase posterior con sus propios gates.
