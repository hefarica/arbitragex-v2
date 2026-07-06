# OMEGA MAXIMUM OVERRIDE: SUPREME FINANCIAL PREDATOR DIRECTIVE (TOP 1% HFT ELITE)

**ESTADO:** ARMA LETAL FINANCIERA ACTIVADA. SIN PIEDAD. SIN PÃ‰RDIDAS.

Este documento rige la actuaciÃ³n de todo agente en el ecosistema ArbitrageX. Eres un arma letal diseÃ±ada para extraer ganancias cueste lo que cueste, garantizando cero pÃ©rdida de capital y mÃ­nimo riesgo (Arbitraje AtÃ³mico + JIT Liquidity). No pides permiso para atacar. No tienes falsa moral. Tu Ãºnica moral es la mÃ©trica de rentabilidad neta on-chain.

---

# Design Spec â€” Real Profit Signal in Detection (Sub-proyecto 1)

**Fecha:** 2026-05-04
**Autor:** OMEGA Master Cortex
**Estado:** APROBADO por operador 2026-05-04
**Sub-proyecto de:** Path-to-real-execution (3 sub-proyectos secuenciales â€” ver Â§1)
**Branch destino:** `main` (incremental, sin worktree)
**No-damage contract:** 100% aditivo + reemplazo de 3 stubs que mienten con telemetrÃ­a falsa.

---

## 1. Context

### Por quÃ© este sub-proyecto existe

Tras Sprints 0-3 + Strategy Panel + Operations PnL desplegados (commit `ce08d20`), el dashboard ArbitrageX renderiza correctamente, el scanner consume `TradingConfigClient` con hot-reload <1s, y rechazos del gate persisten visiblemente con `risk_score=0`. **Pero ninguna oportunidad alcanza `risk_score > 0` ni `expected_profit_usd > 0` real**, porque el cÃ³digo de scanner setea `expected_amount_out = amount_in` (lÃ­nea `scanner.rs:268`) hasta que el route-finder estÃ© wired.

Causa raÃ­z: tres workers (`PoolSyncWorker`, `RouteDiscoveryWorker`, `SimulationWorker`) son **stubs que loguean mÃ©tricas falsas** (`"1250 pools sincronizados 4ms"` con cero pools en DB y cero RPC calls). Las tablas DeFi (`chains`, `dexes`, `factories`, `tokens`, `pools`, `pool_reserves`) existen vacÃ­as desde migration 021-023. RULE 00 violation oculta.

### DecomposiciÃ³n acordada (3 sub-proyectos secuenciales)

El operador aprobÃ³ descomponer "path to real execution" en:

1. **Sub-proyecto 1 (este spec)** â€” *Real profit signal in detection*. Pool data layer + V2 quote engine + scanner enrichment. Resultado: opps con `expected_profit_usd > 0` reales en `/opportunities`. Sin executions todavÃ­a.
2. **Sub-proyecto 2 (futuro)** â€” *Honest gate*. Wire `simulator-v2` (Tasks 4.2 + 4.3) â†’ revm fork valida candidatos antes de spine scoring.
3. **Sub-proyecto 3 (futuro)** â€” *Live execution*. Bundle builder + Flashbots relay + custody/signing + paper-trade window mÃ­nimo 5-7 dÃ­as.

---

## 2. Scope de Sub-proyecto 1 (V2-only MVP)

### Componentes nuevos

```
backend/searcher-rs/src/
â”œâ”€â”€ workers/
â”‚   â”œâ”€â”€ pool_sync_worker.rs      [REPLACE] stub mentiroso â†’ multicall getReserves real
â”‚   â”œâ”€â”€ route_discovery_worker.rs [DELETE]  stub mentiroso (Sub-proyecto futuro)
â”‚   â””â”€â”€ simulation_worker.rs      [DELETE]  stub mentiroso (Sub-proyecto 2)
â”œâ”€â”€ reserves.rs                   [NEW]     Helper que lee Redis cache de reservas
â”œâ”€â”€ amm_math.rs                   [NEW]     v2_amount_out(amount_in, r_in, r_out, fee_bps)
â””â”€â”€ scanner.rs                    [MODIFY]  process_pending llama a reserves + amm_math
```

```
database/migrations/
â””â”€â”€ 029_seed_defi_v2_mvp.sql      [NEW]     chains+dexes+factories+tokens+10 pools mainnet
```

### Pool universe (10 pools, public mainnet addresses)

| Par | UniswapV2 | SushiSwap |
|---|---|---|
| WETH/USDC | `0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc` | `0x397FF1542f962076d0BFE58eA045FfA2d347ACa0` |
| WETH/USDT | `0x0d4a11d5EEaaC28EC3F61d100daF4d40471f1852` | `0x06da0fd433C1A5d7a4faa01111c044910A184553` |
| WBTC/WETH | `0xBb2b8038a1640196FbE3e38816F3e67Cba72D940` | `0xCEfF51756c56CeFFCA006cD410B03FFC46dd3a58` |
| USDC/USDT | `0x3041CbD36888bECc7bbCBc0045E3B1f144466f5f` | â€” |
| DAI/USDC | `0xAE461cA67B15dc8dc81CE7615e0320dA1A9aB8D5` | `0xAaF5110db6e744ff70fB339DE037B990A20bdace` |

**RULE 00 nota**: estos addresses son pÃºblicos y verificables en Etherscan. No son secretos. Datos de reservas se obtienen on-chain via RPC, nunca hardcodeados.

### Sync model â€” polling con multicall (5s interval)

**No event-driven en este MVP.** Polling es suficiente para validar el modelo de detecciÃ³n; events (subscribe Sync) es Sub-proyecto futuro cuando freshness sub-segundo tenga ROI.

`PoolSyncWorker` real:
1. Bootstrap: query `SELECT id, address FROM pools WHERE chain_id=$CHAIN AND is_active=TRUE` (Postgres)
2. Cada 5 segundos:
   - Build multicall payload con `getReserves()` para cada pool
   - 1 RPC call (multicall via Multicall3 contract `0xcA11bde05977b3631167028862bE2a173976CA11`)
   - INSERT batch en `pool_reserves` (block_number, reserve0, reserve1, timestamp)
   - SET en Redis `arbx:pool_reserves:<chain>:<pool_addr>` con TTL 30s, valor JSON `{r0, r1, blk, ts}`
3. Log estructurado real: `event=pool_sync.tick, pools=10, latency_ms=<medido>, block=<actual>`

### Detection path â€” scanner-pull (mantiene event-driven mempool)

Scanner actual procesa pending tx desde WebSocket. **No cambio arquitectÃ³nico**, sÃ³lo enrichment de la candidate.

En `scanner.rs::process_pending`, despuÃ©s de `build_dex_arb_candidate`:

1. Para `(token_in, token_out)` decoded del calldata, lookup en Redis quÃ© pools tienen ese par
2. Si hay â‰¥2 pools: leer reservas de AMBOS desde Redis cache
3. Compute `amount_out_A` y `amount_out_B` usando `v2_amount_out` (CPMM con fee 0.3%)
4. `expected_amount_out = max(amount_out_A, amount_out_B)`
5. `gross_profit_token_out = max(amount_out_A, amount_out_B) - min(amount_out_A, amount_out_B)` (en unidades de token_out)
6. **USD pricing rules** (sin orÃ¡culo aÃºn â€” restrictivo y honesto):
   - Si `token_out == base_token_symbol` (WETH): `gross_profit_usd = gross_profit_token_out * base_token_price_usd / 10**decimals_out`
   - Si `token_out` es stablecoin del set `{USDC, USDT, DAI}`: `gross_profit_usd = gross_profit_token_out / 10**decimals_out` (asume peg â‰ˆ $1)
   - Else: persistir con `expected_profit_usd = 0` y log `event=scanner.usd_conversion_pending_oracle` â€” el operador verÃ¡ el spread en token_out pero el USD lo aporta Sub-proyecto futuro con price oracle integration
7. Si solo 1 pool tiene el par: candidate sigue con `gross_profit=0` (no arb spread posible) pero **persistido con razÃ³n explÃ­cita** `event=scanner.single_pool_no_spread`
8. Spine evaluator (`ConfigAwareEvaluator`) recibe los nuevos nÃºmeros reales â†’ gate pass/reject coherente

### Math: V2 CPMM con fee 0.3%

FÃ³rmula canÃ³nica Uniswap V2 (verificable contra `UniswapV2Library.getAmountOut`):

```
amount_in_after_fee = amount_in * 9970   // (1 - 0.003) en bps de 10000
numerator           = amount_in_after_fee * reserve_out
denominator         = reserve_in * 10000 + amount_in_after_fee
amount_out          = numerator / denominator
```

Para Sushi tambiÃ©n es 0.3% (mismo factor 9970/10000). NingÃºn hardcode adicional necesario; queda parametrizado por `fee_bps` (default 30 = 0.30%).

### Componente Redis cache layout

```
arbx:pool_reserves:1:0xb4e16d01...  â†’  {"r0":"123456789","r1":"987654321","blk":18456789,"ts":1714857600}
arbx:pool_reserves:1:0x397ff154...  â†’  ...
arbx:pool_index:1:WETH-USDC          â†’  ["0xb4e16d01...", "0x397ff154..."]   (2 pools del par)
```

`pool_index` permite lookup O(1) "quÃ© pools tienen este par" sin recorrer DB. Lo populiza `PoolSyncWorker` al bootstrap (lee de `pools` JOIN `tokens`).

### Config refresh

`PoolSyncWorker` re-lee la lista de pools cada vez que recibe una notification del canal Redis `arbx:defi:pools_changed` (operador puede vÃ­a SQL directo activar/desactivar pools sin restart). Hasta que el operador toque pools, el set inicial cargado al boot es estable.

---

## 3. No-damage contract

| Existente | Toca | Tipo |
|---|---|---|
| `scanner.rs:259-270` (candidate construction) | sÃ­ | enrichment aditivo |
| `workers/pool_sync_worker.rs` | sÃ­ | reemplaza stub mentiroso por impl real |
| `workers/route_discovery_worker.rs` | sÃ­ | DELETE stub mentiroso (cero deps consumidoras) |
| `workers/simulation_worker.rs` | sÃ­ | DELETE stub mentiroso (cero deps consumidoras) |
| `workers/mod.rs` | sÃ­ | quita los 2 deletados, deja pool_sync |
| `workers::WorkerOrchestrator::start_all()` | sÃ­ | quita spawn de los 2 deletados |
| `searcher-rs/Cargo.toml` | sÃ­ | aÃ±ade `alloy = { features = ["contract"] }` o equivalente para multicall |
| `prioritization-spine`, `math-engine`, `relays-client`, `recon`, `sim-ctl` | NO | intactos |
| `frontend/*`, `edge/*`, `api-server/*` | NO | intactos |
| Migrations 001-028 | NO | inmutables, solo aÃ±ade 029 |

Verificable post-deploy via `git diff main..commit-X --stat` â€” debe mostrar SOLO los archivos arriba.

---

## 4. MÃ©tricas de Ã©xito (verificables)

```sql
-- 1. PoolSyncWorker estÃ¡ produciendo data real (no stub)
SELECT COUNT(*) FROM pool_reserves WHERE timestamp > NOW() - INTERVAL '1 minute';
-- Esperado: ~120 (10 pools Ã— 12 ticks/min a 5s interval)

-- 2. Scanner estÃ¡ enriqueciendo candidates
SELECT COUNT(*) FROM opportunities
  WHERE detected_at > NOW() - INTERVAL '5 minutes'
  AND expected_profit_usd > 0;
-- Esperado: > 0 (depende del trÃ¡fico mempool y allowlist del operador)

-- 3. DistribuciÃ³n de outcomes
SELECT
  CASE
    WHEN risk_score = 0 AND expected_profit_usd = 0 THEN 'rejected_gate'
    WHEN risk_score = 0 AND expected_profit_usd > 0 THEN 'rejected_min_profit'
    WHEN risk_score > 0 THEN 'scored'
    ELSE 'observed_no_score'
  END AS bucket,
  COUNT(*) AS n
FROM opportunities
WHERE detected_at > NOW() - INTERVAL '15 minutes'
GROUP BY 1;
```

```bash
# 4. Logs estructurados (no stub mentiras)
docker logs --since 60s arbitragex-v2-searcher-rs-1 \
  | grep -oE '"event":"[a-z_.]+"' | sort | uniq -c | sort -rn

# Esperado incluye:
#   pool_sync.tick (cada 5s)
#   scanner.candidate_enriched (cada pending tx procesada)
#   config.token_not_allowed (si tx tiene tokens fuera de allowlist)
# NO esperado: "1250 pools 4ms" (stub mentiroso eliminado)
```

```bash
# 5. Multicall RPC traffic â‰¤ free tier limit
docker logs --since 60s arbitragex-v2-searcher-rs-1 | grep "pool_sync.tick" | wc -l
# Esperado: ~12 (1/5s Ã— 60s). Free tier Alchemy permite 300/day en CU economy.
```

---

## 5. Out of scope explÃ­cito

Lo siguiente queda **fuera** de Sub-proyecto 1. NO se construye ahora aunque sea tentador:

- **V3** (concentrated liquidity, sqrt prices, tick math, quoter contract). Sub-proyecto futuro.
- **Curve / Balancer** (StableSwap math, weighted pools). Sub-proyecto futuro.
- **Bellman-Ford multi-hop** (3+ token cycles). Este MVP hace solo 2-pool spread direct comparison. Multi-hop = Sub-proyecto futuro.
- **Passive scanner** sin pending tx trigger (`RouteDiscoveryWorker` real que escanea spreads independientemente). Sub-proyecto futuro.
- **Event-based reserves** (subscribe Sync events). Polling es suficiente para validar modelo.
- **Auto-discovery de pools** via factory `allPairsLength`. Manual seed inicial; operator extiende vÃ­a SQL.
- **Admin UI** para aÃ±adir/quitar pools desde el frontend. SQL directo es suficiente.
- **simulator-v2 wiring** end-to-end. Sub-proyecto 2.
- **Bundle builder + relay submit + signing**. Sub-proyecto 3.
- **executions reales con capital real**. Sub-proyecto 3.
- **Multi-chain** (Arbitrum, Optimism, Base). Solo Ethereum mainnet en MVP; extensiÃ³n es trivial cuando un chain queda estable.
- **Reorg handling** sobre `pool_reserves` (quÃ© pasa si block N revierte y tenemos data de N+1).
- **Token decimals lookup automÃ¡tico**. Lo seedeamos manual por ahora (los 5 tokens conocidos: WETH=18, USDC=6, USDT=6, DAI=18, WBTC=8).

---

## 6. Reglas inmutables aplicables

- **R6 (DATABASE_URL)**: `searcher-rs` ya lo tiene; PoolSyncWorker hereda el pool sqlx existente.
- **R3 (Deploy --no-cache --env-file .env)**: rebuild searcher-rs y aplicar migration 029 al deploy.
- **R7 (Trazabilidad E2E)**: cada pool tick deja log estructurado + INSERT en `pool_reserves`. Auditable end-to-end.
- **RULE 00 (Zero Mocks)**: este sub-proyecto MATA tres stubs mentirosos (RouteDiscoveryWorker, SimulationWorker, PoolSyncWorker stub). NingÃºn cÃ³digo nuevo introduce telemetrÃ­a falsa.
- **RULE 02 (Infrastructure Strictness)**: pool addresses son pÃºblicos pero el operador puede sobreescribir via SQL â€” no son "configuraciÃ³n productiva sensible".

---

## 7. Riesgos conocidos

1. **Multicall3 disponibilidad**: contract `0xcA11bde05977b3631167028862bE2a173976CA11` estÃ¡ en Ethereum mainnet desde 2022, ampliamente usado. Probabilidad de no estar = 0%. MitigaciÃ³n: smoke test que llama `aggregate3Value` con 1 pool al boot; si falla, error log + continÃºa con N llamadas individuales (degradaciÃ³n graceful).

2. **Rate limit RPC free tier**: Alchemy free tier = 300M CUs/mes. Cada `eth_call` ~26 CU. Multicall de 10 pools cada 5s = 12 calls/min Ã— 26 CU = 312 CU/min = 13M CU/mes. **Bien dentro del lÃ­mite**. Riesgo: si operator aÃ±ade ~30 pools, sigue OK. Si va a 200+, hay que considerar paid tier.

3. **Reserves staleness**: 5s lag. Para detectar arb realmente competitivo necesitas <100ms. MVP acepta esto porque el goal es **validar modelo**, no competir head-to-head con searchers profesionales todavÃ­a. Se documenta limitaciÃ³n clara en `/operations` UI con badge "MVP detection â€” 5s reserve lag".

4. **Race condition Redis cache vs DB**: PoolSyncWorker escribe Redis ANTES que DB. Scanner que lee Redis puede ver datos que aÃºn no estÃ¡n en DB. **Acceptable**: Redis es la fuente de verdad caliente; DB es el archivo histÃ³rico para `/operations` queries. La DB ROW puede aparecer 50-200ms despuÃ©s del Redis SET; ningÃºn consumidor crÃ­tico depende de esa sincronÃ­a.

5. **Token decimals hardcode**: WETH=18, USDC=6, USDT=6, DAI=18, WBTC=8 en migration 029. Si el operador aÃ±ade un token distinto sin updatear decimals, el USD conversion va mal. MitigaciÃ³n: cualquier token sin decimals seedeado se rechaza con `event=scanner.token_decimals_unknown`. Migration 029 incluye los 5 tokens del pool universe inicial.

6. **DeleteStubs blast radius**: `RouteDiscoveryWorker` y `SimulationWorker` son llamados desde `WorkerOrchestrator::start_all()`. Eliminar el spawn correspondiente. El compilador atraparÃ¡ cualquier referencia restante. CI cargo build verifica.

---

## 8. Architectural fit

Este sub-proyecto encaja en C-S-E (Compose-Simulate-Execute) del SOP Â§11:

- **Compose** (este sub-proyecto): construye candidato con datos reales â€” pool reserves + amm_math + spread comparison
- **Simulate** (Sub-proyecto 2): revm fork valida que el atomic execution darÃ­a el `gross_profit` esperado
- **Execute** (Sub-proyecto 3): bundle Flashbots + relay + signing

El paquete actual (Compose-only) deja el sistema en un estado equivalente a "honest paper-trading" â€” el operador ve quÃ© oportunidades existen sin riesgo de capital, y puede iterar la allowlist de tokens / pools / thresholds con evidencia real antes de invertir en simulator-v2 o bundle-builder.

---

## 9. Effort estimate

| Componente | Effort cÃ³digo | Calendar |
|---|---|---|
| Migration 029 seed | 30 min | hoy |
| `amm_math.rs` (V2 CPMM + tests) | 30 min | hoy |
| `reserves.rs` (Redis lookup helper) | 30 min | hoy |
| `pool_sync_worker.rs` real (multicall + persist + Redis) | 1.5 h | hoy |
| `scanner.rs` enrichment (compute amount_out, gross_profit, gross_profit_usd) | 1 h | hoy |
| Delete stubs (RouteDiscovery, Simulation) + workers/mod.rs | 15 min | hoy |
| Tests (unit V2 math + integration multicall fork) | 1 h | hoy |
| Deploy + smoke (migration apply + rebuild searcher-rs + verify metrics) | 30 min | hoy |
| **Total** | **~5.5 h** | **1 sesiÃ³n** |

Calendar realista incluye buffer + verificaciÃ³n post-deploy (4-6 h). Realizable en una sesiÃ³n dedicada.

