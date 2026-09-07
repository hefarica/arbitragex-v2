# Redis Hot Path Schema v2

Documentation for Redis Streams and Keys used in the <100ms latency pipeline.

## Overview

This schema defines the hot path data structures for the OMEGA pipeline, designed for sub-100ms end-to-end latency from detection to paper execution.

## Redis Streams (Hot Path v2)

### arbx:hot:detected (Stream)
- **Purpose**: Ingest raw opportunities detected by the searcher-rs observer
<!-- WO-02 (2026-09-06): section rewritten against the live code (gang fix ronda 2 — drift residual WO-02-FIXG3 §4.1/§4.3); fields/consumers verified against backend/searcher-rs/src/hot_path_emitter.rs:71-94 and backend/api-server/src/websocket.ts:801-907. -->
- **Producer**: searcher-rs `HotPathEmitter::emit_detected` (`hot_path_emitter.rs:71`) — **no call-sites today** (WO-02-CROSS §1): the method exists but nothing invokes it, so the stream stays honestly empty (XLEN=0) until it is wired
- **Consumers**: ws-emitter-g0 (only)
- **Fields** (XADD, `hot_path_emitter.rs:79-94`):
  - `id`: Opportunity UUID (stringified)
  - `chain_id`: Target blockchain network identifier
  - `strategy_kind`: Strategy variant, snake_case (`StrategyKind::as_str`)
  - `detected_at_ms`: Unix timestamp in milliseconds
- **MAXLEN**: ~10000 (approximate trimming, `hot_path_emitter.rs:81-83`)
- **Side effect**: also stores the full serialized `Opportunity` at `arbx:hot:opp:{id}` with 300s TTL (see Keys below)
- **Consumer Groups**:
  - `ws-emitter-g0`: api-server `OpportunityHotStreamer` — XREADGROUP → WS room `opportunities`, event `opportunity:detected` (`websocket.ts:885,906`). `paper-executor-g0` consumes `arbx:hot:simulated`, NOT this stream (`paper/executor.ts:20`)

### arbx:hot:simulated (Stream)
- **Purpose**: Store REVM simulation results for every simulation that actually ran (passed | failed, REVM verdict verbatim) <!-- WO-02 (2026-09-06): R-1 of WO-02-FIX un-staled — the stream also carries failed verdicts; pre-REVM candidates are never emitted -->
<!-- WO-02 (2026-09-06): section rewritten per WO-02-DESIGN §5.3 (audits/omniscience-integration-2026-09-06); verified against the live XADD in backend/searcher-rs/src/hot_path_emitter.rs emit_simulated. -->
- **Producer**: searcher-rs decode_and_score_tx (WO-02, 2026-09-06) — post-REVM,
  AFTER the canonical arbx:opps:detected publish; emitted for every sim that
  actually RAN (status passed|failed, REVM verdict verbatim), never for
  candidates that failed before REVM dispatch
- **Fields**:
  - `id`: Opportunity UUID (stream-message correlation)
  - `opportunity_id`: same UUID — PaperExecutor FK into opportunities.id
  - `status`: passed | failed (REVM verdict, verbatim)
  - `net_profit_wei`: decimal string — REVM gross token_in delta; net-of-gas is a downstream decision
  - `gas_used`: gas consumed by the REVM round trip
  - `gas_price_wei`: decimal string — gas price the simulator used
  - `chain_id` / `strategy_kind` / `token_pair`: correlation fields
  - `timestamp_ms`: Unix epoch millis
- **MAXLEN**: ~5000
- **Consumers**: ws-emitter-g0 (api-server OpportunityHotStreamer → WS room
  opportunities, event opportunity:validated), paper-executor-g0
  (dormant unless ARBX_PAPER_EXECUTOR_MODE=on)
<!-- WO-02-CROSS G1/G3 (2026-09-07), added by G3 fixer — additive, no line above touched: -->
- **Liveness (fail-honest)**: the wiring sits in the legacy `decode_and_score_tx`
  leg — under `ARBX_ORCHESTRATOR_MODE=v2` the scanner returns before the sim gate
  (backend/searcher-rs/src/scanner.rs:1589-1592), so this stream stays honestly
  empty (XLEN=0) until the V2 leg is wired or the mode flips; production ran
  mode=v2 with this wiring deployed and XLEN=0 (WO-02-CROSS, 2026-09-07)

### arbx:hot:paper_executed (Stream)
- **Purpose**: Archive paper trade execution outcomes for metrics and audit
<!-- WO-02 (2026-09-06): section rewritten against the live code (gang fix ronda 2 — drift residual WO-02-FIXG3 §4.2); producer/fields/MAXLEN verified against backend/api-server/src/paper/executor.ts:21,209-216,303-319 and index.ts:1900-1917. -->
- **Producer**: api-server `PaperExecutor` (paper shadow terminus; consumer group `paper-executor-g0`, dormant unless `ARBX_PAPER_EXECUTOR_MODE=on` — `index.ts:1904-1917`). Emits one entry per processed passed simulation; `failed` entries from `arbx:hot:simulated` are acked without emitting (`executor.ts:209-216`)
- **Fields** (XADD, `executor.ts:306-318`):
  - `id`: Opportunity UUID (correlated from the `arbx:hot:simulated` entry)
  - `status`: `ACCEPTED` | `REJECTED` (net-yield gate)
  - `net_yield_wei`: decimal string — net Topological Yield (gross − gas − decoherence penalty)
  - `executed_at_ms`: Unix epoch millis
  - `execution_time_ms`: wall time of the executor's processing
  - `rejection_reason`: optional (`net_yield_non_positive`, `calculation_failed`, …)
- **MAXLEN**: ~5000 (`executor.ts:318`)
- **Side effect**: accepted runs persist to PostgreSQL `paper_trade_runs` (USD conversion happens there, NOT on this stream); throughput/latency counters at `arbx:metrics:throughput:paper_executed` and `arbx:metrics:latency:paper_execution` (`executor.ts:321-328`)

## Keys (Short TTL)

### arbx:hot:opp:{id} (Hash)
- **Type**: Hash
- **TTL**: 300 seconds
- **Content**: Complete opportunity data including:
  - Raw detection payload
  - Decoded token symbols
  - Source manifold identifiers
  - Priority score

### arbx:hot:sim:{id} (Hash)
- **Type**: Hash
- **TTL**: 300 seconds
<!-- WO-02 (2026-09-06): content corrected against the live HSET (gang fix ronda 2 — drift residual WO-02-FIXG3 §4 / R-2 of WO-02-FIX); verified against backend/searcher-rs/src/hot_path_emitter.rs:182-199. -->
- **Content**: single `result` field = JSON-serialized `SimulationResult` (`passed`, `net_profit_wei` decimal string, `gas_used`, `gas_price_wei` decimal string). Written ONLY for passed simulations — no REVM trace summary, state diffs or error logs are stored

### arbx:metrics:throughput:detected (String)
- **Type**: String (counter)
- **TTL**: 60 seconds
- **Purpose**: Rolling counter for real-time throughput metrics
- **Updated**: INCR on each detection event
- **Consumed**: Dashboard metric scrapers

## Latency Budgets

| Phase | Target | Stream/Key |
|-------|--------|------------|
| Detection | <20ms | arbx:hot:detected |
| Simulation | <30ms | arbx:hot:simulated |
| Redis Write | <5ms | All streams |
| WebSocket Emit | <5ms | ws-emitter-g0 consumer |
| Edge Response | <10ms | arbx:hot:paper_executed |

## Notes

- All timestamps use Unix milliseconds (UTC)
- Stream trimming uses approximate MAXLEN to balance memory vs accuracy
- TTLs are intentionally short to prevent memory pressure on high-frequency detection
- Consumer groups enable parallel processing with automatic offset tracking
