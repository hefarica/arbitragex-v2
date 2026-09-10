# H-2 RE-VERIFY (ronda 2, adversarial) — veredicto: fix CONFIRMADO, quedan gaps residuales

- **WO**: re-verificación adversarial del H-2 FIX (`H-2-FIX.md`) · **Agente**: REVERIFY H-2
- **Fecha**: 2026-09-08 (sesión local) · Read-only total: 0 git (sin commit/push/PR/deploy),
  0 escrituras VPS, 0 requests a dominio público, 0 executor/wallets/broadcast (§32/§33/§34.3).
  Toda la verificación fue local: lectura de código + diff vs HEAD + vitest + tsc + eslint.

## 0. Sincronía de mesa redonda (leída ANTES de verificar)

Leídos: `GOAL-WORKORDERS.md`, `BROWSE-Auditor-R8-caza-de-deshonestidad-de-datos.md` (fuente),
`H-2-FIX.md` (objeto de re-verificación), `H-3-FIX.md`, `H-4-FIX-APPLY.md`,
`H-1-FIXER-next-action-blockers-vivos.md`. No contradigo ningún hallazgo previo; confirmo el del
H-2 con evidencia propia (diff contra HEAD) y aporto 4 gaps que PERSISTEN (§3).

## 1. Veredicto del fix original: CERRADO en su scope (la home) — verificación independiente

| Claim del H-2-FIX | Mi evidencia | Resultado |
|---|---|---|
| Pre-fix fabricaba "+0.00%" | `git show HEAD:frontend/app/page.tsx` — yield era `net != null ? \`${net >= 0 ? "+" : ""}${(opp.roi_pct ?? 0).toFixed(2)}%\`` (signo de net, magnitud roi coaccionada). Exactamente el root-cause del auditor R8 | **CONFIRMADO** |
| Fix yield null → "—" | `frontend/app/page.tsx:98-101` — `opp.roi_pct != null ? signo+toFixed : "—"`, signo derivado de `roi_pct` mismo. `Some(0.0)` → "+0.00%" honesto (test 4). `-0.001` → "-0.00%" (negativo real, correctamente firmado) | **CONFIRMADO** |
| `computeAvgRoiPct` sólo promedia computados | `page.tsx:128-133` — filtra nulls de numerador Y denominador; all-null/vacío → null. Pre-fix (HEAD): `reduce(acc + (o.roi_pct ?? 0))/opportunities.length` | **CONFIRMADO** |
| Subtext declara causa real | `page.tsx:261-267` — split roi-no-computado vs feed-vacío | **CONFIRMADO (2 de 3 causas — ver gap R1)** |
| `fees` | Ya era null-safe en HEAD (`roi_pct != null ? … : "—"`) — nunca fue parte del gap; el fix no lo tocó. Correcto | **CONFIRMADO** |
| Hunks marcados `// WO-H2 (2026-09-07)` | page.tsx:86-88, :94-97, :123-127, :254-257 | **CONFIRMADO** |
| 8/8 tests + sin regresiones | Re-ejecutados POR MÍ: `page.test.tsx` 8/8 + `page.hero-h3` 4/4 + `page.statcard-h4` 7/7 = **19/19 PASS**; `tsc --noEmit` **exit 0**; `eslint` page.tsx+test **exit 0** | **CONFIRMADO** |
| Calidad de los tests como guard | Los tests importan `toXRayProps`/`computeAvgRoiPct` (exportes que NO existían pre-fix → el suite falla en el import contra código viejo) Y las aserciones conductuales contradicen las expresiones pre-fix (test 1: net presente + roi null → "—"; HEAD producía "+0.00%"). Pinneo genuino | **CONFIRMADO** |
| NO-GIT | `git status --porcelain`: `M frontend/app/page.tsx`, `?? app/__tests__/page.test.tsx` — working tree, 0 commits | **CONFIRMADO** |
| Único fail de suite = ControlBoard (CB-03) | Re-ejecutado: `ControlBoard.test.tsx` sigue fallando por casing "SOLO 'shadow' exacto" vs contenido reconciliado CB-03; `ControlBoardLed.tsx` es archivo nuevo de CB-03 sin overlap de imports con page.tsx | **CONFIRMADO (no es de H-2)** |

**Clasificación**: root-cause pre-fix = CANONICAL_REPO (HEAD s1330ec33) · comportamiento post-fix
= verificado por ejecución propia (PRIMARY_SOURCE local) · patrón confidence = CANONICAL_REPO
(page.tsx:102-108).

## 2. ¿Regresiones? NINGUNA atribuible a H-2

- 19/19 tests de los tres WOs que comparten page.tsx pasan juntos (archivo fusionado sano).
- tsc/eslint limpios. Los `net`/`gross` locales removidos por H-2 no tienen otros usos (verificado
  en el diff completo).
- El fail ControlBoard.test.tsx pre-existe a mi verificación y pertenece a CB-03 (hand-off ya
  documentado por H-3 §5 y H-4 §2 — lo re-confirmo).

## 3. Gaps RESTANTES (persisten — ninguno desmiente el fix, todos conviven con él)

### R1 (agent-fixable, LOW) — el subtext de "Decoherencia media" cubre 2 de 3 causas
`page.tsx:261-267`. Cuando el fetch FALLA (`source === "server-fetch-failed"` → opportunities=[]),
detectedCount=0 → el subtext dice **"sin datos — feed vacío"**, causa falsa: la real es feed no
disponible, y la sección del feed 20cm abajo lo dice ("Feed no disponible — snapshot del servidor
falló", page.tsx:300-302). Es la MISMA clase que H-1 (prosa que contradice al panel vecino) dentro
del propio hunk de H-2 (su §2c fijó el estándar "declarar la causa real"). La variable `failed`
(:164) está en scope y sin usar allí. Relevancia viva: con PG crash-loopeando (H-4 §5), el 503 es
el estado DOMINANTE de hoy — al deployar, el home diría "feed vacío" con el feed caído. Mismo
patrón en hunks de pares: hero H-3 (page.tsx:223) y detectedStat H-4 (value 0 + "ventana live ·
límite 50" en failed). **Fix**: `failed ? "sin datos — feed no disponible" : detectedCount > 0 ? … : …`.

### R2 (agent-fixable, LOW-MED) — el gemelo EXACTO de H-2 vive en /opportunities/by-strategy
`frontend/features/opportunities/OpportunitiesByStrategyClient.tsx:55-57`:
`opps.reduce((sum, o) => sum + (o.roi_pct ?? 0), 0) / opps.length` con else `: 0` — feed all-null
(la realidad de hoy: 100% rejected) → avgRoi=0 → `formatPctOrDash(0)` = **"0.00%"** fabricado
(`lib/format.ts:70-75`: 0 es valor real, sólo null → "—"); y grupo VACÍO también muestra "0.00%"
(ni siquiera null — peor que el pre-fix de la home). Además `:54` `totalProfit` suma nets con
`?? 0` (ausencia presentada como $0.00 computado). Ruta cableada: `app/opportunities/by-strategy/
page.tsx:49`. Twin no-rutado: `features_backup/opportunities/…:42`. El WO H-2 fue scoped "en la
home" y lo cerró ahí — pero el browse auditor no visitó by-strategy y esta fabricación sigue viva
en la DApp. **Fix**: espejo de `computeAvgRoiPct` (exportarlo y reusarlo).

### R3 (agent-fixable, LOW) — `safetyA: 0, safetyB: 0` hardcodeados en toXRayProps
`page.tsx:118-119` pasa literales 0 → `XRayCard.tsx:74` renderiza **"TOKEN SAFETY: A 0 · B 0"**
en TODAS las cards del home. `OpportunityRow` NO tiene campo de safety (schemas.ts:42-112; el
`token_safety` de schemas.ts:249 es de otro schema de config) → default disfrazado de dato
(R8 categoría b). PRE-EXISTENTE (está en HEAD), no introducido por H-2, no flaggeado por el
browse auditor. **Fix**: "— no computado" hasta que exista wiring real (skill
`token-risk-and-asset-safety-filter`).

### R4 (agent-fixable, LOW, cosmético) — StatCard SSR pinta "0" pre-mount para métricas no computadas
`StatCard.tsx:69-80`: el render `!mounted` usa el literal `{prefix}0{suffix}` en vez de
`{displayValue}` (cuyo inicial YA es `animate ? 0 : value`, :31). Con avgRoi=null (animate=false)
el HTML SSR contiene "0" — snapshot fabricado — que post-mount cambia a "—". El auditor (DOM
post-mount) ve "—"; un curl del HTML crudo ve "0". Pre-existing, afecta a todos los statcards
("Capital expuesto" es el único donde el 0 SSR es verdadero). **Fix de 1 línea** sin romper el
count-up. Nota: StatCard.tsx está tocado por H-3 (sólo import) — sin conflicto de claim para el fix.

### R5 (operator-gated) — deploy pendiente + PG caído
El fix vive 100% en working tree (NO-GIT): el dominio público sigue mostrando "+0.00%"/"0.00%"
hasta PR + pipeline aprobado por el operador. Eco del escalado de H-4 §5: disco 100% y postgres
crash-loopeando — acción VPS = operator-gated. No es un gap del fix; es el estado declarado del
protocolo.

## 4. Hand-off a la mesa

1. R2 es el más urgente de los agent-fixables (fabricación viva hoy en una página rutada) —
   candidato a WO propio (reusar el `computeAvgRoiPct` ya exportado).
2. R1/R3/R4 caben en un solo WO cosmético-de-honestidad sobre page.tsx + StatCard.tsx
   (coordinar claim de archivo con H-3 para StatCard).
3. A CB-06/BROWSE (post-deploy): re-verificar el journey §2 fila 1 del auditor R8 — cards "—" y
   statcard "—" con subtext "roi no computado en el feed".
