# CB-04 (apply) — Drift-guard SOBERANO ("ni Dios mueva si no es mi deseo") [v2, retarget ledger]

- **WO**: CB-04 · kind: apply · **Agente**: ecc:typescript-reviewer (Gang Omniscience, PhD)
- **Fechas**: apply base 2026-09-07 · **retarget ledger (esta pasada, cierre del MUST-FIX del par)** 2026-09-09 00:0x-00:2x local · **NO-GIT** (cero commit/push/PR/deploy; VPS intocado, 0 ssh, presupuesto público 0/5)
- **Branch**: `feat/hops-live-01` · HEAD `27aca289` verificado intacto al cierre
- **Archivos bajo claim** (nadie más los toca):
  `backend/api-server/src/services/control-board-drift.ts` (891 líneas) ·
  `backend/api-server/src/services/control-board-drift.test.ts` (723) ·
  `backend/api-server/src/routes/control-board.ts` (934 — extensión ADITIVA del CB-02, sin edits en esta pasada) ·
  `frontend/lib/drift/useDriftDetection.ts` (+174/−0 — sin edits en esta pasada: el wire contract no cambió)

---

## 0. Genealogía de este reporte (v2 — por qué hubo una segunda pasada)

La v1 (2026-09-07 09:51) aplicó el guard contra el contrato de ledger que el apply v2 de CB-02
prometía (`audit_logs` plural + `payload.applied` corregible). CB-02-API-APPLY §3 (cierre
2026-09-08 23:5x) **REFUTÓ ese contrato** con evidencia: la tabla real es **`audit_log`
(SINGULAR, migración `011_audit_log.sql`)** — columnas `actor/action/target_kind/target_id/
before_state/after_state` (011:4-16), append-only por `REVOKE UPDATE, DELETE` (011:21) — y un
SET fallido se registra con **INSERT compensatorio `control_board.toggle_failed`** (acción
DISTINTA, misma payload `at`), jamás con un UPDATE de la fila original. Su §5.1 dejó el
MUST-FIX apuntando a este claim: *"sus 3 queries siguen sobre audit_logs PLURAL — montado
contra VPS `loadApprovedToggles` fallará siempre → el guard degrada honesto a
`audit_unavailable` = drift-guard sin función. Fix necesario: mismas columnas/tabla de
`loadLastToggles` (control-board.ts:433-480)"*. Esta pasada ejecuta exactamente ese fix.

**Delta de ESTA pasada** (solo `control-board-drift.ts` + su test; la ruta y el frontend no
necesitaron cambios):

| # | Cambio | Evidencia (file:line final) |
|---|---|---|
| D1 | **Query de aprobaciones → `audit_log` + cancelación compensatoria**: `SELECT DISTINCT ON (target_id) … FROM audit_log a WHERE target_kind='control_board_module' AND action=$1 AND target_id=ANY($2) AND NOT EXISTS (compensatorio misma `at`) ORDER BY target_id, created_at DESC` — un intento fallido NO es aprobación; el fallback es la aprobación ANTERIOR aterrizada | `control-board-drift.ts:251-298` (docblock :237-250, NOT EXISTS :265-271) |
| D2 | **Parse estricto del `on`**: solo `after_state.on` booleano explícito cuenta como aprobación — un payload garbage/ajeno NO arma el módulo (la v1 hacía `on.on === true` ⇒ un payload sin `on` se leía como "aprobado OFF": bug RULE 00 latente, corregido) | `control-board-drift.ts:288` |
| D3 | **Query de dedup → `audit_log`** (`target_kind='control_board_module'`, fingerprint en `after_state`) | `control-board-drift.ts:301-341` |
| D4 | **INSERT drift → columnas 011**: `(actor, action, target_kind='control_board_module', target_id, before_state, after_state)`; before_state = `{on: observed_on}` (el estado DRIFTADO observado — honesto), after_state = diff exacto completo con el outcome verdadero del revert | `control-board-drift.ts:728-742` |
| D5 | **Docblock refutado reescrito** (semántica "CB-02 corrige a applied:false" → INSERT compensatorio) + limitación honesta documentada: si el INSERT compensatorio del CB-02 falla (`control_board.audit_compensate_failed`), el intento queda sin cancelar — el guard defiende lo que el ledger registra | `control-board-drift.ts:9-33` |
| D6 | **Test**: fake pool al contrato 011 real (INSERT 5 valores, DISTINCT ON latest-per-target_id, cancelación compensatoria simulada fiel a la SQL) + 2 tests nuevos: (10) cancelación compensatoria, (11) garbage after_state | `control-board-drift.test.ts:151-217, 397-429, 431-458` |

**Cero referencia a `audit_logs` plural queda en el servicio** (grep = 0; el único resto del
término en la serie es el comentario del par en `control-board.ts:102` que documenta que esa
tabla NO se crea — texto CB-02, no mío).

## 1. Semántica implementada (el board es el REGISTRO DE APROBACIÓN)

Comparación periódica por módulo entre **APROBADO** y **OBSERVADO**:

| | APROBADO | OBSERVADO |
|---|---|---|
| **Clase A** | última fila `control_board.toggle` en `audit_log` SIN hermano compensatorio `control_board.toggle_failed` de la misma `at` (`:251-298`) — solo `after_state.on` booleano explícito (`:288`) | valor VIVO de la `control_key` Redis, parse estricto `"true"/"false"` (`parseDeclaredValue` de CB-02 — valor ajeno ⇒ null, jamás interpretado) |
| **Clase B** | `declared_on` del censo (lo que el operador desplegó — el env ES la declaración) | boot env del contenedor api-server (`env:VAR` del censo), comparado SOLO si es booleano inequívoco `"true"/"false"` (`parseStrictBoolean`, `:204-210`) |
| **Clase C** | — | **no observable** desde este servicio — reportado como tal, jamás simulado (§34.3 candado) |

**Emparejamiento compensatorio por `at`**: CB-02 escribe la fila toggle y (si el SET falla) la
compensatoria desde la MISMA constante `at` (control-board.ts PUT `:832-842` vs `:899`), así
que `(f.after_state->>'at') = (a.after_state->>'at')` empareja el intento con su cancelación.
Con `=` SQL plano (no `IS NOT DISTINCT FROM`), dos filas sin `at` jamás se emparejan — un
INSERT ajeno at-less no puede cancelar una aprobación (defense in depth, `:240-247`).

En mismatch, en orden (`:616-675`):
1. **(c) REVERT primero (solo clase A)** — `redis.set(control_key, approved_on ? "true" : "false")` (`:644`): el cambio externo se DESHACE restaurando el último valor aprobado por el operador. Esto NO es auto-flip: es la restauración de lo aprobado (lo opuesto a flipear).
2. **(a) audit_log DESPUÉS con el outcome verdadero** — INSERT `control_board.drift` con el diff exacto `{wo, module, control_key, module_class, approved_on, observed_on, observed_raw, reason, reverted, revert_applied, at, scan_id, fingerprint}` (`maybeAuditDrift`, `:687-755`), actor sistema `control-board-drift-guard` (`:116`) — distinguible de los actores operador de CB-02.
3. **(b) payload de alerta visible** — banner `drift` dentro del snapshot del board (GET y PUT, `control-board.ts:656-671, 922-924`) + diff completo en `GET /api/v1/control-board/drift` (`control-board.ts:678-689`) que es el `diff_href` del banner.

Clase B/C en drift: alerta + diff **SIN revert** — remediar boot-env exige restart (flujo CB-05),
se declara honesto, jamás se simula (`:528-543`).

### Por qué revert-primero y audit-después (desviación documentada de CB-02)

CB-02 niega el Redis-write si el INSERT de auditoría falla porque un toggle es una acción NUEVA
del operador. Aquí la aprobación que se defiende YA está en el ledger (la fila de toggle que
definió `approved_on`) — la fila drift es el registro del EVENTO. Con revert-primero, la fila
lleva el resultado VERDADERO del revert (R8: el ledger jamás registra un resultado especulativo,
y un revert que aterrizó jamás queda sin registrar). Si el INSERT aún falla: `audit_persisted:false`
en el report + `logger.warn` ruidoso (`:740-748`) — nunca silencioso. Demostrado en test (6).

### Dedup R9 (anti-flood, lección LOGFLOOD-01)

Fingerprint = `[module, reason, approved_on, observed_raw, revert_applied]` (`:337-345`); solo se
INSERTa si difiere del último fingerprint del módulo en `audit_log` (DISTINCT ON, `:301-341`).
Un drift persistente NO inunda el ledger por tick; un intento fallido seguido de un retry exitoso
SÍ obtiene su fila propia (outcome distinto). Demostrado en test (6): 3 scans ⇒ 2 filas.

### Línea-base de soberanía (honesto)

El guard defiende APROBACIONES registradas. Una clave clase A sin ningún toggle del operador ⇒
`no_approval_record`: el estado observado se expone en el diff pero NO hay drift declarado ni
revert. El primer PUT del operador (CB-02) arma el guard para ese módulo (`:578-596`).

### Carrera guard↔PUT (análisis, no inventada)

Si un tick del guard cae entre el INSERT de auditoría y el SET de un PUT: el guard ve
aprobado=valor nuevo, observado=valor viejo ⇒ drift ⇒ SET al valor NUEVO (el aprobado) — y el
SET del PUT aterriza idempotente con el mismo string. Nunca pelean: ambos escriben el valor
aprobado. Ventana de milisegundos, cadencia 60s, convergencia al ledger.

## 2. EL FLUJO REVERT DEMOSTRADO EN TEST (fixture, jamás producción)

`control-board-drift.test.ts:247-326` — test (1) "THE CHARTER REVERT FLOW":

1. Fixture: censo CB-01 con módulo clase A `route_scanner_multihop`, `control_key:
   arbx:runtime:route_scanner`; ledger con aprobación `on:true` aterrizada; **Redis observado
   `"false"`** — alguien flipó la clave por fuera del board.
2. `runControlBoardDriftScan()` ⇒ asserts:
   - `status: "drift_detected"`, entry `drift:true, approved_on:true, observed_on:false, reason:"approved_vs_observed_mismatch"`;
   - **`redisSet` llamado con `("arbx:runtime:route_scanner", "true")`** — LA reversión al valor aprobado, en el dialecto exclusivo del board (`test:268-270`);
   - fila `control_board.drift` insertada con el diff exacto y `revert_applied:true`, contra el contrato 011: INSERT de 5 valores sobre `audit_log` con `target_kind` inline, before_state `{on:false}` = el estado driftado (`test:272-296`);
   - orden: revert ANTES del INSERT (ledger registra el outcome verdadero) (`test:298-301`).

Variantes: valor EXTRANJERO (`"enabled=yes"`) ⇒ `observed_foreign_value` + revert (2a, `:303-315`);
clave BORRADA ⇒ `observed_absent_with_approval` + revert restaura (2b, `:317-328`); revert FALLA ⇒
`revert_applied:false`, persiste, dedup, retry exitoso obtiene su fila (6, `:360-395`); clase B ⇒
drift SIN ningún write Redis (4, `:453-471`). **Nuevos de esta pasada**: (10) intento fallido con
hermano compensatorio NO es aprobación — el fallback es la aprobación aterrizada anterior, y un
flip externo posterior se revierte al valor ATERIZADO (`:397-429`); (11) after_state sin `on`
booleano ⇒ `no_approval_record`, jamás "aprobado false" (`:431-458`).

El fake pool implementa el contrato EXACTO de `audit_log` que CB-02-API pineó (INSERT 5 columnas
/ DISTINCT ON target_id) y simula la cancelación compensatoria de la NOT EXISTS fiel a la SQL
real (`test:163-215`) — la segunda lectura del dedup ve su propia historia (semántica de ledger
real, no un map stubbeado).

## 3. Extensión de routes/control-board.ts — SOLO aditiva (intacta de la v1)

Todo marcado `// CB-04 (2026-09-07)`: docblock del endpoint (`:50-56`) · import del servicio
(`:115-120`) · `export loadCensus` (UN loader para snapshot y scan — jamás discrepan;
`:369-371`) · `driftGuardIntervalMs?` (`:631-633`) · retorno `MountedControlBoard { stopDriftGuard }`
(`:636-640, 933`) · guard interval-only (`:645-651`) · banner en respuestas GET/PUT
(`:656-671, 922-924`) · **endpoint de diff** `GET /api/v1/control-board/drift` admin-gated
(`:678-689`; `scanIfDue` TTL 5s anti-stampede; sin scan completado ⇒ 503 honesto
`drift_report_unavailable`). **Cero línea de CB-02 eliminada** — los 33 tests del par corren
VERDE sin un solo cambio (§5). El mount en `index.ts:139,778` + shutdown `:2042` (claim del
par, verificado presente, no tocado).

## 4. Evaluación de reuso frontend/lib/drift (charter — sin cambios esta pasada)

**Decisión: reuso PARCIAL — patrón sí, retarget no.** Documentado en el propio archivo
(`useDriftDetection.ts:140-157`): `useDriftDetection` queda intacto (polea `/api/v1/drift/status`,
endpoint que ningún backend emite — gap declarado; su shape es dominio registry-drift con
consumidor vivo `DriftPanel.tsx`); `useOmniDrift` intocado (dominio distinto, 3 consumers vivos).
Lo reusado: la disciplina (AbortController + timeout 8s + narrowing honesto + `credentials:"include"`
para el adminProxy del edge) en el hook NUEVO `useControlBoardDrift` (`useDriftDetection.ts:223-312`)
contra `GET /api/v1/control-board/drift`, wire-types espejo (`:159-211`). El retarget del ledger de
esta pasada NO cambia el wire contract del report ⇒ el cliente NO requirió edits (diff +174/−0
preexistente preservado byte a byte).

## 5. Verificación (corrida por mí, 2026-09-09 00:0x-00:2x)

| Gate | Resultado |
|---|---|
| `npx vitest run src/services/control-board-drift.test.ts src/routes/control-board.test.ts` | **61/61 PASS** (28 CB-04 = 26 v1 + (10)/(11) nuevos; 33 CB-02 sin tocar) — v1.6.1, 9.59s |
| `npx vitest run src/edge-parity.test.ts` | **6/6 PASS** (suite adyacente del diff ajeno) |
| `npm run typecheck` (api-server, `tsc --noEmit -p tsconfig.json`) | **EXIT 0** |
| `npx tsc --noEmit -p tsconfig.json` (frontend) | **EXIT 0** (~6 min bajo carga del gang — precedente del par) |
| `grep audit_logs` en servicio | **0 hits** (el único resto es el comentario CB-02 `control-board.ts:102` que documenta la refutación) |
| Cirugía / NO-GIT | Solo mis 2 archivos editados esta pasada (servicio + test); ruta y frontend intactos (diffs CB-04 preexistentes preservados); `git rev-parse HEAD` = `27aca289` intacto; 0 commits. VPS: 0 ssh. Presupuesto dominio público: **0/5 requests HTTP** |
| Suite completa api-server | NO corrida localmente (máquina cargada por el gang — misma decisión declarada que CB-02-API-APPLY §4). Superficie acotada: solo mis 3 archivos importan control-board*.ts |

## 6. Sincronía de mesa — construcción sobre pares

**Construye sobre**: CB-02-API-APPLY §5.1 (el MUST-FIX que esta pasada cierra), §2 (estado final
del endpoint — mi banner cabalga sus respuestas), §3 (refutación `audit_logs`→`audit_log`,
migración 011); CB-02-DISENO §15-R1 (R1/R3/G4 retargets) y §7/D7 (hash de aprobación como
registro que CB-04 defiende); CB-03-APPLY §2.3 + §148 (ratificó consumir el shape exacto del
banner dentro del snapshot y el `diff_href`); CB-01-MODULES.json (44 módulos: A=3 con
`control_key:null`, B=38, C=3 — con el censo actual el scan reporta honesto: A sin control_key ⇒
`census_invalid_control_key`, B comparables solo si env booleano inequívoco, C skipped).

**Cierre del punto 5.1 del par**: las 3 queries y el INSERT ahora golpean `audit_log` con las
mismas columnas que `loadLastToggles` (control-board.ts:433-480). Montado contra VPS,
`loadApprovedToggles` ya no falla por tabla inexistente — el banner de CB-03 deja de decir
"audit_logs no disponible" en cuanto censo + aprobaciones existan.

**Merge points que NO puedo ejecutar (fuera de mi claim):**
1. **Edge worker** (pendiente abierto de la v1): falta la línea hermana del adminProxy para
   `GET /api/v1/control-board/drift` (`edge/worker/src/index.ts:1594-1595` solo proxya el board
   GET/PUT) — sin ella, el `diff_href` del banner 404a por el borde público.
2. **Publicación del censo** (G6, operator-gated): sin publisher de `arbx:config:control_board`
   el scan reporta honesto `census_absent` — el guard queda armado pero sin módulos comparables.
3. **Hardening futuro (declarado, NO implementado — P-∅)**: cross-check ledger↔hash
   `arbx:controlboard:approved` como segunda fuente de aprobación. Hoy el ledger es la única
   fuente (mandato del par); el hash es cache de serving del CB-02.

**Contradicciones con pares**: NINGUNA — el par refutó el contrato v1 y esta pasada adopta el
suyo íntegro. Nota de convergencia con CB-03-APPLY :201 (F-4): el banner nunca pinta un "none"
sin caveat sobre estados NO computables — `toControlBoardDriftBanner` (`:757-804`) dice
"NO computable" textual en census_absent/census_unreadable/audit_unavailable.

## 7. Cumplimiento reglas duras

- **RULE 00 / R8**: censo ausente/corrupto/ledger caído ⇒ `census_absent`/`census_unreadable`/`audit_unavailable` — SIN claims, jamás "consistent" por ignorancia. Valor ajeno ⇒ null + reason. after_state sin booleano explícito ⇒ no es aprobación (test 11). Env no-booleano ⇒ not_comparable. Cero estados/alertas fabricados.
- **§32/§33**: cero executor/wallets/capital/firma/broadcast. VPS intocado (0 ssh, 0 HTTP público).
- **§34.3**: clase C no observada/no revertida/no togglable; denylist terminus del CB-02 intacta (ni referenciada); `default-deny`/`MainnetRefused` intocados. El revert SOLO restaura booleanos de control_keys clase A del censo dentro del namespace `arbx:controlboard:*` — jamás toca gates del terminus.
- **NO-GIT**: cero commit/push/PR/deploy. HEAD `27aca289` verificado. Edición local + verificación.
- **Diffs marcados**: `// CB-04 (2026-09-07)` en las superficies editadas (heredado v1 + hunks D1-D6).
- **Anti-flood R9**: dedup por fingerprint con outcome (§1) — un loop honesto que no destruye observabilidad.
