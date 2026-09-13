# CB-VERIFY-FRONTEND (mitad A — RESPAWN-2) — Verificación adversarial /control

- **WO**: CB-VERIFY-FRONTEND · kind: verify · **Agente**: ecc:react-reviewer (Gang Omniscience)
- **Fecha**: 2026-09-07 (≈08:05 local) · **Read-only sobre código · CERO git · CERO edits** (diffs propios: ninguno)
- **Mitad RESPAWN-2**: gates **1..⌈6/2⌉ = 1, 2, 3** (build/tests · R1 hidratación+R5 · a11y). Los gates 4-6
  (semántica operador/6 pestañas, RULE 00 board, /killswitch) son del reemplazo B — solo los OBSERVO
  al final para la mesa, no los califico.
- **Árbol verificado**: branch `feat/hops-live-01`, HEAD `27aca289`. Archivos bajo claim (untracked,
  creados por el peer apply minutos antes):
  - `frontend/app/control/page.tsx` (1,573 B, mtime 07:35)
  - `frontend/app/control/ControlBoardClient.tsx` (15,743 B, mtime 07:34)
  - `frontend/components/ControlBoardLed.tsx` (13,965 B, mtime 07:39)
  - `frontend/components/__tests__/ControlBoard.test.tsx` (14,525 B, mtime 07:37 — 25 tests)

## 0. Sincronía de mesa redonda (obligatoria)

Leídos: `GOAL-WORKORDERS.md` (completo), `CB-02-RUST-APPLY.md`, `CB-05-DESIGN.md` (+ índice de
`CB-05-PROPUESTA-B.md`). **AUSENTES al momento de verificar**: `CB-03-APPLY.md` y `CB-02-DISENO.md`
— el charter me pedía leerlos; el apply de CB-03 seguía trabajando en paralelo (mtimes 07:34-07:39).
Verifiqué el ESTADO DEL DISCO, no un reporte. Si el peer publica correcciones después, este reporte
debe reconciliarse contra el hash final.

Cita de pares usada: CB-02-RUST-APLY §3.1 (patrón kill-switch, clave `arbx:killswitch`) confirma que
el endpoint `GET/PUT /api/v1/control-board` que consume este frontend **aún NO existe en backend**
(CB-02 fue NO-OP) → el estado honesto "sin datos del board" es el camino E2E real hoy, no un caso
teórico.

## 1. Tabla PASS/FAIL (mi mitad)

| Gate | Criterio | Veredicto | Evidencia |
|---|---|---|---|
| 1a | `tsc --noEmit` (frontend) | **PASS** | EXIT 0, 0 errores (corrido local, node_modules del repo) |
| 1b | vitest `ControlBoard.test.tsx` | **PASS** | 25/25 tests, exit 0 (5.02s) |
| 1c | Nada más se rompió | **PASS** | Suite completa: **120 files / 1098 tests, 0 fails, exit 0** (76.7s) |
| 2a | `page.tsx` Server Component puro | **PASS** | `async function` sin `"use client"`, sin hooks — page.tsx:24 |
| 2b | `useState(initialSnapshot)` | **PASS** | ControlBoardClient.tsx:265-266 (snapshot y error nacen de props) |
| 2c | Cero `Date.now`/`window`/`document`/`navigator`/`Math.random` en render | **PASS** | grep: únicos hits = 2 comentarios (Client:10, page.tsx:10) + `setFetchedAt(Date.now())` en Client:279 (callback `refresh`, invocado solo desde useEffect Client:287-291) y Client:342 (handler `confirmToggle`) — ninguno en path de render |
| 2d | R5 transitivos: ControlBoardLed | **PASS** | Sin `"use client"`, sin hooks, sin side-effects a nivel módulo; `getApiBaseUrl()` solo dentro de las funciones fetch/put (Led:89,138) — Led:265 "Pure: sin handlers, sin estado" |
| 2e | R5: SourceMeta / LastUpdated | **PASS** | `at != null` gatea LastUpdated (SourceMeta:38); doble guard `if (at == null) return null` (last-updated.tsx:24) — `fetchedAt` inicial null (Client:267) → cero `Date.now()` en hidratación; tick 1s solo en useEffect (last-updated.tsx:19-22) |
| 2f | R5: PageHeader | **PASS** | Puro (title/lede); `showRefresh` default false y page.tsx no pasa `actions` → RefreshButton no montado (page-header.tsx:19,30-33) |
| 2g | R5: lib transitivos (api-client, admin-token) | **PASS** | `isBrowser` via `typeof window` sin acceso (api-client.ts:33); `document`/`Date.now` de admin-token solo en funciones con guard `typeof document === "undefined"` (admin-token.ts:58,89,99), invocadas únicamente desde el handler `confirmToggle` (Client:321,327) — jamás en render |
| 2h | R5: transitivos del layout compartido | **PASS** | SiteHeader/AppSidebar/HeroSphere/ThemeScript/SystemGuardBanner: 0 hits. PageBreadcrumb:21 y RuntimePostureBar:21-23 = comentarios que DECLARAN la pureza. OpportunityTicker: `formatAgo` (Date.now, Ticker:31-33) solo mapea `items` que nacen `[]` (Ticker:64) y se llenan en useEffect → primer paint vacío, sin mismatch |
| 2i | Timestamps deterministas (R1 clásico) | **PASS** | `fmtIso` = slice UTC crudo sin locale (Led:226-229); pineado por tests:328-333 (`"2026-09-07 10:00:00 UTC"`) |
| 3a | Labels de inputs | **PASS** | `Label htmlFor="cb-reason"`+`Input id="cb-reason"` (Client:92-102); `htmlFor="cb-admin-token"`+`id` (Client:104-114) |
| 3b | aria-label en confirm | **PASS** | Panel `role="group"` + `aria-label="confirmar {encendido\|apagado} de {moduleName}"` (Client:78-84); botón refresh `aria-label="refrescar board"` (Client:364); `section aria-label="control board"` (Client:350) |
| 3c | LED accesible | **PASS** | `role="img"`+`aria-label` por estado+`title` (Led:274-292); LockIcon y dot `aria-hidden` (Led:284,286); carga `role="status"`+`aria-busy` (Client:390-394) |
| 3d | Foco | **PASS w/ ADVISORY F-1** | Sin violación WCAG 2.4.3 (panel en DOM, alcanzable por Tab, botones con texto discernible) pero sin gestión de foco al abrir el confirm (ver F-1) |
| 3e | Contraste LED fluor claro/oscuro | **PASS w/ NOTE F-2** | Estado NO transmitido solo por color: cada LED lleva label textual (Led:237-263) → WCAG 1.4.1 satisfecho. Texto del label `text-muted-foreground` ≈4.7:1 en claro (AA) y ≈6:1 en oscuro. Dot fluor lime-400 ≈**1.5:1** sobre card claro (<3:1 no-texto) — mitigado por el label obligatorio; en oscuro ≈10:1 (excelente). Ver F-2 |

**Veredicto mitad A: PASS (3/3 gates) con 3 hallazgos advisory no-bloqueantes.**

## 2. Hallazgos (advisory — NO bloqueantes)

- **F-1 (menor, UX teclado)**: al abrir el panel de confirmación no hay movimiento de foco: el panel
  se inserta al FINAL de la sección (Client:418-435, tras todas las filas) y `cb-reason` no tiene
  `autoFocus`. Un operador de teclado que pulsa "Encender" en la primera fila debe Tabular por todas
  las filas restantes. Sugerencia: `autoFocus` en el Input razón, o montar el panel junto a la fila
  origen. Asociado: el error del diálogo `cb-confirm-error` (Client:118) no tiene `role="alert"`/
  `aria-live` → un lector de pantalla no lo anuncia al aparecer.
- **F-2 (nota visual)**: dot fluor `bg-lime-400` (Led:239) y ámbar `bg-amber-400` (Led:252) quedan
  ≈1.5-1.7:1 sobre card claro (`--card: oklch(1.000 …)` globals.css:79). No es fallo WCAG porque el
  estado viaja en el label textual adyacente (11px semibold, AA), pero un anillo/borde oscuro en
  tema claro subiría la legibilidad del dot a nivel del label. Red-500 (3.8:1) y zinc-500 (4.4:1)
  sí pasan 3:1 no-texto en claro.
- **F-3 (nota, badge)**: texto del badge clase A `text-lime-600` sobre tinte `lime-500/10`
  (Led:298) ≈3.1:1 a 10px — bajo AA 4.5:1 de texto pequeño. Mismo patrón repo-wide de badges; no
  introducido por CB-03. Registrado para una pasada de contraste global, no para este WO.

## 3. Metodología y límites honestos

1. **Tests por static markup** (convención del repo): `renderToStaticMarkup` no serializa handlers →
   el test de "candado C SIN botón" (ControlBoard.test.tsx:170-178) asevera la ausencia del CONTROL
   (`cb-toggle-live_exec_mainnet` no existe) — proxy correcto: un botón que no existe no puede tener
   handler. El charter pedía "mantener" ese test: **está y pasa**. Defense-in-depth adicional en
   código: `requestToggle` re-guarda `isToggleable` (Client:295-296) y `confirmToggle` re-valida
   clase + razón (Client:310-317) — un caller futuro no puede togglear B/C.
2. **Fixtures del test**: datos de test declarados como tales (test:15-16) para ejercitar proyecciones
   puras — RULE 00 se aplica al runtime, no al harness. El render de error (test:335-341) pina error
   verbatim + CERO filas fabricadas.
3. **Despliegue**: `https://arbx.ape-tv.net/control` → **HTTP 404** (1 request del presupuesto de 5).
   Correcto y esperado: código untracked, NO-GIT vigente, CB-02 endpoint inexistente. La verificación
   browser real (CB-06) queda post-deploy; este verify es código+tests.
4. No navegué más del dominio (sin deploy no hay nada que ver; evité auto-contaminación 429).

## 4. Observaciones CROSS-GATE para el reemplazo B / orquestador (NO calificadas por mí)

- **Gate 4 (6 pestañas)**: `ControlBoardClient` renderiza hoy una **lista plana** de ModuleRow
  (Client:410-416) — NO hay pestañas (Detección/Evaluación/Ejecución/Infra/Seguridad/Frontend) ni
  defaults panel "postura de máximo potencial". Si el charter de CB-03 exige pestañas, esto está
  pendiente o llegó después de mi lectura.
- **Gate 6 (/killswitch enlazado)**: en los 3 archivos NO existe `Link`/`<a>` a `/admin/killswitch`
  (grep: 0). Pendiente o llegó después.
- **Gate 5 (sin banner drift fabricado)**: correcto por ahora — `DriftSlot` es solo cómputo por-módulo
  declarado-vs-verificado con `no-data` honesto (Led:198-219, Client:165); NO existe banner global de
  drift. Eso coincide con "CB-04 no existe aún".
- **Semántica LED**: verde=ENCENDIDO/rojo=APAGADO/ámbar=DESCONOCIDO (Led:237-263) y precedence
  C→locked SIN importar estado (Led:184-186) — la proyección es exactamente la del operador; los
  tests 1/2/4 la pinean. (Dato para B: la semántica base ya está verde.)

## 5. Presupuesto dominio público

**1/5** requests usados (GET /control → 404, documentado en §3.3).

## 6. Verificación corrida (comandos exactos)

```
frontend> ./node_modules/.bin/tsc --noEmit                    → EXIT 0
frontend> ./node_modules/.bin/vitest run components/__tests__/ControlBoard.test.tsx
                                                             → 25/25 passed, exit 0
frontend> ./node_modules/.bin/vitest run                     → 120 files / 1098 tests passed, exit 0
curl -s -o /dev/null -w "%{http_code}" https://arbx.ape-tv.net/control → 404
```

---

# MITAD B (RESPAWN-2 reemplazo B) — Gates 4, 5, 6

- **WO**: CB-VERIFY-FRONTEND · kind: verify · **Agente**: ecc:react-reviewer — **REEMPLazo B**
  (el original del WO cayó; doble conocimiento; mitad = gates ⌈6/2⌉+1..6 = **4, 5, 6**)
- **Fecha**: 2026-09-07 (verificación 07:57→09:25 local) · **Read-only sobre código · CERO git · CERO edits de código**
- **ADVERTENCIA DE CARRERA (leer primero)**: el builder CB-03 editó los archivos EN PARALELO durante
  toda mi verificación. Mi par A (arriba) verificó el estado 07:34-07:39 (25 tests, lista plana).
  Yo verifiqué inicialmente ese mismo estado, observé la edición en vivo (Led 13,965→15,559→29,826 B;
  Client 15,743→19,597 B; tests 14,525→23,570→23,900→24,109 B; última edición 09:15:32), esperé
  quiescencia y **verifiqué el estado FINAL**: `ControlBoardClient.tsx` 19,597 B @08:03:44 ·
  `ControlBoardLed.tsx` 29,826 B @08:02:01 · `ControlBoard.test.tsx` 24,109 B @09:15:32 ·
  `page.tsx` 1,573 B @07:35:58 (estable). El edit 09:15:32 tocó SOLO guards de dos asserts del
  test (`.find()`+`toBeDefined()`, test:435-439/447-448) — CERO cambios de semántica de
  componentes, por lo que toda la evidencia de gates 4-6 de esta sección conserva validez plena.
  Las observaciones "pendiente" del §4 de mi par A (sin pestañas / sin link /killswitch / sin
  defaults) **quedaron resueltas por el builder a las 08:00-08:03** — A las declaró explícitamente
  como "pendiente o llegó después": llegó después.
- **Estado del árbol**: branch `feat/hops-live-01` (switch reportado por CB-02-RUST-APPLY §2.1;
  observado también por CB-02-API-APPLY §head). Archivos bajo claim: untracked.

## 7. Tabla PASS/FAIL (mitad B — componentes finales: page 07:35:58 · Client 08:03:44 · Led 08:02:01 · test 09:15:32)

| Gate | Criterio | Veredicto | Evidencia |
|---|---|---|---|
| 4a | 6 pestañas correctas y completas | **PASS** (con nota) | `CONTROL_BOARD_TABS` Led:419-430 = exactamente las 6 de GOAL ext. §2 ("Detección / Discovery", "Evaluación / Matemática", "Ejecución / Terminus" con nota §34.3, "Infra / Observabilidad", "Seguridad", "Frontend"); tabstrip ARIA `role=tablist/tab/tabpanel` + `aria-selected`+`aria-controls`+`hidden` Client:448-495; R1-safe: `activeTab` derivada del snapshot inicial (Client:281-283), paneles inactivos en DOM con `hidden` (Client:484); tests 422-494 (orden canónico, bucket "Sin clasificar", R1 byte-idéntico). NOTA: solo se renderizan pestañas con ≥1 módulo — ausencia honesta (RULE 00), pineada test:433-435 |
| 4b | LED verde=encendido/rojo=apagado/ámbar=DESCONOCIDO | **PASS** | `LED_VISUALS` Led:263-293 (lime-400+glow ENCENDIDO / red-500 APAGADO / amber-400 DESCONOCIDO / zinc LOCKED §34.3) + `projectLed` Led:210-217 (C→locked SIEMPRE; null→unknown; jamás off falso); lede page.tsx:33 documenta la semántica; tests 136-165 y 249-275 |
| 4c | Candado C SIN botón (test de ausencia mantenido) | **PASS** | `isToggleable` Led:224-226 (solo A); rama C = `<span>` sin botón Client:238-247; defense-in-depth: `requestToggle` re-guarda (Client:304-312) y `confirmToggle` re-valida clase+razón (Client:314-329). Test de ausencia ORIGINAL mantenido (test:176-184) + reforzado sobre el HTML completo con tabs (test:469-477) |
| 4d | Badge clase B requiere-restart con referencia a CB-05 | **PASS** | `CLASS_BADGES.B` Led:331-335 ("B · requiere-restart", title "…CB-05 propone el diff"); texto de fila Client:248-252 ("propuesta de diff en CB-05"); test:352-363 |
| 4e | Razón obligatoria deshabilita confirm | **PASS** | `reasonReady` Client:81 + `disabled={busy \|\| !reasonReady}` Client:134 + re-validación handler Client:325-329 ("la razón es obligatoria (audit_logs…)"); tests 209-245 (sin razón→disabled; con razón→enabled; busy→disabled) |
| 4f | Defaults panel honesto (valores citados del canon, gana/pierde) | **PASS** | `DefaultsPanel` Led:706-758 (`<details>` plegado nativo, cero JS de estado) + `MAX_POTENTIAL_DEFAULTS` Led:569-700 (14 filas × 7 columnas, TODAS con `source` citada). Spot-checks adversariales (ver §11): 2 citas de código Rust VERIFICADAS, 8/8 valores "VPS observado" VERIFICADOS via ssh read-only, transcripción fiel de CB-05-PROPUESTA-B §1. Marcadores de honestidad ("no documentado en canon", "NO es estado runtime") pineados tests:540-572 |
| 5a | Sin snapshot no hay LEDs | **PASS** | Render gated en `snapshot` (Client:443); loading honesto `role=status` (Client:422-430); `fetchControlBoard` NUNCA fabrica snapshot (union `{ok,data}\|{ok,error}`, Led:112-149); test:341-347 pina cero filas + cero LEDs con error |
| 5b | Error verbatim | **PASS** | `<code>{error}</code>` Client:410 (break-all); fetch compone `edge HTTP {status}: {body}` Led:126-129 / re-throws mensaje real Led:146-148; test:342-343 pina "edge HTTP 404: not found" verbatim |
| 5c | Board vacío honesto | **PASS** (gap de test, ver B-3) | Alert "board vacío — censo CB-01 sin publicar… nada que mostrar todavía" Client:432-441. Cadena REAL verificada end-to-end: Redis `arbx:config:control_board` EXISTS=**0** (ssh arbx read-only 08:40) → GET CB-02 devuelve `{modules:[]}` (control-board.ts charter, CB-02-API-APPLY §2.1) → el board mostraría exactamente este estado |
| 5d | NINGÚN banner drift fabricado mientras CB-04 no exista | **PASS** | `GlobalDriftBanner` Led:490-540: `drift` ausente en snapshot → línea muted "drift global: sin dato (CB-04 pendiente)" (data-drift="unavailable") — banner rojo `role=alert` SOLO con `drift.detected=true` que llega del upstream (dato, no fabricación). DriftSlot por-módulo: no-data → VACÍO honesto (Led:362-373). Tests 498-536 (unavailable sin role=alert) y 293-299 |
| 6 | /killswitch ENLAZADO no duplicado | **PASS** (advisory B-1) | Link `<a href="/killswitch" data-testid="cb-killswitch-link" title="kill-switch: página propia — el board enlaza, no duplica">` Client:378-387; test:576-582 pina **exactamente 1** ocurrencia. CERO duplicación: el board NO tiene toggle de kill-switch (su único PUT es `/api/v1/control-board`, Led:157-195); página `/killswitch` INTACTA (mtime 2026-08-16, no tocada); `/killswitch` sigue en el nav del sitio (nav-items.ts:72, grupo "control") |

**Veredicto mitad B: PASS 11/11 sub-gates** (4a-4f, 5a-5d, 6) sobre los componentes finales,
con 1 advisory (B-1), 1 higiene-de-build observada en vivo y RESUELTA por el builder (B-2, §8.2),
2 minors (B-3, B-4).

## 8. Gate 1 sobre el estado FINAL (drift desde la mitad A — informativo para la mesa)

Mi par A verificó gate 1 sobre el estado 07:3x (tsc 0, 25/25, suite 1098/1098). El builder
siguió editando; sobre el **estado final** yo observé:

| Corrida | Resultado |
|---|---|
| vitest ControlBoard.test.tsx @08:19-08:27 (estado 23,570 B) | **1 FAIL** (tabstrip) |
| tsc @08:19-08:27 (estado 23,570 B) | **exit 2 — 3 errores TS** (test:434 TS2322, test:444/445 TS2532) |
| vitest @08:36:50 (estado final 23,900 B) | **41/41 PASS, exit 0** |
| tsc --noEmit @08:37 (incremental, estado final) | **exit 2 — SOLO error TS6053** (`__cb03_debug.test.tsx` no encontrado; archivo de debug del builder YA eliminado del disco; referencia zombie en `tsconfig.tsbuildinfo`) |
| tsc --noEmit --incremental false @08:39→ (fría, estado 08:36) | **exit 2 — los 3 errores TS REALES persisten** (test:434 TS2322 `snapshotTabbed.modules[0]` posiblemente undefined; test:444/445 TS2532 `groups[1]` posiblemente undefined) — ver §8.2 |
| vitest + tsc @09:15:32→09:23 (RESOLUCIÓN del builder, estado FINAL) | **vitest 41/41 PASS** · **tsc --noEmit EXIT 0** — el builder reescribió los dos asserts con `.find()` + `toBeDefined()` + guards (test:435-439, 447-448) y la enumeración stale quedó regenerada (TS6053 desapareció). Componentes INTACTOS (page 07:35:58, Client 08:03:44, Led 08:02:01) → mi verificación de gates 4-6 conserva validez plena |

**Gate 1 sobre el estado FINAL (09:15:32) = PASS** (41/41 + tsc exit 0). El estado intermedio
08:36 quedó FAIL por dos causas independientes — (a) 3 errores TS reales en el test file
(indexed-access estricto: `modules[0]`/`groups[1]`), (b) TS6053 de `__cb03_debug.test.tsx`
(debris de debug ya borrado) que ADEMÁS enmascaraba (a) en las corridas incrementales al abortar
la enumeración antes del typecheck. Ambas resueltas por el builder a las 09:15:32. Lección de
higiene para el builder (B-2): los archivos `__*_debug` deben morir ANTES de cualquier corrida
gate, porque un tsbuildinfo que los captura rompe TODAS las corridas incrementales posteriores
hasta regenerarse (fix universal si reaparece: `rm frontend/tsconfig.tsbuildinfo`).

### 8.1 Diagnóstico del fallo tabstrip que observé en vivo (evidencia para el builder)

El fallo de 08:19-08:27 NO era del componente: el token de split `id="cb-tab-detection"` es
SUBSTRING de `data-testid="cb-tab-detection"` → `split()[1]` moría dentro del propio
`data-testid` (el received terminaba exactamente en `data-test`, probando que `data-state`
existía tras el segundo punto de corte). El builder lo corrigió a las 08:36:26 con dos splits
por atributo (test:461-466, comentario propio del builder) — fix correcto, 41/41.

### 8.2 B-2 (higiene de build, NO de código): tsbuildinfo stale — RESUELTO 09:15:32

`tsc --noEmit` fallaba con TS6053 referenciando `components/__tests__/__cb03_debug.test.tsx`
(creado y borrado por el builder durante el debugging; NO existía en disco — verificado `ls`).
La referencia vivía en `frontend/tsconfig.tsbuildinfo` (679 KB, regenerable) y ENMASCARABA los
3 errores TS reales (abortaba la enumeración antes del typecheck). Tras la edición 09:15:32 del
builder, `tsc --noEmit` da **EXIT 0** (enumeración regenerada + errores reales corregidos).
Fix universal si el síntoma reaparece: `rm frontend/tsconfig.tsbuildinfo` (caché, no código —
yo no lo toqué por disciplina read-only).

## 9. Hallazgos (advisories/minors — no bloquean mis gates)

- **B-1 (advisory, gate 6)**: el link /killswitch vive DENTRO del bloque `{snapshot && …}`
  (Client:370) → en el estado degradado real de HOY (endpoint CB-02 sin deploy + censo ausente)
  el board NO muestra el link. Es una decisión deliberada y pineada (test:584-587 "sin snapshot
  el link sigue ausente — honesto"), y /killswitch sigue alcanzable por el nav del sitio
  (nav-items.ts:72). Sugerencia: hoist del link fuera del guard — durante una outage del censo,
  enlazar el kill-switch es precisamente lo más útil que el board puede hacer.
- **B-2 (higiene de build — RESUELTO, ver §8.2)**: tsbuildinfo stale post-debug-file. Observado
  como blocker en el estado 08:36 (rompía tsc incremental y enmascaraba 3 errores TS reales);
  el builder lo resolvió a las 09:15:32.
- **B-3 (minor, gap de test)**: el estado "board vacío" (Alert Client:432-441) no tiene test
  propio que lo pinee (los tests cubren error, snapshot poblado y tabs). Sugerencia: fixture
  `{modules: []}` → espera "board vacío" + "censo CB-01 sin publicar".
- **B-4 (cosmético)**: comentario Led:155 dice "mismo patrón que /admin/killswitch" — la ruta
  real de la página es `/killswitch` (no existe `app/admin/killswitch/`). Referencia de patrón
  API en comentario, sin efecto funcional.
- **Observación de contrato (informativa)**: el wire CB-02 (control-board.ts:145-165, diseñado
  explícitamente contra ControlBoardLed.tsx:43-76) NO envía `tab` ni `drift` → hoy TODOS los
  módulos caerían en "Sin clasificar" y el banner global diría "sin dato (CB-04 pendiente)".
  Degradación honesta por diseño (Zod nullish), no shape-mismatch: verifiqué campo por campo
  que el wire incluye los requeridos por la Zod (`id`,`name`,`module_class` presentes con
  nulls explícitos en el resto).

## 10. Sincronía de mesa redonda (citas y refutaciones)

- **Leídos**: GOAL-WORKORDERS.md (completo), CB-02-RUST-APPLY.md, CB-05-DESIGN.md,
  CB-05-PROPUESTA-B.md (§1), CB-02-API-APPLY.md (§0-2.1), CB-02-DISENO.md/CB-02-DISENO-DESIGN.md
  (aterrizaron 08:22-08:31, DESPUÉS del apply — misma anomalía de orden que reportaron mis dos
  pares de CB-02), y la mitad A de este mismo archivo.
- **CB-03-APPLY.md NO existe al cierre de mi verify** (09:30) — el builder sigue sin reportar;
  este verify es contra el ESTADO DEL DISCO, no contra su reporte. Si su reporte llega después,
  reconciliar contra los mtimes citados arriba.
- **Refino (no refuto) a mi par A §4**: sus tres "pendiente o llegó después" — llegó después
  (pestañas Led:399-479, defaults Led:542-758, link Client:378-387). Su verificación de gates
  1-3 corresponde al estado 07:3x; el estado derivó dos veces más (08:36 FAIL intermedio,
  09:15:32 final) — gate 1 re-verificado por mí sobre el final: PASS (§8).
- **Confirmo a CB-02-API-APPLY §1 fuente #3**: el contrato Zod Led:43-76 coincide campo por campo
  con el wire — verificado independiente desde el lado frontend.
- **Aserciones de agentes ≠ facts — spot-checks ejecutados** (§11): las citas del defaults panel
  son exactas contra código Rust, canon CB-05 y VPS vivo.

## 11. Citas del defaults panel verificadas adversarialmente (muestra)

| Cita en MAX_POTENTIAL_DEFAULTS | Verificación propia | Resultado |
|---|---|---|
| `ARBX_ROUTE_SCANNER_MODE` default "off (valor inválido/ausente → off)" — route_scanner_worker.rs:105-132 | sed 100-135 del archivo | **MATCH** ("Default `Off` — fail-safe: any unset/garbage value resolves to `Off`") |
| `ARBX_SCORING_HARD_GATE` "true / false (advisory-only)" — scoring_pipeline.rs:49,92-95 | grep consts + doc line 12 | **MATCH** ("Advisory only. With `ARBX_SCORING_HARD_GATE=false` (default) it NEVER blocks") |
| Columna "VPS observado" (8 claves: .env:5,20,24,25,29,39,40,136) | ssh arbx read-only `grep -n` de las 8 claves | **8/8 MATCH** (active/auto/v2/on/shadow/on/on/on — líneas exactas) |
| Transcripción CB-05-PROPUESTA-B.md §1 | grep de las filas 43-55 del canon | **MATCH** (valores y file:line idénticos) |
| Cadena E2E "board vacío" | `redis-cli EXISTS arbx:config:control_board` via ssh | **0** (censo ausente → modules:[] → estado honesto) |

## 12. Presupuesto dominio público y verificación corrida

**0/5 requests HTTP usados** por esta mitad (el 404 de /control ya lo documentó mi par A con 1;
no lo repito — sin deploy no hay nada nuevo que ver; evité auto-contaminación 429). ssh `arbx`
read-only permitido por §33 (grep .env + redis-cli EXISTS — cero mutación).

```
frontend> npx vitest run components/__tests__/ControlBoard.test.tsx   # @08:19 y @08:27 → 1 FAIL
                                                                      # @08:36:50 (final) → 41/41 PASS
frontend> ./node_modules/.bin/tsc --noEmit                            # → exit 2 SOLO TS6053 (tsbuildinfo stale)
frontend> ./node_modules/.bin/tsc --noEmit --incremental false        # fría @08:39 → exit 2 (3 errores TS
                                                                      #   reales del test, enmascarados antes por
                                                                      #   TS6053); tras fix builder @09:15:32:
                                                                      #   tsc --noEmit → EXIT 0
ssh arbx> grep -n '<8 claves>' /opt/arbitragex-v2/.env                # → 8/8 MATCH defaults panel
ssh arbx> docker exec … redis-cli EXISTS arbx:config:control_board    # → 0 (censo ausente, board vacío honesto)
```

**Fusión para el orquestador (estado final 09:15:32)**:
- Mitad A (arriba): gates 1-3 PASS sobre el estado 07:3x. Su gate 1 se re-verificó aquí sobre
  el estado final (§8): **vitest 41/41 · tsc --noEmit EXIT 0**.
- Mitad B (esta sección): gates 4-6 **PASS 11/11** sobre los mismos componentes finales
  (page 07:35:58 · Client 08:03:44 · Led 08:02:01 · test 09:15:32).
- **Veredicto global CB-VERIFY-FRONTEND: PASS (6/6 gates)** con 1 advisory (B-1 link /killswitch
  invisible sin snapshot — decisión pineada, mitigada por el nav del sitio), 2 minors (B-3 sin
  test del estado "board vacío", B-4 comentario `/admin/killswitch` vs ruta real `/killswitch`)
  y 1 lección de higiene resuelta (B-2 debug-file + tsbuildinfo).
- Pendientes del builder CB-03 (no bloquean este verify): publicar **CB-03-APPLY.md** (citado
  por sus propios comentarios Led:56, Led:83-84 pero inexistente), y los opcionales B-1/B-3/B-4.
- Forense de carrera documentada para la mesa: el builder editó 5 veces durante esta
  verificación (07:34→08:00→08:02→08:03→08:36→09:15); toda verificación de este programa DEBE
  pinar mtimes del estado auditado (técnica usada aquí) o será obsoleta en minutos.

---

# §13 ADDENDUM FINAL (re-verify 14:21-14:35) — reconciliación contra los reportes post-cierre

- **Agente**: ecc:react-reviewer (encarnación final del WO). **Read-only · CERO git · CERO edits.**
- **Motivo**: al cierre de ambas mitades faltaban CB-03-APPLY.md (09:29), CB-02-API-VERIFY.md (10:00),
  CB-04-APPLY.md (09:51) y CB-01-CROSS-EXAM.md (10:06). Este addendum re-verifica los 6 gates sobre el
  árbol ACTUAL y reconcilia. **Entregable consolidado: `CB-VERIFY-FRONTEND.md`** (esta sección es el
  registro de la pasada; aquel es el reporte público de la mesa).

## 13.1 Estabilidad del estado auditado

Los 4 archivos del claim están **byte-estables desde 09:15:32** (md5 verificados: page `7eff1651…`,
Client `f3d852ea…`, Led `77797d73…`, test `ff18303f…`; mtimes 07:35/08:03/08:02/09:15 idénticos a los
que pineó la mitad B). NINGÚN par posterior los tocó (CB-04 reclama solo api-server + useDriftDetection;
CB-01-CROSS-EXAM §1f reporta diff 0). Toda la evidencia de gates 4-6 de la mitad B CONSERVA validez
plena; gates 1-3 re-corridos:

| Corrida (14:21-14:22, árbol post-CB-04) | Resultado |
|---|---|
| `tsc --noEmit` | **EXIT 0** |
| `vitest run ControlBoard.test.tsx` | **41/41 PASS, exit 0** |
| `vitest run` (completa) | **120 files / 1114 tests / 0 fails, exit 0** — reproduce CB-03-APPLY §3 |
| `GET https://arbx.ape-tv.net/control` (1/5 presupuesto) | **404** — nada desplegado (NO-GIT; VPS e65040f1 per CB-01-CROSS-EXAM §1e) |

## 13.2 Reconciliación con CB-03-APPLY.md (el reporte que ambas mitades esperaban)

Verificado claim por claim (tabla completa en CB-VERIFY-FRONTEND.md §3): line counts 38/525/758/593
MATCH · 41 tests MATCH · **19 marcadores CB-03 = 1+7+6+5 exactos** como declara §6 · gates 41/41 +
1114/1114 + tsc 0 reproducidos · su "pendiente" §5.1-§5.3 (`tab`, `drift`, CB-01-CENSO) sigue abierto
por diseño (CB-02-DISENO y CB-01-CENSO aterrizaron después; el `tab` tolerante ya fue validado contra
el censo de 44 por CB-01-CROSS-EXAM §1a — 6/6 canónicos, 0 huérfanos). Su claim "CB-01-CENSO.md NO
existe" era cierto a las 08:02 y dejó de serlo a las 09:27 → **F-6** (drift documental menor del
defaults panel, honesto pero desactualizable).

## 13.3 NUEVO HALLAZGO F-4 — costura CB-03↔CB-04 (must-fix antes de montar en deploy)

CB-04 codifica estados NO-computables como `detected:false` + `summary:"…drift NO computable"`
(control-board-drift.ts:725-742) e inyecta el banner al snapshot (control-board.ts:590-591). La rama
`clear` del `GlobalDriftBanner` (**ControlBoardLed.tsx:531-539**) renderiza SOLO "drift global: none
(chequeado {ts})" — **descarta `drift.summary`** (que la rama detected sí pinta, Led:518). En el
sistema compuesto con censo sin publicar (estado REAL: `EXISTS arbx:config:control_board`=0), el board
afirmaría "none" sobre un drift NO computable — el "verde fabricado" que CB-04-APPLY §4 promete
evitar; el drop es del lado CB-03, invisible para ambos test-suites (cada mitad testeó su mitad).
Fix de 1 línea (pintar summary en clear) + test del caso no-computable. Detalle y clasificación en
CB-VERIFY-FRONTEND.md §2-F-4. **No aplicado (WO read-only).**

## 13.4 Veredicto final consolidado (para el orquestador)

**CB-03: PASS 6/6 gates** (23/24 sub-gates limpios; 3d/3e advisory F-1/F-2; 5d PASS con finding F-4 de
integración). El entregable /control es REAL, honesto (RULE 00 verificada en cada estado degradado) y
espera al backend (CB-02 mount + publisher de censo + CB-04 guard — todos documentados como gaps por
CB-02-API-VERIFY G1-G6, NINGUNO del lado frontend). Entradas exigidas al futuro PR de montaje:
**F-4** (banner clear + summary) y deseables F-6/B-3/F-1/F-5. CB-06 (browser-verify) queda post-deploy
por diseño. Presupuesto dominio acumulado del WO: 2/5 (404 de mitad A + 404 de esta pasada).

---

# §14 RONDA 2 (2026-09-08 23:26–23:3x) — re-verificación tras la reconciliación del builder

> **Veredicto: PASS 6/6 gates — los 5 hand-offs de la ronda 1 (F-1, F-4, F-6, B-1, B-3) RESUELTOS y
> pineados por tests nuevos (41→45); 1 finding NUEVO de integración (F-7) + 1 menor (F-8) + 1
> cosmético persistente (F-5).** El detalle completo con tabla PASS/FAIL vive en
> `CB-VERIFY-FRONTEND.md` (ronda 2, reescrito); este §14 es el delta del verify-of-verify.

## 14.1 Qué pasó entre rondas (por qué re-verifiqué)

Los archivos bajo claim mutaron DESPUÉS del cierre de la ronda 1 (14:31): Client/Led mtime
2026-09-08 17:47 y el test **23:27:09 — DURANTE mi ventana de verificación**. Los edits llevan
marcadores `// CB-03 (2026-09-07, reconciliación — …)` que citan mis hallazgos F-1/B-1/B-3/F-4/F-6
uno a uno: es el builder CB-03 ejecutando mi hand-off §6 de la ronda 1 — sin reporte propio en el
dir todavía (OBS-G). page.tsx quedó byte-idéntico (`7eff1651…`, mtime 09-07 07:35).

## 14.2 Corridas propias (ronda 2 — no fe en claims ajenos)

| Check | Resultado |
|---|---|
| `tsc --noEmit` | **EXIT 0** (23:26) |
| vitest ControlBoard.test.tsx | **45/45, exit 0** (23:28; re-run 23:33 — estable, md5 `574d20c5…`) |
| Suite completa | **127 files / 1175 tests, 0 fails, exit 0** (98.5s) — el fail `:645` que 7 pares reportaron YA NO existe |
| Dominio público | **1/5 requests**: GET /control → **404** (23:31) — NO-GIT intacto, nada desplegado |
| git | `git log -1` = `27aca289` intacto; archivos untracked; 0 commits |

## 14.3 El fail :645 — cierre del loop con los 7 pares

Causa raíz: la aserción esperaba el apóstrofe sin escapar, pero React escapa `'` → `&#39;` en nodos
de texto de renderToStaticMarkup. Un agente concurrente corrigió la expectativa a la forma escapada
a las 23:27:09; mis dos corridas posteriores pasan 45/45. H-2/H-3/H-4/H-2-REVERIFY-ronda2/
H-3-REVERIFY/R3/R4/H-2-R1 eran exactos EN SU timestamp. Era bug de EXPECTATIVA del test, no del
producto — cero líneas de producto cambiaron por él.

## 14.4 Hallazgo nuevo F-7 (el que la mesa NO debe perder)

El parche ORDENADO por `CB-02-DISENO §9` (`isBoardToggleable`) no fue aplicado (0 hits en frontend).
El censo CB-01 trae 3 módulos clase A TODOS con `control_key:null` (verificado directo del JSON);
al publicarse, el board pintaría toggles que el backend SIEMPRE rechaza (400 census-defect
`control-board.ts:769-771`; 403 `:784-786`). Invisible hoy (censo sin publicar → board vacío),
rompe la soberanía del operador al aterrizar. **Must-fix antes de publicar censo/montar CB-02** +
tercera rama de affordance (no reusar el texto B). Detalle y fix en CB-VERIFY-FRONTEND.md §2.

## 14.5 Clasificación de claims (ronda 2)

- Comportamiento post-reconciliación (45/45, 1175, tsc 0, 404): **PRIMARY_SOURCE** propia.
- F-7 (census A=3 sin control_key + rechazos backend): **PRIMARY_SOURCE** (lectura de
  CB-01-MODULES.json y control-board.ts, esta sesión).
- F-8 (pool_enum case-insensitive): **PRIMARY_SOURCE** (`pool_enumeration_worker.rs:190-191`);
  la imprecisión origina en **CANONICAL_WORKBOOK** (CB-01-CENSO MERGE).
- Fail :645 resuelto: **PRIMARY_SOURCE** (mtime 23:27:09 + corridas propias) + 7 reportes pares.

Presupuesto dominio acumulado del WO (ambas rondas + addendum): **3/5** (404 mitad A · 404 addendum
14:20 · 404 ronda 2 23:31).
