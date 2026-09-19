# E2 — Forense VPS logs/infra (stall 01:16:25 UTC)
> Agente: vps-forensics · Ventana 01:00–01:35 UTC + checks a 02:04–02:10Z · Solo lectura (docker logs/inspect/stats, redis INFO/XLEN, psql SELECT, df/free) · Salida completa en transcript (task a888b301f007b3002)

## Infra (HP-06 rule out)
- df: /dev/sda1 150G, 103G usado, 42G libres (72%). RAM 15Gi (11Gi avail), swap 0B usado.
- 4 contenedores clave Up 3h (healthy); searcher StartedAt=2026-09-15T22:57:09Z — ARRANQUE PRECEDE al incidente, sin restart en la ventana.
- docker stats: searcher 197.8MiB/0.07% CPU · sim-ctl 3.9MiB/0.03% · postgres 3.77GiB · redis 716.9MiB — CPU en piso = "no hay trabajo", no busy-loop.
- Redis: aof_enabled=1, aof_last_write_status=ok, rdb ok. CERO MISCONF. used_memory 687M.

## Logs searcher-rs (ventana retenida desde 00:10:14Z; 26,305 líneas 00:55–01:35)
- Histograma top: cartridge.active_opportunity_detected 6,048 · v2.emitter.input 4,598 · alchemy_failed 2,320 (429 crónico, ~58/min IDÉNTICO antes/después) · v2.engine.output 1,672 · pool_sync.multicall 955 · route_scanner.done 200 · route_discovery.tick 200 · scanner.heartbeat 40 · scanner.subscription_error 1 (01:08:25 "pending tx stream ended" → re-suscripción OK 100 ms después; detecciones continuaron 8 min más → NO causal; drops 01:24/01:38/02:03 auto-recuperados <1 s).
- POST-stall (01:16:30+): DESAPARECEN active_opportunity_detected / active_eval_* / v2.emitter.input / v2.engine.output / optimizer / orchestrator.intent_received / impact.resolved. SIGUEN VIVOS: pool_sync (445 multicalls ok), route_scanner (92), route_discovery (92), heartbeat, gas_oracle, price_worker, cartridge.registry_published.
- **route_discovery.tick: routes=0 durante TODA la ventana, incluso antes del UPDATE** (00:55:02 pools=282 edges=382/91 routes=0).
- **Cambio de universo entre ticks 01:15:02→01:17:02: edges 382/91 → 374/95 (−8 built, +4 rejected); pools_total 282 constante.**
- pool_sync set: 01:15:30 v2:118+v3:124 (=242 = 237+5 XEN) → 02:10:42 v2:116+v3:121 (=237, exactamente −5). dirty_marked:0 dirty_unchanged:all en TODOS los ticks (pre y post): el universo restante no cambia reserves on-chain.
- Mecanismo pre-stall: pending tx → cartridge.active_eval_enter (tx_hash) → v2.impact.resolved impacted_pools:5 (= los 5 pools XEN) → active_count:271 cartridges → detecciones. Última detección 01:16:25.399. Sin panics, sin MISCONF, sin backoff infinito. 11 ERRORs en 40 min = deserialize WS + reconnect crónicos no correlacionados.

## Detección ¿reanudó?
SELECT max(detected_at), count(10min) @02:04Z → NULL, 0. **NO reanudó.** Buckets 5-min: 2,400-2,440 estable 23:10→01:10 → 768 (01:15-20, corta 01:16:25) → 0 desde 01:20. Cliff exacto.

## Redis streams
- XLEN arbx:opps:detected = 10,003 (trim ~10k). Grupos enricher/paper-archiver-g0/selector-g0 con last-delivered-id = última entrada (al día; lag 11k-29k = artefacto del trim). pending 2/2/6,050.
- **Composición del stream retenido (00:56→01:16): 10,003/10,003 con token_in=XEN (100%)**; ~49 cartridges × 204 c/u (fan-out del mismo evento).

## Freshness reserves Redis
- Keyspaces reales: arbx:pool_reserves:1:* (116 keys) y arbx:v3_slot0:1:* (119) — NO existen arbx:reserves:*/arbx:slot0:*.
- TTL=18 en TODAS las muestreadas (pool_sync VIVO, refresca universo no-XEN cada ~12 s).
- **Las 5 XEN: TTL=-2 (keys inexistentes) en ambos namespaces** → pool_sync las eliminó del set en el refresh ~01:16:30. Único rastro XEN: arbx:tokens + token-icons.

## sim-ctl
20 líneas desde arranque, última 23:46:32Z (pel_observed + 2 ghost dead-letters) — silencio PREVIO al incidente, no correlacionado (logger baja frecuencia; grupos al día).

## Veredicto E2
- **H2 CONFIRMADA**: 100% del stream era XEN; motor = pending-txs spam → impact sobre exactamente los 5 pools XEN → 271 cartridges. Refresh aplicó is_active=false ~01:16:30 (set 242→237, keys purgadas) y la detección paró el mismo segundo. Fail-honest por diseño.
- **H1 DESCARTADA**: el searcher procesó el cambio limpio (universo re-leído, edges recalculados 382→374, sync reducido, claves purgadas; pipeline downstream ciclando).
- **H3 DESCARTADA**: infra sana; 429 con tasa idéntica pre/post; firehose drops auto-recuperados sin coincidencia con 01:16:25.
- Indeterminados: pending_received contador roto (0 pre y post); trigger exacto del refresh (~11.5 min post-UPDATE, sin log explícito); pools_total=282 vs 242/237 (filtro interno distinto); estado runtime del battery post-refresh.
