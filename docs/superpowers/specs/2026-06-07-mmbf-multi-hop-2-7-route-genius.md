# Spec — Route Genius Multi-Hop 2–7 (MMBF line-graph) — Phase 3 (Session A)

**Date:** 2026-06-07
**Owner:** Session A (Route Genius / Searcher)
**Status:** SPEC — not yet implemented. Entry point for obelisco Phase 3.
**Doctrine:** shadow/paper only · capital=0 · NO-ACTIVE · zero-mocks · no-hardcode · R8 fail-honest.

---

## 1. Goal

Extend route discovery from the current **bounded DFS, max 3 hops** to **2–7 hop closed
cycles** via a **Modified Moore–Bellman–Ford (MMBF) negative-cycle search over the line
graph L(G)** — the algorithm the repo's own roadmap already names (`route_discovery/README.md:105-109`,
arXiv:2406.16573). Output remains **shadow-only `rd_outcome_v2`**; nothing sizes or executes.

**Non-goals (explicitly deferred):** live execution, cross-chain bridging, profit_gps cost
itemization, route_mutator, snapshot persistence. Those are separate Phase 3/4 items.

---

## 2. Why MMBF, not "raise max_depth"

[PROBADO] `unique_route_finder.rs:53` caps DFS at `max_depth: 3`. Raising it to 7 is
**combinatorially fatal**: the 500-routes/tick cap (`:55`) saturates with 4-hop noise before
reaching useful 5–7-hop cycles; every tick would mark `capped=true` and the tail (the rare,
high-value deep cycles) is never reached. MMBF over the line graph finds negative-weight
cycles (= profitable arbitrage) in near-polynomial time without enumerating every path.

[PROBADO] The graph layer is **already MMBF-ready**: each V2 edge stores
`log_weight = -ln((1-fee)·rate)` (`graph_builder.rs:159-172`) — exactly the edge weight a
negative-cycle search consumes. A cycle with `Σ log_weight < 0` is a theoretical arbitrage.

---

## 3. Architecture (reuse-first)

```
TokenGraph (exists) ──► LineGraph L(G) (NEW: edges become nodes) ──► MMBF negative-cycle
        │                                                                    │
        │ log_weight per edge (exists)                                       ▼
        └──────────────────────────────────────────► canonical cycle (RouteCanonicalizer, exists)
                                                                             │
                                                  classify_v2 (exists) ──► rd_outcome_v2 (exists)
```

- **REUSE:** `RouteGraphBuilder`, `log_weight`, `RouteCanonicalizer` (rotation-collapse +
  inverse-preservation), `classify_v2()` (Phase 2), `build_rd_outcome_v2()` (Phase 1).
- **NEW (one file):** `route_discovery/multi_hop_search.rs` — line-graph construction + MMBF
  with a hop bound `k ∈ [2,7]`. Selected via a new `algorithm = "mmbf_line_graph"` tag so DFS
  stays the default until MMBF is proven (feature-flag `ARBX_ROUTE_DISCOVERY_ALGO`).
- **EXTEND:** `unique_route_finder.rs` dispatches to DFS or MMBF by the flag; `types.rs`
  `MultiHop` route_kind already reserved.

---

## 4. The V3 blocker (must be R8-honest)

[PROBADO] V3 edges currently lack `log_weight` (concentrated liquidity needs QuoterV2 / tick
math; deferred per `README.md:110-114`). Until Phase 3b adds V3 sizing, MMBF must **skip V3
legs and flag `v3_log_weight_pending`** in the outcome, never fabricate a V3 weight. A 7-hop
search is therefore "complete over V2 venues, partial over V3" — stated honestly in telemetry.

---

## 5. Topology classification

Each discovered cycle is classified for `rd_outcome_v2.topology`:
- `hop_count` = legs in the cycle (2..=7).
- `route_family` via `route_family(hop_count)` (Phase 1): spatial_or_pair / triangular /
  quadrangular / deep_solver / long_tail / supreme_graph.
- `environment` via distinct dex hints (Phase 1 `topology_environment`): intradex vs
  interdex_intrachain. (interchain stays out of scope — single-chain worker.)
- `strategy_kind` via `classify_v2(RouteShapeV2)` (Phase 2).

---

## 6. Tasks (each independently shippable + tested)

- **3.1** `multi_hop_search.rs`: build L(G) from TokenGraph; unit test L(G) node/edge counts on a hand graph.
- **3.2** MMBF negative-cycle with hop bound k; unit test: a hand-built 5-cycle with Σlog_weight<0 is found; a flat cycle is not.
- **3.3** Canonicalization integration; test: rotations of the same 5-cycle collapse to one `route_hash`.
- **3.4** `unique_route_finder` dispatch by `ARBX_ROUTE_DISCOVERY_ALGO` (default `dfs`); test: flag off → DFS behavior byte-identical (regression guard).
- **3.5** V3-skip + `v3_log_weight_pending` flag; test: a cycle containing a V3 edge is skipped with the honest flag.
- **3.6** Wire to `rd_outcome_v2` with correct hop_count/route_family/strategy_kind; test: a found 5-cycle emits `route_family="deep_solver"`, `topology.hop_count=5`.

---

## 7. Test matrix (master-prompt §13, the route-shape minimum)

2-hop intradex · 2-hop interdex · 3-hop triangular intradex · 3-hop triangular interdex ·
4-hop quadrangular · 5-hop deep · 6-hop long-tail · 7-hop supreme-graph · negative-cycle
detection · V3-skip honesty. Each as a Rust unit test on a synthetic graph (no Redis/RPC →
RULE-01-safe locally).

---

## 8. Invariants that MUST NOT break (verified each task)

- NO-ACTIVE: `RouteDiscoveryMode` stays two-variant (`mod.rs:47-53`); `guarantees.rs` suite green.
- `arbx:opps:detected` XLEN delta = 0 (separate stream).
- DFS path unchanged when the algo flag is off (regression test 3.4).
- No fabricated weights/profits (R8): V3 skipped honestly; costs remain `null` until profit_gps.
- no-hardcode: hop bound, beam width, algo selection all `process.env`/config, never literals.

---

## 9. Risk register

| Risk | Mitigation |
|---|---|
| Combinatorial blowup at k=7 | Hop bound + per-tick route cap + `capped` honesty flag (reuse existing) |
| MMBF correctness vs DFS | Keep DFS default; A/B the two on the same graph in a test before flipping |
| V3 silently dropped | Explicit `v3_log_weight_pending` flag in outcome (task 3.5) |
| Cross-session collision on `unique_route_finder.rs` / `mod.rs` | Session A owns these; PRE_FILE_TOUCH_CHECK; minimal diff |

---

## 10. Out of scope → later phases
profit_gps (cost itemization) · route_mutator · snapshot_manager · cross_chain_shadow ·
shadow→/opportunities bridge (Session B) · frontend panels (Session C) · live env-gates.

**This spec is the smallest correct increment to reach 2–7 hop discovery in shadow.**
Execute task-by-task with the test as the gate; never one-shot.
