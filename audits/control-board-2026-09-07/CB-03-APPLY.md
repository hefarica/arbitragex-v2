# CB-03 — APPLY COMPLETO: /control con pestañas, badge de defaults y slot de drift

- **WO**: CB-03 · kind: apply · **Agente**: frontend-architect (Gang Omniscience)
- **Fecha**: 2026-09-07 · **Branch de trabajo**: `feat/hops-live-01` (heredada de la sesión
  paralela — la observó también CB-02-RUST-APPLY.md §2.1). **NO-GIT: cero commit/push/PR/deploy.**
- **Charter**: completar el esqueleto `/control` de la sesión paralela SIN reporte,
  PRESERVÁNDOLO (solo aditivo), más la EXTENSIÓN del operador 2026-09-07 (GOAL-WORKORDERS.md
  §extensión ítems 2 y 4 + slot CB-04).

---

## 1. Lo que PRESERVÉ (forense: leído íntegro antes de tocar; cero reescrituras)

Los 4 archivos del claim estaban **untracked** (nunca commiteados; `git status`: `?? frontend/app/control/`,
`?? frontend/components/ControlBoardLed.tsx`, `?? frontend/components/__tests__/ControlBoard.test.tsx`).
Esqueleto leído completo ANTES de la primera edición:

| Archivo | Esqueleto (líneas al leer) | Preservado | Estado tras mi apply |
|---|---|---|---|
| `frontend/app/control/page.tsx` | 38L | **100% INTACTO — 0 edits míos** (SSR R1, fetch INTERNAL edge, error verbatim) | 38L |
| `frontend/app/control/ControlBoardClient.tsx` | 440L | TODO el esqueleto intacto: R1 `useState(initialSnapshot)`, poll 30s (`useEffect`), `ControlToggleConfirm` con razón OBLIGATORIA + token admin fallback, `requestToggle`/`confirmToggle` con defense-in-depth `isToggleable`, `ModuleRow`, estados honestos (error verbatim, board vacío, cargando) | 525L (+85 aditivas) |
| `frontend/components/ControlBoardLed.tsx` | 367L | TODO el esqueleto intacto: contrato Zod `GET/PUT /api/v1/control-board`, `projectLed` (fluor verde glow/rojo/ámbar DESCONOCIDO/locked gris), `isToggleable` (solo A), `driftStatus` (R8 no-data), `fmtIso` determinista, `ControlBoardLed`/`ControlClassBadge`/`DriftSlot` | 758L (+391 aditivas) |
| `frontend/components/__tests__/ControlBoard.test.tsx` | 357L (25 tests) | **Los 25 tests originales intactos y verdes** — solo añadí fixtures/describes nuevos; 3 líneas de aserción NUEVAS mías se ajustaron durante el debug (ninguna línea original modificada) | 593L (+236, tests 25→41) |

Invariantes mantenidos (verificados por los tests originales, que siguen pasando):
R1 render determinista · LED por estado VERIFICADO (no declarado) · DESCONOCIDO ≠ apagado
(RULE 00) · candado C §34.3 sin toggle (ausencia de handler aseverada) · razón obligatoria ·
drift-slot vacío sin datos.

## 2. Lo que AÑADÍ — extensión del operador (GOAL-WORKORDERS.md:40-51)

### 2.1 Pestañas clasificadas (extensión §2)

**Contrato del campo `tab`** — CB-02-DISENO NO aterrizó (confirmado: dir solo tenía
GOAL-WORKORDERS.md, CB-02-RUST-APPLY.md, CB-05-*), así que CB-03 **define** el tipo y lo
documenta aquí para reconciliación (regla del charter):

- `ControlBoardModuleSchema.tab: z.string().nullish()` — **tolerante por diseño** (campo de
  agrupación/display, no de control): `ControlBoardLed.tsx:52-58`.
- IDs canónicos (orden EXACTO del operador): `detection|evaluation|execution|infra|security|frontend`
  → etiquetas "Detección / Discovery · Evaluación / Matemática · Ejecución / Terminus ·
  Infra / Observabilidad · Seguridad · Frontend" — `CONTROL_BOARD_TAB_VALUES` /
  `CONTROL_BOARD_TABS`: `ControlBoardLed.tsx:401-441`.
- Tab `null`/ausente/desconocida → bucket honesto **"Sin clasificar"** al final, con nota
  "CB-01 censo pendiente" — JAMÁS se inventa la categoría (RULE 00): `groupModulesByTab`
  `ControlBoardLed.tsx:444-475` (solo pestañas con ≥1 módulo: vacías no se fabrican).
- Pestaña **Ejecución / Terminus** lleva nota fija "Categoría C §34.3: candado por diseño —
  sin toggle visible; flips LIVE_MAINNET = operador-only" (`note` en `CONTROL_BOARD_TABS:423`).
  El candado ES orthogonal a la pestaña (defense in depth): un módulo C en cualquier pestaña
  sigue locked+no-toggleable (`projectLed`/`isToggleable` intactos).
- UI en `ControlBoardClient.tsx`: estado `activeTab` derivado del snapshot inicial
  (`:281-283`, determinista R1), derivación de pestaña activa EFECTIVA si la elegida quedó
  vacía tras refresh (`:363-366`), tabstrip ARIA (`role=tablist/tab/tabpanel`, `aria-selected`,
  `aria-controls`, `hidden` en inactivas) `:448-498`. **Los paneles inactivos se renderizan
  con `hidden` (no se desmontan)**: markup completo y greppable, snapshot no apostado a la
  pestaña abierta, y el patrón de tests de static-markup del repo sigue funcionando.
- Tests: `ControlBoard.test.tsx:422-499` (orden canónico, bucket Sin clasificar, tabstrip
  activa, nota §34.3 execution, R1 byte-idéntico).

### 2.2 Badge discreto de defaults — «postura de máximo potencial» (extensión §4)

- `MAX_POTENTIAL_DEFAULTS` (14 filas) + `DefaultsPanel`: `ControlBoardLed.tsx:545-758`.
  Panel plegable `<details>` nativo (cerrado por defecto, cero estado JS — R1), renderizado
  al fondo del board (`ControlBoardClient.tsx:499-501`), **también sin snapshot** (es
  documentación, independiente del runtime).
- Columnas: módulo/clave · servicio · **default fail-safe (código, con file:line)** ·
  **máx. potencial (canon)** · VPS 2026-09-07 observado · **gana/pierde (nota honesta)** · fuente.
- **Valores SOLO del canon, cada fila citada** (RULE 00): fuente primaria
  `CB-05-PROPUESTA-B.md §1` (inventario clase B verificado ssh read-only ~12:40Z) + `§8-E1/E2/E3`
  (trade-offs documentados) + `CB-02-RUST-APPLY.md §3.1` (kill-switch fail-safe). **CB-01-CENSO.md
  NO existe** (verificado) → el panel lo declara y se reconciliará al aterrizar.
- Honestidad por construcción: donde el canon NO documenta trade-off, la celda dice
  **"no documentado en canon"** (no lo invento); la única recomendación "máx. potencial"
  con respaldo canon es `ARBX_ROUTE_SCANNER_MODE=on` (RU-3, aplicado hoy por el operador,
  CB-05 §8-E1) y `ARBX_SCORING_HARD_GATE=true` (propuesta E2 **PENDIENTE — NO aplicada**,
  con la nota de pérdida documentada: "puede reducir emisiones aceptadas por FLAT_PRIOR").
  El encabezado del panel declara "NO es estado runtime — el estado vivo está en su LED".
- Tests: `ControlBoard.test.tsx:545-577` (details plegado, fila-por-fila = constante canon,
  citas presentes, "no documentado en canon" presente, renderiza sin snapshot).

### 2.3 Slot de banner GLOBAL de drift (consume a CB-04)

- **CB-04 no existe** → CB-03 define el campo propuesto en el snapshot para reconciliación:
  `ControlBoardDriftSchema` (`detected`/`checked_at`/`summary`/`diff_href`),
  `ControlBoardLed.tsx:80-97`; snapshot `drift: ControlBoardDriftSchema.nullish()` `:104`
  (OPCIONAL — snapshots actuales sin el campo siguen parseando).
- `GlobalDriftBanner`: `ControlBoardLed.tsx:486-540`, montado en
  `ControlBoardClient.tsx:401-403`. Tres estados honestos:
  1. campo ausente → **AUSENCIA HONESTA**: línea muted `data-drift="unavailable"` "drift
     global: sin dato (CB-04 pendiente)". **NINGÚN banner fabricado** (RULE 00, test lo pincha).
  2. `detected=true` → banner `role="alert"` con summary + link a diff + timestamp.
  3. `detected=false` → línea discreta con timestamp del chequeo.
- Cuando CB-04 defina su campo/endpoint REAL, se reconcilia el shape aquí (§6).

### 2.4 Link a /killswitch (ENLAZA, no duplica)

- `ControlBoardClient.tsx:378-390`: link discreto `data-testid="cb-killswitch-link"` →
  `/killswitch` en el header del board, **exactamente una vez** (test lo asevera). El
  kill-switch YA tiene página propia (`frontend/app/killswitch/page.tsx`) con su toggle+razón —
  el board no lo replica. Nota honesta de alcance: el link vive en el header del snapshot;
  sin snapshot (edge caído) no se renderiza — la navegación global del sitio mantiene el acceso.
- Defaults panel lo referencia además como fila documental (fuente CB-02-RUST-APPLY.md §3.1).
- Tests: `ControlBoard.test.tsx:581-593`.

## 3. Verificación (gates del charter)

| Gate | Comando | Resultado |
|---|---|---|
| Tests del WO | `npx vitest run components/__tests__/ControlBoard.test.tsx` | **41/41 PASS** (25 originales + 16 nuevos) |
| Suite frontend completa | `npx vitest run` | **EXIT 0 — 1114/1114 PASS** (pasada final 09:25:45) |
| Typecheck | `npx tsc --noEmit` | **EXIT 0** (0 errores) |

- Flake observado y caracterizado (NO mío): en 2 de 4 pasadas de la suite completa
  `ControlScopeBadge.test.tsx > /settings §53 > every card labeled LOCAL_PREFS` falló por
  **timeout de 5s bajo carga** (corriendo concurrente con tsc; transform 438s esa pasada).
  En aislamiento pasa **6/6 en 280ms**, archivos NO tocados por mí (`git status` limpios),
  grafo de imports sin intersección con mi claim, y la pasada final completa fue EXIT 0.
- Fix propio durante el apply: 3 errores TS que introduje (`noUncheckedIndexedAccess`) en
  tests NUEVOS — corregidos con acceso seguro (`find`/`?.`); cero errores en archivos ajenos.

## 4. Sincronía de mesa (construyo sobre los pares; contradicciones: ninguna)

- **CB-02-RUST-APPLY.md** (NO-OP honesto): confirma que `GET/PUT /api/v1/control-board` NO
  existe → la página hoy renderiza su estado honesto de error verbatim ("sin datos del
  board… verifica que el endpoint CB-02 exista", esqueleto preservado `ControlBoardClient.tsx:405-424`).
  **La página queda completa y esperando al endpoint** — sin datos fabricados mientras tanto.
  Su §3 (mapa kill-switch/canonical_knobs) fue insumo directo de la fila kill-switch del
  panel de defaults.
- **CB-05-PROPUESTA-B.md §1/§8**: canon EXACTO del panel de defaults (valores, defaults
  fail-safe con file:line, estado VPS ~12:40Z, trade-offs E1/E2/E3). Su borde §9-CB-03 dice
  "el botón «Proponer cambio» + sección propuestas viven en /control" — eso es **apply-fase
  de CB-05** (requiere endpoint + `audit_logs`, que CB-02 aún no crea); mi badge B existente
  ya lo anuncia ("clase B: el cambio exige restart — propuesta de diff en CB-05",
  `ControlBoardClient.tsx:250` esqueleto intacto). No lo implementé aquí: sería trabajo
  sin backend que lo reciba (P-∅).
- **CB-05-DESIGN.md hallazgo 3**: `audit_logs` NO existe (PG vacío) — consistente con que el
  confirm del toggle declara "queda registrado en audit_logs" como CONTRATO del endpoint
  futuro CB-02, no como afirmación de que la tabla exista hoy.
- **GOAL-WORKORDERS.md extensión §3 (soberanía/reversión clase A)**: es backend (CB-02/CB-04);
  el frontend ya expone el slot de drift global (2.3) y drift por módulo (esqueleto) donde
  esa reversión se hará visible.

## 5. Deuda de reconciliación (para los pares posteriores)

1. **`tab`**: CB-02-DISENO/CB-01 deben confirmar el enum exacto; hoy es `string` tolerante y
   los 6 IDs canónicos son los de §2.1. Si CB-02 aterriza otro nombre de campo (p.ej.
   `category`), el cambio es de una línea en `ControlBoardModuleSchema` + `toBoardTabId`.
2. **`drift`**: shape propuesto en §2.3 — CB-04 lo ratifica o reemplaza (campo snapshot vs
   endpoint separado).
3. **CB-01-CENSO.md**: al aterrizar, reconciliar `MAX_POTENTIAL_DEFAULTS` (hoy 100%
   CB-05-PROPUESTA-B + CB-02-RUST-APPLY) y poblar `tab` de los módulos server-side.
4. **CB-06 (browser-verify)**: pendiente por diseño — cada LED contra realidad, toggles A
   end-to-end, capturas. El tablero está listo para ese viaje.

## 6. Cumplimiento de reglas duras

- **RULE 00**: cero datos fabricados — LEDs solo del snapshot; DESCONOCIDO ámbar; drift
  "unavailable" mientras CB-04 no exista; defaults documentales citados con "no documentado
  en canon" donde falta.
- **§32/§33**: cero executor/wallets/capital/firma/broadcast; no toqué VPS (ni lectura —
  no hizo falta: todo provino del canon de la mesa). 0 de 5 requests HTTP del dominio público.
- **§34.3**: candado C intacto y reforzado (nota de pestaña Ejecución + fila kill-switch del
  panel cita el fail-closed). default-deny/MainnetRefused intocables (ni nombrados como
  proposables — denylist CB-05 §7 respetada).
- **NO-GIT**: cero commit/push/PR/deploy; edición local + verificación únicamente.
- Diffs marcados `// CB-03 (2026-09-07)` en todos los sitios de adición (grep total: 19
  marcadores — 4 son los headers de esqueleto que ya los traían, 15 son míos:
  5 en ControlBoardLed.tsx, 6 en ControlBoardClient.tsx, 4 en el test; page.tsx solo el
  header del esqueleto, sin edits míos).

---

# §7 ADDENDUM (2026-09-08) — segunda pasada CB-03: forense de la reconciliación 14:31 + cierre del único fail de la suite completa

- **Agente**: frontend-architect (Gang Omniscience, re-despacho como dueño CB-03) ·
  **Fecha**: 2026-09-08 · **NO-GIT intacto** (`git log -1` = `27aca289`; los 4 archivos del
  claim siguen untracked; mi diff es working-tree only).
- **Mesa leída ANTES de actuar** (completo o relevante): `GOAL-WORKORDERS.md`,
  `CB-03-APPLY.md` (secciones 1-6, el reporte previo MÍO que preservo íntegro abajo),
  `CB-04-APPLY.md`, `CB-VERIFY-FRONTEND-VERIFY.md` (mitades A+B+§13 addendum),
  `H-1-FIXER-next-action-blockers-vivos.md`, `H-2-R1-REVERIFY.md`, `H-3-REVERIFY.md`,
  `R3-REVERIFY.md`, `R4-REVERIFY-ronda2.md`. Presupuesto dominio público: **0/5** (todo
  verificado local; VPS intocado).

## 7.1 Forense de la «pasada 14:31» (edición SIN reporte que encontré al llegar — PRESERVADA)

El estado del disco NO coincidía con este reporte (§1-§6 describe el estado 09:15:32:
38/525/758/593 líneas, 41 tests). CB-VERIFY-FRONTEND-VERIFY §13.1 pineó byte-estable ese
estado a las 14:21-14:22 (41/41 PASS + tsc 0) y su §9-B-1/test:675 documentan una
«pasada 14:31» posterior que aplicó los advisories del verificador y reconcilió con
CB-01-CENSO.md (que aterrizó 09:27, DESPUÉS del apply original — el §2.2 de abajo decía
"CB-01-CENSO.md NO existe", cierto a las 08:02 y falso desde las 09:27). Leí los 4 archivos
completos antes de tocar; esa pasada está marcada `CB-03 (2026-09-07, reconciliación …)` en
todos sus sitios. **Preservada al 100% — cero edits míos sobre ella.** Mapa (evidencia
file:line del estado actual):

| Archivo | Estado 09:15 | Estado hoy | Deltas de la 14:31 (todos preservados) |
|---|---|---|---|
| `page.tsx` | 38L | 38L | ninguno (intacto desde el esqueleto) |
| `ControlBoardClient.tsx` | 525L | 539L | **F-1**: `autoFocus` en la razón `:107-111` + `role="alert"` en el error del confirm `:127-135` · **B-1**: link /killswitch + refresh hoisted FUERA del guard de snapshot `:380-413` |
| `ControlBoardLed.tsx` | 758L | 848L | **F-4** (must-fix del verificador §13.3): rama `clear` del `GlobalDriftBanner` pinta `drift.summary` verbatim + enlaza `diff_href` `:547-563` — un "none" sin caveat sobre un drift NO computable ya no es posible · docblocks de reconciliación `tab` (CB-01 ratificó 44/44) `:52-58` y `drift` (CB-04 sirve el shape exacto) `:83-88` · **B-4** aclarado `:158-160` · nota "Sin clasificar" re-escrita (censo YA clasifica; publicación = aprobación) `:474-477` · defaults panel merge CB-01: filas nuevas SIM_BACKEND `:636-645`, macro-mev coma decimal CONFIRMADA `:694-703`, CSP BUILT-NOT-WIRED `:753-762`, pool_enum default OFF (refuta CB-05 §1) `:764-774`, trading_config 4/5 cadenas IDLE `:778-787`, killswitch censado `:604-611` |
| `ControlBoard.test.tsx` | 593L (41 tests) | 690L (45 tests) | **B-3**: test board-vacío `:355-368` · **F-4** ×2: clear NO-computable pinta summary verbatim `:568-587` + clear consistente informativo `:589-602` · test reconciliación CB-01 `:641-657` · **B-1**: link presente también en outage `:679-684` · assertions actualizadas (nota unclassified `:514`, texto unavailable `:530`, honestidad del panel `:635-636`) |

Aritmética 41→45: la corrida 41/41 de las 14:22 fue a mitad de pasada (el propio verificador
advierte de la carrera §ADVERTENCIA); los +5 tests (B-3, F-4×2, reconciliación, B-1) aterrizaron
hasta las ~14:31. La pasada 14:31 **no dejó reporte ni re-verificó la suite completa** — su
aserción nueva nació rota (§7.2), y ese fail fue reportado como deuda CB-03 por 7 pares
(H-2-REVERIFY-ronda2, H-3-REVERIFY, H-2-R1-FIX, H-2-R1-REVERIFY §3.4, R3-REVERIFY §1,
R4-STATCARD-FIX, R4-REVERIFY-ronda2 — todos vieron 1171/1172 o equivalente con el único
fail en `ControlBoard.test.tsx`).

## 7.2 El defecto (born-failing assertion) — root cause

`ControlBoard.test.tsx:645` (pre-fix) esperaba `SOLO &#39;shadow&#39; exacto spawnea`, pero
React 18+ escapa el apóstrofe en nodos de texto como **`&#x27;`**, no `&#39;` (entidad
HTML4 que su propio comentario asumía). El componente es CORRECTO (la fila del panel dice
`SOLO 'shadow' exacto spawnea`, ControlBoardLed.tsx:769 — refutación CB-01 a CB-05 §1,
RULE 00 intacta); la aserción era la equivocada. Evidencia primaria: corrida propia pre-fix
→ recibido `SOLO &#x27;shadow&#x27; exacto spawnea` en el diff de vitest, expected `&#39;`.
Nació fallando (última corrida verde documentada 14:22 = antes de que la aserción existiera)
y falló en TODA corrida posterior — 8º agente que lo observa, primero que lo cierra.

## 7.3 El fix (quirúrgico, 1 aserción + comentario)

`ControlBoard.test.tsx:643-650`: esperado corregido a `SOLO &#x27;shadow&#x27; exacto
spawnea` + comentario que documenta el escape correcto y la historia del fail, marcado
`// CB-03 (2026-09-07 — fix 2026-09-08)`. Cero cambios en componentes: el pin mantiene su
fuerza total (sigue exigiendo la refutación CB-01 textual, solo con la entidad que React
realmente emite). El pinneo es genuino: si alguien "simplifica" la fila del panel y elimina
el `SOLO 'shadow' exacto`, el test vuelve a romper.

## 7.4 Verificación (gates del charter, corridas propias 2026-09-08 23:27-23:31)

| Gate | Comando | Resultado |
|---|---|---|
| Tests del WO | `npx vitest run components/__tests__/ControlBoard.test.tsx` | **45/45 PASS** |
| Suite frontend completa | `npx vitest run` | **EXIT 0 — 127 files / 1175 tests / 0 fails** (99.1s) — primera corrida 100% verde desde la pasada 14:31 (peers venían viendo 1160-1172 con 1 fail) |
| Typecheck | `npx tsc --noEmit` | **EXIT 0** (0 errores) |
| Cirugía / NO-GIT | `git log -1` + `git status --porcelain` | HEAD `27aca289` intacto; claim sigue untracked; único delta del árbol mío = el fix §7.3 |

Deriva de suite 1172→1175 en ~20 min desde R3-REVERIFY: trabajo paralelo de pares aterrizando
tests — sin nuevas fallas (0 fails).

## 7.5 Sincronía de mesa (construyo sobre los pares; contradicciones: ninguna)

- **CIERRO el ítem que 7 pares me asignaron**: "CB-03 (agent-fixable, territorio vivo de
  otro WO): ControlBoard.test.tsx 1 fail — dueño CB-03" (H-2-R1-REVERIFY §3.4, R3-REVERIFY
  §1, R4-REVERIFY-ronda2 §2, y sus fijadores). La suite completa vuelve a ser 0-fails para
  cualquiera que la corra desde ahora.
- **Confirmo (no aplico yo) los advisories del verificador**: F-4 (must-fix pre-mount),
  B-1, B-3, F-1 y B-4 ya están aplicados por la pasada 14:31 — verificados por lectura
  propia en §7.1 y pineados por sus tests (F-4 ×2 `:568-602`, B-1 `:679-684`, B-3 `:355-368`).
  Con esto el §13.4 de CB-VERIFY-FRONTEND-VERIFY ("entradas exigidas al futuro PR de
  montaje: F-4") queda satisfecho del lado frontend.
- **Deuda §5 de este reporte — estado**: (1) `tab` RATIFICADO (CB-01-MODULES.json clasifica
  44/44 en los 6 IDs exactos; CB-01-CROSS-EXAM §1a lo re-validó contra este schema) ·
  (2) `drift` RATIFICADO (CB-04 sirve el shape exacto dentro del snapshot) ·
  (3) CB-01-CENSO aterrizó y YA fue mergeado al panel de defaults por la 14:31. La deuda
  documental del §2.2 ("CB-01-CENSO NO existe") queda obsoleta por §7.1.
- **NO cierro (fuera de mi claim)**: R2 de la saga H (`OpportunitiesByStrategyClient.tsx`
  "0.00%" fabricado — último agent-fixable de esa lista, hand-off de R3-REVERIFY §4-G3);
  mount de CB-02 mitad B (index.ts); línea hermana del edge proxy para
  `/api/v1/control-board/drift` (CB-04-APPLY §6.1); fase apply de CB-05; CB-06 post-deploy.

## 7.6 Reglas duras (esta pasada)

- **RULE 00**: el fix NO toca datos — corrige una aserción para que exprese lo que React
  realmente serializa; el texto del panel sigue siendo la cita CB-01 textual.
- **§32/§33/§34.3**: cero executor/wallets/capital/firma/broadcast; VPS intocado;
  candado C y default-deny intactos (ningún edit de componentes).
- **NO-GIT**: cero commit/push/PR/deploy; 1 aserción + comentario en working tree.
- Marcador: `// CB-03 (2026-09-07 — fix 2026-09-08)` en `ControlBoard.test.tsx:643`.
