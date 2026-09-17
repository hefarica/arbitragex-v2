# Root-Cause: v3_quote_unavailable (60→80% del embudo) + Freeze de detección

Fecha: 2026-09-16 · Sesión: real-cycles-audit-20260916 (continuación) · Modo: §32 read-only + mantenimiento restaurativo

## Resumen ejecutivo

Dos defectos independientes bloqueaban el embudo a sim passed:

1. **FREEZE de detección (13:21:30Z → 21:42:05Z, ~8h20m)**: el searcher dejó de
   recibir pending txs (`pending_received=0`) con subscripción "sana". Restart
   del contenedor restauró el flujo (581→664/min). Estado degradado del proceso
   (WS/ConnectionManager), no defecto de código.
2. **`v3_quote_unavailable` = auto-DDOS RPC**: 103/109 fallos reales son
   `"v3 quote rpc failover exhausted: all providers unhealthy for chain_id=1"`.
   Cada intent genera quotes V3 individuales (batch=1) contra 9 proveedores
   públicos; los circuit breakers abren en cascada (429/403) y TODAS las
   quotes fallan → rechazo masivo `v3_quote_unavailable`.

## Evidencia (verificada, reproducible)

### Freeze
- Redis stream `arbx:opps:detected` last-generated-id `1789564890184-0` = 13:21:30.184Z, XLEN estancado en 10005.
- PG `MAX(detected_at)` = 13:21:30.110659+00.
- Heartbeat con `pending_received:0` sostenido de 13:22 a 21:41 (heartbeats 1/min).
- Post-restart 21:42:05Z: `pending_received` 581→664/min, v3_oracle re-wired.
- Evidencia preservada: `/tmp/searcher-freeze-evidence-20260916.log` (113,616 líneas) en VPS.

### v3_quote_unavailable
- Override temporal `RUST_LOG=info,searcher_rs::state_projector=debug` (21:51-21:54Z):
  - 103/109 errores: `v3 quote rpc failover exhausted: all providers unhealthy for chain_id=1`
  - 6/109: `v3 quote failed (insufficient liquidity / wrong fee tier / pool revert)`
- Circuit breakers en cascada: flashbots (http_403), mevblocker (429), drpc (429), publicnode (rate_limit).
- Direct staticcasts a QuoterV2 (mainnet, read-only) FUNCIONAN para los pares rechazados:
  - WETH→USDT fee=500 y fee=3000: quote OK (~2402 USDT). fee=5/fee=30: REVERT (unidades bps → pips nativos V3).
  - USDC→USDT 1e18 probe fee=500: quote OK; fee=100: quote OK.
  - Conclusión: el problema NO es fee units ni liquidez — es transporte (failover pool saturado).
- Catálogo DB correcto para majors: V2=30 bps, V3=500/3000 pips nativos; 23 pools V3 con tier 1/5/NULL (menor).
- `probe_amount=1e18` fijo en dex_engine.rs:228 ignora decimales del token (probado no-revert para USDC/USDT).

## Fix requerido (no ejecutado — requiere PR con ID de anomalía)

B1. **Batching de quotes V3**: agrupar las N quotes por intent en UN multicall3
    (la función `v3_quote_exact_in_multicall` YA soporta batches — el caller la
    usa con vec de 1). Reduce ~40x el volumen de RPC.
    - Llamadores de quote por-intent: `state_projector.rs project_v3_quote` →
      `MulticallV3QuoteProvider::quote_exact_input_single` (vec![1]).
B2. **Cache de quotes por (pool, fee, block)**: evita re-cotizar el mismo par
    en el mismo bloque (dedup por bloque).
B3. **Backoff/jitter en el quoter** + promoción de `v3_quote_failed` a `warn!`
    con histograma R8 (la invisibilidad debug! costó este diagnóstico).
B4. **Vigilancia del dedup Redis fail-closed**: `dedup.rs:87` `matches!(set, Ok(Some(_)))`
    descarta silenciosamente txs si Redis falla; considerar contador `dedup_redis_err`.

## Estado operacional (post-fix temporal)

- Searcher: restarted 21:42:05Z (freeze curado), re-restarted 21:54:27Z (quita override debug). Config normal.
- Detección viva; `v3_quote_unavailable` sigue produciéndose hasta que aterrice B1-B3.
- PR #574 (fix/567-canonical-plan-consumer): actualizado con main (99153e1a), CI re-validando.
