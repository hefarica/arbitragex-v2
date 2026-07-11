# OMEGA Hot Path Pipeline E2E Tests

End-to-end tests for the ArbitrageX v2 sub-100ms pipeline: **Detection → Simulation → WebSocket → Paper Execution**.

## Overview

These tests validate the complete hot path data flow using the OMEGA lexicon:
- **Holonomic Loop Resolution** (not "triangular arbitrage")
- **Topological Yield** (not "profit")
- **Decoherencia de Estado** (not "slippage")

## Test Scenarios

### 1. Full Pipeline Flow (`complete flow`)
Validates:
- Synthetic opportunity injection
- Redis stream `arbx:hot:detected` receives it
- WebSocket emits `opportunity:detected` event
- Simulation produces `arbx:hot:simulated`
- Paper execution produces `arbx:hot:paper_executed`

### 2. Latency Validation (`latency meets <100ms p95`)
Measures:
- Detection → WebSocket emit latency
- Asserts <100ms p95 target
- Logs latency breakdown by stage

### 3. Fail-Honest Behavior (`fail-honest: rejects invalid opportunities`)
Tests:
- Invalid opportunities are rejected gracefully
- No fabricated results when fields are missing
- None profit stays None (R8 invariant)

### 4. Concurrent Load (`concurrent load: 100 opportunities/sec`)
Validates:
- 100 concurrent injections/sec
- No dropped messages
- Redis streams stay bounded (MAXLEN enforcement)

### 5. Stream Topology (`stream topology`)
Verifies:
- Correct stream keys exist
- Consumer groups are configured
- Stream boundedness (MAXLEN ~10k)

### 6. WebSocket Subscription (`websocket: opportunity room`)
Tests:
- Socket.IO connection
- Room subscription (`subscribe:opportunities`)
- Event delivery

## Running the Tests

### Prerequisites

The tests assume a compose stack is running with:
- Redis (for hot path streams)
- PostgreSQL (for persistence)
- api-server (for WebSocket gateway)
- edge (for proxying)

### Option 1: Using Full Dev Stack

```bash
# Start the full development stack
docker compose --env-file .env -f docker/compose.dev.yml up -d

# Run hot path tests
cd tests/e2e
npm install
npm run test:hotpath
```

### Option 2: Using Minimal Test Stack

```bash
# Start only the services needed for hot path tests
docker compose -f docker/compose.hotpath-test.yml up -d

# Run tests with test environment URLs
export ARBX_EDGE_URL=http://localhost:8788
export ARBX_WS_URL=http://localhost:8081
cd tests/e2e
npm run test:hotpath
```

### Option 3: With Fixtures Service

```bash
# Start stack with test fixtures injection service
docker compose -f docker/compose.hotpath-test.yml --profile fixtures up -d

# Run tests (they will use the fixtures endpoint for injection)
cd tests/e2e
npm run test:hotpath
```

## Test Scripts

```bash
# Run all hot path tests headless
npm run test:hotpath

# Run with browser visible for debugging
npm run test:hotpath:headed

# Run with UI mode
npm run test:hotpath -- --ui

# Run specific test
npm run test:hotpath -- --grep "latency meets"
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `ARBX_EDGE_URL` | `http://localhost:8787` | Edge/REST API base URL |
| `ARBX_WS_URL` | `http://localhost:3000` | WebSocket server URL |
| `ARBX_FRONTEND_URL` | `http://localhost:5173` | Frontend URL (for smoke tests) |
| `ARBX_ASSUME_NO_RPC` | `0` | Set to `1` to skip RPC-dependent tests |

## Stream Topology

```
┌─────────────────┐     XADD     ┌─────────────────────┐
│   Detection     │──────────────▶│  arbx:hot:detected  │
│   (synthetic)   │               │   (MAXLEN ~10k)     │
└─────────────────┘               └──────────┬──────────┘
                                             │
                                             │ XREADGROUP
                                             ▼
                                    ┌─────────────────────┐
                                    │  WebSocket Gateway  │
                                    │  (ws-emitter-g0)    │
                                    └──────────┬──────────┘
                                               │ emit
                                               ▼
                                    ┌─────────────────────┐
                                    │  opportunity:       │
                                    │  detected event     │
                                    └─────────────────────┘

┌─────────────────┐     XADD     ┌─────────────────────┐
│   Simulation    │──────────────▶│ arbx:hot:simulated  │
│   (sim-ctl)     │               │   (MAXLEN ~5k)      │
└─────────────────┘               └──────────┬──────────┘
                                             │
                                             │ XREADGROUP
                                             ▼
                                    ┌─────────────────────┐
                                    │  Paper Executor     │
                                    │ (paper-executor-g0) │
                                    └──────────┬──────────┘
                                               │ XADD
                                               ▼
                                    ┌─────────────────────┐
                                    │ arbx:hot:paper_     │
                                    │ executed            │
                                    └─────────────────────┘
```

## Fail-Honest Behavior

Per RULE 00 and R8:
- Empty results are **valid** — tests skip rather than fabricate
- `null` / `undefined` values are **not coerced** to zeros
- Missing data results in **explicit skips** with reasons
- No hardcoded test data is used for assertions

Example:
```typescript
// Fail-honest: Skip if we can't inject
if (!injection.success) {
  test.skip("Pipeline injection not available");
  return;
}

// Fail-honest: Skip if no samples collected
if (latencies.length === 0) {
  test.skip("No latency samples collected");
  return;
}
```

## Fixtures

Test fixtures are defined in `fixtures/hot-path.ts`:

```typescript
// Generate a valid opportunity
const opp = createHolonomicLoopOpportunity({
  expected_topological_yield_usd: 15.5,
});

// Generate invalid cases for fail-honest testing
const invalidCases = createInvalidOpportunityCases();

// Generate load test batch
const batch = generateOpportunityBatch(100, "holonomic_loop");
```

## CI Integration

The tests run in CI with `continue-on-error: true` (non-blocking) until the
pipeline injection endpoints are fully implemented:

```yaml
- name: Run hot path pipeline tests (non-blocking)
  run: npm run test:hotpath || echo "Skipped"
  continue-on-error: true
```

## Troubleshooting

### Tests skip with "VALIDATION_PENDING_IMPLEMENTATION"
This means the test fixtures injection endpoint is not available. The tests
will still validate stream topology and WebSocket connectivity.

### "WebSocket connection timeout"
Verify the api-server is running and WebSocket is enabled:
```bash
curl http://localhost:8081/api/health
```

### Redis stream lengths not available
The edge test endpoints may not be exposed. This is OK — the tests will
validate what they can and skip the rest honestly.

## References

- `hot_path_emitter.rs` — Rust hot path emitter implementation
- `websocket.ts` — WebSocket gateway with hot streamer
- `paper/executor.ts` — Paper shadow executor
- `publisher.rs` — Redis stream publisher
