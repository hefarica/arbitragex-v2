# T29 — Protocolo M1-M6 · reporte · attempt f4ec9222

> 2026-10-14 01:40–02:15 UTC · 100% read-only · PAPER ON (killswitch=false en cada lectura de status).

## M1 — ERROR RATE n≥200 por endpoint: **VERDE — 1400/1400 HTTP 200**

| endpoint | n | dist | no-2xx | ventana |
|---|---|---|---|---|
| /api/paper-mode/state | 200 | 200×200 | 0 | 01:44:50Z |
| /api/readiness/decision | 200 | 200×200 | 0 | 01:46:39Z |
| /api/readiness/steps | 200 | 200×200 | 0 | 01:48:16Z |
| /api/readiness | 200 | 200×200 | 0 | 01:52:37Z |
| /api/strategies/runtime-status?chain_id=1 | 200 | 200×200 | 0 | 01:54:39Z |
| /api/scanner/heartbeat?chain_id=1 | 200 | 200×200 | 0 | 01:56:19Z |
| edge /api/opportunities/live (params exactos) | 200 | 200×200 | 0 | 02:05:31Z |

- **C1 respondido**: error-rate HOY = 0/1400 = 0.0%, **CI95 [0%, 0.21%]** (regla de 3/n). El
  baseline t16 (n=20, 0%) tiene CI95 [0%, ~15%] — la nueva medición ACHICA el intervalo ~70×:
  "el cluster no está activo ahora" deja de ser CLAIM para la ventana medida.
- Timestamps por request registrados en tandas (m1_tanda{1,2,3}.txt); lista de no-2xx VACÍA.

## M2 — CORRELACIÓN 500↔RECONNECT ±10s: **INDETERMINADO (declarado, con vía)**

- Durante los 25min de M1: **cero 500s** ⇒ la correlación no es medible hoy (cluster inactivo).
- El field-log del operador NO tiene timestamps (DECLARADO en el propio archivo) ⇒ no sirve para
  ±10s. Vía para cerrarlo: (i) captura con timestamps del browser del operador la próxima vez que
  el storm aparezca (console.log con Date), o (ii) access-log por-código en api-server (hallazgo
  menor de t28), o (iii) reproducir el storm controladamente.
- **Indicio a favor de UN-evento (no prueba)**: el 503×1 de simulate YA se explicó como
  `searcher_api → searcher-rs /route 404/unreachable` (M5 abajo) — un recurso del searcher cayendo
  durante su recreate: MISMA CLASE de causa (ventana de recreación) que la hipótesis del storm.
  Si el cluster 500 comparte esa ventana, es UN evento. SIN timestamps no se firma.

## M3 — SMOKE DE RUTAS contra producción: **VERDE** (subsumido en M1: n=200 ≥ 50 por endpoint,
distribuciones 200-only arriba). C4 respondido: las rutas del control-plane responden correctas
HOY — los 500 del operador NO son artefacto persistente del rollback.

## M4 — SMOKE contra SHADOW (v4): **NO EJECUTABLE como smoke directo — DECLARADO con evidencia indirecta**

- El stack efímero aislado (imagen a9b3ef50 + sleep) NO sirve control-plane: los endpoints del
  cluster viven en api-server/edge, NO en el searcher. El binario que se swap-ea (searcher) no
  cambia esos endpoints.
- Evidencia indirecta disponible: durante TODAS las ventanas con v4 corriendo (t21 W1/W2 18:2xZ;
  t24 PRE/POST 19:5x-20:1xZ) los GETs medidos a /api/status y /opportunities/live dieron 200.
- **Veredicto M4**: los 500 no son atribuibles al binario searcher (ni viejo ni v4): el fix de
  CAUSA no viaja en R-<ts2> como cambio de searcher. SI la causa resulta el storm del WATCHDOG
  FRONTEND legacy (M2 pendiente), el fix (watchdog re-emit v4) SÍ viaja en R-<ts2> — doble motivo
  de prioridad del operador.

## M5 — SIMULATE RUTA vs MÉTODO: **MAPA COMPLETO (m5_mapa_completo.txt + t28)**

| método/payload | código | causa (del body) |
|---|---|---|
| GET (id expirado) | 404 | not_found (max_age 300s) |
| GET (id vivo) | 404 | — (ruta solo POST/OPTIONS) |
| PUT / DELETE | 404 | método no registrado |
| OPTIONS | 204 | preflight OK |
| POST sin body | 400 | invalid_body (faltan route_source+id) |
| POST route_source inválido | 400 | **invalid_route_source — ENUM: {pg_metadata, searcher_api, simctl_lookup}** |
| POST simctl_lookup | **501 ×10/10** | real_sim_unavailable: "SIM_BACKEND!=revm — real multi-step REVM path requires SIM_BACKEND=revm" |
| POST pg_metadata | **501** | "Route source 'pg_metadata (A1)' should be enriched by api-server before reaching sim-ctl; no candidate provided" (gap de pipeline documentado) |
| **POST searcher_api** | **503** | **searcher_route_unavailable: "searcher-rs /route endpoint unreachable or returned 404"** — ¡EL 503×1 DEL OPERADOR, EXPLICADO! |

- **El 503×1 ya no es misterio**: el operador usó route_source=searcher_api una vez y el /route
  del searcher estaba 404/unreachable — coherente con la ventana de recreate del searcher
  (nuestros swaps/rollbacks 18:2x, 19:5x-20:11Z). Determinista por route_source.
- Los 501×3 del operador: mismo real_sim_unavailable (id distinto, misma causa de config).

## M6 — M1 vs BINARIO PREVIO en ventana simétrica: **M1 CORRIÓ contra el previo** (producción
actual = dfd71012e1dc). Ventana simétrica bajo v4: requiere swap (gated por EXC-001/decisión
operador) — PENDIENTE declarado; evidencia parcial: GETs 200 durante ventanas v4 (M4).

## REGISTROS OBLIGATORIOS (hechos)

- **(a) §B DESIGN DEFECT + ENMIENDA**: hallazgo registrado (rollback verificaba liveness no
  rutas — C4 del operador) y enmienda integrada al **runbook V8 del ESCALERA_REPORT** (todo
  rollback futuro incluye smoke de rutas ≥50 GETs/endpoint con distribución; no-2xx reabre
  incidente) **y a WATCH-V7-001** (mismo paso 5 del runbook).
- **(b) ADM-WATCHDOG → OBSERVADO**: fila ADM-7 del A7 actualizada con el OBSERVADO en producción
  (storm ×6 ventanas, contador SIEMPRE 1/3 — cada recarga reinicia; +1 CLOSING/CLOSED con stack
  doWrite→send→flush→_sendPacket; +~14 bfcache) citando el field-log como ℳ; A7 re-hash en cadena
  (nuevo sha f6f28d866e031f8983b985b3b7f290f079314cd2d1c1c070f31a2c90102c4162, fila ℛ
  actualizada); console-20261013-operator-field-log.md registrado en ℛ (productor operador/capitán,
  verificador t17/t7).

## AUTORIZACIÓN DE REPARACIÓN (criterio del operador)

M1 VERDE ∧ M3 VERDE ⇒ **NO se cumple la condición de reparación** (error-rate 0% con n adecuado;
rutas correctas) ⇒ **cero reparación** (correcto: no hay nada activo que reparar). El cluster 500
queda como TRANSITORIO-INFERENCIA (t28 V2) con M2 pendiente de timestamps y la vía de cierre
documentada arriba.

## ℛ

85 filas, sha 21c2b58c6463683dc0f3f9f99f247a818957d3ad5cd807536e0a0bd104a2c9a3 (fila field-log ℳ
operador + A7 re-hash + 4 evidencias m1/m5 + reporte).
