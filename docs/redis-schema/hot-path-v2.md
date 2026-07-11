# Redis Hot Path Schema v2

Documentation for Redis Streams and Keys used in the <100ms latency pipeline.

## Overview

This schema defines the hot path data structures for the OMEGA pipeline, designed for sub-100ms end-to-end latency from detection to paper execution.

## Redis Streams (Hot Path v2)

### arbx:hot:detected (Stream)
- **Purpose**: Ingest raw opportunities detected by the searcher-rs observer
- **Producer**: searcher-rs (upon detecting a Holonomic Loop opportunity)
- **Consumers**: paper-executor-g0, ws-emitter-g0
- **Fields**:
  - `id`: Unique identifier (UUID v4)
  - `chain_id`: Target blockchain network identifier
  - `strategy_kind`: Topology type (e.g., `HolonomicLoopResolution`)
  - `token_path[]`: Array of token addresses in the loop path
  - `amounts[]`: Array of input amounts for each hop
  - `detected_at_ms`: Unix timestamp in milliseconds
- **MAXLEN**: ~10000 (automatic trimming to maintain memory bounds)
- **Consumer Groups**:
  - `paper-executor-g0`: Processes opportunities for simulation
  - `ws-emitter-g0`: Emits to WebSocket clients for real-time UI updates

### arbx:hot:simulated (Stream)
- **Purpose**: Store REVM simulation results for opportunities that passed validation
- **Producer**: searcher-rs (post-REVM simulation, only for passed results)
- **Fields**:
  - `id`: Reference to the original opportunity
  - `sim_result`: JSON-encoded simulation output
  - `net_profit_wei`: Net Topological Yield in wei (after gas estimation)
  - `gas_used`: Estimated gas consumption
  - `trace_hash`: Hash of the execution trace for verification
- **MAXLEN**: ~5000

### arbx:hot:paper_executed (Stream)
- **Purpose**: Archive paper trade execution results for metrics and audit
- **Producer**: api-server paper archiver component
- **Fields**:
  - `id`: Reference to the executed opportunity
  - `execution_time_ms`: Duration from detection to execution completion
  - `paper_pnl_usd`: Paper Topological Yield in USD (for dashboard metrics)
  - `status`: Execution status (`success`, `failed`, `rejected`)
- **MAXLEN**: ~1000

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
- **Content**: Simulation result details including:
  - Full REVM trace summary
  - State diffs
  - Error logs (if failed)

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
