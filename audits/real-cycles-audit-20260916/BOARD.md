# Real-Cycles Audit — BOARD (2026-09-16)

Programa: auditoría repo + entrega dapp honesta (ciclos reales / ganancias reales).
Skill: arbitragex-omniscience (§0 LEARNINGS leído) + arbx-live-engineering (VPS_READ_ONLY).

## GOAL
Auditar estado actual del repo y entregar la dapp mostrando ciclos completos de
arbitrages reales con ganancias reales — bajo RULE 00/R8 y §32/§34.3/§34.5.

## STATUS: FASE AUDITORÍA CERRADA (7/7 WOs) + DELTA POST-#574

## WORK-ORDERS

- [x] WO-01 Repo local vs origin/main: branch fix/567-canonical-plan-consumer@0e72a7cc
      SIN merge a 22:40Z; PR #574 abierto. — DELTA 23:0xZ: PR #574 MERGEADO por
      sesión paralela, main=f380a8bf deployado en VPS (searcher 21:54Z, sim-ctl 22:20Z).
- [x] WO-02 Mapa VPS (delta post-#573/post-#574): 24 contenedores; 6 core healthy;
      deploy-lock vivo; disk_guard roto (0644); sin backups. Imagen searcher 1d8ce6d2.
- [x] WO-03 Funnel detección→sim→paper: PRE-#574: 76 opps/9h, todas rejected
      (v3_quote_unavailable 60/76). — DELTA: burst 340 opps/7h (últ. 21:53Z);
      razones: v3_quote_unavailable 215, non_positive_profit 67, spot_product_le_one 37.
- [x] WO-04 Ledger paper vs sim passed: 598,878 paper runs TODOS rejected (R-0001);
      0 join con sims passed; executions=0 EN TODA LA HISTORIA; solo 1 paper run post-01-sep
      (expected=459.88, actual=NULL).
- [[x]] WO-05 Gates G1-G8: 0/8 con artefactos reproducibles; readiness dapp 0/4 NO-GO.
- [x] WO-06 Dapp viva verificada por browser (arbx.ape-tv.net): muestra la verdad R8
      (0 asimetrías, sin ciclos completos §44, /executions "No executions yet",
      /paper/history filas non_positive_profit). Conciliación browser↔PG 100%.
- [x] WO-07 Camino a ciclos reales (00-SYNTHESIS.md): merge 567 → fix quote V3 →
      primera sim passed → G1 → G4-G6 → G7/G8 canary §34.5.

## DELTA CRÍTICO POST-#574 (22:40–22:45Z, verificado ssh+psql+docker logs)

- PR #574 mergeado: main=f380a8bf; VPS deployado. PASO 1 del camino CERRADO.
- Burst de detección 19:00–21:53Z: 340 opps (v3_quote_unavailable 215).
- Post-restart searcher (21:54Z): routes_found=0 sostenido, cycles_found=0,
  capped=true, 374 edges/238 pools, dirty_seeds=64, drain_drained=235.
  route_scanner: cycles_anchor_rejected no computado en el done más reciente →
  ver arriba; provenance_rejected=0; enumeration_ms=6.
- sim-ctl nuevo (22:20Z): booted OK, consumer g0 en arbx:opps:validated, PEL
  pending=2 con reclaims de entries stale (ids 1789595606708-0/711-0). Anvil fork
  block 25989797 (≈33k blocks atrás = fork-cache, no head).
- Precio: Alchemy 429 masivo (chunk 5, ~40 warns/s en ventana 60s), Chainlink 5/146,
  cache_misses 141. Alchemy key en logs (alch_7Yw8…) — key presente, quota agotada.
- pool_sync: v3_sqrt_overflow en 2 pools (0xb138…7e66, 0x6577…5423) — sqrtPriceX96
  uint160 clamp a u128; representa mal esos pools para siempre.

## PRÓXIMOS DEFECTOS EN LA COLA (ver análisis operator-side y 00-SYNTHESIS)
1. Discovery muerto post-restart: routes_found=0 con grafo vivo (374 edges) —
   upstream del quoter V3; sin esto no hay candidatos nuevos.
2. Alchemy 429 masivo → sin USD verificable para el funnel económico.
3. v3_sqrt_overflow (2 pools) — representación V3.
4. v3_quote_unavailable sigue siendo el mayor bucket de rechazo (215/340).
5. paper accounting #570 (predicted vs observed) — deuda documentada.

## VEREDICTO
La dapp está viva y honesta (R8). "Ganancias reales" NO existen en ninguna capa:
executions=0, sims passed=0 en toda la historia. Requiere camino 6 pasos
(00-SYNTHESIS.md) + gates G1-G8. Fabricarlas viola RULE 00 y precedente GATE-2.
