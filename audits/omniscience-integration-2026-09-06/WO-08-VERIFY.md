# WO-08 — VERIFY (re-despacho del reviewer tras 429)

- **Reviewer:** ecc:react-reviewer · 2026-09-06 · kind: verify · READ-ONLY (cero ediciones de código, cero git).
- **Reporte completo:** `audits/omniscience-integration-2026-09-06/WO-08-PEER-REVIEW.md` (este archivo es el registro corto para el board).

## VEREDICTO: **APPROVE-WITH-NOTES**

## Matriz de verificación

| # | Ítem del charter | Resultado | Evidencia clave |
|---|---|---|---|
| 1 | Semántica: ningún camino LIVE con subsistemas CONNECTING/degradados; fail hacia el MENOS optimista | **PASS** | `RuntimePostureBar.tsx:107-131` (LIVE exige todos LIVE/POLLING; peor-token gana); caso exacto §5 doble-testeado: `test:151-159` (unit) + `test:318-338` (regresión de barra) |
| 2 | R1 hydration: Date.now/window/navigator solo en useEffect; suppressHydrationWarning jamás | **PASS** | Grep cero hits en scope de render; único useEffect `:383-408`; test doble render byte-idéntico `test:340-344`; SSR del dominio público sirve el snapshot inicial honesto (1 request) |
| 3 | Contrato con `backend/api-server/src/websocket.ts` y writers | **PASS** | rooms reales `websocket.ts:297-300` (route_discovery, emisión `:811`, evento `route_discovery.tick` doc `:44-48`), `:313-324` (runtime_ack capability re-check, broadcast `:428`); REST pairs/anchor reales `catalog-slices.ts:162-176` / `quote-slices.ts:44-58` |
| 4 | Re-ejecución vitest 25/25 (18+7) y tsc exit 0 | **PASS** | Ejecutado por este reviewer: `25 passed (25)` EXIT=0; `tsc --noEmit` EXIT=0. Test diff = +89/−0 (18 intactos estructuralmente) |
| 5 | Radix/accessibility | **PASS** | Sin Radix (lección proxy N/A); `role="status"`+aria-label `:411-414`; iconos aria-hidden; 0 unlabeled |
| 6 | NO editar código | **PASS** | Working tree intacto (solo estos reportes añadidos en `audits/`) |

## Notas (no-bloqueantes)

1. **MINOR pre-existente:** `projectChannel` defaultea a LIVE ante `status` desconocido (`RuntimePostureBar.tsx:89`) — fail-open contenido hoy por la unión cerrada (`realtime-slices.ts:63-68`) y un solo writer; switch exhaustivo como follow-up.
2. **Brecha del provider (declarada por el applier, CONFIRMADA):** `markFresh` jamás promueve `status` (`ArbxRealtimeProvider.tsx:72-76`) y `connect` fuerza `live` sin payload (`:123-124`) ⇒ chip socket en CONNECTING persistente honesto (dirección pesimista correcta); follow-up badge+provider juntos.
3. **TRIVIAL:** aserción vestigial `not.toContain("not connected")` (`test:332`); `role="status"` region aria-live pre-existente (ruido potencial para lectores de pantalla).

## Reglas duras

RULE 00/R8 ✔ · §32/§33 read-only ✔ · cero git/deploy ✔ · presupuesto dominio 1/5 ✔ · defecto CRITICAL: **ninguno**.
