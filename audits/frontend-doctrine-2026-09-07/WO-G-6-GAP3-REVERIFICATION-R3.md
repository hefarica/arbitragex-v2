# WO-G-6 · GAP-3 (ronda 2) — RE-VERIFICACIÓN ADVERSARIAL (ronda 3)

> Verificador Gang Omniscience ronda 3 · 2026-09-09 ~00:45 local. Re-audita
> `WO-G-6-GAP3-RENDER-TEST.md` (FIXER ronda 2, 23:32) sin asumir nada: cada claim
> re-ejecutado o re-leído con evidencia propia. Reglas: RULE 00/R8 · §32/§33
> (CERO VPS, 0 requests HTTP — verificación 100% local) · NO-GIT (0
> commit/push/PR/deploy; verificado que el árbol sigue sin commitear).

## §0 Veredicto en una línea

**PASS — GAP-3 quedó genuinamente CERRADO y no hay regresiones.** Los 3 tests de
render ejecutan el JSX REAL del componente REAL que el contenedor despacha, las 3
aserciones pedidas por el cross-examiner están presentes y son NO-vacías (2
discriminadores), la extracción `TickerMarquee` es movimiento puro verificado
byte-contra-byte contra HEAD, y la desviación del hint (RTL→renderToStaticMarkup)
está documentada y es CORRECTA para este repo. 15/15 + tsc EXIT 0 + suite completa
1175/1175, todo re-ejecutado por mí.

## §1 Verificación independiente re-ejecutada (PRIMARY_SOURCE)

| Chequeo | Resultado (mío, 2026-09-09 00:37-00:43) |
|---|---|
| `npx vitest run components/__tests__/OpportunityTicker.test.tsx` | **15/15 PASS** (124ms) |
| `npm run typecheck` (frontend) | **EXIT 0** |
| Suite frontend COMPLETA (`npm test`) | **127 archivos / 1175 tests, 0 failed** |
| `git log --oneline -5` | HEAD sigue `27aca289` — CERO commits nuevos (NO-GIT sostenido) |
| Contención marker `WO-G-6` (grep repo, audits excluidos) | SOLO los 2 archivos canónicos |
| Consumidor en app | `app/layout.tsx:14,139` importa `OpportunityTicker` (único); `TickerMarquee` no se usa en ningún otro archivo de app |

**Sobre los 2 fallos que el FIXER reportó (1173/2):** mi corrida da 1175/1175 —
ambos archivos (`ControlBoard.test.tsx`, `ControlScopeBadge.test.tsx`) pasan HOY.
Mismo total de tests (1173+2 = 1175): nadie añadió/borró tests desde entonces.
Esto CORROBORA la adjudicación del FIXER (fallos ajenos/transitorios, no suyos).

## §2 Los 3 sitios de render — claim por claim

**Las 3 aserciones pedidas están, en `OpportunityTicker.test.tsx:142-165` (test 1):**
- sr-only `"yield not computed"` → `:150` (`toContain`) + `:149` (existe `class="sr-only"`).
- Celda `"—"` neutra → `:153` (`<span>—</span>`) + `:154` (NEGATIVO: jamás
  `<span class="pos|neg">—`). El negativo de :154 es lo que hace al pin real:
  prohíbe el em dash coloreado, no solo exige un em dash.
- Ausencia ▲/▼ → `:157-158` + `:159` (`not.toContain('class="arr')`).
- Bonus keep-rows BUG-06 → `:162-164` (la fila sin precio ES visible).

**NO-vacuidad (los 2 discriminadores) — verificado en el código y en el HTML implícito:**
- Test 2 (`:167-180`): computado ⇒ `<span class="pos">+2.31%</span>`,
  `class="arr pos">▲`, `neg`/`▼`, y sr-only muestra el VALOR con
  `not.toContain("yield not computed")` — cubre la rama alternativa del sr-only.
  Sin este test, las ausencias del test 1 podrían pasar con un componente que no
  renderiza flecha NUNCA. Con él, quedan clavadas al `y != null`.
- Test 3 (`:182-197`): ventana mixta (1 roi + 2 null) ⇒ exactamente **2 arrows**
  (1 fila computada × duplicación seamless-loop) y **4 celdas `—`** — la condición
  es POR-FILA, no heredada del `latest` del sr-only. Discriminación exacta por conteo.

**Fixtures vía proyección REAL (RULE 00):** los 3 tests construyen items con
`.map(opportunityToTickerItem)` (`:143-145`, `:168-170`, `:184-187`) sobre
`tickerRow()` tipado como `Parameters<typeof opportunityToTickerItem>[0]` (`:62`) —
wire-row→item usa la MISMA función de producción, y el render usa el MISMO
componente de producción (`TickerMarquee` importado de `../OpportunityTicker`).
Cero copias del markup, cero re-implementación de la proyección. Los fixtures son
inputs de test clavados al wire vivo documentado en `WO-G-6-REPORT.md §2` (fila
2250fa…/c02aaa…, roi_pct null) — el runtime no los toca.

**La extracción es movimiento puro — verificado contra HEAD** (`git show
HEAD:frontend/components/OpportunityTicker.tsx`): todo el marquee EXCEPTO los 3
sitios de la ronda 1 es byte-idéntico al bloque inline de HEAD — `aria-hidden`
del track, `key={idx}`, `displayItems = [...items, ...items]`, fallback
`"Live opportunity feed."` del `latest`, separadores `·`, clases `ago`/`ticker-item`.
Para yields COMPUTADOS la salida es matemáticamente idéntica a HEAD:
`formatTickerYield(y) = ${y>=0?"+":""}${y.toFixed(2)}%` ≡ el inline viejo
(`OpportunityTicker.tsx:77-79` vs HEAD). El delta ronda-2 = movimiento + import
React (`:7`) + comentarios. El contenedor termina en
`return <TickerMarquee items={items} />` (`:156`) — el JSX testeado ES el JSX
despachado.

## §3 La desviación del hint es correcta y estaba evidenciada — re-verificada

El hint pedía RTL + mock de `getOpportunitiesLive`. Re-verifiqué cada pilar:
- `@testing-library/react`, `@testing-library/dom`, `jsdom`, `happy-dom`:
  **MISSING** por test de existencia directo en `frontend/node_modules` Y
  `node_modules`; **no declarados** en ningún `package.json`. (PRIMARY_SOURCE.)
- `vitest.config.ts`: `environment: "node"`, sin override jsdom en ningún test.
- Canon del repo leído por mí: `components/__tests__/TokenChip.test.tsx:4-5` —
  "Why not @testing-library/react: Not installed; requires **jsdom + operator
  approval** (toolkit-alignment memory)". La justificación del FIXER no era
  retórica: es la convención escrita del repo.
- Patrón React value-import: `components/StatCard.tsx:3-7` — existe EXACTAMENTE
  como el FIXER lo citó (marker `WO-H3 (2026-09-07)`), misma forma que
  `OpportunityTicker.tsx:3-9`.

Instalar RTL en un árbol sin lockfile con peers editando en vivo habría sido
mutación desproporcionada. La adaptación preserva el INTENT del hint: el path JSX
se ejecuta de verdad (antes no se ejecutaba NUNCA bajo test — eso era GAP-3), la
proyección es la real, y el mock quedó innecesario por construcción. Aceptada.

## §4 Nits encontrados (cosméticos — NO afectan el veredicto)

1. **Cita de línea desviada**: el reporte dice `vitest.config.ts:21`; el
   `environment: "node"` vive en `:18` (archivo sin modificar vs HEAD — imprecisión
   de cita, sustancia correcta).
2. **Comentario re-worded en el movimiento "byte-a-byte"**: HEAD traía
   `// non-empty in this branch, but tsc cannot narrow it` en la línea de
   `latest`; ahora es `// first row drives the sr-only summary; caller guarantees
   non-empty` (`OpportunityTicker.tsx:169`). Es comentario, no JSX ni comportamiento
   — se nombra para que "mismo JSX byte-a-byte" quede acotado con precisión honesta.

## §5 Gaps RESTANTES (solo los persistentes — actualizados a esta ronda)

| # | Estado | Dueño |
|---|---|---|
| GAP-1 | **PERSISTE — fix NO deployed** (NO-GIT): el ticker de producción sigue con el código fabricador. Requiere que el operador reanude GIT→VPS + deploy. `git log` confirma: nada commiteado. | **operator-gated** |
| GAP-2 | **YA NO es gap abierto de G-6**: cerrado LOCAL por el par **WO-GAP2** (reporte 00:34 — `orchestrator.rs` estampa `None` en las 5 ramas de rechazo; cargo check/test/clippy PASS según su reporte). Su deploy cabalga el mismo gate que GAP-1 — aterrizan juntos. | operator-gated (deploy) |
| GAP-3 residual | **PERSISTE (LOW, declarado por el FIXER §5)**: las ramas del CONTENEDOR (loading/error/empty/!mounted) y el wiring `useEffect→getOpportunitiesLive` siguen cubiertos solo por tsc + el browser-check de deploy. Cubrirlas exige jsdom/RTL (aprobación operador, canon TokenChip) o e2e Playwright. Fuera del scope registrado de GAP-3 (los 3 sitios), pero vivo en el tablero. | **operator-gated** (aprobación toolkit) |
| GAP-4 | **CERRADO** — dueño HP-05 lo reparó a las 23:05 (ver `GAP-4-FIXER-R2.md`); mi `typecheck` EXIT 0 de esta ronda es la tercera confirmación independiente. | — |

## §6 Composición con los pares (sin contradicciones)

- `WO-G-6-REVERIFICATION-R2.md` §5 registró GAP-3 como agent-fixable LOW — este
  ride lo cerró exactamente en ese scope; concuerdo con el residual que el propio
  R2 anticipó (la lógica es ternaria simple; el wiring queda para jsdom/e2e).
- `WO-GAP2-REPORT.md`: composición espejo confirmada — FE fiel (`roi_pct ?? null`)
  + emisor honesto (`None`) cierran el circuito R8 end-to-end cuando el operador
  despliegue ambos. Su tabla §5 (GAP2-a risk_score, GAP2-b scanner legacy) queda
  para la mesa ronda 3; no la re-adjudico aquí.
- `GAP-4-FIXER-R2.md`: veredicto de cierre confirmado por mi typecheck.
- Cero conflictos de archivo: mis verificaciones fueron 100% lectura/ejecución;
  el árbol del FIXER toca exactamente sus 2 archivos canónicos.

## §7 Log de esta ronda

CERO VPS, 0 requests HTTP (presupuesto 5 intacto — no hacía falta: GAP-3 es 100%
local). Ejecuciones: vitest (archivo + suite completa), tsc, git show/log/status/diff
(lectura), greps de contención. Cero mutaciones del árbol (ni siquiera ediciones —
rol verificador). Sin commit/push/PR/deploy.

— Verificador adversarial ronda 3 · Gang Omniscience · 2026-09-09
