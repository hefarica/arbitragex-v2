# 04_MOTOR.md — Motor del Pipeline (searcher-rs stage por stage)

> SHA: `35627908` · Vivo: 2026-08-14T04:20:00Z · Heartbeat: **todos 0** (B-02 deserialize fix pendiente deploy #335)

## searcher-rs — 134 archivos Rust

### Estructura del motor

```
main.rs (entrypoint, #[tokio::main])
  ├─ chain_supervisor.rs    — spawn por chain, build_orchestrator
  ├─ scanner.rs             — WS subscribe → detection_loop → run_subscription
  │   ├─ chain_client.rs    — WsChainClient (subscribe_pending, subscribe_blocks)
  │   ├─ calldata/          — decode univ2/univ3 router calldata
  │   ├─ detector.rs        — route intent construction
  │   └─ process_pending()  — per-tx processing (dedup → get_tx → decode_and_score_tx)
  │
  ├─ orchestrator.rs        — V2 path: on_route_intent → process_candidate
  │   ├─ cartridge_boot.rs  — Rhai cartridge evaluation (264 cartridges)
  │   ├─ size_optimizer.rs  — SizeOptimizer (Kelly + golden-section + gas floor)
  │   ├─ engines/           — 12 strategy engines
  │   │   ├─ dex_engine.rs          (V2/V3 DEX arbitrage)
  │   │   ├─ triangular_atomic_engine.rs (triangular A→B→C→A)
  │   │   ├─ flashloan_engine.rs    (flash loan arb)
  │   │   ├─ backrun_engine.rs      (backrunning)
  │   │   ├─ cex_dex_engine.rs      (CEX-DEX)
  │   │   ├─ cross_chain_bridge_engine.rs (bridge)
  │   │   ├─ liquidation_engine.rs  (Aave liquidation)
  │   │   └─ ... (5 más)
  │   └─ amm_math.rs        — V2 CPMM + V3 concentrated liquidity math
  │
  ├─ opportunity_emitter.rs — emit_accepted / emit_rejected (PG + Redis)
  ├─ persistence.rs         — INSERT INTO opportunities (+ route_metadata)
  ├─ publisher.rs           — XADD arbx:opps:detected
  ├─ counters.rs            — AtomicU64 per-chain heartbeat counters
  ├─ workers/               — background workers
  │   ├─ heartbeat_worker.rs     (60s snapshot → Redis)
  │   ├─ triangular_worker.rs    (cycle scanner, MVP_CYCLES)
  │   ├─ flashloan_arb_worker.rs (pair scanner)
  │   ├─ liquidation_worker.rs   (Aave positions)
  │   ├─ pool_sync_worker.rs     (reserves cache refresh)
  │   └─ price_worker.rs         (Alchemy/CoinGecko/Chainlink price oracle)
  │
  ├─ route_discovery/       — shadow radar (DFS bounded, 118 edges)
  │   ├─ dfs_bounded.rs
  │   ├─ impact_index.rs
  │   └─ route_intent_dispatcher.rs
  │
  └─ shared state
      ├─ config_reload.rs        (Redis pub/sub → TradingConfigClient)
      ├─ dedup.rs                (Redis SETNX, 5min TTL)
      └─ connectors/             (mempool_listener, reserve_reader, rpc_multiplexer)
```

## Pipeline V2 (canónico actual) — stage por stage

### Stage 1: DETECCIÓN
| Componente | Entrada | Salida | Archivo:línea | Contador |
|---|---|---|---|---|
| WS subscribe | RPC_WS_1 (PublicNode) | raw pending-tx hashes | `chain_client.rs:94` subscribe_pending | `pending_received` |
| Dedup | hash | skip if seen (5min TTL) | `scanner.rs:1251` dedup.check_and_mark | — |
| get_tx | hash | Transaction body | `chain_client.rs:155` get_transaction | — |

### Stage 2: DECODE
| Componente | Entrada | Salida | Archivo:línea | Contador |
|---|---|---|---|---|
| calldata decode | tx.input + router.kind | RouteIntent[] | `scanner.rs:1380` route_decoder | `decoded_ok` |
| decode fail | — | skip + log | `scanner.rs:1392` | `decoded_err` |

### Stage 3: ORCHESTRATOR V2 (early return salta stages 4-7 legacy)
| Componente | Entrada | Salida | Archivo:línea | Contador |
|---|---|---|---|---|
| on_route_intent | RouteIntent | StrategyCandidate | `orchestrator.rs:277` | — |
| process_candidate | StrategyCandidate | SizedCandidate or Rejected | `orchestrator.rs:932` | — |

### Stage 4: SIZE + EVALUATE (dentro del orchestrator)
| Componente | Entrada | Salida | Gate | Contador |
|---|---|---|---|---|
| SizeOptimizer | candidate + reserves + price | SizedCandidate | — | — |
| Golden-section | profit function | optimal x* | — | — |
| Kelly criterion | f_raw × multiplier | kelly_cap | `NonPositiveNetUsd` → `passed_all_gates` | gate counters |
| Gas floor | net vs cost_proxy | accept/reject | `GasFloorBreach` | gate_anomalous_math |
| Cap clamp | x* vs cap_wei | clamped amount | `CapClampFailed` | — |

### Gates del SizeOptimizer (enum OptimizeRejectReason, size_optimizer.rs:56)

| Gate | Constante | Condición | Contador heartbeat |
|---|---|---|---|
| `NonPositiveProfit` | `non_positive_profit` | profit_wei ≤ 0 | gate_other_rejected |
| `NonPositiveGrossUsd` | `non_positive_gross_usd` | gross_usd ≤ 0 | gate_other_rejected |
| `NonPositiveNetUsd` | `non_positive_net_usd` | net_usd ≤ 0 | gate_other_rejected |
| `CapClampFailed` | `cap_clamp_failed` | clamp_to_cap returns None | — |
| `GasFloorBreach` | `gas_floor_breach` | net < gas_floor | — |
| `KellyNegativeEdge` | `kelly_negative_edge` | W_ratio ≤ 0 | — |
| `MissingPoolAddress` | — | pool not in reserves cache | — |
| `MissingReservesPoolA/B` | — | reserves lookup miss | — |
| `NoConfig` | — | trading_config not seeded | gate_no_config |
| `ZeroCapitalCap` | — | kelly cap rounds to 0 | — |

### Stage 5: EMIT
| Componente | Entrada | Salida | Archivo:línea | Contador |
|---|---|---|---|---|
| emit_accepted | Opportunity + route | PG opportunities + Redis XADD | `opportunity_emitter.rs:211` | `passed_all_gates` + `db_persisted` |
| emit_rejected | Opportunity + reason | PG opportunities (rejected) + Redis XADD | `opportunity_emitter.rs:294` | gate_* (N-01b fix) |

### Stage 6: ARCHIVE (api-server consumers)
| Consumer | Stream | Output | Archivo |
|---|---|---|---|
| PaperArchiver | arbx:opps:detected | PG paper_trade_runs | `paper-trade-archiver.ts` |
| PaperExecutor | arbx:hot:simulated | PG paper_trade_runs (actual) | `paper/executor.ts` |
| ScoringArchiver | arbx:scoring:scored | PG scored_opportunities | `scored-opportunities-archiver.ts` |
| RouteDiscoverySink | arbx:opps:detected | PG route_discovery_outcomes | `route-discovery-outcome-sink.ts` |

## Servicios auxiliares del motor

| Servicio | Puerto | Función | Entrada | Salida |
|---|---|---|---|---|
| selector-api | 3002 | Scoring spine: evalúa oportunidades detectadas, aplica gates de riesgo | arbx:opps:detected | arbx:scoring:scored + PG |
| sim-ctl | 3003 | Simulador REVM (fork mainnet, ejecuta tx sin broadcast) | scoring stream | arbx:hot:simulated |
| recon | 3004 | Reconciliación post-ejecución (drift tracking) | executions | recon_reports |
| math-engine | 3006 | §IV Bayesian evidence vectors | opportunities | math evidence snapshots |
| token-enricher | 9004 | Token metadata (symbol, decimals, logo) | Redis tokens:* | DexScreener fallback |

## Estado VIVO del motor (heartbeat 2026-08-14T04:20Z)

| Contador | Valor | Interpretación |
|---|---|---|
| pending_received | **0** | B-02 deserialize fix pendiente deploy (#335) |
| decoded_ok / decoded_err | 0 / 0 | consecuencia de pending=0 |
| Todos los demás | 0 | pipeline idle (espera fix B-02) |

**Hipótesis:** Tras deploy de #335 (raw WS subscribe), `pending_received > 0` y el pipeline reactiva.

## Divergencias repo ↔ vivo (FASE 4)

| # | Hallazgo | Severidad |
|---|---|---|
| 1 | Pipeline en 0 absoluto (B-02 deserialize bug) | 🔴 CRÍTICO — fix en #335 |
| 2 | 12 strategy engines definidas pero solo ~5 activas (dex, triangular, flashloan, backrun, liquidation) | ℹ️ INFO |
| 3 | Route discovery radar: 118 edges / 61 pools (vivo, shadow mode) | ✅ confirmado R3 |
| 4 | 264 cartridges Rhai registrados | ✅ cartridge_registry |

## Checklist FASE 4

- [x] searcher-rs 134 archivos mapeados por módulo
- [x] Pipeline V2 stage por stage (6 stages + gates)
- [x] 10 gates del SizeOptimizer catalogados con condición + contador
- [x] 4 api-server consumers identificados (archiver/executor/scoring/sink)
- [x] 5 servicios auxiliares descritos (entrada/salida)
- [x] Heartbeat vivo verificado (todos 0 — B-02 pendiente)
- [ ] Ficha individual por engine (los 12) — siguiente ciclo
- [ ] Diagrama de flujo de gates con reject paths

**Cobertura FASE 4: 80%**
