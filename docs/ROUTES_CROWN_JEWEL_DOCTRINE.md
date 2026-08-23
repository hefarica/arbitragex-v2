# ROUTES CROWN JEWEL DOCTRINE

> Doctrina canónica de descubrimiento y optimización de rutas — workbook ULTRA hoja
> `09_REFERENCIAS` (18 referencias) + la librería de investigación mundial
> (`skills/arbitragex-ultra/world/`). Cada referencia mapea a **(1)** su uso
> canónico declarado en el workbook y **(2)** dónde vive en este repo.
>
> Este documento es la hoja de ruta doctrinal que `.claude/skills/arbitragex-omniscience`
> referencia: el Excel es el 5% (la especificación ejecutable); el mundo es el 95%
> (el estado del arte). Nada aquí es decorativo — cada entrada cita su consumidor real.

## Contexto de uso

- **Fuente de verdad de la matriz**: `artifacts/excel_requirements.json` (441
  requisitos VERIFICADOS contra el working tree) — regenerable con
  `py scripts/excel_canon/build_canonical_artifacts.py`.
- **Superficie canónica de config**: `backend/searcher-rs/src/canonical_knobs.rs`
  (43 knobs de `01_CONFIG`, precedencia env `ARBX_KNOB_*` > yaml > workbook).
- **Vocabulario canónico**: `backend/searcher-rs/src/canonical_enums.rs`
  (ALGORITHMS ×7, DEXES ×6 — hoja `10_LISTAS`).

## Tabla canónica (09_REFERENCIAS — 18 filas)

| # | Tema | Referencia | Uso canónico (workbook) | Donde vive en el repo |
|---|------|-----------|--------------------------|------------------------|
| 1 | Negative cycles | DEFIPOSER — Just-In-Time Discovery of Profit-Generating Transactions | BFM negative-cycle discovery | `backend/searcher-rs/src/route_discovery/multi_hop_search.rs` (enumeración de ciclos `find_profitable_cycles`); deep-dive `world/graph-algorithms/FINDINGS.md` |
| 2 | Cyclic arbitrage | Cyclic Arbitrage in Decentralized Exchanges | Empirical cycle context | `multi_hop_search.rs` + `unique_route_finder.rs` (ciclos cerrados 2..7 hops); `world/graph-algorithms/FINDINGS.md` |
| 3 | Convex routing | Optimal Routing for Constant Function Market Makers | Post-discovery routing/sizing | `SizeOptimizer` (el motor económico vivo; sizing convexo por ruta); `world/quant-math/` |
| 4 | Efficient routing | An Efficient Algorithm for Optimal Routing Through CFMMs | Decomposition / scaling | `world/graph-algorithms/` (descomposición convexa multi-pool); knob `enable_convex_size` |
| 5 | MMBF | An Improved Algorithm to Identify More Arbitrage Opportunities | MMBF + line graph discovery pass | knob `enable_mmbf` (`canonical_knobs.rs`) + `multi_hop_search.rs`; `world/graph-algorithms/` |
| 6 | Real-time negative cycles | RICH | k-hop candidate prioritization | knob `enable_rich`; priorización por hop-tier en `route_discovery_worker.rs` |
| 7 | Optimization | Marginal Price Optimization | Post-discovery sizing/ranking | `SizeOptimizer` (marginal price por pool); ranking weights (`rank_*` knobs) |
| 8 | Graph structure | Network Analysis of Uniswap | Hot-token/core pruning | `graph_builder.rs` — poda por liquidez (`min_pool_liquidity_usd`) + concentración hot-token (XLS-GRAPH-01) |
| 9 | Line graph routing | A Line Graph-Based Framework for Identifying Optimal Routing in DEXs | Parallel edge / structure | aristas paralelas por (DEX×versión×fee) en `graph_builder.rs`; `route_discovery.rs` `parallel edges` |
| 10 | Token graph routing | PRIME | Candidate routing benchmark | `cycle_enumerator.rs` (universo dinámico de ciclos, tabla `pool_cycles` migration 107) |
| 11 | Multi-path routing | Multi-Path Routing in AMM Exchange Networks | Split/multi-path routing | `route_metadata` multi-path (migration 099 + `persistence.rs` `build_route_metadata_from_plan`) |
| 12 | Production | Flashbots simple-arbitrage | Discover/evaluate/rate reference | patrón discover→evaluate→rate del pipeline C-S-E; `world/mev-practice/` |
| 13 | Production | Paradigm Artemis | Event-driven framework | arquitectura event-driven (Collector WS → engines); `world/mev-practice/` |
| 14 | Production | Uniswap smart-order-router | Production routing | `route_discovery_worker.rs` (dispatch per-strategy); `world/defi-protocols/` |
| 15 | Production | Sorella Brontes | MEV analytics state | `math_evidence.rs` (estado por estrategia); `world/mev-practice/` |
| 16 | Production | evm-amm-search | Incremental AMM route search/state sync | `ImpactIndex` + `ReservesCache` (sync incremental de reservas on-chain) |
| 17 | Financing filter | Aave V3 Flash Loans | Provider fee/capacity input | modo `AAVE_FL` (`canonical_knobs.rs::FINANCING_MODES`, capacity/fee_bps hoja `02_FINANCING`); fees SIEMPRE leídos on-chain (gobernable) |
| 18 | Financing filter | Balancer Protocol Fees | Provider fee input | modo `BALANCER_FL` (idem `02_FINANCING`) |

## Reglas doctrinales (resumen operativo)

1. **Dos capas**: DISCOVERY (enumerar topología — ciclos cerrados 2..7 hops) ≠
   EVALUATION (gates + sizing + EV por financing mode). Nunca mezclar.
2. **Financing = dimensión de ruta** (`OWN_CAPITAL` / `AAVE_FL` / `BALANCER_FL` /
   `V2_FLASH_SWAP`): cambia qué rutas son viables, no cuántas se descubren.
3. **Nada muere en silencio**: cada rechazo emite (hop_tier, gate, razón,
   financing_mode) — R8 fail-honest.
4. **Fees on-chain**: Aave/Balancer/Uniswap se leen de la cadena, jamás se
   hardcodean (Aave = 5bps HOY, gobernable mañana).
5. **Convexidad después de enumerar**: la enumeración (BFM/MMBF/DFS/Johnson)
   produce candidatos; el sizing convexo (CFMM routing) decide el tamaño
   óptimo. El orden importa.
6. **El grafo es dinámico**: reservas via `ReservesCache`/`ImpactIndex` sync
   (ref. 16); el universo de ciclos persiste en `pool_cycles` (ref. 10).

## Deep-dives mundiales (el 95%)

| Dominio | Path | Qué aporta |
|---------|------|------------|
| Graph algorithms | `skills/arbitragex-ultra/world/graph-algorithms/` | BFM/MMBF/RICH/convex — `FINDINGS.md` (16.6K), `BETTER_THAN_EXCEL.md`, `NEW_STRATEGIES.md` |
| MEV practice | `skills/arbitragex-ultra/world/mev-practice/` | searchers reales, bundles, márgenes (refs. 12-15) |
| DeFi protocols | `skills/arbitragex-ultra/world/defi-protocols/` | UniV4, Morpho, Hyperliquid, intents (refs. 14, 17, 18) |
| Quant math | `skills/arbitragex-ultra/world/quant-math/` | Kyle, HJB, Kelly multi-armed, VPIN (refs. 3, 4, 7) |
| Security simulation | `skills/arbitragex-ultra/world/security-simulation/` | REVM, formal verify, attack surface |

## Procedencia

- Workbook ULTRA sha256 `362ba8762e…` — hoja `09_REFERENCIAS` (18 filas),
  extraída por `scripts/excel_canon/build_canonical_artifacts.py`
  (REQ-REF-001..018, familia `doctrine`).
- Investigación mundial fechada `2026-08-19` (`world/*/DOMAIN.json`).
- Mantenimiento: al añadir una referencia al workbook, añadir la fila aquí con
  su consumidor real — una fila sin consumidor es doctrina muerta.
