# STRATEGY DATAFLOW E2E AUDIT

Date: 2026-05-12
Scope: `opportunities-live`, `strategy-runtime-status`, `trading-config`, `strategy-catalog`, `searcher-rs orchestrator`, reserves/index contracts.

## Executive summary

1. Dataflow path exists end-to-end: `trading_config (PG)` -> `Redis mirror/pubsub` -> `searcher-rs engines` -> `opportunities (PG)` -> API read models.  
2. `/api/v1/opportunities/live` is fail-honest and bounded by freshness window; returns 503 on query failure.  
3. `/api/v1/strategies/runtime-status` is fail-honest for PG main query and uses real heartbeat + PG counters for most fields.
4. Liquidation semantic fields currently preserve null for metrics without source (`impacted_lending_positions_1h`, `hf_below_one_count`) and keep `enabled: false` as config-style flag.
5. High-risk gap: contradictory orchestrator header comments still describe old phase skeleton while code executes triangular/flashloan/liquidation paths.

## Evidence matrix

| Module | Signal | Evidence |
|---|---|---|
| trading-config | PG source of truth + Redis propagation | `trading_config` route comments and channel/key constants |
| strategy-catalog | active strategies from `enabled_strategies` | `/strategy-catalog/active` query |
| opportunities-live | reads real `opportunities` + freshness gate | SQL WHERE on `detected_at` and `viable_only` behavior |
| runtime-status | PG query fail -> 503 | catch block returns `opportunities_query_failed` |
| runtime-status | flashloan `no_provider_rejections` | explicit counter for `flashloan_no_provider` |
| runtime-status | liquidation unknown telemetry | null fields for impacted/hf_below_one |
| orchestrator | engines are executed | live blocks for triangular/liquidation/flashloan candidate build |

## Strategy-level status (code-level)

| Strategy | Config wiring | Runtime invocation wiring | Publish/read wiring | Notes |
|---|---|---|---|---|
| dex_arb | yes | yes | yes | baseline path |
| triangular_arb | yes (naming caveat in some layers) | yes | yes | verify naming consistency in persisted rows/config values |
| flashloan_arb | yes | yes (wrapping base candidates) | yes | monitor no-provider and sanity rejects |
| liquidation | yes (flag semantics separate) | yes | yes | engine_invoked can be true while `enabled=false` in status (intentional semantics) |

## Key risks and gaps

### P0
- None confirmed in current tree for duplicate liquidation keys; current file has single definitions and null fail-honest values.

### P1
1. **Documentation drift in orchestrator header**: comments claim only Dex engine is real, but runtime code executes multiple engines.
2. **Operator confusion risk**: `viable_only=false` default in live feed can overrepresent rejected flow as "activity".
3. **Naming consistency audit still needed** for `triangular` vs `triangular_arb` across persistence/config/reporting layers.

### P2
1. Missing unified readiness endpoint combining config + heartbeat + PG publish health by strategy.
2. Missing explicit dex/pool coverage summary endpoint for operational dashboard.

## Recommended actions

### Action A (safe, no behavior change)
- Update stale orchestrator header comments to match current runtime behavior.

### Action B (operational clarity)
- Add a readiness read model per strategy with explicit states:
  - `config_enabled`
  - `engine_invoked_recently`
  - `candidates_seen_1h`
  - `opportunities_published_1h`
  - `blocked_reason`

### Action C (data source transparency)
- Maintain null semantics for fields without strict telemetry source.
- Add per-field source metadata in docs for runtime-status.

## Repro commands used

```bash
rg -n "mountStrategyRuntimeStatus|opportunities/live|flashloan_arb_pairs_scanned|triangular_cycles_scanned|liquidation_positions_scanned|enabled_strategies|strategy-catalog|trading_config|orchestrator" backend/api-server/src backend/searcher-rs/src

nl -ba backend/api-server/src/routes/opportunities-live.ts | sed -n '150,210p'
nl -ba backend/api-server/src/routes/opportunities-live.ts | sed -n '462,520p'
nl -ba backend/api-server/src/routes/opportunities-live.ts | sed -n '640,690p'

nl -ba backend/api-server/src/routes/strategy-runtime-status.ts | sed -n '60,130p'
nl -ba backend/api-server/src/routes/strategy-runtime-status.ts | sed -n '165,220p'

nl -ba backend/searcher-rs/src/orchestrator.rs | sed -n '1,40p'
nl -ba backend/searcher-rs/src/orchestrator.rs | sed -n '330,460p'

nl -ba backend/api-server/src/routes/trading-config.ts | sed -n '1,30p'
nl -ba backend/api-server/src/routes/strategy-catalog.ts | sed -n '1,110p'
```

## Final verdict

Current code is materially fail-honest in opportunities/runtime-status read paths. Main remaining work is operational observability hardening and consistency cleanup (comments/naming/readiness aggregation), not a core wiring rewrite.
