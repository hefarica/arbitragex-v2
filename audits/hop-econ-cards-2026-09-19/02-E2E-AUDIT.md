# Auditoría E2E — 2026-09-19 18:55Z (orquestador, read-only)

## Veredicto: PIPELINE VIVO, PRODUCTO VACÍO

**Infraestructura: VERDE**
- 24 contenedores Up, 0 unhealthy / exited. Stack completo (searcher, api-server, edge,
  frontend, observabilidad thanos/loki/grafana, vault, relays-client, anvil).
- Dominio https://arbx.ape-tv.net → 200 (~1s). `/api/health` → ok, uptime 15h.
- Deploy VERAZ: VPS SHA 62a9d379 == origin/main (merge #588 price-resilience; contiene el
  histórico de feat/s1-fee-dual-unit — local HEAD cdb4c890 está contenido en main).
- Redis stream: 10.003 entries. PG: 35,4M oportunidades, última detectada hace 19s.
- Searcher en hot-loop real: evaluaciones activas + v3_source_priced con sqrt_price_x96 real.

**Economía del pipeline: DOMINADA POR UN SOLO GATE**
Últimas 6h: 703.755 detecciones → 100% rejected. 0 viables.
| rejection_reason | count | % |
|---|---|---|
| v3_quote_unavailable | 583.407 | 82,9% |
| spot_product_le_one | 64.794 | 9,2% |
| non_positive_profit | 29.913 | 4,3% |
| single_pool_no_spread | 22.812 | 3,2% |
| unknown_token_price | 1.734 | |
| TokenNotAllowed:0x7777…116c | 1.095 | |

**Síntoma de usuario**: `/api/opportunities/live` devuelve 200 con `count: 0` — el dashboard
está vacío NO por gate de frontend sino porque no hay viables que servir (fail-honest OK).
Sin viables no hay cards que mostrar → WO-03/WO-04 del gang quedan sin dato real hasta
desbloquear `v3_quote_unavailable`.

**Riesgos**
1. `v3_quote_unavailable` (83%) apunta directo a la anomalía abierta "segundo path de quote
   no instrumentado" — es EL bloqueante económico del sistema hoy.
2. Disco VPS 93% (11GB libres) — cerca del umbral de deploy guard (15GB). Riesgo de
   DISK-FULL repetición (precedente 2026-09-19 madrugada).
3. `/api/runtime-status` → not_found en dominio (posible path distinto o endpoint no montado
   en edge) — verificar contract de la familia arbx-runtime-status.

**Conclusión para el gang (run_00a4aa…)**: el buffet de gates WO-02 debe priorizar el path
de quote V3 (583K rechazos/6h) sobre cualquier cosmética de cards — las cards con economía
por hop son necesarias pero mostrarán "—" hasta que existan viables.
