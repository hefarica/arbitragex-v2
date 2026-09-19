# BOARD — E2E-CARDS-20260919

**/goal (operador, 2026-09-19 03:45Z):** "necesito que se muestre el arbitrage end to end con
valores en las cards y todos sus campos listos para la ejecución".

Criterio de aceptación: en la DApp pública (https://arbx.ape-tv.net) las cards de
oportunidad muestran, con datos REALES del pipeline (RULE 00 / R8), el arbitraje completo:
token_in/out con símbolo+decimals, dex_a/dex_b reales (no "unknown"), amount_in > 0,
expected/net profit, gas/cost breakdown, ruta (legs con pools), y los campos que el
terminus de ejecución (relays-client) exige — chain, block, calldata/plan, financing,
signer role, límites. Modo PAPER_SHADOW (mode-invariant §34): sin capital, sin broadcast.
Flips de modo, deploy y VPS-write = operador.

## Ground truth (orquestador, 2026-09-19 03:30-03:45Z, VPS read-only)

| Hecho | Evidencia |
|---|---|
| VPS disco 100% → PG PANIC; builder prune 21.4GB → 87% | df, docker logs postgres |
| api-server + selector-api parados 21:44Z→03:35Z (estado Created por deploys fallidos); arrancados | docker ps -a; edge 500→200 |
| Auto-Deploy de main 62a9d379 FAIL: conflicto nombre contenedor frontend viejo; `/tmp/arbx-deploy.lock` vivo | GH run 35338355620; /tmp/deploy341.log |
| Imágenes en ejecución = 09:00Z (S1 #590 mergeado 10:40Z NO desplegado) | docker inspect |
| 24h opportunities: total 3,078,226 · non_rejected **0** · with_net 10,737 · with_amount>0 2,585,685 | psql |
| Rechazos última hora: v3_quote_unavailable 88,903 · spot_product_le_one 9,626 · non_positive_profit 4,011 · single_pool_no_spread 3,055 · missing_reserves_pool_b 1,956 | psql |
| Card muestra: amount_in_wei "0", dex_a "unknown", dexes_used ["unknown"], profit null, route_metadata.dex_adapters ["unknown"×4], simulated_* null, paper_status paper_rejected | GET /api/opportunities/live?limit=2 |
| Modo: ARBX_TRADE_MODE=paper, ORCHESTRATOR v2, CARTRIDGE active | env searcher |
| Sink RDO sin consumir 21:45Z→03:35Z: hueco de telemetría irrecuperable (stream cap 1M, lag 5.5M) | XINFO GROUPS |
| Hermes local 8642 = 401 (key drift tras restart VS Code); run S1 perdido | curl |

## Anomalías abiertas heredadas (memoria 2026-09-17/18)
- 93 claves sin reparar (e2e-demo-20260917).
- SEGUNDO path de quote no instrumentado.
- Página '0 viable / 0 total' cuando quote/anchor 503-ea.
- v3_quote_unavailable = cuello del embudo (cobertura de datos, no transporte: cache_neg_hit 74%).
- ENABLED_STRATEGIES allowlist stale (UPDATE aplicado 09-17; WO-CONFIG-DERIVE-01 fichado).

## Work orders

| WO | Título | Dueño | Claims de archivo | Gate | Estado |
|---|---|---|---|---|---|
| WO-01 | Mapa campo-a-campo: card UI ↔ API ↔ PG ↔ emisor Rust ↔ requisitos relays-client. Denominador de "todos los campos listos para ejecución" | code-explorer | read-only | lista con fuente por campo | OPEN |
| WO-02 | Por qué dex_a/dexes_used/dex_adapters = "unknown": trazar find_router/catálogo → route_metadata en emisión | rust-reviewer | read-only | causa + fix propuesto con test | OPEN |
| WO-03 | amount_in_wei=0 en cards: ¿SizeOptimizer no corre en path rechazado o no se persiste? | rust-reviewer | read-only | causa + fix | OPEN |
| WO-04 | v3_quote_unavailable 89K/h: clasificar por pool/fee_tier con liquidity() on-chain; separar muertas (RULE 00: expulsar) de cotizables (backfill slot0) | data-analytics | read-only VPS/RPC | tabla pool×razón | OPEN |
| WO-05 | spot_product_le_one + non_positive_profit + single_pool_no_spread: ¿gates correctos o falsos negativos? vector independiente por gate | math-validator | read-only | veredicto por gate | OPEN |
| WO-06 | Frontend: qué renderiza la card para null/0/unknown (fail-honest vs vacío mudo); campos que faltan en OpportunityCard para ejecución | frontend-architect | read-only | diff propuesto | OPEN |
| WO-07 | Deploy roto: contenedor frontend huérfano + lock; receta de recuperación para el operador (sin ejecutar) | devops-platform | read-only | runbook | OPEN |
| WO-08 | Browser: dar fe de lo que ve el humano hoy en /opportunities (screenshots, consola, WS) — máx 5 requests | dapp-browser-verifier | read-only | PASS/FAIL por campo | OPEN |
| WO-09 | Cross-examen de WO-01..08 + síntesis: plan de PRs en orden (un ID por PR, §37) y lo operador-gated | cross-examiner | — | 00-SYNTHESIS.md | OPEN |

Reglas: RULE 00 (cero mocks/hardcode), R8 fail-honest, §34.4 pregunta canónica, no-git hasta gate del operador, VPS read-only (ssh arbx), presupuesto 5 requests/agente al dominio público.
