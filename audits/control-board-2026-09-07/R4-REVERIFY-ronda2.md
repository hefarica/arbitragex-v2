# R4 RE-VERIFY (ronda 2, adversarial) — veredicto: fix CONFIRMADO CERRADO, sin regresiones

- **WO**: R4 (re-verificación adversarial del fix de `R4-STATCARD-FIX.md`, gap de origen
  `H-2-REVERIFY-ronda2.md` §3-R4) · **Agente**: REVERIFY R4
- **Fecha**: 2026-09-08 23:0x–23:2x local · Read-only total: 0 git-writes (sin
  commit/push/PR/deploy; `git status` verificado pre/post), 0 VPS (innecesario), 0 requests
  a dominio público, 0 executor/wallets/broadcast (§32/§33/§34.3, NO-GIT intactos).
  El A/B adversarial (§2) editó el working tree ~15s con trap de restauración garantizada —
  árbol verificado íntegro post-A/B.

## 0. Sincronía de mesa redonda (leída ANTES de verificar)

Leídos: `GOAL-WORKORDERS.md` (completo), `H-2-REVERIFY-ronda2.md` (fuente del WO §3-R4),
`R4-STATCARD-FIX.md` (objeto), `H-3-REVERIFY.md` (dueño del otro claim sobre
`StatCard.tsx` :3-7). Nada refutado. Aportes: (a) A/B re-ejecutado por mí con síntomas
idénticos a los del fixer, (b) mapa residual R1–R5 re-censado HOY — R1 y R3 ya fueron
adoptados por pares paralelos, R2 sigue vivo (§4).

## 1. Verificación del fix (leído + corrido por mí)

| Claim de R4-STATCARD-FIX | Mi evidencia | Resultado |
|---|---|---|
| `:83` renderiza `{prefix}{displayValue}{suffix}` en el branch `!mounted` | `StatCard.tsx:83` leído | **CONFIRMADO** |
| Estado inicial `:31` YA es `animate ? 0 : value` | `StatCard.tsx:31` + `git diff HEAD` — la línea `useState` es CONTEXTO intacto del diff (pre-existente en HEAD): R4 NO la tocó, el fix sólo consume el inicial que ya existía | **CONFIRMADO** |
| Scope del diff = exactamente 2 hunks disjuntos | `git diff HEAD -- frontend/components/StatCard.tsx`: hunk1 `:3-7` (import WO-H3) + hunk2 `:69-75` (comentario WO-R4) con la línea `{prefix}0{suffix}` → `{prefix}{displayValue}{suffix}`. Cero cambios adicionales | **CONFIRMADO** |
| Aritmética de líneas del claim (":83, pre-edit :76") | Literal en HEAD = :71; +5 líneas del import H-3 = :76 pre-edit; +7 del comentario R4 = :83 | **CONFIRMADO** |
| Marcador `// WO-R4 (2026-09-07)` :69-75 | Leído | **CONFIRMADO** |
| Test de pinneo nuevo de 5 casos | `components/__tests__/StatCard.test.tsx` — 5 `it()`, importa el `StatCard` REAL (`../StatCard`), renderiza vía `renderToStaticMarkup` (superficie exacta del gap: HTML crudo). `valueDiv()` auditado: `leading-none` aparece UNA sola vez en el markup pre-mount (sólo el div de valor; el span del label usa `mb-3`, el subtext `mt-2`) → el slice no puede falsear. El branch mounted es inalcanzable en SSR (`useEffect` jamás corre) → el test mide exactamente lo que el curl mediría | **CONFIRMADO (tests REALES, no teatro)** |
| 5/5 + hermanos sin regresiones | Corrida propia conjunta: `StatCard.test` 5/5 + `page.test` 9/9 + `page.statcard-h4` 7/7 + `page.hero-h3` 4/4 + `page.subtext-causes` 5/5 = **30/30 PASS** | **CONFIRMADO** |
| **A/B de pinneo** (discrimina el gap) | Re-ejecutado POR MÍ: revertí el literal in-place (backup+trap, restauración verificada: líneas :83/:96 de vuelta, backup borrado, `git status` sólo el `M` esperado) → **3/5 FAIL con los síntomas exactos**: `expected '0' to be '—'`, `expected '$0' to be '$4.2279'`, `expected '0%' to be '0.42%'`; los 2 que pasan son los comportamientos a PRESERVAR (count-up SSR 0; cero computado $0.00). Post-restore re-corrida: 5/5 PASS | **CONFIRMADO** (coincide 1:1 con §2 del fixer) |
| tsc / eslint | `npx tsc --noEmit` **exit 0** (HOY limpio del todo — los 7 errores `PreferenceVectorPanel.tsx` que el fixer reportó fueron limpiados por un paralelo); `npx eslint` ambos archivos **exit 0** | **CONFIRMADO (mejor aún que el entorno del fixer)** |
| Sin hydration drift | Inicial `animate ? 0 : value` es derivado SÓLO de props (determinista) → markup SSR == primer render del client (ambos `mounted=false`); `useEffect` no corre en hidratación (patrón R1). Para `animate=false` el efecto post-mount setea el MISMO valor → sin flicker | **CONFIRMADO** |
| Consumers: sólo `page.tsx`, 4 usos | Grep propio `<StatCard` sobre `*.tsx`: 4 usos (`features_backup` = cero). **Nota**: las líneas MOVIERON desde el reporte del fixer (:226→:241 etc.) porque pares paralelos aterrizaron los hunks R1/R3 en `page.tsx` — hero `:241` (`animate={bestNet != null}` :254), detected `:263` (`:269` animate sólo number>0), capital `:272` (`:279` animate={false}, value=0 → "$0.00" honesto), decoherencia `:290` (`:302` `animate={avgRoi != null}`). Los 4 patrones cierran la clase fabricante (animate=false + "—" → SSR "—") | **CONFIRMADO** |

**Clasificación**: root-cause y fix = **CANONICAL_REPO** (working tree + `git diff HEAD`,
HEAD 27aca289-side); corridas 30/30, A/B 3/5, tsc 0, eslint 0 = **PRIMARY_SOURCE** (mías).

## 2. Coordinación de claims (re-confirmada post-fix)

- `StatCard.tsx`: H-3 `:3-7` intacto byte a byte; R4 `:69-75` + `:83`. Hunks disjuntos,
  sin conflicto. Nada que refutar a `H-3-REVERIFY.md` — su observación no-gap §4 ("StatCard
  animado renderiza '$0' pre-mount, por diseño") sigue EXACTA tras R4: esa es la clase
  `animate=true` (métrica computada, primer frame del count-up/shimmer), no la clase
  fabricante que R4 cerró. **Que nadie la re-audite como R4**: cuando bestNet/avgRoi SÍ
  están computados, el HTML crudo muestra "$0"/"0" por un paint hasta hidratar (honesto en
  estado final, transitorio declarado; cambiar eso sería rediseñar el count-up, fuera de
  scope).

## 3. ¿Regresiones? NINGUNA

- 30/30 en la corrida conjunta de los 5 archivos que tocan StatCard vía page.tsx
  (incluido `page.subtext-causes.test.tsx`, el test nuevo del R1 adoptado).
- tsc exit 0 y eslint exit 0 sobre los archivos del WO.
- El branch mounted (`:96`) ya renderizaba `{displayValue}` desde HEAD — el fix hace
  pre-mount ≡ mounted salvo el shimmer: consistencia interna, no un fork visual.

## 4. Gaps RESTANTES (re-censo HOY del mapa H-2-REVERIFY §3 — sólo los que persisten)

1. **R2 (agent-fixable, LOW-MED) — SIGUE VIVO, el más urgente de los residuales**:
   `frontend/features/opportunities/OpportunitiesByStrategyClient.tsx:56` retiene
   `opps.reduce((sum, o) => sum + (o.roi_pct ?? 0), 0) / opps.length` (+ else `: 0`):
   feed all-null → "0.00%" fabricado, grupo vacío → "0.00%", `totalProfit` con `?? 0`.
   Ruta cableada `app/opportunities/by-strategy/page.tsx`. Fix: reusar el
   `computeAvgRoiPct` ya exportado de `page.tsx`. (R1 y R3 ya NO persisten: ver §5.)
2. **R5 (operator-gated) — no-desplegado**: el fix vive 100% en working tree (NO-GIT);
   la DApp pública sigue sirviendo el SSR viejo ("0" fabricado + hero verde viejo —
   evidencia primaria de `H-3-REVERIFY.md` §3). El deploy pasa por PR + pipeline del
   operador; el browser-verify post-deploy sigue gated por el outage PG/disco (H-4 §5).
   No es un gap del fix: es el estado declarado del protocolo.

## 5. Hand-off a la mesa

- **R1 ADOPTADO** (verificado): markers `WO-H2-R1` en `page.tsx:236-240` y `:286-289`
  (split de 3 causas, rama `failed` PRIMERA) + `page.subtext-causes.test.tsx` 5/5 PASS.
- **R3 ADOPTADO** (verificado): marker `WO-R3` en `page.tsx:118-126` — `safetyA/B: null`
  → "— no computado" en las cards, literal 0 eliminado.
- **R2 queda como único agent-fixable del paquete H-2-REVERIFY §3** — candidato a WO
  propio (mitad A/B si se quiere velocidad: componente + su página).
- A CB-06/BROWSE post-deploy: el journey de re-verificación es `curl -s <home> | grep
  "—" ` sobre el HTML crudo (sin navegador) + browser check del valor final.
