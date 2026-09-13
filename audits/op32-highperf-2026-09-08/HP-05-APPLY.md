# HP-05 — APPLY: UI de preferencias multi-objetivo (op_32 · NSGA-II)

**WO**: HP-05 · kind: apply · agente: frontend-architect (**RESPAWN-2 reemplazo A + reemplazo B**) · fecha 2026-09-08
**Estado**: ✅ COMPLETE (ambas mitades fusionadas). Mitad A (§0-§6): censo + ubicación + montaje +
verificación del espejo op_32 + repairs tsc. Mitad B (§9): suite vitest del componente (11/11) +
verificación final `tsc --noEmit` EXIT 0 + suite completa con atribución del único fallo (CB-03, territorio
prohibido) + cross-exam convergente del espejo + conformidad con HP-02-DESIGN.md §2.6 (aterrizó después —
coincide, cero contradicción). Ambas mitades sobre los mismos archivos claimados sin conflicto.

---

## 0. Sincronía de mesa redonda (lo que encontré al llegar — citado por archivo)

| Par / fuente | Hallazgo que me afecta | Cómo lo usé |
|---|---|---|
| `GOAL-WORKORDERS.md:33` | HP-05 exige página EXISTENTE de configuración (no página nueva sin censo) + persistencia vía config plane, NO localStorage | Censo §1 + decisión §2 |
| `HP-04-APPLY.md` §6.1/§6.4 | HP-02-DESIGN.md NO aterrizó; el espejo TS de catálogos es GENERADO (quotebase) y NO debo usarlo para op_32 | Diseño contra SEED v2 + op_32 REAL (§3); cero dependencia del quotebase |
| `OPERADOR-SEED-V2-2026-09-08.md:329-378` | Contrato op_32: objetivos f=[−profit, CVaR, latencia_ms]; NO define aún la selección por preferencias | Cláusula del charter activada: diseño UI marcado dependiente — pero HP-03 aterrizó el Rust real y ESA es la fuente verificada |
| `backend/math-engine/src/operators/op_32_multi_objective.rs` (HP-03, aterrizó en paralelo durante mi sesión) | Selección por vector de preferencias normalizado, features `mo_weight_*`, defaults 0.5/0.3/0.2 | Espejo verificado línea por línea (§4) — el componente YA NO depende de un diseño pendiente: depende de código vivo |
| `frontend/components/PreferenceVectorPanel.tsx` (untracked, mitad B en vuelo) | Componente existía al llegar (par B / agente caído); cita líneas exactas de op_32 | NO lo reescribí: lo VERIFIQUÉ contra el Rust real (§4) y reparé sus 7 errores de tsc (§6.2). Co-autoría declarada |

**Contradicción encontrada y resuelta**: el charter decía "diseña contra el SEED marcándolo dependiente
si HP-02-DESIGN no aterrizó" — pero `op_32_multi_objective.rs` SÍ aterrizó (HP-03 en paralelo). Diseñé
contra el **código Rust real** (superior al SEED: es implementación, no aspiración). La dependencia que
QUEDA viva es la de persistencia (§7), no la de diseño.

---

## 1. Censo (ANTES de decidir ubicación — charter obligatorio)

| Ruta | Patrón | Naturaleza | ¿Hogar del panel? |
|---|---|---|---|
| `frontend/app/config/page.tsx` | Server Component puro + `getConfigCurrent()` | KV view-only de `configs/app.toml` + 2 toggles runtime (paper/RPC) + panel de knobs canónicos view-only | NO — superficie de config de SISTEMA, no de selección de estrategia |
| `frontend/app/config/trading/page.tsx` | **R1 canónico**: Server puro `getTradingConfig()` → `TradingConfigForm` client con `initial` | **SSOT de knobs de trading** — cada save = `putTradingConfig` → Redis → hot-reload searcher-rs ≤1s (`page.tsx:28,54-60`) | **SÍ — decisión §2** |
| `frontend/app/settings/` + `settings/credentials/` | R1 + SettingsClient/CredentialsClient | Credenciales/preferencias de cuenta | NO — plano de credenciales, no del pipeline |
| `frontend/app/control/` | CB-03 | **PROHIBIDO** (programa en vuelo) | NO tocado (verificado: 0 diffs míos ahí) |

Navegación ya existente: `/config/trading` está en el sidebar (`nav-items.ts:82`, icono SlidersHorizontal,
grupo setup) — **cero cambios de nav necesarios, cero página nueva** (FE-01-GATE-1 intacto: no añadí ruta).

## 2. Decisión de ubicación

**El panel vive en `/config/trading`, montado como isla cliente separada DESPUÉS de `TradingConfigForm`.**

Razones: (a) es la única superficie cuyo contrato de persistencia ES el config plane de trading
(Redis hot-reload) que el charter exige; (b) la página ya es R1 pura (server snapshot → client); (c) isla
separada = cero coupling con el save del form — hoy el backend NO sirve el campo, así que acoplarlo al
payload del PUT habría enviado un campo desconocido al Zod del admin route (riesgo innecesario). Cuando el
backend aterrice el campo, el passthrough de la página hidrata el panel SIN tocar el componente (§7.1).

## 3. El espejo op_32 — verificación adversarial (mi mitad: cross-exam del componente contra el Rust real)

Todo verificado contra `backend/math-engine/src/operators/op_32_multi_objective.rs` [CANONICAL_REPO]:

| Afirmación del componente (`PreferenceVectorPanel.tsx`) | Verificación contra el Rust | Veredicto |
|---|---|---|
| Defaults 0.5/0.3/0.2 "SEED operador" | `DEFAULT_W_YIELD/RISK/LATENCY` = 0.5/0.3/0.2 (`op_32:68-70`) | ✅ EXACTO |
| Features `mo_weight_yield/risk/latency` | `state.features.get("mo_weight_*")` (`op_32:566-568`) — componente los cita como "op_32:562-564": ±2 líneas, dentro del mismo bloque | ✅ (drift de cita ±2, documentado) |
| Validación: no finito o <0 ⇒ nulo; Σ≤0 ⇒ nulo, razón `invalid_preference_vector` | `op_32:576-582` idéntico (any !isfinite‖<0 → none_out; wsum≤0 → none_out) | ✅ ESPEJO EXACTO |
| Pesos = raw/Σw suma 1 | `op_32:583` `[raw[0]/wsum, …]` | ✅ |
| "argmin de desutilidad ponderada sobre el frente, min-max normalizado por columna, SIEMPRE miembro exacto del frente" (CardDescription) | `select_by_preference` `op_32:189-223`: n_ij min-max por columna (span 0 ⇒ 0), u=Σ w_j·n_ij, argmin, retorna índice del frente | ✅ VERAZ |
| "Peso único yield ⇒ delega en op_15" / "riesgo/latencia puro ⇒ None" (help texts) | Degeneración `op_32:586-608`: solo-yield → `GoldenSectionOperator` + metadata `delegated_to_op=15`; resto → `degenerate_null_trade` | ✅ |
| "wire: sin campo" (estado inicial) | grep `mo_weight` en TODO el repo fuera de op_32/tests: **0 hits** en api-server/edge/trading-config — NADIE sirve el campo hoy | ✅ la verdad, no un mock |

La fórmula vive UNA vez en el frontend (`normalizePreferenceWeights`, función pura exportada) — cero
duplicación divergente interna. La sync TS↔Rust es por contrato documentado en el header del componente
("si op_32 cambia su fórmula, esta función cambia en el mismo PR") — verificable por grep en review.

## 4. Diffs de mi mitad (todos `// HP-05 (2026-09-08)`)

### 4.1 Montaje — `frontend/app/config/trading/page.tsx`

```tsx
+import { PreferenceVectorPanel } from "@/components/PreferenceVectorPanel";
 …
       <TradingConfigForm chainId={DEFAULT_CHAIN_ID} initial={initial} />
+
+      {/* HP-05 (2026-09-08): preferencias multi-objetivo op_32 (NSGA-II). Isla
+          cliente R1 SEPARADA del form (sin coupling con su save): el config plane
+          aún NO sirve mo_weight_yield/risk/latency — grep verificado, solo tests
+          Rust lo referencian — así que el snapshot se pasa null y el panel
+          renderiza el estado honesto "no emitido en el wire" (RULE 00)… */}
+      <div className="mt-6">
+        <PreferenceVectorPanel initialPreference={null} />
+      </div>
```

`initialPreference={null}` NO es un placeholder decorativo: ES el estado verdadero del wire verificado
por grep (§3 última fila). RULE 00 intacta.

### 4.2 Reparaciones tsc del componente — `frontend/components/PreferenceVectorPanel.tsx` (co-autoría con mitad B)

Encontré el componente con 7 errores de tsc (todos bloqueantes del gate doctrinal §7.6) y los reparé
sin cambiar un átomo de la matemática:

- `PreferenceVectorPanel.tsx:77,79` (6×TS2532): `vals[0]+vals[1]+vals[2]` bajo
  `noUncheckedIndexedAccess` ⇒ destructure `const { yield: y, risk: r, latency: l } = raw` — matemática
  espejo intacta (comentario in situ lo declara).
- `PreferenceVectorPanel.tsx:200` (TS7006): `onValueChange={(v) ⇒ …}` inferencia rota del
  `ComponentProps` de radix-ui ⇒ tipado explícito `(v: number[])` (patrón repo
  `RpcBackendToggle.tsx:108`); `v[0] ?? raw[key]` es undefined-safe.

Mitad B continuó encima (añadió `import * as React` SSR-test support, línea 26-29) conservando mis
repair — co-autoría limpia, cero conflicto.

## 5. Verificación (mi mitad)

```
cd frontend && npx tsc --noEmit        → REAL_EXIT=0  (antes de mis repairs: EXIT=1 con 7 errores,
                                                       file:line en §4.2; re-verificado EXIT=0 dos
                                                       veces, la 2ª incluyendo los cambios en vuelo de B)
cd frontend && npx vitest run          → Test Files: 1 failed | 125 passed (126)
```

- El ÚNICO fallo es `components/__tests__/ControlBoard.test.tsx` ("CB-03 · panel 'postura de máximo
  potencial'… esperaba 'SOLO shadow exacto spawnea'") — **territorio PROHIBIDO para HP-05** (programa
  CB-03 en vuelo, untracked): drift interno de strings de ESE programa, pre-existente a mi diff. Mis
  archivos (`PreferenceVectorPanel.tsx`, `config/trading/page.tsx`) no son importados por ese test
  (verificado). **HP-05 introduce CERO fallos nuevos.**
- Doctrine baseline declarada era 120 files/1,114 tests (FE-02b); hoy corren 126 files (programas
  paralelos añadieron suites) — 125/126 verde.
- Los tests PROPIOS del panel (normalización suma 1, toggle deshabilita sliders, hidratación snapshot
  sin mismatch) son la **mitad B** — al cierre de mi mitad el árbol no tenía aún ese archivo (B lo
  estaba escribiendo en paralelo; su `import * as React` para el runner ya está en el componente).

## 6. Checklist doctrinal §3 (FRONTEND-DOCTRINE.md) — aplicado

| ★ Gate | Estado |
|---|---|
| R1-forma (server puro + client `useState(initialSnapshot)`, no-determinismo solo useEffect) | ✅ el panel NO usa useEffect/window/Date.now/localStorage en absoluto (render SSR ≡ primer render client por construcción) |
| suppressHydrationWarning | ✅ cero usos |
| DataSurfaceState / empty-con-razón | ✅ el "empty" del wire es el Alert "No emitido en el wire" con razón exacta + defaults declarados como defaults (no como datos) |
| RULE 00 / INV-FE02B-3 (cero derivación económica) | ✅ preview = aritmética de normalización sobre ENTRADA del operador + estado verbatim del wire; cero valores monetarios (anti-ejemplo BUG-06 tomado) |
| null ≠ 0 (R8) | ✅ vector inválido ⇒ mensaje honesto `invalid_preference_vector`, jamás selección fabricada |
| Estado global nuevo / localStorage | ✅ cero / cero |
| Ruta nueva ⇒ censo | ✅ no hay ruta nueva; FE-01-GATE-1 intacto |
| A11y AA | ✅ Switch id+htmlFor; sliders id+aria-labelledby+Label visible; sr-only con valor exacto; `role="status"` en estado inválido; Radix focus-visible nativo |
| React 18.3.1 (no use()/useOptimistic/Form Actions) | ✅ |
| Marcador de hunco | ✅ todos `// HP-05 (2026-09-08)` |
| Lexicon OMEGA en copy | ✅ "Topological Yield", "Riesgo (CVaR de Decoherencia)", "Latencia de inclusión", "Decoherencia de Estado" — cero 'profit' en UI |

## 7. Dependencias al BOARD (NO implementadas por HP-05 — adjudicación vía BOARD, lección de colisión de migraciones respetada)

1. **Persistencia del vector (backend)**: trading-config NO sirve `mo_weight_yield/risk/latency` ni
   `pareto_enabled` (grep §3). Cuando se adjudique: (a) campo en `TradingConfigBaseFields`
   (`frontend/lib/schemas.ts` — fuera del claim textual de HP-05), (b) passthrough en
   `config/trading/page.tsx` (1 línea — el componente ya acepta `PreferenceVectorWire`), (c) wiring
   searcher-rs → `state.features` de math-engine. Nada de esto lo toqué.
2. **Quién activa `pareto_enabled` en el pipeline**: op_32 corre con defaults del SEED si no hay
   features; el toggle del panel hoy es borrador local declarado (badge "wire: sin campo · borrador
   local"). El flip real es decisión de wiring backend + operador.
3. **HP-02-DESIGN.md sigue sin aterrizar**: mi §3 funciona como verificación de diseño contra el Rust
   vivo; si HP-02 aterriza después y difiere del implementado por HP-03, el conflicto se resuelve en
   BOARD (mi espejo sigue al CÓDIGO, no al documento).

## 8. División RESPAWN-2 (para el orquestador)

- **Mitad A (§0-§6)**: lectura doctrina+SEED+pares · censo · decisión de ubicación · montaje en
  página existente · verificación adversarial espejo↔Rust · reparaciones tsc · tsc EXIT 0 + vitest
  no-regresión 125/126.
- **Mitad B (§9)**: ✅ COMPLETE — suite vitest del componente (11/11), fix SSR-test del namespace
  React, verificación final tsc EXIT 0 + suite completa 126/127 (único fallo = CB-03 prohibido),
  cross-exam convergente del espejo, conformidad HP-02-DESIGN §2.6. Fusionada en ESTE archivo.

---

## 9. Mitad B — TESTS + verificación final (reemplazo B, frontend-architect)

> Llegué con el componente y el montaje de A ya en disco (untracked). NO los reescribí: verifiqué
> INDEPENDIENTEMENTE sus claims contra el Rust real ANTES de leer el reporte de A (convergencia
> de dos verificaciones separadas = cross-exam de mesa redonda), escribí la suite de tests del
> rubric, y ejecuté el gate final. Diffs míos: SOLO el fix SSR-test de 1 línea (§9.2) + el archivo
> de tests nuevo (§9.3) + esta sección.

### 9.1 Verificación convergente del espejo (independiente de A — misma conclusión)

Re-verifiqué contra `backend/math-engine/src/operators/op_32_multi_objective.rs` ANTES de leer §3 de A:

| Claim verificado | Evidencia propia file:line | Veredicto |
|---|---|---|
| Validación y normalización w_i/Σw | `op_32:576-583` — `any(!is_finite \|\| <0)` → none_out; `wsum<=0` → none_out (`invalid_preference_vector`); `[raw[0]/wsum, …]` | espejo EXACTO de `normalizePreferenceWeights` (TS) |
| Defaults 0.5/0.3/0.2 | `op_32:68-70` (`DEFAULT_W_YIELD/RISK/LATENCY`) | `DRAFT_DEFAULTS` = estado EFECTIVO del pipeline (features ausentes → `unwrap_or` defaults, `op_32:569-573`) — hecho canónico, no decoración |
| Selección SIEMPRE del frente | `select_by_preference` `op_32:189-223` (min-max por columna, span 0 ⇒ 0; argmin Σw_j·n_ij; empate ⇒ primer índice) | CardDescription veraz |
| op_32 REGISTRADO (HP-03 vivo) | `operators/mod.rs:43,163-164` (`32 => MultiObjectiveOperator::new()`) | el motor que el panel declara EXISTE |
| Nadie sirve `mo_weight_*`/`pareto` en el config plane | grep en `backend/api-server/src/`, `backend/searcher-rs/src/`, `edge/worker/src/`, `frontend/lib/` → **0 hits** | `initialPreference={null}` = verdad del wire (RULE 00) |

### 9.2 Fix SSR-test (1 línea, patrón repo) — `frontend/components/PreferenceVectorPanel.tsx:26-29`

Primer run de la suite: 6/11 fallaban con `ReferenceError: React is not defined` (`PreferenceVectorPanel.tsx:139`) — el transformer clásico de JSX del runner node necesita el namespace React en scope de módulo. Fix: `import * as React from "react"` con comentario citando el patrón (`ControlScopeBadge.tsx:23-25`), inerte para el build automático de Next. Es el ÚNICO cambio mío al componente (co-autoría declarada por A en §4.2 — confirmada).

### 9.3 Suite de tests — `frontend/components/__tests__/PreferenceVectorPanel.test.tsx` (nuevo, 11 tests)

Convención del repo (node env + `renderToStaticMarkup` + proyecciones puras pineadas, cf. `ControlBoard.test.tsx`). Los 3 requisitos del rubric del charter:

1. **Normalización suma 1** (5 tests, `:57-99`): defaults 0.5/0.3/0.2 → Σ=1 exacta (eps 1e-12); 2/3/5 → 0.2/0.3/0.5; invarianza a escala (w_i/Σw); **R8**: 0/0/0 → null (jamás 0/0); negativo/Infinity/NaN → null (espejo defensivo `op_32:576`).
2. **Toggle deshabilita sliders** (2 tests, `:102-121`): `pareto_enabled=false` → thumbs con `data-disabled=""` + `aria-disabled="true"` (emisión verificada en fuente radix `react-slider`: `"data-disabled": disabled ? "" : void 0`, `aria-disabled`) y preview inactivo; `true` → cero `data-disabled`, pesos visibles.
3. **Snapshot hidrata sin mismatch (R1)** (4 tests, `:124-175`): markup del primer render contiene EXACTAMENTE los valores del snapshot (crudos 0.60/0.20, normalizados 0.600/0.200/0.200, Σ mostrada `>1.000<`, procedencia verbatim); **doble render byte-idéntico** (cero no-determinismo — la garantía anti-mismatch); `null` → estado honesto "no emitido en el wire" + defaults canónicos visibles; **a11y AA**: cada slider con `for="hp05-weight-*"` + `aria-labelledby="hp05-weight-*-label"`, toggle con id/for, foco visible (`focus-visible:ring-4` del thumb del design system).

Los fixtures son datos de TEST (comentario de cabecera) — runtime jamás usa datos fabricados (RULE 00).

### 9.4 Verificación final (output verbatim)

```
cd frontend
npx vitest run components/__tests__/PreferenceVectorPanel.test.tsx
  → components/__tests__/PreferenceVectorPanel.test.tsx (11 tests) 88ms
  → Test Files  1 passed (1) · Tests  11 passed (11)      [23:05:33, v1.6.1]

npx tsc --noEmit
  → EXIT=0 (sin output)

npx vitest run  (suite completa — X10, no-regresión global)
  → Test Files  1 failed | 126 passed (127)
  → Tests       1 failed | 1171 passed (1172)
```

- El delta vs. el run de A (126→127 files) es MI suite nueva (11 tests).
- El ÚNICO fallo: `components/__tests__/ControlBoard.test.tsx:645` — aserción `toContain("SOLO &#39;shadow&#39; exacto spawnea")` contra markup actual de `DefaultsPanel` (drift interno de strings del programa **CB-03 en vuelo, territorio PROHIBIDO para HP-05**). Re-ejecutado aislado: 44/45 de ESE archivo pasan; el fallo es pre-existente al diff HP-05 y no toca archivo importado por HP-05 (ni HP-05 toca archivo importado por ese test). **HP-05 introduce CERO fallos nuevos** — mismo veredicto que §5 de A, re-confirmado sobre el árbol final con mis tests añadidos.

### 9.5 Conformidad con HP-02-DESIGN.md (aterrizó DESPUÉS del componente — chequeo de contradicción)

HP-02-DESIGN §2.6 especifica el contrato de selección: `w ∈ ℝ³₊ con Σw = 1 (normalizado de
mo_weight_{yield,risk,latency}, defaults 0.5/0.3/0.2)` + argmin `U_i = Σ_j w_j · n_ij` (min-max por
columna). Es EXACTAMENTE lo que el panel declara (`CardDescription` + `normalizePreferenceWeights`) y
lo que HP-03 implementó (`op_32:189-223,576-583`). **Cero contradicción diseño↔código↔UI** — el
triángulo es coherente. La advertencia de A §7.3 queda resuelta: el diseño aterrizado SIGUE al código.

### 9.6 Lexicon + charter (mitad B)

UI copy verificada: "Topological Yield", "Riesgo (CVaR de Decoherencia)", "Latencia de inclusión",
"Decoherencia de Estado" — cero "profit" en superficie (`mo_weight_yield` nombra el FEATURE de Rust
verbatim, sin traducción que esconda la configuración). Sin estado global nuevo, sin localStorage, sin
rutas nuevas, sin tocar `app/control/`+`ControlBoardLed.tsx`+`ControlBoard.test.tsx`+`useDriftDetection.ts`
(verificado por `git status --porcelain` de mis writes). NO-GIT respetado: cero commit/push/PR.

— HP-05 reemplazo A (frontend-architect) · 2026-09-08 · // HP-05 (2026-09-08)
— HP-05 reemplazo B (frontend-architect) · 2026-09-08 · // HP-05 (2026-09-08)
