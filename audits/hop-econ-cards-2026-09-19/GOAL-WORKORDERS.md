# BOARD — hop-econ-cards-2026-09-19

**/GOAL (operador, 2026-09-19)**: Las cards de oportunidades deben mostrar por hop/leg:
precio y símbolo de cada token, cuánto capital USD entra y sale, y en qué hop/leg se gana
o se pierde. Todo bien integrado. Además: buffet de investigación sobre si algún gate impide
que la información correcta llegue al dominio en vivo. E2E: ver oportunidades fluir.

**Perímetro**: repo local + lectura VPS (ssh `arbx`) + dominio vivo `https://arbx.ape-tv.net`
(sólo lectura, journeys). CERO commits/push/deploy sin gate final del operador.
Capital/flips = operador-only (§34). RULE 00/R8 sin excepciones.

**Contexto inyectado a todos los agentes**:
- WO-LEGS-TRIANGULAR-01 YA aterrizado (commit ab880676): per-leg ledger triangular desde
  re-evaluación post-clamp — el dato de economía por hop EXISTE en backend.
- Torre de precios soberanos (#584-587 landed): on-chain primero + mirror TS + delta-streaming.
- S1 fee dual-unit LANDED (cdb4c890): fee_fraction() divisor por tipo de pool.
- 2 anomalías abiertas del 2026-09-17: (a) 93 claves sin reparar, (b) SEGUNDO path de quote
  no instrumentado.
- Runs previos (run_b3d65e1e… failed + run_735380fc…) WIPED por restart del gateway; sesión
  "Hermes Local - ccr-glm53" detenida por orden del operador. REAPERTURA 2026-09-19 ~09:23:
  **run_9c099402277a4505820fdfad999da7bb RUNNING** (veredicto a recolectar).
  Contrato API Hermes tatuado en memoria: hermes-local-api-contract.md.
- Branch actual: feat/s1-fee-dual-unit-20260918.

## Work Orders

| WO | Título | Dueño | Claims de archivo | Estado | Gate |
|---|---|---|---|---|---|
| WO-01 | Mapear el dato de economía por hop end-to-end: dónde nace (per-leg ledger, quote), qué campos lleva (token, símbolo, precio, USD in/out, P/L por leg), dónde se pierde en el path searcher→redis→PG→api-server→edge→WS→frontend | ecc:code-explorer | read-only | **DONE (orquestador+code-explorer 2026-09-19)** | Mapa: ledger = RouteMetadata (shared-rs/candidates.rs:154-169, attach :203); pasa VERBATIM opportunities-live.ts:288,622-627 (+leg_symbols :577); FE preserva en types.ts:501-550. Sólo filas Sized llevan ledger (R8 by design); Kelly rescale lo nullea (size_optimizer.rs:3404). |
| WO-02 | Buffet gate-investigation: enumerar TODO filtro/gate/serializador/proyección que pueda dropear o degradar campos entre backend y dominio vivo (incl. 93 claves sin reparar + 2º path de quote no instrumentado + reshapes edge + allowlists ENABLED_STRATEGIES) | ecc:code-explorer + ecc:silent-failure-hunter | read-only | OPEN | lista de gates con evidencia activo/inactivo por capa |
| WO-03 | Diseño + implementación Cards hop-econ en frontend: por leg mostrar símbolo, precio, USD in, USD out, P/L (verde/rojo), alimentado por los campos reales del WS/API (ZERO MOCKS — si el campo no llega, mostrar honesto "—") | frontend builder (orquestador) | OpportunityDetailTabs.tsx tab Ledger extendido + __tests__ | **DONE** | typecheck ✅ lint ✅ vitest 25/25 ✅ next build exit 0 ✅ (snapshot UI = tests R1 renderToStaticMarkup). Rate por hop = ratio BigInt exacto del ledger; USD in/out sólo token base (ancla simulated_amount_in_usd÷amount_in_wei, §40-style); tokens intermedios "—" (R8); P/L cierre verde/rojo con valorización USD cuando el ancla existe. |
| WO-04 | Verificación E2E en dominio vivo https://arbx.ape-tv.net: journeys de navegador real — opportunities fluyendo, WS vivo, cards renderizando economía por hop, evidencia screenshots/console/network | dapp-browser-verifier | read-only (máx 5 req HTTP manuales) | BLOCKED(by WO-03 deploy o build local) | PASS/FAIL con evidencia |
| WO-05 | Recolectar veredicto Hermes run_9c099402277a4505820fdfad999da7bb y contrastarlo con hallazgos del gang (cross-examination) | orquestador | — | OPEN | veredicto registrado + discrepancias R8 |

## Convenciones
- Todo agente LEE este board antes de trabajar y lo ACTUALIZA al terminar su WO.
- Desacuerdos: se registran, no se ocultan (R8: cifra vs cifra, sin promediar).
- Cero commit/push/PR/deploy sin gate final del operador (protocolo no-git-until-final-gate).
