# WO-G-6 — BUG-06: veredicto vivo + fix P0-2 (ticker fail-honest)

> FIXER Gang Omniscience ronda 1 · 2026-09-07/08 · Diffs marcados `// WO-G-6 (2026-09-07)`.
> Reglas respetadas: RULE 00/R8 · §32/§33 read-only (VPS solo lectura, 0 requests al dominio
> público) · NO-GIT (0 commit/push/PR/deploy — edición local + verificación SOLAMENTE).

## §0 Veredicto en una línea

**La fila "+0.00%" observada por el browse-auditor era el pseudo-Topological-Yield FABRICADO
por el cliente (BUG-06), NO un cero computado legítimo** — el wire vivo lleva `roi_pct: null`
en 10,001/10,001 filas (cero con `0.0`) y abundan filas con net diminuto positivo cuyo
`×0.1` renderiza exactamente "+0.00% ▲". Fix aplicado local (doctrina P0-2, unión de dos
pares), 12/12 tests + tsc EXIT 0.

## §1 Contexto de mesa (a quién construyo encima)

- **BROWSE-Auditor-de-honestidad-R8.md §2.7 y fila G-6**: observó en vivo
  `"2250fa…c02aaa_ · UniswapV2 → SushiSwap · +0.00%"` bajo corte y dejó el veredicto abierto
  (cotizada-R8-legítima vs pseudo-yield). Ese es el gap adjudicado a este WO.
- **FE-02a-DESIGN.md §2 BUG-06 + §3 FIX-7**: catalogó la fabricación (`profit * 0.1`,
  comentario "Rough scaling", ambos brazos del ternario idénticos) y el drop de filas sin
  precio que mentía el estado vacío; diseñó la cura `roi_pct ?? null` + keep-rows.
- **FE-02b-DESIGN.md FE-02b-01 + D-01**: convergencia independiente del otro par; diff más
  completo en render (3 sitios: sr-only `:140`, marquee `:149-153`, flecha oculta si null);
  invariante INV-FE02B-3 (prohibido derivar/escalar valores monetarios en el cliente).
- **FE-04-DESIGN.md convergencia #2 + FRONTEND-DOCTRINE.md P0-2**: la doctrina ordena aplicar
  UNA VEZ la unión (render de FE-02b + empty-state con razón de FE-02a). Este WO ejecuta esa
  unión — es el hotfix 0b de la ola 0, prerrequisito declarado de la migración ola-5 del
  ticker al store ("honestidad P0-2 ANTES de migrar").

## §2 Veredicto vivo (la discriminación que exigía G-6)

**Camino de la evidencia.** G-1 SIGUE VIVO (verificado hoy read-only: disco 144G/150G 100%,
`postgres` y `api-server` en crash-loop) → el veredicto por PG es imposible. Pivote a fuente
equivalente viva: el stream Redis `arbx:opps:detected` (redis UP 37h). El publisher serializa
el `opp` COMPLETO (`backend/searcher-rs/src/publisher.rs:151-155`, `serde_json::to_string(opp)`)
→ el stream es proxy fiel del `roi_pct` que el searcher persiste. Ventana retenida (MAXLEN
~10K): 10,001 entradas cubriendo **21:04:25→21:09:01 local 2026-09-08** — los ~5 minutos
inmediatamente ANTERIORES al journey del browse-auditor (21:11–21:16 local, archivos del dir
`snapshots/`), i.e. exactamente la población que alimentó el ticker observado.

**Hallazgos sobre la ventana (10,001 filas):**

| Hecho | Valor | Clasificación |
|---|---|---|
| `roi_pct: null` | **10,001/10,001 (100%)** | observación wire vivo |
| `roi_pct: 0.0` | **0 filas** | observación wire vivo |
| `net_expected_profit_usd: 0.000055` | 56 filas | observación wire vivo |
| `net_expected_profit_usd: 0.000039952…` | 46 filas | observación wire vivo |
| `expected_profit_usd: null` | 8,945 filas (dropeadas por el ticker viejo) | observación wire vivo |
| razones de rechazo top | v3_quote_unavailable 7,212 · spot_product_le_one 1,313 · non_positive_profit 1,087 | observación wire vivo |

**La matemática de la fabricación** (`OpportunityTicker.tsx:51` pre-fix):
`yieldPct = roi_pct ?? profit * 0.1` → con `roi_pct: null` y `profit = net = 0.000055`:
`0.000055 × 0.1 = 0.0000055` → `isPositive = true` → `"+0.00%" ▲`. Exactamente la fila
observada. La hipótesis alternativa ("computado y exactamente cero", R8-legítimo) requiere
`roi_pct = 0.0` en el wire: **cero filas** lo traen.

**Caveat [INFERRED]:** la fila literal `2250fa…c02aaa_` ya rotó fuera de la ventana (grep = 0;
el PG que la sirvió estaba crash-loopeando y sirvió datos previos) — el veredicto se adjudica
por mecanismo + composición del wire (100% roi_pct null + 102 filas/5min cuyo `×0.1` da
"+0.00%"), no por la fila exacta. La distinción no afecta al fix: con la cura, `roi_pct: 0.0`
del wire se mostraría "+0.00%" (cero COMPUTADO, legítimo) y `roi_pct: null` muestra "—" —
ambas verdades quedan renderizadas como lo que son.

**Hallazgo backend [CANONICAL_REPO, dependencia declarada — NO lo arreglo aquí (scope de otro
programa):]** `backend/searcher-rs/src/orchestrator.rs:1242,1269,1297,1332,1377` escriben
`opp.roi_pct = Some(0.0)` como PLACEHOLDER en ramas de rechazo (TokenNotAllowed,
StrategyDisabled, gate bloqueado, EvaluatedRejected, MacroMevGate) — un cero NO computado que,
de llegar al wire del ticker, blanquearía un placeholder como "computado-y-cero" (violación R8
aguas arriba). Hoy NO llega a este stream (0/10,001); cuando llegue, el FE fiel lo mostrará
"+0.00%" creyéndolo computado. La cura correcta es backend (`None` en esas ramas, o un flag
`roi_computed`). Queda registrado para la mesa — es el espejo exacto de G-6 del lado emisor.

## §3 El fix aplicado (unión P0-2, diff propio `// WO-G-6 (2026-09-07)`)

Archivo: `frontend/components/OpportunityTicker.tsx` (único consumidor: `app/layout.tsx:139`).

1. **`TickerItem.yield: number | null`** (`:11-14`) — FE-02b D-01: sin roi_pct del wire no
   hay %.
2. **`opportunityToTickerItem`** (`:40-66`): (a) eliminado el drop `profit === null → null`
   (FE-02a FIX-7: la fila sin precio entra al marquee con yield pendiente — el drop mentía el
   empty state y ocultaba el feed: 8,945/10,001 filas de la ventana viva eran invisibles);
   (b) eliminada la fabricación `profit * 0.1` (RULE 00): `yieldPct = opp.roi_pct ?? null`
   (`:58`, INV-FE02B-3); (c) firma estrechada a `Pick<OpportunityRow, …los 7 campos que
   lee…>` — documenta las dependencias reales y permite fixtures tipados sin casts.
3. **`formatTickerYield`** (`:69-73`, export): proyección pura — null ⇒ "—", nunca un %
   inventado (convención de tests del repo: proyecciones pineadas).
4. **Fetch sin filter** (`:88-92`): cero drops — el `.filter(item !== null)` quedó muerto al
   no existir ya retornos null.
5. **Render, 3 sitios** (FE-02b D-01): sr-only `:155` usa "yield not computed" para null;
   marquee `:162-176` — color pos/neg y flecha ▲/▼ SOLO para yield computado (`y != null`),
   "—" neutral para null, flecha no renderizada.
6. **Estado vacío** (`:128-133`): con keep-rows, la rama `items.length === 0` dispara SOLO
   con 0 filas reales en la ventana → "No topological convergence detected" es ahora
   literalmente cierto. **Decisión de composición documentada:** el copy-corrección de FE-02a
   FIX-7 ("N detecciones sin precio aún") corregía un escenario (filas dropeadas → empty
   mentiroso) que el keep-rows de FE-02b DISUELVE estructuralmente — adoptarlo verbatim sería
   contradictorio; su intención queda cubierta porque esas filas ya no desaparecen. La doctrina
   mandaba la unión de ambos y así se compone.

**Test de regresión** `frontend/components/__tests__/OpportunityTicker.test.tsx:56-117`
(8 nuevos, fixtures clavados al wire vivo del §2): pin del caso exacto
`roi_pct null ⇒ yield null` (antes: 0.0000055 → "+0.00%"); roi computado verbatim
(`0 → 0`, R8); no-drop + fallback de pair; gate P0-2 (3 filas: 1 con roi, 2 sin → 3 items,
2 con yield null); `formatTickerYield` (null→"—", 0→"+0.00%", signos).

## §4 Verificación

- `vitest run components/__tests__/OpportunityTicker.test.tsx` → **12/12 PASS** (4
  pre-existentes + 8 nuevos).
- `npm run typecheck` (tsc --noEmit, workspace frontend completo) → **EXIT 0**.
- Suite frontend COMPLETA → **1145/1146 PASS, 124 archivos**. El único fallo es
  `ControlBoard.test.tsx:645` ("SOLO &#39;shadow&#39;…"), archivo UNTRACKED del programa CB-03
  en vuelo por otro WO — cero ruta de import hacia el ticker (grep OpportunityTicker en
  `app/control/` + `ControlBoardLed.tsx` = 0) y mi diff no lo toca. No es mío; reportado para
  el dueño del programa CB, no pisado.
- Blast radius: `grep -rln 'TickerItem|opportunityToTickerItem|OpportunityTicker'` = layout +
  componente + test. Nada más consume el contrato.

## §5 Claims de archivo y conflictos

- **WO-H4 (2026-09-07)**, detectado en el árbol compartido: `frontend/lib/schemas.ts:116-117`
  (+`window_total` opcional) y `backend/api-server/src/routes/opportunities-live.ts`
  (+COUNT OVER ()). **Aditivo y ortogonal** a G-6 (no tocan `roi_pct` ni el ticker) —
  componen sin conflicto; no pisado.
- **Doctrina ola-5** (ticker al store, 5ª vía de fetch eliminada): mi P0-2 es su
  prerrequisito declarado ("honestidad P0-2 ANTES de migrar") — quedan ambas pendientes; este
  fix no resuelve la 5ª vía (scope ola-5).
- **G-5 (BUG-08, chip IDLE→CONNECTING)**: el OTRO agent-fixable de la tabla del browse — NO
  tocado (FIX-8 de FE-02a ya diseñado; es de ese WO).

## §6 Log de interacción con el VPS (todo read-only)

`df -h /`, `docker ps` (salud), `redis-cli XLEN/XINFO/XRANGE` sobre
`arbx:opps:detected` (lectura). Único artefacto: agregación en `/tmp/wo-g6-window.txt`
(scratch de análisis, sin tocar servicios/config/datos) — **eliminado** (`rm -f`, verificado).
**0 requests HTTP al dominio público** (presupuesto 5 intacto). 0 writes en servicios. Sin
commit/push/PR/deploy (NO-GIT). Pendiente operador: desplegar el fix cuando el pipeline
GIT→VPS se reanude (el deployed seguirá fabricando "+0.00%" hasta entonces).
