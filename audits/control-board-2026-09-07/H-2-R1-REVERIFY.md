# H-2-R1 RE-VERIFY — re-verificación adversarial del fix WO-H2-R1 (Gang Omniscience ronda 2)

- **WO**: re-verificación del fix de `H-2-R1-FIX.md` (gap R1 de `H-2-REVERIFY-ronda2.md` §3
  + gap G1 de `H-3-REVERIFY.md` §4) · **Agente**: REVERIFY H-2-R1
- **Fecha**: 2026-09-08 23:0x–23:1x local · **Read-only total**: 0 git-writes (HEAD intacto
  `27aca289`, working tree only), 0 VPS, **0 requests a dominio público** (presupuesto 5/5
  intacto — no re-sondeé el estado desplegado; cito la evidencia primaria del H-3-REVERIFY §3),
  0 executor/wallets/broadcast (§32/§33/§34.3). No muté el árbol compartido para A/B (page.tsx
  tiene 5 WOs con co-edición viva documentada — §3 surgical; el poder discriminante de los tests
  se estableció estáticamente, ver §2 fila "pinneo").

## 0. Sincronía de mesa redonda (leída ANTES de verificar)

Leídos completos: `GOAL-WORKORDERS.md`, `H-2-R1-FIX.md` (objeto), `H-2-REVERIFY-ronda2.md`
(fuente R1), `H-3-REVERIFY.md` (fuente G1), `R4-STATCARD-FIX.md` y `R3-FIX-token-safety-no-computado.md`
(pares más recientes que co-editan page.tsx/StatCard.tsx — interacción con mi WO). Código leído
íntegro: `frontend/app/page.tsx`, `frontend/components/StatCard.tsx`,
`app/__tests__/page.subtext-causes.test.tsx`, `app/__tests__/page.statcard-h4.test.tsx`.
Nada refutado; una mejora del árbol desde la corrida del fixer (tsc ahora LIMPIO, §3) y dos
precisiones forenses (§1) que FORTALECEN el fix.

## 1. Veredicto: **fix CONFIRMADO — R1 CERRADO + G1 CERRADO, sin regresiones**

| Claim del H-2-R1-FIX | Mi evidencia (lectura + corrida propia) | Resultado |
|---|---|---|
| Split 3 causas, rama `failed` PRIMERA, en los 3 statcards | `page.tsx:244-252` (hero), `:293-301` (Decoherencia), `:164-166` (detectedStat early-return). Orden del ternario verificado: `failed` domina antes de mirar counts | **CONFIRMADO** |
| `detectedStat` param 4º `failed = false` → `{value:"—", subtext:"sin datos — feed no disponible"}` | `page.tsx:150-166`; call site `:189` pasa `failed`; `failed` = `source === "server-fetch-failed"` (`:181`) | **CONFIRMADO** |
| Default `false` preserva las llamadas pineadas de H-4 byte a byte | `page.statcard-h4.test.tsx:74,81,88,95,99` — las 5 llamadas son a 3 args, 0 marcadores WO-H2-R1 en ese archivo (no editado); 7/7 PASS en mi corrida | **CONFIRMADO** |
| Guard animate para `number \| string` | `page.tsx:269` = `typeof detected.value === "number" && detected.value > 0`; `StatCard.tsx:12` props YA eran `value: string \| number` y el effect ya guardaba `typeof value !== "number"` (`:37`) — el ensanchamiento tipa y ejecuta limpio | **CONFIRMADO** |
| Coherencia intra-página con la sección del feed | Banner pre-existente **en HEAD:191 y main:191** (`git show`, grep -i) — la contradicción que R1 señalaba era CANÓNICA y desplegada; ahora los 3 cards declaran la misma causa que el banner (`page.tsx:248,297,165` ↔ `:335`) | **CONFIRMADO (+precisión: el banner NO es del H-series, es anterior)** |
| Pre-fix 2-causas | `git show HEAD:frontend/app/page.tsx:129,157` — hero y Decoherencia hardcodeaban `"sin datos — feed vacío"` incondicional; H-4 no manejaba failure (0 bajo "ventana live · límite 50"). Cita del reverify confirmada línea a línea | **CONFIRMADO** |
| Semántica soundness de `failed`-primero | Ambos caminos de failure de `getHomeData` devuelven `opportunities: []` (`:50-56`, `:74-81`) → `failed ⇒ detectedCount=0`: no existe estado con failed=true y ventana poblada; el orden es correcto y la rama 2 (dominancia sobre window_total stalé) es defensiva pura (R8-compatible) | **CONFIRMADO** |
| 5 tests de regresión reales | `page.subtext-causes.test.tsx`: 2 puros + 3 SSR (failed / poblado-sin-nets / vacío real). Asertos negativos scoped por `segment()` (`:83-87`, 400 chars desde el label) — sin falso-positivo contra cards vecinos | **CONFIRMADO** |
| **Pinneo genuino (sin A/B por mutación)** | Test puro 1 llama `detectedStat(null,0,null,true)`: contra el helper pre-R1 (3 params) el 4º arg se ignora en runtime → devolvería `{value:0, subtext:"ventana live · límite 50"}` → `toEqual` FALLA. Tests SSR asertan strings ("sin datos — feed no disponible") que SOLO las ramas nuevas producen (grep: `page.tsx:165,248,297` únicos productores) y ausencias de los strings pre-fix. Los tests NO pueden pasar contra el estado pre-R1 | **CONFIRMADO (estático, sin tocar el árbol)** |

## 2. Corridas propias (no confío en claims de corrida ajena)

| Check | Resultado |
|---|---|
| `npx vitest run` — 6 suites del territorio (page 9 + hero-h3 4 + statcard-h4 7 + **subtext-causes 5** + StatCard 5 + XRayCard 7) | **37/37 PASS** |
| `npx vitest run` suite completa | **1171/1172 PASS, 1 fail** — `ControlBoard.test.tsx:645` casing "SOLO 'shadow' exacto spawnea" = CB-03 pre-existente (7º peer que lo re-confirma: H-2/H-3/H-4/ambos reverifies/R3/R4). Cero overlap con H-2-R1 (ese test importa ControlBoardLed/ControlBoardClient, sin page.tsx en el grafo) |
| `npx tsc --noEmit` | **EXIT 0 — ÁRBOL COMPLETO LIMPIO**. Nota para la mesa: el fixer vio 7 errores en `PreferenceVectorPanel.tsx` (HP-05 paralelo); su dueño YA los reparó — el gate typecheck global está VERDE ahora (mejor que el snapshot del fixer) |
| `npx eslint app/page.tsx app/__tests__/page.subtext-causes.test.tsx` | **EXIT 0** |
| NO-GIT | `git log -1` = `27aca289` intacto; working tree only; 0 commits |
| Marcadores `// WO-H2-R1 (2026-09-07)` | `page.tsx:154, :161, :187, :236, :286` + header del test (`:2`) | **PRESENTES** |

**Interacción con fixes pares verificada**: en estado failed, hero/decoherencia llevan
`animate=false` + `value="—"`, y el fix R4 (`StatCard.tsx:83` renderiza `displayValue` =
`animate ? 0 : value` pre-mount) hace que el SSR crudo también muestre "—" — la fila de 3
cards es honesta incluso en el HTML estático pre-mount (curl ≠ 0). Los cuatro fixes
(H-2/H-3/H-4/R4/H-2-R1) componen coherentes; 37/37 juntos lo demuestran.

## 3. Gaps RESTANTES (persisten — NINGUNO introducido por H-2-R1; todos conviven)

1. **R2 (agent-fixable, LOW-MED — el más urgente de los libres)**: gemelo exacto en
   `/opportunities/by-strategy`. Re-verificado por mí HOY:
   `frontend/features/opportunities/OpportunitiesByStrategyClient.tsx:54` (`totalProfit` suma
   nets con `?? 0`), `:56` (avg roi `(o.roi_pct ?? 0)` + `/ opps.length`, grupo vacío →
   "0.00%"), `:112` (`opp.net_expected_profit_usd ?? 0` por fila). Ausencia presentada como
   $0.00/0.00% computados (RULE 00/R8). Fix: reusar `computeAvgRoiPct` (ya exportado,
   `page.tsx:135`). Cita: H-2-REVERIFY §3-R2 — lo declaró primero, confirmo que sigue vivo.
2. **R5 / no-deploy (operator-gated)**: el fix vive 100% en working tree (NO-GIT correcto);
   el dominio público sigue mostrando los strings viejos (evidencia primaria del par:
   H-3-REVERIFY §3, RSC payload con `variant:"success"` + `"sin datos — feed vacío"` desplegados).
   Además PG crash-loopea por disco (H-4 §5) — acción VPS operator-gated. Post-deploy,
   CB-06/BROWSE debe ver los 3 cards "—" + "sin datos — feed no disponible" en outage, y
   "nets/roi no computados en el feed" con feed sano 100%-rejected.
3. **G2 (operator-gated, semántica)**: `bestNet` sigue siendo max sobre TODAS las filas
   (viables + rejected) — decisión de operador declarada por H-3 §6.1; no es defecto del fix.
4. **CB-03 (agent-fixable, territorio vivo de otro WO)**: `ControlBoard.test.tsx` 1 fail —
   dueño CB-03; ya documentado por 6 pares antes que yo.

NO-gaps (no perseguir): pluralización "1 viables" (cosmético, pineado por test de H-3);
SSR "0" pre-mount de statcards `animate=true` (primer frame del count-up, por diseño — R4
honró exactamente esa frontera).

## 4. Confirmaciones a pares

- **CONFIRMO H-2-R1-FIX completo** (§2 tabla): todos sus claims reproducen; su expectativa
  post-deploy (§7.1) queda como la checklist de CB-06.
- **CONFIRMO H-2-REVERIFY-ronda2 §3-R1 y H-3-REVERIFY §4-G1 como CERRADOS** por este fix —
  el split de 3 causas con failed-first mata ambas clases (causa falsa en failed + causa falsa
  en poblado-sin-nets).
- **CONFIRMO R4-STATCARD-FIX §4.1** (R1/R3 quedaban libres al despacharse R4 solo): R1 ya está
  cerrado por H-2-R1 (este verify); R3 cerrado por R3-FIX (verificado de paso: `page.tsx:125-126`
  `safetyA: null` + XRayCard 7/7). **R2 es el único R-agent-fixable que queda abierto.**
- **CORROBORO la deriva de suite 1160→1172 tests** con el mismo y único fail CB-03: trabajo
  paralelo sano, sin nuevas fallas.

## 5. Clasificación de claims

- Defecto pre-fix (2-causas + banner contradictorio): **CANONICAL_REPO** (HEAD:129,157,191 y
  main:191 vía `git show`).
- Comportamiento post-fix: **PRIMARY_SOURCE** local (corridas propias: 37/37, 1171/1172, tsc 0,
  eslint 0 — §2).
- Estado desplegado viejo: **PRIMARY_SOURCE ajena** (H-3-REVERIFY §3) — no re-sondeado,
  presupuesto intacto.
- R2 vivo hoy: **PRIMARY_SOURCE** (grep propio §3.1, esta sesión).
