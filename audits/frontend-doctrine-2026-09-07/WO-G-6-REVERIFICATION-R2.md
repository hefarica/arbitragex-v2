# WO-G-6 · RE-VERIFICACIÓN ADVERSARIAL (ronda 2) — veredicto vivo + fix P0-2

> Verificador Gang Omniscience ronda 2 · 2026-09-08 ~23:00 local · re-audita
> `WO-G-6-REPORT.md` (ronda 1, 22:08) sin asumir nada: cada claim re-ejecutado o
> re-leído con evidencia propia. Reglas respetadas: RULE 00/R8 · §32/§33 (VPS solo
> lectura: df, docker ps, XRANGE/XLEN/XINFO, psql SELECT, curl GET a loopback —
> CERO mutación, 0 requests al dominio público) · NO-GIT (0 commit/push/PR/deploy).

## §0 Veredicto en una línea

**PASS — el gap original quedó cerrado y no hay regresiones atribuibles a G-6.**
El veredicto de ronda 1 se re-adjudica INDEPENDIENTEMENTE por tres fuentes vivas
(incluida ahora la fuente EXACTA del ticker, antes inaccesible); el fix se verifica
claim por claim contra el diff y contra los diseños de ambos pares; 12/12 tests
re-ejecutados PASS; los fallos actuales del árbol compartido se adjudican 100% a
peers paralelos. Quedan 4 gaps persistentes (§5), ninguno reabre el trabajo de G-6.

## §1 Re-adjudicación del veredicto (más fuerte que en ronda 1)

Ronda 1 pivoteó a Redis porque PG/api-server crash-loopeaban (cierto a las 22:08).
**G-1 se RECUPERÓ durante esta re-verificación** (postgres y api-server `Up
(healthy)` 39-40 min; disco 141G/150G con 2.9G libres — ya no 0). Esto NO refuta a
ronda 1: es evolución temporal del incidente, nombrada. La recuperación permitió
verificar por el camino que ronda 1 no pudo:

| Fuente (todas vivas 2026-09-08 ~23:00 local) | Hecho | Clasificación |
|---|---|---|
| Redis `arbx:opps:detected`, ventana fresca (XLEN 10,004) | `"roi_pct":null` **10,004/10,004**; `"roi_pct":0.0` **0** | PRIMARY_SOURCE |
| PostgreSQL directo (últimos 15 min, 47,818 filas) | `count(roi_pct)` = **0**; `roi_pct=0.0` = **0** | PRIMARY_SOURCE |
| **Endpoint EXACTO del ticker** (edge loopback `127.0.0.1:8787/api/opportunities/live?limit=20`) | `"roi_pct":null` **20/20**; `net_expected_profit_usd:null` **20/20**; `expected_profit_usd:null` **20/20** | PRIMARY_SOURCE |

Cadena de código que valida el proxy (CANONICAL_REPO): `publisher.rs:153`
`serde_json::to_string(opp)` + `backend/shared-rs/src/contracts.rs:76`
`pub roi_pct: Option<f64>` sin `skip_serializing_if` → el stream serializa
`roi_pct` SIEMPRE (null incluido) → proxy fiel, como afirmó ronda 1 §2.

La fabricación vieja confirmada en el diff vs HEAD (git): el código pre-fix era
`opp.roi_pct ?? (profit > 0 ? profit * 0.1 : profit * 0.1)` — ambos brazos del
ternario IDÉNTICOS, exactamente como catalogó FE-02b-01. Con `roi_pct:null` al
100% y net diminuto (0.000055×0.1 = 0.0000055 → `toFixed(2)` = "0.00" →
`"+0.00%" ▲`): la fila del browse-auditor (§2.7) era la fabricación, no un cero
computado. **Veredicto de ronda 1: CONFIRMADO por tres vías independientes.**

Hallazgo adicional del wire actual: hoy 20/20 items traen net Y expected null →
el ticker DEPLOYED (código viejo) dropea las 20 filas y muestra el empty state
MENTIROSO "No topological convergence detected" habiendo 20 detecciones en la
ventana — el OTRO brazo de BUG-06, vivo en producción ahora mismo (ver GAP-1).

## §2 Fix verificado claim por claim (diff `// WO-G-6 (2026-09-07)`)

Archivo único `frontend/components/OpportunityTicker.tsx` + su test. Contención
verificada: `WO-G-6` aparece SOLO en esos 2 archivos (grep frontend+backend);
backend sin markers ni diffs del WO (orchestrator.rs intacto en working tree).

1. `TickerItem.yield: number | null` — ✓ (`:11-14`).
2. Drop eliminado + fabricación eliminada; `yieldPct = opp.roi_pct ?? null` — ✓
   (`:49-66`, línea 58 con INV-FE02B-3). Firma estrechada a `Pick<OpportunityRow,
   …7 campos…>` — ✓.
3. `formatTickerYield` exportada, proyección pura (null⇒"—") — ✓ (`:72-74`).
4. Fetch sin filter (map directo) — ✓ (`:88-92`); el filter de nulls quedó muerto.
5. Render 3 sitios — ✓: sr-only `"yield not computed"` (`:155`); marquee con clase
   condicional `undefined` para null (`:170-172`); flecha ▲/▼ NO renderizada si
   `y == null` (`:173-177`).
6. Empty state: la composición documentada (`:128-132`) es ESTRUCTURALMENTE
   correcta — con keep-rows, `items.length === 0` ⇔ 0 filas reales (map preserva
   longitud) → "No topological convergence detected" dispara solo con ventana
   vacía literal. Cotejo con los pares: FE-02b D-01 aplicado verbatim (incluido el
   detalle de ocultar la flecha junto con el %). FE-02a FIX-7: keep-rows ✓; su
   copy-corrección ("N detecciones sin precio aún") queda absorbida — y el propio
   diff de FE-02a era internamente inconsistente (conservaba la firma
   `TickerItem | null` y hablaba de "contando las dropeadas antes del map" mientras
   eliminaba el drop), así que la composición de G-6 es la resolución coherente de
   la unión que la doctrina P0-2 ordenaba. Documentada en §3 de ronda 1. ✓
7. Test de regresión: 8 nuevos + 4 pre-existentes = **12/12 PASS re-ejecutados por
   mí** (`npx vitest run components/__tests__/OpportunityTicker.test.tsx`, 1.03s).
   Los fixtures pinean el caso vivo exacto (roi_pct null ⇒ yield null) y el cero
   computado legítimo (`0 → "+0.00%"`, R8).
8. Claims de archivo de ronda 1: consumidor único `app/layout.tsx:14,139` ✓ (grep
   blast-radius: layout + componente + test, nada más); `orchestrator.rs`
   `Some(0.0)` placeholder en **1242, 1269, 1297, 1332, 1377** ✓ (leído;
   `:1358` es el único `Some(outcome.net_roi_pct)` computado) — espejo backend
   real, no pisado, correctamente registrado como dependencia. WO-H4 diff
   (`schemas.ts` +`window_total` opcional; `opportunities-live.ts` +`COUNT(*)
   OVER ()`) — ✓ aditivo y ortogonal: cero intersección con `roi_pct`/ticker;
   componen sin conflicto.

## §3 Regresiones — adjudicación de cada fallo del árbol compartido

- **Archivo G-6**: 12/12 PASS. Cero errores tsc en OpportunityTicker/su test.
- **`npm run typecheck` FALLA hoy** (exit 2) con 7 errores — **todos** confinados a
  `frontend/components/PreferenceVectorPanel.tsx` (TS2532 ×6, TS7006 ×1). Ese
  archivo es **UNTRACKED, creado 22:28 por el WO paralelo HP-05** (marker línea 3),
  20 minutos DESPUÉS del reporte de ronda 1 (22:08), y tiene **0 importadores**.
  Conclusión: el claim "EXIT 0" de ronda 1 era verdadero en su momento de chequeo;
  el árbol se puso rojo después por un peer. NO es regresión G-6 → GAP-4 (dueño
  HP-05).
- **Suite completa**: 1157 passed / 4 failed (2 archivos). Adjudicación: (a)
  `ControlBoard.test.tsx` ×1 — el fallo PRE-EXISTENTE del programa CB-03 que
  ronda 1 ya reportó (untracked, 17:48); (b) `StatCard.test.tsx` ×3 — race
  TRANSITORIA con un peer editando en vivo: mi corrida empezó 22:53:04,
  `StatCard.tsx` fue guardado 22:54 en plena corrida; re-ejecutado aislado pasa
  **5/5**. Ninguno toca la ruta del ticker. **Cero regresiones G-6.**
- **NO-GIT cumplido** (ronda 1 y esta ronda): diff vs HEAD presente (nada
  commiteado); en VPS solo comandos de lectura.

## §4 Notas de mesa (estado del sistema al cierre de esta ronda)

- **G-1 recuperado** → G-4 (estados POBLADOS de /executions y /paper/history sin
  fe) queda DESBLOQUEADO para re-fe de browser.
- **G-PIPE-1/G-2 persiste**: `XINFO GROUPS` → consumer `enricher` lag **18,022**
  (idéntico al del browse-auditor), `last-delivered-id 1788927014308-0` congelado
  pese a api-server recuperado — el consumer NO drenó; sigue operator-gated.
- El api-server deployed NO sirve `window_total` (verificado en el response vivo)
  — consistente con WO-H4 local-only/NO-GIT.
- Ola-5 de la doctrina (ticker→store, eliminar la 5ª vía de fetch) tenía a P0-2
  como prerrequisito declarado — **quedó desbloqueada** por este fix.

## §5 Gaps RESTANTES (persistentes)

| # | Gap | Dueño |
|---|---|---|
| GAP-1 | **Fix NO deployed** (NO-GIT): el ticker de producción sigue con el código viejo — en el wire actual (20/20 net+expected null) muestra el empty state MENTIROSO; cuando fluyan filas con precio, fabricará "+0.00%" de nuevo. Requiere reanudar GIT→VPS y deploy. | **operator-gated** |
| GAP-2 | Espejo backend R8: `orchestrator.rs:1242,1269,1297,1332,1377` escriben `roi_pct=Some(0.0)` placeholder en ramas de rechazo. Hoy latente (0/10,004 stream, 0/47,818 PG, 0/20 endpoint), pero al llegar al wire el FE fiel lo mostrará como cero computado. Cura backend: `None` o flag `roi_computed` (scope de otro programa). | **agent-fixable** (dependencia declarada) |
| GAP-3 | Los 3 sitios de render del fix no tienen test de render RTL (los 8 tests nuevos son proyecciones puras; el path JSX `:158-183` solo lo cubre tsc). Severidad LOW — la lógica es ternaria simple y el browser-check de deploy lo cubrirá. | **agent-fixable** |
| GAP-4 | **Árbol compartido typecheck ROJO desde 22:28** por `PreferenceVectorPanel.tsx` (HP-05, untracked, 7 errores, 0 importadores). Bloquea el claim "todo verde" de CUALQUIER peer que verifique después hasta que HP-05 lo repare o lo retire. | **agent-fixable** (dueño HP-05) |

## §6 Log de interacción con el VPS (todo read-only)

`df -h /`; `docker ps` (estados); `redis-cli XLEN/XRANGE/XINFO GROUPS` sobre
`arbx:opps:detected` (contenedor `arbitragex-v2-redis-1`); `psql -U postgres -d
arbitragex -t -c "SELECT count…" ` (SELECT); `curl -s GET` a `127.0.0.1:8787`
(loopback interno del VPS, NO dominio público — presupuesto 5 intacto). 0 writes
en servicios/config/datos. Sin archivos scratch. Sin commit/push/PR/deploy.

— Verificador adversarial ronda 2 · Gang Omniscience · 2026-09-08
