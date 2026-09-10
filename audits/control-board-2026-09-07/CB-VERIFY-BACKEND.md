# CB-VERIFY-BACKEND — validación adversarial del control plane backend (CB-02 + CB-04)

- **WO**: CB-VERIFY-BACKEND · kind: verify (read-only sobre código; corre tests, no edita)
- **Agente**: cs-validator (Gang Omniscience) · **Fecha**: 2026-09-09 00:44–01:2x local
- **Branch/HEAD**: `feat/hops-live-01` @ `27aca289` (verificado intacto al inicio; CERO git)
- **Archivos validados (claims CB-02-API/CB-02-RUST/CB-04)**:
  `backend/api-server/src/routes/control-board.ts` (934 L) ·
  `backend/api-server/src/services/control-board-drift.ts` (891 L) ·
  `backend/searcher-rs/src/runtime_knobs.rs` (421 L) + wiring `route_scanner_worker.rs` /
  `main.rs` + montaje `index.ts` + proxy `edge/worker/src/index.ts` (read-only, claims ajenos)
- **Presupuesto dominio público**: **0/5 requests** — NO aplicable: TODO este trabajo está
  sin commitear (árbol local, VPS corre `main @ e65040f1` sin el endpoint); probar el dominio
  verificaría código desplegado ajeno, no este árbol. Declarado, no gastado.

---

## 0. Método

1. Leí completos los 3 archivos bajo verificación + el montaje + el proxy + `requireAdminToken`
   (shared-ts) + shutdown. Cada gate del charter se verificó por (a) lectura de código con
   file:line y (b) test ejecutado por MÍ esta pasada (no citado de pares).
2. Cruce de la denylist §34.3 contra `CB-01-MODULES.json` (44 módulos: A=3, B=38, C=3 —
   extraído por script del propio JSON).
3. Sincronía de mesa: GOAL-WORKORDERS + CB-01-CENSO + CB-02-DISENO (§4.2/§4.3/§15 completo) +
   CB-02-API-APPLY + CB-02-RUST-APPLY + CB-04-APPLY + CB-VERIFY-FRONTEND (citados abajo).

---

## 1. Gates del charter — tabla PASS/FAIL

| # | Gate | Resultado | Evidencia (file:line + run propio) |
|---|---|---|---|
| 1a | `cargo check -p searcher-rs` | **PASS** | EXIT 0 en 1m09s (target caliente, árbol quieto tras serie rust; run propio 00:48–00:49, tras waiter de procesos cargo ajenos) |
| 1b | `cargo clippy -p searcher-rs -- -D warnings` | **PASS** | **EXIT 0** (6m16s, run propio 00:49–00:56, árbol quieto) |
| 1c | `cargo fmt -p searcher-rs -- --check` | **PASS** | **EXIT 0** (run propio 00:56) |
| 1d | `cargo test -p searcher-rs --lib runtime_knobs::` (PENDIENTE del par Rust §5.1) | **BLOCKED por diff ajeno en vuelo (BR-06)** — ver hallazgo V-5 | Run propio: el target lib-test NO compila por 2 errores E0689 en `size_optimizer.rs:4327,4329` (diff +769 sin commitear del programa paralelo **BR-06**, aterrizó DURANTE esta sesión — no estaba en el git-status inicial). Los 8 tests de runtime_knobs quedan ESCRITOS y revisados estáticamente (runtime_knobs.rs:276-420) pero SIN run-verificar — idéntico estado que dejó el par, ahora con el bloqueador identificado y atribuido |
| 2a | vitest `control-board.test.ts` | **PASS 33/33** | run propio, vitest 1.6.1 |
| 2b | vitest `control-board-drift.test.ts` | **PASS 28/28** | run propio |
| 2c | vitest `edge-parity.test.ts` (diff ajeno M preservado) | **PASS 6/6** | run propio |
| 2d | Suites adyacentes (importers de mis exports) | **PASS 21/21** | admin-chains 16 + canonical-knobs 5, run propio |
| 2e | `tsc --noEmit` api-server | **PASS** | EXIT 0 (`npm run typecheck`) |
| 3a | PUT sin admin-token = 401/403 | **PASS (401)** | `requireAdminToken` shared-ts/src/middleware/index.ts:124-133 → **401** `{error:"unauthorized"}` comparación constant-time; montado en el PUT control-board.ts:692 y el GET :664; tests (a) GET :255 / (a) PUT :525 → 401 (corridos). Edge: adminProxy traduce cookie httpOnly → `x-arbx-admin-token` (edge/worker/src/index.ts:1594-1595). El proxy público sin sesión tampoco pasa el adminProxy |
| 3b | Razón vacía rechazada SERVER-side | **PASS** | `PutToggleSchema.reason = z.string().trim().min(1).max(1000)` control-board.ts:227; PUT gate 2 → 400 :709-713 ANTES de cualquier censo/Redis/audit; test (d2) whitespace-only reason → 400, corrido. (UI además la exige — peer frontend 4e, defense in depth, no la única capa) |
| 3c | PUT a clase C = 403; denylist vs CB-01-CENSO, ningún C escapó | **PASS** | Census C = 3: `live_exec_policy` (id contiene "live_exec" → substring denylist control-board.ts:182 → 403 `terminus_denied_c343` :736-745 **ANTES** del check de clase), `paper_mode_terminus` y `capital_key_lockout` (class C → 403 `module_class_c_locked` :747-755). Tests corridos: (e) clase C → 403 sin audit/sin write; (e2) terminus mal-etiquetado A → 403 igual. Piso extra verificado: una fila mal etiquetada A con clave ajena muere en el namespace gate :782-792 (503 `control_key_not_board_writable`) — la única superficie escribible es `arbx:controlboard:*`, sin consumidores terminus. Los 3 A del censo llevan `control_key:null` → 503 `census_invalid_class_a` :768-774 |
| 3d | audit se escribe ANTES del Redis write | **PASS** | INSERT `audit_log` (columnas 011) :843-854 → después SET toggle + HSET approved :874-880; test (j) pinea `invocationCallOrder` INSERT < SET < HSET; audit-fail → 500 y Redis NUNCA escrito (test k); Redis-fail → INSERT compensatorio `control_board.toggle_failed` (test l, jamás UPDATE — 011:21 REVOKE) |
| 3e | Idempotencia del PUT | **PASS** | SET de exactamente `"true"/"false"` (re-PUT mismo valor = no-op natural); HSET campo module_id con mismo `{on,reason,actor}`; el ledger append-ea cada acción del operador POR DISEÑO (CB-02-DISENO §4.3 decisión (b): "PUT idempotente audita IGUAL" — el registro de intención ES el entregable). Rust: `resolve_toggle` estricto, valor repetido no cambia veredicto (cache TTL 1s) |
| 3f | Fail-safe clave ausente = comportamiento desplegado | **PASS** (lectura + tests TS corridos; los 8 tests Rust BLOCKED por V-5, revisados estáticamente) | Rust `resolve_toggle` (runtime_knobs.rs:104-110): ausente/ajeno → `default_when_absent` = veredicto boot; `is_on()` Redis-down → default (:155-157); default bindeado en spawn_route_scanner:860-864 (`mode == On`); tests escritos runtime_knobs.rs:283-321 (ausente/garbage→default, "True"/"1"/JSON → default) — run bloqueado por BR-06 (V-5). TS: `parseDeclaredValue` null → null (DESCONOCIDO, jamás false) control-board.ts:276-280; test (c2) valor ajeno → declared null, CORRIDO |
| 3g | §34.1 mode-invariance | **PASS** | El gate clase A SOLO omite INVOCAR `scan_block` (route_scanner_worker.rs:754-763: `continue` antes del scan; matemática/despacho intactos — INV-CB02-6/7); `runtime_knobs.rs` no importa ni toca módulos de matemática; clases B/C se niegan a runtime (409/403) — §34.2 flags de migración NO runtime-flipeables; terminus intocado (ningún write fuera de `arbx:controlboard:*` + denylist server-side) |
| 3h | RULE 00 (verified_on:null fluye hasta UI; nada fabricado) | **PASS** | hb ausente/expirada/garbage → `verified_on:null` conservando último `verified_at` (control-board.ts:313-323, :606-610); wire contract `nullish` (ControlBoardLed.tsx:64-66) y `projectLed` null → unknown jamás off (:217-220); censo ausente → `modules:[]` (:502-503), corrupto → 503 ruidoso; snapshot del peer frontend 6/6 PASS (4b). Drift banner: no-computable dice "NO computable" textual (control-board-drift.ts:780-788) |
| 3i | Soberanía (único path de escritura = admin-token + board; drift-guard revierte y audita) | **PASS** | Writers de toggle keys + hash approved = SOLO el PUT (admin-gated); worker read-only en la toggle key (runtime_knobs sin `set()`, solo SETEX de su propio `:hb`); drift-guard clase A revierte al valor APROBADO + INSERT `control_board.drift` (:644, :727-737) — restauración, no flip. **Con hallazgo V-1 abajo** (el revert carece del piso de namespace que el PUT sí tiene) |

**Veredicto gates: 1a/1b/1c (check/clippy/fmt) PASS con run propio; 1d BLOCKED por diff
ajeno BR-06 (V-5, no es CB); 2a-2e y 3a-3i (17 gates TS + adversariales) PASS.** Ningún gate
CB falla por causa CB; los hallazgos §2 son defectos de defense-in-depth latentes (G6-gated)
o ajenos (BR-06), no violaciones activas.

---

## 2. Hallazgos adversariales (propios — con refutación de pares donde toca)

### V-1 — CB-04 revert SIN piso de namespace NI normalización de dialecto `redis:` (MUST-FIX antes de G6; NO bloqueante hoy)

**Refutado el claim de CB-04-APPLY.md:193**: *"El revert SOLO restaura booleanos de
control_keys clase A del censo dentro del namespace `arbx:controlboard:*`"* — **esa
contención NO existe en el código del guard**. Evidencia:

- `control-board-drift.ts:428` observa `redis.get(m.control_key)` y `:644` revierte
  `redis.set(m.control_key, ...)` — `m.control_key` **verbatim**, sin `resolveControlKey`,
  sin check de prefijo, sin strip de `redis:`. Grep del servicio: **0 hits** de
  `controlboard`/`namespace`/`resolveControlKey`. El piso existe SOLO en el PUT de CB-02
  (control-board.ts:782-792).

Dos cadenas de fallo (ambas latentes — requieren censo publicado, que hoy es G6 operator-gated):

1. **Corrupción de clave ajena**: censo v1 declara módulo X clase A con clave in-namespace →
   operador hace PUT (fila de aprobación existe) → censo re-publicado (defecto o mutación)
   con `X.control_key = arbx:killswitch` → guard observa el JSON ajeno → `observed:null ≠
   approved` → **REVERT escribe `"true"/"false"` sobre el JSON `KillSwitchState`** → parse
   roto + fail-closed = sistema halteado. Es EXACTAMENTE el riesgo que CB-01-CROSS-EXAM
   corrección 2 documentó y que el PUT de CB-02 previene — el guard re-abre la puerta.
   (Mitigante estructural: `/admin/killswitch` audita con `target_kind:'killswitch'`
   index.ts:271-284, no `control_board_module`, así que la aprobación sólo puede existir
   para un id que el board toggoleó con clave in-namespace en su momento.)
2. **Dialecto fantasma (el dialecto del PROPIO diseño)**: CB-02-DISENO §15-R5/R6 fija el
   registry en claves prefijadas (`redis:arbx:killswitch`, prefijo board `redis:arbx:controlboard:`).
   El PUT normaliza (resolveControlKey control-board.ts:302-308 strip del prefijo) pero el
   guard NO: un censo publicado en el dialecto del diseño haría que el PUT escriba
   `arbx:controlboard:x` mientras el guard lee/revierte `redis:arbx:controlboard:x` (clave
   fantasma) → **drift falso perpetuo + revert inútil a una clave que nadie lee, y el drift
   REAL de la clave verdadera queda indetectado** (violación RULE 00 en potencia del reporte).

**Exposición HOY = cero**: `arbx:config:control_board` no publicado (G6); los 3 módulos A
del censo real llevan `control_key:null` → rama honesta `census_invalid_control_key`
(control-board-drift.ts:555-573). Los 28 tests del CB-04 pasan porque sus fixtures usan el
dialecto bare — el caso prefijado no está testeado.

**Clasificación: NO bloqueante para este árbol; MUST-FIX antes del publisher del censo (G6).**
Fix acotado para la mesa: en `runControlBoardDriftScan`, resolver con `resolveControlKey`
(export de control-board.ts) y observar/revertir SOLO si `kind === "board_toggle"`; el resto
entra a `census_invalid_control_key` (misma postura que el PUT). + 1 test con dialecto `redis:`.

### V-2 — Banner "consistent" sobre-cuenta "comparables" (advisory, menor)

`toControlBoardDriftBanner` (control-board-drift.ts:773-777): `comparable =
modules_scanned - not_comparable`, pero `modules_scanned` incluye las filas C (skipped, no
observables) y las A `no_approval_record`/`census_invalid_control_key` que NO se compararon
contra aprobación. El texto puede decir "41 módulos comparables" cuando se compararon menos.
`detected` sigue correcto (solo `drifts>0`); es imprecisión de TEXTO, no un verde fabricado.
No bloqueante. Sugerencia: contar solo `class_a_scanned + class_b_scanned` efectivamente
comparados (excluyendo no_approval_record/census_invalid).

### V-3 — Comentarios que sobrevenden el invariante single-writer (advisory, doc)

control-board.ts:134-138 dice que `arbx:controlboard:*` lo escribe EXCLUSIVAMENTE el PUT y
"nada más en la flota puede escribirlo" — pero el heartbeat del worker
`arbx:controlboard:route_scanner:hb` vive DENTRO del namespace (así lo manda el diseño §1.2
y lo escribe runtime_knobs.rs:183-203). El invariante que IMPORTA (toggle keys + hash
approved single-writer = PUT) SÍ se cumple; el comentario debería exceptuar el canal `:hb`.
Ídem, menor y claim ajeno: edge/worker/src/index.ts:1587 dice "audit_logs" (plural — tabla
refutada por CB-02-API-APPLY §3); texto stale, no código.

### V-4 — Transitorio SET-aterrizado/HSET-fallido: convergente por diseño (observación, sin acción)

Si el SET aterriza pero el HSET falla → fila compensatoria cancela la aprobación → en el
próximo tick el guard revierte la clave al ÚLTIMO valor aprobado (semántica test (10)). El
estado converge al ledger en ≤1 tick de guard (60s); ventana transitoria documentada. OK.

### V-5 — El target `lib test` de searcher-rs NO COMPILA: diff ajeno en vuelo BR-06 (hand-off a la mesa, NO es CB)

**Run propio**: `cargo test -p searcher-rs --lib runtime_knobs::` → **EXIT 101**,
`error[E0689]` ×2 — `size_optimizer.rs:4327` `(f_raw * 1.0).clamp(0.0, 1.0)` y `:4329`
`….floor()` — *"can't call method on ambiguous numeric type `{float}`"*. Forense de
atribución:

- Los errores viven DENTRO del diff **sin commitear** `+769` líneas marcadas
  `// BR-06 (2026-09-07) — property tests: convex CFMM sizing…` (programa paralelo cerebro;
  el archivo NO estaba modificado en el git-status inicial de esta sesión — aterrizó mientras
  verificaba, árbol compartido §36).
- `cargo check`/`clippy` (1a/1b) PASAN porque **no compilan el perfil `#[cfg(test)]`**
  (lección #460: check ≠ compila tests). El par Rust CB-02 nunca vio esto: su intento quedó
  atascado en el build-lock antes de compilar (su §5.1) — el pendiente que asumo era en
  realidad DOS cosas: el lock (pasajero) y este error (real, ajeno).

**Impacto sobre CB**: NINGUNO directo — runtime_knobs.rs compila limpio en el perfil lib
(check/clippy verdes) y sus 8 tests son unitarios de funciones puras revisados estáticamente;
el defecto está en el claim BR-06. **Consecuencia operativa**: NADIE puede correr tests unit
del lib searcher-rs (CB-02 u otros) hasta que BR-06 tipe su test (fix de 1 línea: anotar
`f64` en `f_raw`/`f_capped` o sufijar literales). Hand-off: BR-06 debe cerrarlo ANTES del
merge general; quien fusione re-ejecuta `cargo test -p searcher-rs --lib runtime_knobs::`.

---

## 3. Confirmaciones y refutaciones contra pares (sincronía de mesa)

| Par | Claim | Mi veredicto |
|---|---|---|
| CB-02-API-APPLY §1-§2 (D1-D4, gates orden, mount, shutdown) | — | **CONFIRMADO íntegro**: mount index.ts:778-784 + import :139, shutdown stopDriftGuard :2042, gates en el orden declarado, D1 (:795-815), D2 (:864-880), D3 (:513-520, :595-598), D4 (sin BOOT_CENSUS_REDIS_KEY en TS — grep 0 usos). 33/33 reproducidos |
| CB-02-API-APPLY §3 (audit_log singular, migración 121 NO) | — | **CONFIRMADO**: query INSERT/SELECT sobre `audit_log` (:442-449, :845, :892); ninguna migración 121; compensatorio con action distinta |
| CB-02-API-APPLY §4 (33/33 + adyacentes 53/53 + tsc) | — | **REPRODUCIDO**: 33+28+6+16+5 = 88/88 y tsc EXIT 0 (runs propios). Su §6 "suite completa PENDIENTE" sigue honestamente pendiente — yo tampoco corrí la suite completa (~8.5 min bajo carga del gang); superficie acotada verificada (único importer no-test = index.ts:139) |
| CB-02-RUST-APPLY §2 (wiring file:line) | — | **CONFIRMADO**: boot census publish en main.rs dentro del bloque canonical-knobs (bloque verificado ~:388-409, patrón no-fatal idéntico); wiring route_scanner_worker :740-763 (poll→gauge→hb SIEMPRE→halt continue→scan) y :860-864 (client ANTES del early-return Off, default = mode==On); sin `set()` en runtime_knobs; killswitch.rs/canonical_knobs.rs intactos (0 diffs CB-02) |
| CB-02-RUST-APPLY §5.1 (cargo test runtime_knobs NO run-verificado) | — | **DIAGNOSTICADO por mí, sigue abierto**: el intento del par no murió solo por el build-lock — el target lib-test NO compila hoy por un error E0689 ajeno (diff BR-06 en vuelo, hallazgo V-5). runtime_knobs:: sigue sin run-verificarse hasta que BR-06 tipe su test; check/clippy/fmt (los 3 gates del charter Rust) sí VERDES por mí |
| CB-04-APPLY §1-§2 (audit_log + NOT EXISTS compensatorio, revert-primero, dedup fingerprint) | — | **CONFIRMADO** salvo una claim: ver V-1 |
| CB-04-APPLY :193 "El revert SOLO restaura… dentro del namespace `arbx:controlboard:*`" | — | **REFUTADO** con evidencia (V-1): el guard no valida namespace ni dialecto; :428/:644 usan `m.control_key` verbatim |
| CB-01-CENSO / CB-01-MODULES.json (44 módulos, C=3) | — | **CRUZADO**: los 3 C (`live_exec_policy`, `paper_mode_terminus`, `capital_key_lockout`) quedan bloqueados (3c); denylist substring atrapa `live_exec_policy` incluso mal etiquetado; namespace gate atrapa cualquier A apuntando afuera |
| CB-VERIFY-FRONTEND (6/6 PASS, verified null → DESCONOCIDO) | — | **CITADO** para el gate 3h: el flujo null→UI lo verificó el par con tests propios; yo verifiqué el origen backend del null |

---

## 4. Runs de verificación (propios, 2026-09-09 00:44–01:2x)

| Comando | Resultado |
|---|---|
| `npx vitest run control-board.test.ts control-board-drift.test.ts edge-parity.test.ts` | **67/67 PASS** (33+28+6), 34.9s |
| `npx vitest run admin-chains.test.ts canonical-knobs.test.ts` | **21/21 PASS** |
| `npm run typecheck` (api-server) | **EXIT 0** |
| `cargo check -p searcher-rs` | **EXIT 0** (1m09s, árbol quieto) |
| `cargo clippy -p searcher-rs -- -D warnings` | **EXIT 0** (6m16s) |
| `cargo fmt -p searcher-rs -- --check` | **EXIT 0** (3s) |
| `cargo test -p searcher-rs --lib runtime_knobs::` | **NO compila el target lib-test: EXIT 101** — `error[E0689]` ×2 en `searcher-rs/src/size_optimizer.rs:4327,4329` (`can't call method clamp/floor on ambiguous numeric type {float}`), dentro del diff **sin commitear** `BR-06 (2026-09-07)` (+769 L de property tests, programa paralelo — no CB). check/clippy NO compilan `#[cfg(test)]` (lección #460) → por eso 1a/1b PASAN y esto no. runtime_knobs:: inalcanzable hasta que BR-06 tipe su test (`let f_capped: f64 = …` o sufijos `f64`) |
| Dominio público | **0/5** (no aplicable — código no desplegado; §0) |

Nota de serie rust: al iniciar había 3 procesos cargo/rustc de sesión paralela (lock del
árbol compartido); esperé árbol quieto (waiter) ANTES de mis gates — cero contención de
target/, cero muerte por lock (lección §5.1 del par Rust aplicada).

---

## 5. Veredicto

**NO-BLOQUEANTE para CB — 19/20 gates PASS; 1 gate (1d cargo test runtime_knobs) BLOCKED por
un diff ajeno en vuelo (BR-06), no por código CB** (`feat/hops-live-01` @ 27aca289, sin
commitear). La construcción CB-02-API + CB-02-RUST + CB-04 satisface todos los gates
adversariales del charter con evidencia de test corrida por un tercero (yo): 88/88 tests TS
reproducidos + tsc EXIT 0 + los 3 gates Rust del charter (check/clippy/fmt) verdes. Los dos
pendientes de los pares quedan así: suites TS reproducidas (cerrado); cargo test
runtime_knobs DIAGNOSTICADO — bloqueado por BR-06 (V-5), re-ejecutar al fusionar.

**Condiciones para la mesa (ordenadas)**:
1. **V-1 (MUST-FIX antes de G6)**: piso de namespace + normalización de dialecto en el
   revert del drift-guard — hoy la exposición es CERO (censo no publicado, verificado:
   ningún writer de `arbx:config:control_board` en el repo; EXISTS=0 en VPS según
   BROWSE-Auditor :21), pero el guard re-abre exactamente la puerta que el PUT cierra.
2. **V-5 (hand-off BR-06)**: tipear su test en size_optimizer.rs:4327-4329 — sin eso,
   NINGÚN test unit del lib searcher-rs corre (incl. runtime_knobs) ni el merge general
   puede declarar "tests verdes".
3. V-2/V-3 advisory de texto; V-4 convergente por diseño.

**Reglas duras**: RULE 00/R8 sin violaciones activas (todo null/absente se declara; este
reporte anotó cada resultado Rust al observarlo, nada pre-declarado); §32/§33 cero
executor/wallets/capital/firma/broadcast, VPS intocado (0 ssh); §34.3 denylist +
default-deny/MainnetRefused intocados (ni referenciados como toggleables; clase C 403);
§34.1 mode-invariance verificada (gate de invocación, no de matemática); NO-GIT: 0
commit/push/PR/deploy, HEAD `27aca289` verificado al cierre.
