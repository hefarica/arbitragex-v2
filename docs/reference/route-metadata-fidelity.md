# route_metadata Round-Trip Fidelity Contract

> Source of truth for how a route's multi-hop topology flows from the searcher
> to the operator's screen, and the invariants that keep it truthful (≥99.99%
> fidelity — no mocks, no hardcodes, real-time, mode-invariant).

## The round trip

```
searcher-rs engine
  └─ builds StrategyCandidate { candidate: OpportunityCandidate, route_plan: RoutePlan }
     (orchestrator.rs process_candidate)
       └─ merges route_plan.legs (token traversal path) + candidate (pools/dexes)
          into shared_rs::candidates::RouteMetadata
          └─ opportunity_emitter.emit_accepted/emit_rejected
             └─ persistence::insert_opportunity_with_route
                └─ serde → PG opportunities.route_metadata (JSONB, migration 099)
                   └─ api-server opportunities-live.ts SELECT + passthrough
                      └─ TS parseRouteMetadata → RouteMetadataWire
                         └─ deriveLegs → RouteLeg[]
                            └─ OpportunityExchangeCard "Route A→B" section
```

## Invariants (load-bearing)

1. **`token_addresses.len() == hops + 1`** where `hops = dex_adapters.len()`.
   This is the structural gate in `persistence::insert_opportunity_with_route`
   (searcher-rs/src/persistence.rs). A topology that violates this is persisted
   as `{}` (R8 — no topology, rather than a malformed one).
2. **`pool_addresses.len() <= hops`** — tolerated; some legs have no resolved
   pool/factory at scan time (the calldata decoder doesn't always resolve it).
   Pools are advisory; the token path is authoritative.
3. **`decimals` is optional** — resolved separately downstream (scanner
   TokenDecimalsProvider / sim-ctl A1 enrichment). The persistence gate does
   NOT require decimals (it once did, which silently nuked every topology to
   `{}` — fixed 2026-08-10).
4. **Mode-invariant (§34)** — the topology path is identical in Paper, Testnet,
   and Mainnet. No branch on execution mode.

## The merge (orchestrator.rs)

`sc.candidate` (OpportunityCandidate) carries `pool_addresses` / `dex_adapters`
but often only the entry/exit tokens (e.g. triangular: `[A, A]`). `sc.route_plan`
.legs` carry the per-hop `token_in`/`token_out`, yielding the full traversal
(`A→B→C→A`). The orchestrator builds both and **prefers the source with the
longer token path**, backfilling pools/dexes from candidate when the plan left
them short.

## Silent-breakage history (why the tests exist)

The route_metadata pipeline broke silently across multiple layers before the
fidelity tests (`fidelity_tests` in persistence.rs, `deriveLegs.test.ts`,
`mapper.test.ts`) were added:

1. `RouteMetadata::validate()` required decimals → every builder emits empty
   decimals by design → topology nuked to `{}`. Fixed: persistence gate checks
   structure only.
2. `emit_accepted` always passed `route=None` → topology discarded at emit.
   Fixed: thread route from `sc.candidate`.
3. Live opps are rejected → go through `emit_rejected` (had no route). Fixed:
   `emit_rejected` takes `route`, 15 call sites threaded.
4. Triangular `sc.candidate` had only 2 tokens; the A→B→C→A path lived in
   `route_plan.legs`. Fixed: merge sources, prefer the longer token path.
5. Persistence gate required `pools == hops` → triangular with an unresolved
   pool got nuked. Fixed: `pools <= hops`.

Commits: `ff443fbf`, `758d1ce6`, `e5538711`, `062002cc`, `d958c9f0`,
`bafd08b8`, `ec2579d5`.

## Enforcement

`.github/workflows/opportunities-fidelity-gate.yml` runs on every PR/push to
main touching the opportunities path: `vitest` + `cargo test fidelity_tests` +
a grep that refuses hardcoded sentinel addresses (`0x…dEaD`) and the word
"hardcode" in the opportunities source.
