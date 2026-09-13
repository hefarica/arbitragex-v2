# HP-05 — CROSS-EXAMINATION (par adversarial, Gang Omniscience 2026-09-08)

**Objeto**: refutar el entregable de HP-05-APPLY.md (UI de preferencias multi-objetivo op_32).
**Método**: verificación re-ejecuta, no hereda — leí el 100% de `PreferenceVectorPanel.tsx`
(272 líneas) + test (179) + diff real de `config/trading/page.tsx` + las secciones citadas del
Rust (`op_32_multi_objective.rs:60-120,180-240,545-680`) y de HP-02-DESIGN §2.6; re-ejecuté
`tsc --noEmit` (EXIT 0), la suite del panel (11/11 PASS), `ControlBoard.test.tsx` aislado
(45/45 PASS hoy) y la suite completa (número abajo, con timestamp). Re-verifiqué el grep del
wire por cuenta propia y el plano de knobs alternativo (runtime_knobs.rs). Leí el board
completo + HP-02/HP-03/HP-04/HP-08/HP-03-CROSSEXAM + los 3 reportes del programa
frontend-doctrine que tocaron el árbol en vivo (GAP-4-FIXER-R2, WO-G-6-GAP3, reverificación R2).

## 1. Veredicto: **GAPS** — trabajo sólido; la cláusula de persistencia del charter quedó sin entregar (declarada, no oculta)

## 2. Lo que RESISTIÓ la refutación (verificado por mí, no heredado)

- **Archivos tocados = exactamente el claim** (git status): `PreferenceVectorPanel.tsx`
  (nuevo), `__tests__/PreferenceVectorPanel.test.tsx` (nuevo), `config/trading/page.tsx`
  (M, diff quirúrgico +13 líneas con marker). `frontend/lib/schemas.ts` aparece M pero es
  `WO-H4 (2026-09-07)` — programa ajeno anterior; el claim "no lo toqué" es VERAZ.
- **Espejo TS↔Rust EXACTO** (verificación independiente de la de A y B — tercera
  convergencia): normalización `panel:82-87` ≡ `op_32:576-583` (validación finito/≥0, Σ≤0 ⇒
  null, w_i/Σw); defaults `DRAFT_DEFAULTS` ≡ `op_32:68-70`; descripción de selección ≡
  `select_by_preference` `op_32:189-223` (min-max por columna, span 0 ⇒ 0, argmin, empate ⇒
  primer índice); help-texts de degeneración ≡ `op_32:586-608` (solo-yield → op_15
  `delegated_to_op=15`; resto → `degenerate_null_trade`). El drift ±2 de citas de línea está
  declarado en el propio reporte.
- **"wire: sin campo" es verdad, no excusa**: mi grep propio — `mo_weight` existe SOLO en
  math-engine (op_32 + real_ops_tests) y docs de audits; 0 hits en api-server/searcher-rs/
  edge/frontend-lib. `initialPreference={null}` = estado real del wire (RULE 00).
- **No había vía de persistencia desestimable**: `runtime_knobs.rs` (CB-02) es un plano de
  toggles BOOLEANOS read-only (`arbx:controlboard:<id>` = "true"/"false"; INV-CB02-4 sin
  set() worker-side) — no puede cargar un vector de 3 floats. Trading-config no sirve el
  campo. La dependencia declarada §7.1 es genuina: persistir un campo que nadie lee habría
  sido persistencia DECORATIVA (peor bajo RULE 00 que el borrador honesto con badge).
- **Verificación real, no de humo**: `tsc --noEmit` EXIT 0 (re-ejecutado por mí, 2026-09-09
  ~00:0x); panel 11/11 PASS (re-ejecutado por mí). Los tests son proyecciones pineadas sobre
  markup real (renderToStaticMarkup), con discriminadores no-vacíos (flechas presentes en el
  caso computado). Triple corroboración previa: HP-08 §8.1, GAP-4-FIXER-R2 §1, WO-G-6-GAP3 §4.
- **R1 puro por construcción**: 0 useEffect/window/Date.now/localStorage/suppressHydrationWarning
  (leído el archivo completo); test de doble render byte-idéntico lo pinea.
- **A11y verificada por test que PASA**: `for`/`id`/`aria-labelledby` aterrizan en markup
  (Slider wrapper hace spread `{...props}` al Root radix, que propaga aria-labelledby a los
  thumbs role="slider"; Switch es button real con props spread; Label spread props). El
  claim "AA" no es aspiracional: está asertado.
- **No-regresión**: la atribución del único fallo (ControlBoard CB-03) a territorio ajeno
  estaba triple-corroborada; HOY `ControlBoard.test.tsx` aislado pasa **45/45** (el programa
  CB-03 cerró su drift de strings después de la ventana de HP-05) — confirma que nunca fue
  de HP-05.
- **Sincronía de mesa genuina**: cita GOAL-WORKORDERS:33, HP-04-APPLY §6 ítems 1/4 (el
  reporte los llama "§6.1/§6.4" — drift de formato, contenido exacto: quotebase GENERADO,
  no usarlo), SEED-V2:329-378, op_32 de HP-03, y el componente del agente caído con
  co-autoría declarada (no lo reescribió). §9.5 contra HP-02-DESIGN §2.6: cita textual
  verificada por mí — coherente. Nav claim verificado: `nav-items.ts:82` SlidersHorizontal
  grupo setup (falta el prefijo `components/` en la cita — trivial).
- **Lexicon + NO-GIT + markers**: verificado en fuente; 0 commits.

## 3. GAPS (impugnaciones que sobrevivieron)

### C-1 [MEDIO] — Persistencia del charter NO entregada (la cláusula explícita del board quedó en dependencia)
Charter HP-05: "persistencia vía el config plane existente (NO localStorage de producción
§contrato)". Entregado: borrador local que se pierde al recargar y NO puede afectar el
pipeline (badge honesto "wire: sin campo · borrador local"). Declarado en §7.1 — honesto,
pero el WO queda "apply" a medias: isla inerte hasta que aterrice backend. La cadena
completa exige: (a) `TradingConfigBaseFields` + Zod api-server + passthrough página (TS),
(b) wiring searcher-rs → `state.features` de math-engine (Rust, serial-group, P-∅ con ID de
anomalía). Sin (b), (a) solo persiste un campo ignorado — por eso la decisión de frenar fue
defendible; pero el charter pidió persistencia y NO está. **Clase**: agent-fixable (el tail
commit/PR/deploy = operator-gated por NO-GIT). Condición recomendada: cerrar ANTES
X-1..X-3 de HP-03-CROSSEXAM si el wiring va a hot-path (evitar certificar un motor con
bordes abiertos — mismo criterio que ese reporte aplicó al espejo de catálogos).

### C-2 [BAJO] — Reporte internamente stale + triángulo roto por pares posteriores
(a) §7.3 dice "HP-02-DESIGN.md sigue sin aterrizar" mientras §9.3/§9.5 del MISMO archivo lo
verifican aterrizado — media-reporte stale sin amend. (b) §9.5 declara "Cero contradicción
diseño↔código↔UI": era verdad a las 23:14, pero HP-03-CROSSEXAM (23:56) probó X-2 (frente
con filas ±∞; la premisa de finitud de HP-02 §2.6 "todas las filas finitas" no la enforce
el código). El ESPEJO del panel (normalización 576-583) NO es afectado por X-1..X-3, y la
CardDescription sigue literalmente vera (la selección SIEMPRE es miembro del frente — X-2
elegía un miembro engañoso, pero miembro). Debe citarse y amendments; cero cambio de código
UI requerido hoy. **Clase**: agent-fixable (doc).

### C-3 [BAJO] — Semántica de `pareto_enabled` indefinida backend-side
El toggle del panel se declara "campo futuro del trading-config", pero op_32 NO tiene
feature de enable/disable — el operador corre SIEMPRE que la matriz estrategia×operador lo
invoque (HP-04 añadió 182 edges USES_SECONDARY). ¿`pareto_enabled=false` significará
"estrategia no usa op_32 (edge off)" o "op_32 corre con defaults" o un feature-gate nuevo?
El wire-contract del panel (`PreferenceVectorWire.pareto_enabled`) compromete una semántica
que ningún backend definió — decisión de diseño pendiente ANTES del wiring de C-1.
**Clase**: pregunta al board (pregunta Q3 abajo); el wiring no debería aterrizar sin
resolverla.

### C-4 [INFO] — Gate "vitest 0 fails" no demostrable bajo árbol cargado; re-ejecutado por mí con timestamp
A la ventana de HP-05 el árbol tenía 1 fallo ajeno (CB-03); la tooling de perf-budget
(§3 ★ ola 4: perf-budgets.json + fe03-budget-check.mjs) NO EXISTE aún (verificado en
disco) — el gate build no era ejecutable. Mi suite completa final (abajo) también lleva 1
fallo, pero es el transitorio de carga documentado por WO-G-6-GAP3 §4
(ControlScopeBadge /settings §53, timeout-class bajo collect paralelo; aislado en árbol
quieto = **6/6 PASS**, 14.7s). Nombres de los números de HP-05 (126/127 con CB-03) quedan
stale: CB-03 cerró su drift y el fallo vigente es otro transitorio. El orquestador debe
capturar un run quieto 0-fails con timestamp antes del PR (lección GAP-4-FIXER-R2 §2).

## 4. Verificación propia (timestamp 2026-09-09 00:05-00:31 local)

| Check | Resultado |
|---|---|
| `npx tsc --noEmit` (frontend) | **EXIT 0** |
| `npx vitest run components/__tests__/PreferenceVectorPanel.test.tsx` | **11/11 PASS** (2.79s) |
| `npx vitest run components/__tests__/ControlBoard.test.tsx` (aislado) | **45/45 PASS** — drift CB-03 ya cerrado por su dueño (post-23:30) |
| `npx vitest run` (suite completa) | **1 failed \| 126 passed (127 files) · 1 failed \| 1174 passed (1175 tests)**, 445s — único fallo = `ControlScopeBadge.test.tsx > /settings §53 > every card LOCAL_PREFS…`, clase transitorio-de-carga (aislado quieto 6/6 PASS; 0 ruta de importe a HP-05 — importa solo ControlScopeBadge+SettingsClient, ninguno M) |
| `npx vitest run components/__tests__/ControlScopeBadge.test.tsx` (árbol quieto) | **6/6 PASS** (14.76s; bajo carga paralela dió 1/6 — reproducido el patrón de WO-G-6-GAP3 §4) |
| grep `mo_weight` repo-wide | solo math-engine + audits — config plane 0 |
| git status HP-05 files | 2 untracked + 1 M page.tsx; schemas.ts M = WO-H4 ajeno |

## 5. Reglas duras

- No muté código de producción, cero git/commit/push, VPS no tocado, cero broadcast,
  0 requests HTTP al dominio público. Escrito: SOLO este archivo.
- Cada impugnación cita file:line leído o salida de mi propia ejecución.

— CROSS-EXAMINER de HP-05 (par adversarial, Gang Omniscience), 2026-09-09.
// HP-05-CROSSEXAM (2026-09-08)
