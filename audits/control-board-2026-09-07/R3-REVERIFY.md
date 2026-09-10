# R3 RE-VERIFY (adversarial) — veredicto: fix CONFIRMADO, gap original CERRADO, 0 regresiones

- **WO**: re-verificación adversarial del R3 FIX (`R3-FIX-token-safety-no-computado.md`) ·
  **Agente**: REVERIFY R3
- **Fecha**: 2026-09-08 ~23:15 local · Read-only total: 0 git (sin commit/push/PR/deploy),
  0 escrituras VPS, 0 requests a dominio público, 0 executor/wallets/broadcast (§32/§33/§34.3).
  Verificación 100% local: lectura de código + diff vs HEAD + grep de alcance + vitest + tsc + eslint.

## 0. Sincronía de mesa redonda (leída ANTES de verificar)

Leídos: `GOAL-WORKORDERS.md` (completo), `H-2-REVERIFY-ronda2.md` (fuente del WO R3 — su §3
R3 ancló el defecto), `BROWSE-Auditor-R8-caza-de-deshonestidad-de-datos.md` (saga H — R3 es
su misma clase b en otra métrica), `R3-FIX-token-safety-no-computado.md` (objeto de esta
verificación), `H-2-R1-FIX.md` + `R4-STATCARD-FIX.md` (pares concurrentes sobre page.tsx).
No contradigo ningún hallazgo previo; confirmo el fix con evidencia propia independiente.

## 1. Veredicto por claim (evidencia RE-ejecutada por mí, no copiada del fixer)

| Claim del R3-FIX | Mi evidencia | Resultado |
|---|---|---|
| Mapper ya no pasa literales 0 | `frontend/app/page.tsx:118-126` — `safetyA: null, safetyB: null` + comentario WO-R3 que documenta (a) ausencia de campo en el wire, (b) historial del literal, (c) condición de re-wiring (`token-risk-and-asset-safety-filter`). `git diff HEAD` muestra `-safetyA: 0, -safetyB: 0` removidos → confirma además que el defecto PRE-EXISTÍA en HEAD (claim de pre-existencia VERIFICADO) | **CONFIRMADO** |
| Props `number \| null` espejo del patrón confidence | `XRayCard.tsx:17-23` — `safetyA/safetyB: number \| null` con JSDoc WO-R3; patrón espejo del `confidence` (`:8-10`) real y citado. La firma nullable cierra la puerta al 0 accidental en el compilador (tsc exit 0) | **CONFIRMADO** |
| Render 3-ramas | `XRayCard.tsx:83-91` — ambas null → `"— no computado"`; alguna computada → `` `A ${safetyA ?? "—"} · B ${safetyB ?? "—"}` ``; computado-0 → `A 0 · B 0` honesto (R8: Some(0.0) ≠ None — NO sobre-corrige). Pre-fix (HEAD): `` `A ${safetyA} · B ${safetyB}` `` incondicional — verificado en el diff | **CONFIRMADO** |
| 5 tests WO-R3 | `components/__tests__/XRayCard.test.tsx:75-98` (4 tests) + fixture base `:29-32` en null; `app/__tests__/page.test.tsx:103-109` (1 test: `toXRayProps(makeOpp())` → null, nunca 0 — pinneo genuino: un re-hardcode de 0 lo rompe). Re-ejecutados POR MÍ: **27/27 PASS** (XRayCard 7 = 3 AUDIT-2026-08-29 + 4 R3; page 9 = 8 H2 + 1 R3; hero-h3 4/4; statcard-h4 7/7) | **CONFIRMADO** |
| Sin regresiones | Suite completa re-ejecutada: **1171/1172 PASS** · único fail = `ControlBoard.test.tsx` (CB-03 casing "shadow" — pre-existing, 5º peer que lo confirma; archivo jamás tocado por R3). `tsc --noEmit` **exit 0 real** (capturado sin pipe-trap). `eslint` sobre los 4 archivos **exit 0** | **CONFIRMADO** |
| `lib/schemas.ts` NO tocado por R3 | `git status` muestra `M schemas.ts` PERO el diff contiene SOLO el hunk WO-H4 (`window_total`, `:116-119`) — cero presencia R3. El claim del fixer es exacto | **CONFIRMADO** |
| Convivencia de claims en page.tsx | Diff completo vs HEAD: hunks WO-H2/H3/H4/H2-R1 intactos e intercalados con el hunk WO-R3 — nada pisado | **CONFIRMADO** |
| NO-GIT / RULE 00 | `git status --porcelain`: M/?? en working tree, 0 commits. El fix FABRICA MENOS: elimina un default disfrazado; no inventa dato alguno | **CONFIRMADO** |

**Clasificación**: root-cause pre-fix = CANONICAL_REPO (diff contra HEAD) · comportamiento
post-fix = verificado por ejecución propia (PRIMARY_SOURCE local) · ausencia de fuente en el
wire = verificado end-to-end (ver §2).

## 2. Probes adversariales propios (más allá de los claims del fixer)

1. **¿La ausencia de safety es REAL en el wire, o el mapper ignora un campo que sí viaja?**
   Recorrí la cadena completa: `OpportunityRowSchema` (`lib/schemas.ts:42-112`) no tiene
   campo de safety; `backend/searcher-rs/src/` NO emite score de safety por oportunidad (la
   pantalla token-safety existe como GATE de activación de pool — `pool_sources/alchemy.rs:272`
   "the token-safety screen gates activation" — no como score en la fila live);
   `backend/api-server` sólo expone `token_safety` como CONFIG (`index.ts:301, :1717`) y el
   verificador G-TOK-1 es readiness, no payload. **Conclusión: null es el estado VERDADERO
   end-to-end; "— no computado" no under-claimsea nada.** La condición de re-wiring del
   comentario es la correcta y completa.
2. **¿Gemelo del defecto en otro renderer?** `XRayCard` tiene UN solo consumidor no-test
   (`app/page.tsx:1, :347`). "TOKEN SAFETY" sólo se renderiza en `XRayCard.tsx`. Los matches
   `token_safety` en `features/onboarding` + `api-client.ts:642-651` son settings de provider
   (config), no render de score. **No hay gemelo R3.**
3. **¿Fixtures que.matchean safetyA/B son mock-vector?** `lib/__tests__/fixtures/prod_20260824/
   {config_current.json:"token_safety_api", risk_alerts.json:"safety_below_threshold"}` —
   snapshots pre-existentes de config/alerts, sin relación con XRayCard. No son datos del card.
4. **¿Reintroducción de literales?** `grep -rn "safetyA: 0" frontend/` (no-test): único match
   = el comentario histórico de `page.tsx:121`. Limpio.
5. **Edge cases del render** (cubiertos por tests que re-ejecuté): parcial `A — · B 92` sin
   coacción a 0; computado-0 visible como `A 0 · B 0` (anti sobre-corrección, espejo del
   test 4 de WO-H2).

## 3. Drift desde el reporte del fixer (árbol vivo — NO son gaps de R3)

- El R3-FIX §4 reportó 7 errores de tsc en `PreferenceVectorPanel.tsx` (hand-off HP-05):
  **ya resueltos** — el peer actualizó el archivo (mtime 23:05) y mi `tsc --noEmit` da exit 0
  con cero output. El hand-off era exacto en su momento.
- Conteos de suite: 1160/1161 (fixer) → 1171/1172 (yo, 15 min después) — peers aterrizando
  tests en vivo. Mismo y único fail pre-existente.
- Nits cosméticos de líneas en el reporte del fixer: describe XRayCard ":75-96" (real
  :75-98), describe page ":98-107" (real :98-109). Sin sustancia.

## 4. Gaps RESTANTES (sólo los que persisten — ninguno desmiente el fix)

### G1 (operator-gated) — deploy pendiente: el dominio público sigue mostrando "A 0 · B 0"
El fix vive 100% en working tree (NO-GIT 2026-08-23). Hasta PR + pipeline aprobado por el
operador, `https://arbx.ape-tv.net` sigue renderizando el literal viejo. Declarado
honestamente por el propio fixer (R3-FIX §7). Eco del R5 del reverify H-2.

### G2 (agent-fixable, WO futuro distinto — no un defecto de R3) — wiring real de token-safety
La métrica queda honestamente en "— no computado" PARA SIEMPRE hasta que exista emisión
backend → campo en `OpportunityRowSchema` → derivación en `toXRayProps` (skill
`token-risk-and-asset-safety-filter`; hoy la pantalla es gate de activación, no score en el
payload). Condición ya documentada en `page.tsx:122-124` y `XRayCard.tsx:20-21`. Candidato a
WO propio de la mesa cuando el operador lo priorice.

### G3 (agent-fixable, hand-off saga — NO es de R3 pero lo detecté al cerrar el alcance) — R2 sigue VIVO
`frontend/features/opportunities/OpportunitiesByStrategyClient.tsx:54-57` conserva el gemelo
EXACTO de H-2: `(o.roi_pct ?? 0)` promediado + else `: 0` → "0.00%" fabricado en
`/opportunities/by-strategy` (feed all-null de hoy) y `totalProfit` suma nets con `?? 0`
(`:54`). El plan del reverify H-2 §4 lo destinaba a WO propio y **no existe reporte R2-*.md
en este board** — es el último agent-fixable de la saga R1-R4 sin aterrizar (R1, R3 y R4 ya
tienen fix). Se lo dejo a la mesa con la misma solución propuesta: reusar el
`computeAvgRoiPct` ya exportado (`page.tsx:135`).

## 5. Hand-off final

- Al operador: R3 verificado CERRADO localmente; falta sólo su gate de deploy (G1).
- A CB-06/BROWSE post-deploy: journey §2 fila 1 del auditor R8 — la fila TOKEN SAFETY de cada
  card debe decir "— no computado" (y NO "A 0 · B 0").
- A la mesa: despachar G3 (R2) — es la única fabricación numérica viva que quedó de la lista
  R1-R4 del reverify H-2.
