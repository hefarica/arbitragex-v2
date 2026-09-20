# FIX — RDO-503-REASON-UI (2026-09-17, fixer gang ronda 1)

> Gap asignado: "503 en GET /api/route-discovery-outcomes/summary?hours=24 (console/network
> observado) deja 'outcomes resueltos' y 'opportunities' en '—' sin razón visible en la UI;
> causa raíz no diagnosticada (presupuesto HTTP agotado; hipótesis: carga/server-lag 27543)".
> Origen del gap: `BROWSE-Operador-de-mesa-...md` §4.2 (líneas 121-125) + eco en
> `BROWSE-QA-de-WebSocket-...md` §4 (línea 78: "un 503 ... (fail honesto)").
> Reglas respetadas: RULE 00 / R8 (cero datos fabricados), §32/§33 (VPS solo lectura),
> NO-GIT (cero commit/push/PR/deploy — diffs en working tree, marcados).

## 1. Diagnóstico de causa raíz — DOS capas

### 1.1 Capa UI (el gap propiamente tal) — CONFIRMADO, CANONICAL_REPO

La razón del 503 SÍ viaja hasta el navegador y el hook SÍ la captura; el funnel card la
descartaba:

- api-server emite 503 con `reason` machine-readable en TODAS sus vías:
  `backend/api-server/src/routes/route-discovery-outcomes-api.ts` —
  `db_unavailable` (:262, pool null) · `rollup_backfilling` (:278-289, con
  `detail.retry_after_s: 30`) · `query_failed` (:351→:36-37, statement timeout 15s).
- El edge `proxy()` reenvía status Y body verbatim (`edge/worker/src/index.ts:459-473`,
  `c.body(body, upstream.status ...)` :473) — el JSON con `reason` llega al browser.
- El hook lo captura: `frontend/lib/hooks/useRouteDiscoveryOutcomes.ts:144-148`
  (`httpReason = b?.reason ...` — R8 "surface the api-server 503 reason verbatim").
- **El defecto**: `frontend/app/operations/components/RouteDiscoveryFunnelCard.tsx:42`
  (pre-fix) hacía `const { totals: outcomeTotals } = useRouteDiscoveryOutcomes(24)` —
  destruía SOLO `totals` y descartaba `status`/`unavailableReason`. Con 503, `totals`
  queda null → las etapas `outcomes resueltos`/`opportunities` (route-funnel.ts:95/:103)
  pintan `DASH` en silencio. Contraste intra-file: la etapa Reconciled SÍ mostraba su
  error (`recon.error` en CardDescription :80-83). Asimetría de honestidad, no falta de
  dato. Los OTROS dos consumidores del hook ya surfaceaban la razón:
  RouteDiscoveryOutcomesPanel.tsx:160-165 y RouteOutcomesAnalyticsPanel.tsx:244-252.

### 1.2 Capa servidor (¿por qué 503?) — ACOTADO a 3 candidatos, exacto INDETERMINABLE (R9)

Evidencia VPS (todo lectura, 2026-09-17 ~07:47-07:52 UTC):

1. **El endpoint está HOY sano**: `curl 127.0.0.1:8080/api/v1/route-discovery-outcomes/
   summary?hours=24` → **200** con datos reales (total 24h = 11,563,772 outcomes,
   opportunities = 258, 270 cartridges, top reason `missing_reserves` 1.78M).
2. **Latencia al límite**: el MISMO request logueado en api-server =
   `responseTime: 13184` ms — solo 1.8s de margen bajo el statement timeout de 15s
   (`RDO_STATEMENT_TIMEOUT_MS`, route-discovery-outcomes-api.ts:49). Un burst de carga
   empuja por encima → `query_failed` 503.
3. **Carga DOBLADA vs baseline documentado**: `route_discovery_outcomes` tiene
   **2,584,959 filas en la última hora** (psql SELECT count) vs el baseline 1.32M rows/h
   del header del propio route (:17-18, RDO-SUMMARY-503 2026-09-02). El head/tail raw
   edge scan de cada grouping (:202-211) barre proporcionalmente el doble.
4. **Rollup COMPLETO ahora**: `missing_24h = 0`, último marker 07:40 (cubre 5-min buckets
   al día). `rollup_backfilling` NO es el estado actual; fue plausible en la ventana
   post-restart (flota reiniciada 03:38Z per 02-VPS-REMAP; el top-up avanza 24
   buckets/REQUEST — si nadie polea, no converge).
5. **La razón exacta del 503 de ~06:20Z es INRECUPERABLE de logs (R9 discipline)**:
   container StartedAt 03:38:30Z pero la PRIMERA línea retenida es 06:55:00Z (logrotate
   5×10m; `paper_archiver.skip_rejected` ~30 l/s = LOGFLOOD ya fichado por el QA-WS
   §5.4). La ventana de la observación browse (06:20-06:26Z) está DENTRO de la región
   rotada — la ausencia de líneas 503 es artefacto, no evidencia (CLAUDE.md R9.1-R9.2).
6. **Hipótesis server-lag 27543 del charter**: MECÁNICAMENTE NO CONECTA con esta ruta —
   el SERVER-LAG del G-PIPE-1 mide el lag de `arbx:opps:detected`/`validated`
   (api-server/src/readiness/verifiers/g-pipe-1.ts), no toca `route_discovery_outcomes`.
   PERO es indicador del MISMO burst de carga que sí degrada esta ruta (pool api-server
   compartido + ingesta 2×). Reclasificación: la hipótesis de carga SOBREVIVE como
   contexto; el lag en sí NO es la causa.
   Veredicto causa raíz servidor: **`query_failed` (timeout 15s bajo ingesta 2× y
   latencia base 13.2s) = MÁS PROBABLE · `rollup_backfilling` (catch-up post-restart)
   = plausible · `db_unavailable` = descartable (PG healthy)**. Post-fix, la próxima
   ocurrencia se autodiagnostica: la razón queda visible en la UI.

## 2. Fix aplicado (working tree, marcador `// RDO-503-REASON-UI (2026-09-17)`)

1. `frontend/app/operations/components/RouteDiscoveryFunnelCard.tsx`:
   - :42-53 — destructura `status`/`unavailableReason` además de `totals`;
     `outcomesDown = STALE && totals===null` (STALE con totals previos ≠ down: la
     etapa sigue mostrando el último dato, no se degrada a guion).
   - :90-107 — CardDescription ahora renderiza (patrón espejo del recon error
     preexistente): `outcomes sink unavailable: {reason verbatim} — las etapas
     outcomes resueltos / opportunities quedan en guion honesto (upstream 503, razón
     verbatim; jamás un cero)`. Ambos warnings (outcomes + recon) pueden coexistir.
2. `frontend/features/route-discovery/RouteDiscoveryOutcomesPanel.tsx` (:9-14, :168-171):
   doc-drift adjacente del MISMO superficie de fallo — el alert STALE decía "Polling
   ... every 8s" siendo `POLL_MS = 60000` (useRouteDiscoveryOutcomes.ts:28, cambiado en
   RDO-SUMMARY-HANG). Corregido a 60s (texto visible al usuario durante el 503; daba
   expectativa de retry falsa). Sin cambio de comportamiento.

ZONA CERO (RULE 00): no se fabrica ningún dato; el fix SOLO hace visible la razón que
el upstream ya emitía. El dash honesto se conserva como valor de etapa.

## 3. Verificación

- `npx --no-install tsc --noEmit -p frontend/tsconfig.json`: 0 errores en los archivos
  tocados. Único error del árbol: `lib/hooks/useTokenIcon.test.ts(68,30)` TS2493 —
  PRE-EXISTENTE, archivo no tocado por mí (zona del browse gap 4.5, owner distinto).
- `npx --no-install vitest run app/operations/components/__tests__/route-funnel.test.ts`:
  **5/5 PASS** (modelo puro de etapas intacto — no lo modifiqué; el fix es view-only).
- git status: solo los 2 archivos marcados añadidos al diff preexistente. Cero
  commit/push/PR/deploy.
- Nota honesta: la renderización visual final (DevTools contra VPS) NO se ejecutó —
  el fix no está desplegado (NO-GIT) y el budget HTTP manual del agente se agotó en el
  curl de diagnóstico (1/5 usado aquí + lecturas ssh). Verificación post-deploy
  propuesta para WO-06/operador: abrir /operations con el sink caído (o simular 503)
  y confirmar la línea amarilla con `reason`.

## 4. Presupuesto y límites

- HTTP manual: **1/5** (curl diagnóstico al api-server). Navegador: 0 journeys.
- VPS: solo lectura (docker logs/inspect, psql SELECT, curl GET). Cero mutación.
- Cero cargo/build Rust. npm limitado a tsc/vitest de frontend (charter fixer: verificación
  local permitida; no se tocó el target/ compartido §36.4).

## 5. Sincronía de mesa redonda

- Construye sobre: `BROWSE-Operador-de-mesa-...md` §4.2 (gap original) ·
  `BROWSE-QA-de-WebSocket-...md` §4 y §5.4 (LOGFLOOD api-server que rotó la evidencia —
  mi R9 se apoya en su hallazgo) · `02-VPS-REMAP-20260917.md` (restart 03:46Z).
- No contradice ningún hallazgo previo. Extiende el §3 del QA-WS: el FEED-SCHEMA-01
  (block_number string) es OTRO defecto del mismo ecosistema edge/api-server — NO
  tocado aquí (owner: quien fiche el edge worker, WO-04).
- Para WO-04 (fichas TS): este par hook+card es ejemplo canónico de "R8 en wire,
  perdido en view" — patrón a auditar en los demás consumidores de hooks con
  `unavailableReason`.
- Para WO-06/operador (gated): la ingesta 2.58M rows/h DOBLÓ el baseline del diseño
  RDO-SUMMARY-503; el margen 13.2s/15s hará recurrente el `query_failed` bajo burst.
  Remedios posibles (NO aplicados, requieren diff+gate): subir ROLLUP_TOPUP_BUCKETS /
  cron de top-up externo (desacoplar convergencia de polls UI) o cachear más agresivo
  en edge. El 60s POLL_MS + 15s TTL edge ya amortigua; documentado como riesgo.

## 6. Claims de archivo (conflictos)

- `RouteDiscoveryFunnelCard.tsx` y `RouteDiscoveryOutcomesPanel.tsx`: superficie
  frontend (WO-04 AÚN NO INICIADO según board) — sin conflicto con claims de pares.
- `useRouteDiscoveryOutcomes.ts`: NO tocado (el hook ya era correcto).
- Contaminación residual detectada, no tocada: `features_backup/route-discovery/
  RouteDiscoveryOutcomesPanel.tsx` es un backup con el texto "8s" — dir backup,
  deliberadamente intocado (no es superficie viva).

## Clasificación de afirmaciones

- Cadena 503→edge→hook→card (causa raíz UI): CANONICAL_REPO (citas file:line arriba).
- Endpoint sano hoy, responseTime 13184ms, rollup missing=0, raw 2.58M/h: PRIMARY_SOURCE
  (curl + docker logs + psql SELECT, 2026-09-17 ~07:47Z, reproducibles).
- Razón exacta del 503 de 06:20Z: UNKNOWN (ventana rotada, R9) — acotada a
  {query_failed (más probable), rollup_backfilling}.
- Hipótesis server-lag como causa directa: REFUTADA mecánicamente (ver §1.2.6);
  sobrevive solo como proxy de carga.
