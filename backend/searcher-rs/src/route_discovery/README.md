# OMEGA Route Discovery — Phase 1 radar

A Rust engine, **separate from the Rhai cartridges**, that enumerates *unique
routes* (closed cycles) over the live pool graph, canonicalizes them, decides
which strategies apply, and emits shadow evidence — **without touching capital,
execution, or the native opportunity stream**. It is a *radar*: it answers "what
routes exist, and which strategy applies to each?", not "how much profit?".

> Phase 1 measures **topology only**. No sizing, no profit, no execution. The
> only Redis write is `PUBLISH arbx:route_discovery:telemetry`; it never writes
> `arbx:opps:detected`.

## Pipeline

```
ImpactIndex.all_pools()  +  Redis (reserves / slot0 / token meta)
        │
        ▼
RouteGraphBuilder ── graph_builder.rs ──►  TokenGraph (directed RouteEdges)  +  rejections
        │                                   (V2 log_weight computed; V3 deferred — guardrail #8)
        ▼
UniqueRouteFinder ── unique_route_finder.rs ──►  closed cycles (bounded DFS 2–3 hops)
        │                                         2-cycles (V2V2/V2V3/V3V2/V3V3) + triangulars (from the graph)
        ▼
RouteCanonicalizer ── canonicalizer.rs ──►  deterministic route_hash
        │                                    (rotation-collapsing, inverse-preserving; route_kind derived from canonical order)
        ▼
StrategyApplicabilityEngine ── strategy_applicability.rs ──►  applicable StrategyLabels + rejections (+ tags)
        │                                                       (config-driven; shadow_only forced)
        ▼
RouteIntentDispatcher ── route_intent_dispatcher.rs ──►  shadow_evaluate_intent  (dex_arb only; triangular deferred)
        │                                                  THE ONLY downstream — observe-only, never opps:detected
        ▼
telemetry.rs ──►  PUBLISH arbx:route_discovery:telemetry   (every payload tagged algorithm = "dfs_bounded")
```

Orchestrated per tick by `route_discovery_worker.rs`. The per-tick logic is split
into a pure `evaluate_tick` (graph → events, no Redis/clock) and the thin async
`run_loop`, so it unit-tests in memory.

## Activation

Gated by `ARBX_ROUTE_DISCOVERY_MODE` — **default `off`** (the worker is not even
spawned; zero overhead, binary behavior unchanged):

| Value     | Behavior |
|-----------|----------|
| unset / `off` / anything else | dormant (not spawned) |
| `shadow`  | discovery + classification + telemetry; cartridge eval is observe-only via `shadow_evaluate_intent` |

There is intentionally **no `active` variant** — the type system forbids an
execution path out of route discovery. `"active"` parses to `Off`.

Requirements when `shadow`: an `ImpactIndex` (the live pool source). The worker is
spawned from `scanner::run_chain` (where the `ImpactIndex` and the cartridge
runner both exist — *not* `workers::start_all`, which has neither). When the
orchestrator is off (`ImpactIndex` absent) the worker skips with an honest reason.
When the cartridge runtime is off (`ARBX_CARTRIDGE_MODE=off`), discovery +
telemetry still run, but nothing is dispatched (no runner).

## Configuration

`config/strategies/route_applicability.yaml` (fail-safe: missing/invalid → embedded
safe defaults; `shadow_only` is forced true at load). Path overridable via
`ARBX_ROUTE_APPLICABILITY_CONFIG`.

Env caps (override the YAML `discovery:` section):

| Env var | Default | Meaning |
|---------|---------|---------|
| `ARBX_ROUTE_DISCOVERY_MODE` | `off` | `off` / `shadow` |
| `ARBX_ROUTE_DISCOVERY_INTERVAL_MS` | `12000` | tick interval |
| `ARBX_ROUTE_DISCOVERY_MAX_ROUTES_PER_TICK` | `500` | anti-explosion cap on routes |
| `ARBX_ROUTE_DISCOVERY_MAX_TELEMETRY_PER_TICK` | `200` | cap on per-candidate + dispatch events |
| `ARBX_ROUTE_DISCOVERY_MAX_POOLS_PER_PAIR` | `8` (or YAML) | branching cap |
| `ARBX_ROUTE_DISCOVERY_MAX_DEPTH` | `3` (or YAML) | max cycle hops (2–3) |
| `ARBX_ROUTE_DISCOVERY_MAX_AGE_SECS` | `120` | reject snapshots older than this |

Strategy matrix (default): `dex_arb` (accepts v2v2/v2v3/v3v2/v3v3, has cartridge →
dispatched), `triangular_arb` (accepts triangular, has cartridge but dispatch
deferred — needs a triangular pool-data adapter), `flashloan_arb` (accepts all, no
cartridge → applicable in telemetry only), `stable_arb` (disabled tag), `liquidation`
(`route_based: false` → every DEX route rejected `strategy_not_route_based`).

## Telemetry schema — channel `arbx:route_discovery:telemetry`

| event | key fields |
|-------|-----------|
| `route_discovery.tick` | `algorithm, pools_total, edges_built, edges_rejected, routes_found, routes_dispatched, telemetry_emitted, routes_dropped_for_cap, routes_capped, latency_ms, mode` (`routes_capped: true` ⇒ enumeration hit the cap and `routes_found` is incomplete; `routes_dropped_for_cap` is a lower bound — R8 fail-honest) |
| `route_discovery.route_candidate` | `algorithm, route_hash, route_kind, hops, tokens[], pools[], protocols[], fee_tiers[], directions[]` |
| `route_discovery.strategy_applicability` | `route_hash, route_kind, applicable_strategies[], rejected_strategies[{strategy,reason}]` |
| `route_discovery.rejected` | `pool, reason` (missing_reserves / missing_slot0 / stale_* / missing_token_metadata / unsupported_protocol / invalid_pool_shape / low_liquidity) |
| `route_intent.emitted` | `route_hash, strategy, mode, dispatch_deferred?` |

## NO-ACTIVE GUARANTEE

No active mode · no wallets · no executor · no contracts · no capital · no write to
`arbx:opps:detected` · the only cartridge downstream is `shadow_evaluate_intent`
(never `on_route_intent` / `process_candidate` / the emitter). Enforced by
`guarantees.rs` (suite-level) and verified at runtime (XLEN of `arbx:opps:detected`
unchanged before/after a shadow tick).

## Roadmap (NOT in Phase 1)

- **Phase 2 — MMBF line-graph** (arXiv:2406.16573): replace the bounded DFS with a
  Modified Moore–Bellman–Ford pass over the line-graph `L(G)` to find 7–11-hop
  routes the DFS can't see. Reuses the same `TokenGraph` + the `log_weight` already
  stored on each edge; changes only `unique_route_finder` (`algorithm =
  "mmbf_line_graph"`).
- **Phase 3 — Marginal Price Optimization** (arXiv:2502.08258): a *separate*
  `sizing/` module — root-finding on log-prices (~200× vs convex solvers) + real V3
  sizing (QuoterV2) + `revm` in-process sim. Turns `v3_sizing_pending` into a sized
  opportunity. Gated by `arbx-net-profit-gate` + `arbx-simulation-mandatory` +
  `arbx-paper-trade-first`.
- **Phase 4 — cross-rollup non-atomic** (CAAW 2026) + ERC-7683 intents + shared
  sequencing. High risk (multi-rollup capital, non-atomic); out of shadow scope.
- **Phase 5 — GNN** (arXiv:2502.03194): prune routes pre-simulation. This module
  already produces the labeled dataset such a model would consume.
- **active — Yul/Huff executor**: on-chain atomic execution; requires active mode +
  capital; out of scope.
