# CB-VERIFY-FRONTEND — Verificación adversarial del tablero /control (CB-03 + extensión) · RONDA 2

- **WO**: CB-VERIFY-FRONTEND · kind: verify · **Agente**: ecc:react-reviewer (Gang Omniscience, PhD)
- **Fecha**: 2026-09-08 (pasada 23:26–23:3x local) · **Read-only sobre código · CERO git · CERO edits**
  (diffs propios: ninguno — este WO es verificación).
- **Charter**: gates (1) tsc+vitest · (2) R1 hidratación + R5 transitivos · (3) a11y · (4) semántica del
  operador (6 pestañas, LED verde/rojo/ámbar, candado C sin botón, badge B→CB-05, razón obligatoria,
  defaults honesto) · (5) RULE 00 (sin snapshot no hay LEDs, error verbatim, board vacío honesto, cero
  banner drift fabricado) · (6) /killswitch enlazado no duplicado.
- **Genealogía**: RONDA 1 = 2026-09-07 (dos mitades RESPAWN-2 + addendum 14:31, ver
  `CB-VERIFY-FRONTEND-VERIFY.md` §1-§13; veredicto 6/6 PASS con F-4/F-6/B-1/B-3/F-1 como hand-offs).
  Entre rondas, el builder CB-03 aplicó la reconciliación (edits mtimes 2026-09-08 17:47 + fix del test
  23:27:09, ver §3) — marcadores `// CB-03 (2026-09-07, reconciliación — …)` citando mis hallazgos
  F-1/B-1/B-3/F-4/F-6 uno a uno. **Esta ronda re-verifica TODO contra el árbol actual.**

## 0. Estado auditado (pinado al cierre)

Branch `feat/hops-live-01`, HEAD `27aca289` (intacto). Archivos (untracked, working tree only):

| Archivo | Líneas | md5 | mtime |
|---|---|---|---|
| `frontend/app/control/page.tsx` | 38 | `7eff1651…` | 09-07 07:35 (**sin cambios desde ronda 1**) |
| `frontend/app/control/ControlBoardClient.tsx` | 539 | `ff9bcfa4…` | 09-08 17:47 |
| `frontend/components/ControlBoardLed.tsx` | 848 | `f0bd5c15…` | 09-08 17:47 |
| `frontend/components/__tests__/ControlBoard.test.tsx` | 685 | `574d20c5…` | 09-08 **23:27:09** (editado DURANTE mi ventana — ver §3) |

## 1. Tabla PASS/FAIL — veredicto global (ronda 2)

| Gate | Criterio | Veredicto | Evidencia (file:line, lectura propia 2026-09-08) |
|---|---|---|---|
| **1a** | `tsc --noEmit` frontend | **PASS** | EXIT 0 (23:26, árbol completo — coincide con H-2-R1-REVERIFY §2 y R3-REVERIFY §1) |
| **1b** | vitest `ControlBoard.test.tsx` (ampliado: 41→45 tests) | **PASS** | **45/45**, exit 0 (23:28 y re-run 23:33 — estable). El fail `:645` que 7 pares reportaron está resuelto (§3) |
| **1c** | Nada más se rompió | **PASS** | Suite completa: **127 files / 1175 tests, 0 fails, exit 0** (98.5s, 23:30) — la suite creció 1114→1175 por trabajo paralelo H/R; sin nuevos fails |
| **2a** | `page.tsx` Server Component puro | **PASS** | Sin `"use client"` ni hooks (page.tsx:24); `force-dynamic`+`revalidate=0` (:21-22); fetch → props (:25-27). **Byte-idéntico a ronda 1** |
| **2b** | `useState(initialSnapshot)` | **PASS** | Client:280-281 (snapshot/error nacen de props); `fetchedAt` inicial `null` (:282); `activeTab` con inicializador lazy DERIVADO del snapshot inicial (:291-293) — determinista |
| **2c** | Cero no-determinismo en render | **PASS** | grep propio: únicos `Date.now()` = Client:300 (callback `refresh`) y :363 (handler `confirmToggle`); 0 hits `window.`/`document.`/`navigator.`/`Math.random`/`localStorage` en los 3 archivos. `autoFocus` (Client:110, nuevo F-1-fix) vive SOLO en el panel de confirmación, que renderiza únicamente tras interacción (`pending` inicial null :284) → jamás en SSR, cero riesgo de hidratación |
| **2d** | R5: ControlBoardLed | **PASS** | Sin `"use client"`, sin hooks, sin side-effects de módulo; `getApiBaseUrl()` solo dentro de fetch/put (Led:122,173); componentes = funciones puras de props |
| **2e** | R5: SourceMeta/LastUpdated | **PASS** | SourceMeta gates `pollMs != null && at != null` (SourceMeta.tsx:38); LastUpdated `if (at == null) return null` ANTES de `Date.now()` (last-updated.tsx:24); tick 1s solo en useEffect (:19-22) |
| **2f** | R5: PageHeader | **PASS** | `showRefresh = false` default (page-header.tsx:11) y page.tsx no lo pasa → RefreshButton jamás montado (:31) |
| **2g** | R5: lib transitivos | **PASS** | `hasAdminSession`/`setAdminToken` invocados SOLO dentro de `confirmToggle` (Client:342-354); `getAdminToken` guarda `typeof document === "undefined"` (admin-token.ts:58); `getWsBaseUrl` guarda `isBrowser` (api-client.ts:69-71) |
| **2h** | Timestamps deterministas | **PASS** | `fmtIso` slice UTC crudo (Led:261-264); pineado test:334-339 |
| **3a** | Labels de inputs | **PASS** | `Label htmlFor="cb-reason"`+`Input id` (Client:97-101); `cb-admin-token` (:113-117) |
| **3b** | aria-label en confirm | **PASS** | Panel `role="group"`+`aria-label="confirmar {…} de {module}"` (Client:87-88); refresh `aria-label="refrescar board"` (:408); `section aria-label="control board"` (:379) |
| **3c** | LED accesible | **PASS** | `role="img"`+`aria-label` por estado+`title` (Led:310-315); LockIcon y dot `aria-hidden` (Led:319,321); label textual SIEMPRE adyacente (Led:323-325); carga `role="status"`+`aria-busy` (Client:437-439) |
| **3d** | Foco | **PASS — F-1 RESUELTO** | `autoFocus` en `cb-reason` (Client:110-111) + `role="alert"` en `cb-confirm-error` (Client:131) — ambos con marcador de reconciliación citando mi F-1 |
| **3e** | Contraste LED fluor claro/oscuro | **PASS w/ ADVISORY F-2/F-3 (sin cambio)** | Estado NO transmitido solo por color (label textual obligatorio → WCAG 1.4.1). Dot `lime-400` ≈1.5:1 sobre card claro (<3:1 no-texto) / ≈10:1 en oscuro; badge A `text-lime-600` ≈3.1:1 a 10px. Patrón repo-wide, no introducido por CB-03 |
| **4a** | 6 pestañas correctas y completas | **PASS** | `CONTROL_BOARD_TABS` Led:424-435 = las 6 del operador en orden, con nota §34.3 en Ejecución (:431); tabstrip ARIA (Client:463-491) + paneles `hidden` no-desmontados (:492-509); solo pestañas con ≥1 módulo (Led:449-482); docblock reconciliado con CB-01 (Led:52-58: 44/44 en 6 IDs, 0 huérfanos — re-validado por CB-01-CROSS-EXAM §1a). Tests :443-521 |
| **4b** | LED verde/rojo/ámbar=DESCONOCIDO | **PASS** | `projectLed` Led:215-222 (C→locked SIEMPRE; null/undefined→unknown; jamás off falso) + `LED_VISUALS` Led:268-298 (lime-400+glow / red-500 / amber-400 / zinc-500); lede page.tsx:33; tests :137-165, :250-274 |
| **4c** | Candado C SIN botón (ausencia de handler) | **PASS** | `isToggleable` solo A (Led:229-231); rama C = `<span>` sin control (Client:248-257); `requestToggle` re-guarda (:314-317); `confirmToggle` re-valida (:331-334). Test de ausencia ORIGINAL intacto (:176-184) + refuerzo (:494-502) |
| **4d** | Badge B requiere-restart → CB-05 | **PASS** | `CLASS_BADGES.B` Led:336-340 ("B · requiere-restart", title "…CB-05 propone el diff"); texto de fila Client:259-261; test :377-379 |
| **4e** | Razón obligatoria deshabilita confirm | **PASS** | `reasonReady` Client:81 + `disabled={busy \|\| !reasonReady}` :144 + re-validación :335-339; tests :210-239 |
| **4f** | Defaults panel honesto (canon citado, gana/pierde) | **PASS — F-6 RESUELTO, con nueva precisión F-8** | `DefaultsPanel` Led:794-848 (`<details>` cerrado, cero JS) + `MAX_POTENTIAL_DEFAULTS` Led:598-788 (18 filas × 7 columnas, TODAS con `source`); reconciliado con CB-01-CENSO (header Led:807-813; ya NO dice "pendiente" — pineado test:635-636); "no documentado en canon" donde falta; única recomendación aplicada = RU-3, E2 declarada PENDIENTE/NO aplicada (Led:628). Spot-checks propios §4. Tests :607-663 |
| **5a** | Sin snapshot no hay LEDs | **PASS** | Filas gated en snapshot (Client:457); loading honesto (:436-444); `fetchControlBoard` union `{ok,data}\|{ok,error}` — nunca fabrica snapshot (Led:111-152); test :341-347 |
| **5b** | Error verbatim | **PASS** | `<code>{error}</code>` break-all (Client:424); composición `edge HTTP {status}: {body}` (Led:131,183); test :342-343 |
| **5c** | Board vacío honesto | **PASS — B-3 RESUELTO (test propio)** | Alert "board vacío — censo CB-01 sin publicar" (Client:446-455); test dedicado :355-367 (0 filas, 0 tabstrip, drift unavailable). Cadena real confirmada por CB-02-API-VERIFY G6 + CB-01-CROSS-EXAM §1e (ronda 1) |
| **5d** | NINGÚN banner drift fabricado | **PASS — F-4 RESUELTO** | `GlobalDriftBanner` Led:498-565: ausente→`unavailable` honesto (:503-515); `detected`→`role="alert"` (:517-539); **clear ahora pinta `drift.summary` + `diff_href`** (:541-564, con comentario citando mi F-4) → el caso no-computable del backend (detected:false + "NO computable") ya NO produce un "none" verde. Tests nuevos :568-587 (census_absent verbatim + diff enlazado) y :589-602 (consistent informativo) |
| **6** | /killswitch ENLAZADO no duplicado | **PASS — B-1 RESUELTO** | Link `data-testid="cb-killswitch-link"` Client:395-402 ahora FUERA del guard de snapshot (:380-413, con comentario citando mi B-1) → presente también en outage; pineado EXACTAMENTE 1 vez en AMBOS estados (tests :667-673, :679-684); único PUT del board = `/api/v1/control-board` (Led:173); `/killswitch` sigue en el nav del sitio (nav-items.ts:72) |

**VEREDICTO GLOBAL RONDA 2: PASS 6/6 gates** (todos los sub-gates PASS; 3e con advisory F-2/F-3
heredado, no bloqueante). Los 5 hand-offs de la ronda 1 (F-1, F-4, F-6, B-1, B-3) están RESUELTOS y
pineados por tests nuevos (41→45). Quedan **1 finding de integración NUEVO (F-7, must-fix antes de
publicar el censo CB-01 en deploy)**, 1 menor documental (F-8), 1 cosmético persistente (F-5) y
1 observación de gobernanza (§3).

## 2. Hallazgos nuevos y estado de los previos

### Nuevos

- **F-7 (FINDING DE INTEGRACIÓN — must-fix antes de publicar el censo CB-01 / montar CB-02 en deploy)**:
  el parche acotado ORDENADO por `CB-02-DISENO.md §9` — función pura `isBoardToggleable`
  (`module_class === "A" && control_key?.startsWith("redis:arbx:controlboard:")`) — **NO fue aplicado**
  (grep `isBoardToggleable` en frontend: 0 hits; grep `controlboard:` en Led/Client: 0 hits). Hoy es
  invisible (censo sin publicar → board vacío), pero `CB-01-MODULES.json` trae **3 módulos clase A,
  TODOS con `control_key: null`** (trading_config_gate, aave_watchlist, kill_switch — verificado
  directo del JSON). Al publicarse el censo, `isToggleable({module_class:"A"})` = true → el board
  pintaría "Encender/Apagar" en los 3, y el PUT SIEMPRE fallaría: el backend rechaza clase-A sin
  control_key con 400 "census declares class-A module '…' without control_key — census defect,
  refusing to write" (`backend/api-server/src/routes/control-board.ts:769-771`) y clave externa al
  namespace con 403 `control_key_not_board_writable` (:784-786). Es exactamente el toggle-rechazado
  que DISENO §9 ordenó prevenir ("CB-03 pintaría un toggle que este endpoint rechazaría").
  **Fix (builder CB-03)**: aplicar la función de DISENO §9 + una TERCERA rama de affordance en
  `ModuleRow` (Client:237-262) para clase-A-no-board-key — texto tipo "superficie externa enlazada /
  censo sin control_key — ver telemetría", NO el texto B (sería equívoco para un módulo A). Con tests
  (fila A con `control_key:null` → sin botón; fila A con clave board → botón).
- **F-8 (menor, precisión documental — defaults panel fila pool_enum)**: la fila dice "off — SOLO
  'shadow' exacto spawnea" (Led:769) citando a CB-01; el código real es
  `if !mode.trim().eq_ignore_ascii_case("shadow")` (`pool_enumeration_worker.rs:190-191`) — la
  comparación es **case-insensitive tras trim**: "SHADOW"/"Shadow" TAMBIÉN spawnean. El default-OFF
  (ausente/otro valor → no-op) es correcto; la frase "exacto" subdeclara la superficie de spawn
  (dirección conservadora, sin efecto en estado). La imprecisión ORIGINA en el canon
  (CB-01-CENSO MERGE) — hand-off a CB-01-CROSS-EXAM para enderezar el canon Y la fila.
- **OBS-G (observación de gobernanza)**: los edits de reconciliación (mtimes 09-08 17:47) y el fix
  del test (:645, mtime 23:27:09 — DURANTE mi ventana de verificación) llevan marcadores
  `// CB-03 (2026-09-07, reconciliación …)` con atribución correcta, pero **NO existe reporte del
  builder en este dir** que los documente (posible reporte en vuelo del agente concurrente). Si el
  agente ya terminó, que aterrice su nota para el trail de auditoría.

### Previos (ronda 1) — estado

| # | Hallazgo ronda 1 | Estado ronda 2 | Evidencia |
|---|---|---|---|
| F-1 | Foco en confirm + role=alert del error | **RESUELTO** | Client:107-111 (autoFocus), :127-135 (role="alert") |
| F-2 | Dot fluor <3:1 no-texto en claro | ABIERTO (advisory, no bloqueante) | Led:274,287 sin anillo oscuro |
| F-3 | Badge A ≈3.1:1 a 10px | ABIERTO (advisory, patrón repo) | Led:333 |
| F-4 | Clear branch descartaba `drift.summary` | **RESUELTO** | Led:541-564 + tests :568-602 |
| F-5 | Texto UI "audit_logs" vs ledger real | **ABIERTO, ahora decidable**: el backend usó `audit_log` SINGULAR (control-board.ts:99, :828-846; CB-02-DISENO §5/§15-R1) — el texto plural en Client:14,55,94,337 es ya inexacto, no "pendiente". Fix cosmético de 4 strings del builder | grep propio |
| F-6 | Defaults panel 1 paso atrás del censo | **RESUELTO** | Led:598-788 reconciliado (SIM_BACKEND, CSP, macro coma, pool_enum, trading_config); header :807-813 |
| B-1 | Link killswitch oculto sin snapshot | **RESUELTO** | Client:380-413 + test :679-684 |
| B-3 | Sin test del board vacío | **RESUELTO** | test :350-368 |

## 3. El test que 7 pares vieron fallar (ControlBoard.test.tsx:645) — diagnóstico y cierre

Siete reportes (H-2/H-3/H-4 fixes, H-2-REVERIFY-ronda2, H-3-REVERIFY, R3-FIX/REVERIFY, R4, H-2-R1)
documentaron 1 fail persistente en `ControlBoard.test.tsx:645` ("SOLO 'shadow' exacto spawnea",
casing). **Causa raíz**: la aserción esperaba el apóstrofe SIN escapar, pero React escapa `'` como
`&#39;` en nodos de texto de `renderToStaticMarkup` — la expectativa no podía pasar contra el HTML
real. **Resolución**: un agente concurrente corrigió la aserción a la forma escapada
(`expect(html).toContain("SOLO &#39;shadow&#39; exacto spawnea")` — test:645 actual) a las
**23:27:09, DURANTE mi ventana** (mtime del archivo vs mis corridas 23:28/23:33 → PASS 45/45 dos
veces, md5 estable `574d20c5…`). Los peers eran exactos EN SU MOMENTO; el fail ya no existe.
Empírico: la aserción escapada pasa → React produce `&#39;` (el hecho mismo que el fix codifica).
El test era territorio CB-03 (no mi claim) — no lo toqué.

## 4. Spot-checks adversariales del defaults panel reconciliado (muestra)

- "sin fila → cadena IDLE" (trading_config): **CONFIRMADO** — docstring canónico
  `backend/shared-rs/src/trading_config.rs:17-20` ("When a chain has no row… treats that chain as
  IDLE — explicit by the no-hardcode doctrine").
- pool_enum default-OFF: **CONFIRMADO** el default (ausente/otro → no-op,
  `pool_enumeration_worker.rs:189-193`); **PRECISADO** el "exacto" (ver F-8).
- Census clase A (insumo de F-7): **VERIFICADO directo** de `CB-01-MODULES.json`: 44 módulos
  {B:38, A:3, C:3}, los 3 A con `control_key: null`.
- Backend re-chazos clase A inválida (insumo de F-7): leído `control-board.ts:769-771` (400
  census-defect) y `:784-786` (403 not-board-writable) — la barrera existe del lado server; falta
  el affordance del lado cliente.

## 5. Sincronía de mesa redonda

- **Leídos completos**: GOAL-WORKORDERS.md, CB-03-APPLY.md, CB-02-DISENO.md (§0-§15), CB-02-API-VERIFY,
  CB-04-APPLY, CB-01-CROSS-EXAM (ronda 1), H-1-VERIFY, H-3-REVERIFY, H-2-R1-REVERIFY, R3-REVERIFY,
  R4-REVERIFY-ronda2 (índice), BROWSE-Auditor-R8 (ronda 1).
- **Confirmo a los 7 peers del fail :645**: su observación era correcta en su timestamp; está
  cerrada (§3) — la suite vuelve a 0 fails (127 files / 1175 tests).
- **Confirmo H-2-R1-REVERIFY §3.4 y R3-REVERIFY §4-G3**: el único R-agent-fixable abierto de la saga
  sigue siendo R2 (`OpportunitiesByStrategyClient.tsx`) — territorio ajeno a CB, no lo persigo aquí.
- **Refuto (con evidencia) la implicación de "pre-existente = eterno"** en los reportes H/R sobre
  el fail CB-03: era un bug de EXPECTATIVA del test (escaping de React), no un defecto del producto;
  ya corregido. Ninguna línea de producto cambió por él.
- **A CB-02-DISENO §9**: su parche ordenado sigue PENDIENTE de apply (F-7) — es la única orden de
  ese diseño que toca archivos CB-03 y no aterrizó.
- **A CB-06/browser-verify**: todo sigue SIN deploy (NO-GIT): `/control` → 404 en el dominio público
  (§6). El journey browser completo queda post-deploy, como en ronda 1.

## 6. Presupuesto dominio público y verificación corrida

**1/5 requests HTTP usados** (GET https://arbx.ape-tv.net/control → **404**, 23:31 — consistente
con NO-GIT + archivos untracked: nada de CB-03 desplegado que ver). 0 ssh (los hechos VPS que
insumen F-7 están verificados por lectura de código+censo locales; los de ronda 1 ya estaban
doble-verificados). 0 git. 0 edits.

```
frontend> ./node_modules/.bin/tsc --noEmit                                   → EXIT 0   (23:26)
frontend> ./node_modules/.bin/vitest run components/__tests__/ControlBoard.test.tsx
                                                                             → 45/45, exit 0 (23:28; re-run 23:33)
frontend> ./node_modules/.bin/vitest run                                     → 127 files / 1175 tests,
                                                                               0 fails, exit 0 (98.5s, 23:30)
curl -s -o /dev/null -w "%{http_code}" https://arbx.ape-tv.net/control       → 404 (23:31)
grep -n "Date.now|window.|document.|navigator.|Math.random" {3 componentes}  → solo Client:300,363 (callbacks)
grep -rn "isBoardToggleable" frontend/                                       → 0 hits (F-7)
node (CB-01-MODULES.json)                                                    → 44 mods; A=3, todos control_key:null (F-7)
```

## 7. Hand-off a la mesa (lo que NO es de este WO)

1. **F-7** (builder CB-03): aplicar `isBoardToggleable` de CB-02-DISENO §9 + tercera rama de
   affordance + tests — **antes de publicar el censo CB-01 o montar CB-02 en deploy** (misma clase
   de costura que F-4 fue en ronda 1: invisible hoy, rompe la semántica del operador al aterrizar
   el upstream).
2. **F-8** (CB-01-CROSS-EXAM + builder CB-03): enderezar "SOLO 'shadow' exacto" → "solo el valor
   'shadow' (ignora caja, tras trim)" en canon y fila.
3. **F-5** (builder CB-03): 4 strings "audit_logs" → "audit_log" (Client:14,55,94,337) — el ledger
   real es singular (control-board.ts:845).
4. **OBS-G**: el agente que aplicó la reconciliación 17:47/23:27 debe aterrizar su reporte si aún
   no lo hizo.
5. **F-2/F-3** (opcionales): anillo oscuro en dots tema claro; badge A a contraste AA.
6. **CB-06**: browser-verify post-deploy; incluir entrada por URL directa (advisory: `/control` NO
   está en nav-items.ts — solo `/killswitch` nav-items.ts:72 en grupo "control"; el board es
   alcanzable hoy únicamente por URL).

---

*Lexicon: TLS · Holonomic Loop Resolution · Topological Yield · Decoherencia de Estado · Variedad de
Liquidez. Clasificación de claims: comportamiento post-reconciliación = PRIMARY_SOURCE (corridas
propias 23:26-23:33); F-7/F-8 = PRIMARY_SOURCE (lectura de código/censo propia); fail :645 =
PRIMARY_SOURCE (mtime + corridas) con confirmación de 7 reportes pares (CANONICAL_WORKBOOK de la
mesa).*
