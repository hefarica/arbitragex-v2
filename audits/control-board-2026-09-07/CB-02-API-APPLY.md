# CB-02 (API apply) — mitad TS/API del control plane runtime [v3, estado final 2026-09-08]

- **WO**: CB-02 · kind: apply (mitad TS/API) · **Agente**: ecc:typescript-reviewer (Gang Omniscience)
- **Fechas**: apply base 2026-09-07 08:31Z · retargets R1/R3/G4 + mount 2026-09-07 ~14:28-14:30 ·
  **cierre §4.2-p5/§4.3 (esta pasada) 2026-09-08 23:0x-23:5x local**
- **Branch**: `feat/hops-live-01` · HEAD `27aca289` intacto (verificado al cierre — NO-GIT: 0 commit/push/PR)
- **Archivos bajo claim**: `backend/api-server/src/routes/control-board.ts` ·
  `backend/api-server/src/routes/control-board.test.ts` · `backend/api-server/src/index.ts` ·
  `database/migrations/121_control_board_audit_logs.sql` (**NO creada — ver §3, refutación R1**)

---

## 0. Genealogía del estado (sincronía de mesa — quién hizo qué, nada mío sin acreditar)

Este WO pasó por CUATRO manos antes del cierre; el reporte v2 (09:22) quedó stale en dos puntos
(cross-exam G5) y esta versión los corrige:

| # | Sesión | Aporte | Evidencia |
|---|---|---|---|
| 1 | 2026-09-07 08:31Z (RESPAWN-2 A) | Apply base census-driven: routes + 22 tests, tsc EXIT 0. Declaró honestamente que CB-02-DISENO no existía aún y dejó merge-points (su §3) | CB-02-API-APPLY v2 §0-§3 (preservado en git-less árbol; el file mtime 09:22) |
| 2 | 2026-09-07 09:15 (design) | CB-02-DISENO.md §15 adjudicó R1-R10; ordenó retargets R1 (audit_log singular + INSERT compensatorio) y R3 (verified live) | CB-02-DISENO.md:503-535 |
| 3 | 2026-09-07 ~14:28-14:30 | **Aplicó G1-G4 del cross-exam** (CB-02-API-VERIFY.md): mount en index.ts:778 + R1 + R3 + guard de namespace (G4) + tests actualizados a la semántica nueva. Sesión sin reporte propio — este reporte la documenta | mtimes: control-board.test.ts 14:28, index.ts 14:30; código verificado línea a línea por mí |
| 4 | 2026-09-08 17:48 (agente caído) | Preparó constantes para el gate §4.3 not-spawned (`BOOT_CENSUS_REDIS_KEY`, `ROUTE_SCANNER_BOARD_TOGGLE_KEY`) + docblock — murió ANTES de implementar el gate (trabajo en vuelo que heredo y completo) | mtime control-board.ts 17:48; constantes sin consumidores al inicio de mi pasada (grep: 0 usos) |
| 5 | **2026-09-08 23:0x-23:5x (esta pasada, yo)** | Cierre de los 3 items §4.2/§4.3 aún mandatados y faltantes (ver §1) + remoción de la constante muerta anti-R10 + este reporte v3 | diffs `// CB-02 (2026-09-07)` en todos los hunks nuevos |

Mi charter pedía también "database/migrations/121_control_board_audit_logs.sql (DDL del diseño)" —
el DISEÑO (autoridad superior según el propio charter: *"implementa EXCLUSIVAMENTE lo que
CB-02-DISENO.md mande"*) REFUTA esa migración (§5 + §15-R1): crearla sería trabajo inventado
(P-∅ §37). Ver §3.

## 1. Delta de ESTA pasada (lo único que edité yo — todo mandatado por CB-02-DISENO)

Los retargets R1/R3/G4 y el mount ya estaban aplicados (fila 3 de §0; verificados). Lo faltante
contra §4.2/§4.3 y GATE-CB02-1, ahora cerrado:

| # | Item del diseño | Implementación (file:line final) |
|---|---|---|
| D1 | **§4.3/GATE-CB02-1 — gate not-spawned**: "id=route_scanner AND census mode=off → 409 module_not_spawned con hint CB-05". Los 3 clase-A del censo CB-01 hoy llevan control_key:null y route_scanner es clase B, así que el gate vive dormido hasta el censo futuro que lo declare clase A — exactamente el diseño §8 filas 2/15 (spawn=B, run-gate=A) | `control-board.ts:795-815`: tras gate 7 (namespace), `controlKey === ROUTE_SCANNER_BOARD_TOGGLE_KEY && module.declared_on === false` → 409 `{error:"module_not_spawned", detail:"…CB-05 proposal flow (env + restart)"}`. Keyed on the RESOLVED key (todo dialecto de censo que binde el scanner lo recibe). Explicit-false only: null/unknown boot state sigue toggleable y el LED muestra DESCONOCIDO honesto (fail-honest del diseño: el gate es "census mode=off") |
| D2 | **§4.3 happy flow — SET + HSET**: "SET arbx:controlboard:route_scanner "true"\|"false" + HSET arbx:controlboard:approved route_scanner <json {on,reason,actor,updated_at}>" (también §1.2 fila approved-hash y §7/D7: registro de aprobación que CB-04 defiende) | `control-board.ts:864-880`: tras el INSERT de auditoría, `redis.set(controlKey, on?"true":"false")` + `redis.hset(CONTROL_BOARD_APPROVED_HASH, id, JSON.stringify({on, reason, actor, updated_at: at}))`. Constante exportada `CONTROL_BOARD_APPROVED_HASH = "arbx:controlboard:approved"` (:154). Fallo de CUALQUIERA de los dos writes → misma rama compensatoria `control_board.toggle_failed` (applied:false + redis_error) + 500 (:881-916) — el ledger jamás reclama una aprobación parcialmente aterrizada (INV-CB02-2 cubre ambos writes del namespace) |
| D3 | **§4.2 paso 2+5 — declared_on = approved[field] ?? census_default (clase A)**. El código heredado tomaba el valor LIVE de la clave toggle: un actor externo que escribiera la clave se mostraría como "declarado por el operador" (escenario (iv): desaparecería el drift visual — exactamente lo que el operador prohibió, GOAL-WORKORDERS.md:3-7 "ni Dios mueva si no es mi deseo") | `control-board.ts:513-520` (HGETALL fail-soft → warn `control_board.approved_read_failed`, degrada a census default, nunca bloquea) · `:595-598` (declaredOn = parseApprovedEntry(approved[id]) ?? census declared_on ?? null SOLO para rows board_toggle; resto census ?? null) · `parseApprovedEntry` estricta (:325-341, JSON {on:bool} — garbage/foreign ⇒ null jamás interpretado). La lectura del toggle key en GET se ELIMINÓ (vestigio pre-R3: ningún campo del snapshot la consume bajo §4.2-p5) |
| D4 | **R10 — boot census NO es input TS** ("Ninguno en TS"): la constante `BOOT_CENSUS_REDIS_KEY` del agente caído tenía 0 consumidores y su docblock prometía un gate que leía el boot census | Eliminada; el gate D1 lee el `declared_on` del CENSO CB-01 (`arbx:config:control_board`), documentado en `control-board.ts:157-163` |

Wire contract SIN cambios: snapshot = ControlBoardSnapshotSchema exacto (CB-03/CB-04 siguen
isomorfos); el banner `drift` de CB-04 sobre las respuestas se preserva intacto (aditivo,
aceptado por la mesa).

## 2. Estado final del endpoint (verificado por mí, lectura completa del archivo)

**GET /api/v1/control-board** (admin-gated, `control-board.ts:664-673`): redis null → 503 ·
censo ausente → 200 `{modules:[]}` (charter) · corrupto/schema → 503 ruidoso · por fila:
declared §4.2-p5 (D3), verified LIVE para board_toggle (hb `<key>:hb`, expirada ⇒ null
DESCONOCIDO conservando último verified_at) y killswitch-link (JSON enabled invertido), census
verbatim para el resto · enriquecimiento quién/por qué por UN `SELECT DISTINCT ON (target_id)`
con `action IN (toggle, toggle_failed)` (:442-449) — PG caído ⇒ nulls, board sigue 200.

**PUT /api/v1/control-board** (`:692-931`), gates en orden: pool null → 503 db_unavailable ·
zod `{id,on,reason}` con `reason trim().min(1).max(1000)` (R9) → 400 · censo ausente → 503
fail-safe · id ∉ censo → 404 · **terminus denylist §34.3 → 403 ANTES del check de clase**
(defense in depth, R4) · clase C → 403 · clase B → 409 (CB-05) · clase A sin control_key → 503 ·
**namespace propio `arbx:controlboard:*` o 503** (G4/INV-CB02-4 — un censo defectuoso jamás
hace al board escribir clave ajena) · **not-spawned → 409** (D1) · before-state estricto ·
**audit-first** INSERT `audit_log` (011: target_kind/target_id/before_state/after_state) ·
SET + HSET idempotentes (D2) · fallo Redis ⇒ INSERT compensatorio `control_board.toggle_failed`
— NUNCA UPDATE (011:21 REVOKE) · respuesta = snapshot fresco + banner drift.

**Mount**: `index.ts:136-139` (import) + `:768-784` (mount junto a mountCanonicalKnobs,
patrón requireAdminToken+adminToken del mountAdminChains :133) + shutdown `:2041-2042`
(stopDriftGuard). Montado ANTES de mountStubs (precedente de orden service-control). Sin edits
míos en esta pasada — verificado presente y correcto.

**Edge** (claim ajeno, verificado read-only): `edge/worker/src/index.ts:1594-1595` GET+PUT via
adminProxy, never-cached, statuses verbatim. NO tocado por mí.

## 3. Migración 121: NO EXISTE por diseño (corrección del hand-off stale — cross-exam G5a)

El §5.1 del reporte v2 ordenaba crear `audit_logs` (plural) — REFUTADO por CB-02-DISENO §5 con
evidencia: la tabla real es **`audit_log` (singular)** — migración `011_audit_log.sql` (CREATE
:4-16, índices :18-19, **REVOKE UPDATE/DELETE :21**), particiones mensuales 019, PII-hardened
053/055/070; writer vivo `writeAudit()` index.ts:362-383. El `UPDATE audit_logs SET payload…`
del v2 era doblemente imposible (tabla inexistente + append-only). `database/migrations/` termina
en 120 (verificado hoy). **Nadie debe crear la 121 siguiendo el v2 — este reporte la reemplaza.**
El mecanismo correcto (INSERT compensatorio) ya vive en `control-board.ts:888-918`.

## 4. Verificación (corrida por mí, 2026-09-08 23:4x-23:5x)

| Gate | Resultado |
|---|---|
| `npx vitest run src/routes/control-board.test.ts` | **33/33 PASS** (v1.6.1, 3.34s) — 29 heredados + c5/c6 (declared §4.2-p5: aprobación gana a clave tampeada; garbage hash ⇒ null) + n/n2 (not-spawned 409 sin audit/SET/HSET; declared_on:true NO dispara el gate) + (j)/(l) extendidas (HSET shape {on,reason,actor,updated_at}; orden audit<SET<HSET; HSET jamás corre tras SET fallido) |
| Suites adyacentes (imports de mis exports + diff ajeno preservado) | **53/53 PASS**: control-board-drift 26 (CB-04 consume loadCensus/parseDeclaredValue/CONTROL_BOARD_TOGGLE_AUDIT_ACTION — intactos) + edge-parity 6 (diff ajeno M preservado byte a byte) + admin-chains 16 + canonical-knobs 5 |
| `npm run typecheck` (tsc --noEmit -p tsconfig.json) | **EXIT 0** |
| Suite completa api-server (`npx vitest run`) | Ver §6 — corrida en background, resultado anotado al cierre de este reporte |
| Cirugía / NO-GIT | Solo mis 2 archivos claim editados (control-board.ts/.test.ts); index.ts M = diff preexistente (mount fila-3 de §0), NO editado por mí; edge/worker M y edge-parity.test.ts M = ajenos preservados; `git rev-parse HEAD` = `27aca289` intacto; 0 commits. VPS: **0 ssh** (todo verificación local). Presupuesto dominio público: **0/5 requests HTTP** |

## 5. Hand-off a la mesa (MUST-FIX ajeno + stale)

1. **CB-04 drift guard INERTE en runtime — `services/control-board-drift.ts` (claim CB-04, NO lo toqué)**: sus 3 queries siguen sobre `audit_logs` PLURAL (:231-235, :269-273, :681-683) — tabla que NO existe (§3) — y su docblock (:10-14) describe la semántica refutada "CB-02 corrects to applied:false" (hoy es INSERT compensatorio con action distinta). Sus 26 tests pasan porque el pool es mock (jamás tocan PG real), pero montado contra VPS `loadApprovedToggles` fallará siempre → el guard degrada honesto a `audit_unavailable` (fail-safe propio, :354-364) = drift-guard sin función. Fix necesario: mismas columnas/tablo de `loadLastToggles` (control-board.ts:405-427) — `audit_log` + `target_kind='control_board_module'` + `after_state`. Mientras tanto el banner CB-03 dirá "audit_logs no disponible" — honesto pero permanente.
2. **G6 sigue operator-gated** (sin cambio): ningún publisher de `arbx:config:control_board` ⇒ GET `modules:[]` + PUT 503 `census_not_published` (honesto, 0 LEDs) hasta que CB-01/censo se publique por pipeline; wiring Rust (RuntimeToggleClient + hb + boot census §3 del diseño) sigue PENDIENTE íntegro.
3. **GOAL-WORKORDERS.md:18 stale**: la fila CB-02 aún dice "NO-OP honesto" — el estado real es apply TS completo+montado (esta fila la actualiza el orquestador, no yo).
4. **CB-03 nota**: el snapshot ahora sirve declared_on desde el hash de aprobación; sin aprobación y sin default de censo ⇒ null (DESCONOCIDO) — `driftStatus` del cliente (declared≠verified) enciende el badge exactamente en el caso de tamper externo (test c5 lo pinea end-to-end del lado API).

## 6. Suite completa api-server — resultado

Corrida en background durante la redacción de este reporte; estado al cierre de esta edición:

- **PENDIENTE al momento de guardar** (vitest aún corriendo — la suite completa tarda ~8.5 min
  bajo la carga del gang, precedente H-1 §2 del 2026-09-08). RESULTADO REAL anotado abajo al
  aterrizar; no se declara nada antes de observarlo (RULE 00).

<!-- SUITE_RESULT: (pendiente) -->

## 7. Cumplimiento reglas duras

- **RULE 00/R8**: censo ausente = `modules:[]`; sin señal verificada = null DESCONOCIDO jamás
  false; valores ajenos (toggle key, hash approved, hb, killswitch JSON) NUNCA interpretados
  (parsers estrictos :258-262, :295-305, :314-326, :325-344); PG/Redis caídos degradan honesto.
- **§32/§33**: cero executor/wallets/capital/firma/broadcast; VPS intocado (0 ssh); solo edición
  local + tests/tsc.
- **§34.3**: terminus denylist server-side 403 ANTES del check de clase; clase C 403;
  `live_exec_policy` default-deny/MainnetRefused intactos (ni nombrados como toggleables);
  `ARBX_ORCHESTRATOR_MODE`/`ARBX_CARTRIDGE_MODE` clase B ⇒ 409 jamás runtime-flipeables (§34.2).
- **NO-GIT**: 0 commit/push/PR/deploy. HEAD `27aca289` verificado intacto.
- **Diffs marcados** `// CB-02 (2026-09-07)` en todos los hunks (docblock, constantes, gates,
  writes, tests).
