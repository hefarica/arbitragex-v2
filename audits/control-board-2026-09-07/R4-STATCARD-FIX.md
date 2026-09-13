# R4 FIX — StatCard SSR pinta "0" pre-mount para métricas no computadas (Gang Omniscience ronda 2)

- **WO**: R4 (gap `H-2-REVERIFY-ronda2.md` §3-R4) · **Agente**: FIXER R4
- **Fecha**: 2026-09-08 22:4x–22:5x local · **Read-only total**: 0 git-writes (sin
  commit/push/PR/deploy), 0 VPS (ni lectura — innecesaria), 0 requests a dominio público,
  0 executor/wallets/broadcast (§32/§33/§34.3 intactos, NO-GIT respetado).
- **Scope**: cosmético-de-honestidad, 1 línea productiva + test de pinneo.

## 0. Sincronía de mesa redonda (leída ANTES de editar)

Leídos: `GOAL-WORKORDERS.md` (completo), `H-2-REVERIFY-ronda2.md` (fuente del WO, §3-R4),
`H-3-REVERIFY.md` (dueño del único otro claim sobre `StatCard.tsx`), `BROWSE-Auditor-R8…md`
(marco R8/RULE 00 del que R4 es extensión), y los tests vivos `page.statcard-h4.test.tsx`,
`page.hero-h3.test.tsx` (para no romper sus aserciones de slice). Nada refutado; construyo
directamente sobre el R4 de H-2-REVERIFY (su hint era exacto).

## 1. El gap y el fix

**Gap (pre-existente, verificado por mí en el working tree pre-edit)**: `StatCard.tsx`
pre-mount branch renderizaba el literal `{prefix}0{suffix}` aunque el estado inicial de
`displayValue` YA es `animate ? 0 : value` (`StatCard.tsx:31`). Con `animate=false` y
`value="—"` el HTML SSR contenía "0" para una métrica no computada, que post-mount cambiaba
a "—": el auditor en browser ve "—", un curl del HTML crudo ve "0" — snapshot SSR fabricado
(R8: None ≠ Some(0.0); RULE 00).

**Fix** (`frontend/components/StatCard.tsx:83`, marcado `// WO-R4 (2026-09-07)` en
`:69-75`):

```diff
-          {prefix}0{suffix}
+          {prefix}{displayValue}{suffix}
```

- `animate=true` → estado inicial 0 → SSR idéntico al de antes ("$0", "0", …): **count-up
  preservado byte a byte** (es el primer frame de la animación, comportamiento por diseño).
- `animate=false` → estado inicial `value` → SSR honesto: "—" para no computado, "$0.00"
  para cero computado verdadero, el valor real para valor real.
- **Sin hydration drift**: el pre-mount branch se renderiza igual en server y en el primer
  render del client (ambos con `mounted=false` y estado inicial derivado sólo de props —
  patrón R1; `useEffect` no corre en SSR ni en primer paint).

**Consumers auditados** (`frontend/app/page.tsx`, únicos 4 usos en TODO el frontend —
grep `<StatCard` sobre `*.tsx`): hero `:226 animate={bestNet != null}` y decoherencia
`:268 animate={avgRoi != null}` son los casos fabricados (value "—" cuando null) — ahora
honestos; asimetrías `:241 animate={detected.value > 0}` y capital `:251 animate={false},
value=0` eran ceros verdaderos — salida invariante ("0" y "$0.00").

## 2. Verificación (corrida por mí; incluye A/B que demuestra que el test pinnea)

| Check | Comando | Resultado |
|---|---|---|
| Test nuevo de pinneo | `npx vitest run components/__tests__/StatCard.test.tsx` | **5/5 PASS** (renderToStaticMarkup, sin jsdom, sin red — patrón `XRayCard.test.tsx`) |
| **A/B pinneo** (reviertí mi línea → corrí → restauré) | mismo comando contra `{prefix}0{suffix}` | **3/5 FAIL con el síntoma exacto de R4**: `expected '0' to be '—'`, `expected '$0' to be '$4.2279'`, `expected '0%' to be '0.42%'`; los 2 que pasan son los comportamientos a PRESERVAR (count-up SSR 0; cero computado $0.00). El test discrimina exactamente el gap |
| Tests hermanos que consumen StatCard vía page.tsx | `npx vitest run app/__tests__/page.test.tsx app/__tests__/page.hero-h3.test.tsx app/__tests__/page.statcard-h4.test.tsx` | **20/20 PASS** (9+4+7) — sin regresiones H-2/H-3/H-4 |
| Suite completa | `npx vitest run` (exit capturado SIN pipe) | **1160/1161 PASS, exit 1** — único fail `components/__tests__/ControlBoard.test.tsx` (casing "SOLO 'shadow'…", territory CB-03 en vuelo, pre-existente ya reportado por H-2/H-3/H-4). Overlap verificado = cero: ese test importa `ControlBoardLed` + `app/control/ControlBoardClient`, sin StatCard en el grafo |
| tsc | `npx tsc --noEmit` | exit 2, **7 errores TODOS en `components/PreferenceVectorPanel.tsx:77,79,200`** — archivo untracked (`??`, never-committed, debris de un WO paralelo, NO mío, NO tocado). **Cero errores en StatCard.tsx o su test** |
| eslint | `npx eslint components/StatCard.tsx components/__tests__/StatCard.test.tsx` | **exit 0** |
| NO-GIT | `git status --porcelain` | `M frontend/components/StatCard.tsx` (mi línea + import pre-existente de H-3), `?? …/StatCard.test.tsx` (mi test). 0 commits |

## 3. Coordinación de claims de archivo (requerida por el hint)

- **`StatCard.tsx`**: H-3 posee `:3-7` (import `* as React`, soporte SSR-test — intacto,
  cero cambio de comportamiento, re-confirmado leyéndolo). R4 posee `:69-75` (comentario) +
  `:83` (la línea). Hunks disjuntos — **sin conflicto**.
- **`page.tsx`**: NO tocado por R4 (dueños: H-2/H-3/H-4). El fix es 100% componente.
- **`PreferenceVectorPanel.tsx`** (nota para la mesa): untracked con 7 errores tsc —
  pertenece a un WO paralelo vivo; lo dejo intacto (§3 surgical) y lo reporto para que su
  dueño lo sepa: hoy es el ÚNICO ruido rojo de `tsc --noEmit` en frontend.
- **`page.test.tsx` creció 8→9 tests ENTRE mis corridas** (22:51→22:55) — un par paralelo
  añadió un test concurrente a mi sesión; los 25 de la corrida conjunta pasan. Sin acción.

## 4. Qué queda abierto (NO mío, para la mesa)

1. **R1** (subtext "feed vacío" cuando el fetch falló) y **R3** (`safetyA/B: 0` hardcode
   en toXRayProps) — siguen abiertos según `H-2-REVERIFY-ronda2` §3; R4 NO los cubría.
   Nota: H-2-REVERIFY §4.2 proponía "R1/R3/R4 en un solo WO" — R4 cerró el suyo solo
   porque fue despachado como WO individual; R1/R3 siguen siendo agent-fixables libres.
2. **H-3-REVERIFY §4 obs. no-gap** ("StatCard animado renderiza '$0' en pre-mount SSR"):
   sigue siendo cierto POR DISEÑO para `animate=true` (primer frame del count-up) — R4 no
   cambia ese caso ni debía cambiarlo. La clase fabricante (animate=false + no computado)
   es la que quedó honesta.
3. **R5/deploys** (operator-gated, NO-GIT): el fix vive en working tree hasta PR + pipeline
   del operador.

## 5. Clasificación de claims

- Root-cause pre-fix (literal 0 vs displayValue): **CANONICAL_REPO** (leído en working tree
  pre-edit; H-2-REVERIFY §3-R4 lo diagnosticó primero — confirmo su lectura).
- Comportamiento post-fix (5/5, 20/20, 1160/1161, eslint 0): **PRIMARY_SOURCE** (corridas
  propias, incl. A/B de pinneo y exit codes sin pipe).
- Atribución del ruido tsc a PreferenceVectorPanel.tsx (paralelo): **PRIMARY_SOURCE**
  (`git status --porcelain` + lista completa de errores, todos en ese archivo).
- Consumers de StatCard (sólo page.tsx, 4 usos): **CANONICAL_REPO** (grep exhaustivo
  `*.tsx` en frontend).
