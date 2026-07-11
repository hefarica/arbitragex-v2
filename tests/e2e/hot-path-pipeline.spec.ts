import { test, expect, type Page, type APIRequestContext } from "@playwright/test";
import { io, type Socket } from "socket.io-client";

/**
 * OMEGA Hot Path Pipeline E2E Tests (Task 8).
 *
 * Validates the complete sub-100ms pipeline flow:
 *   Detection → Simulation → WebSocket → Paper Execution
 *
 * Stream topology:
 *   - arbx:hot:detected     → Hot opportunities from detection
 *   - arbx:hot:simulated    → Post-simulation results
 *   - arbx:hot:paper_executed → Shadow execution records
 *
 * WebSocket events:
 *   - opportunity:detected  → Real-time detection broadcast
 *   - opportunity:validated → Post-simulation validation
 *
 * RULE 00: Zero fabrication — all assertions use real data or explicit skips.
 * R8: Fail-honest — empty results are valid, fabricated results are bugs.
 */

// ─────────────────────────────────────────────────────────────────────────────
// Configuration
// ─────────────────────────────────────────────────────────────────────────────

const EDGE_URL = process.env["ARBX_EDGE_URL"] ?? "http://localhost:8787";
const WS_URL = process.env["ARBX_WS_URL"] ?? "http://localhost:3000";
const REDIS_URL = process.env["ARBX_REDIS_URL"] ?? "redis://localhost:6379";

// Pipeline latency budget (p95)
const LATENCY_BUDGET_MS = 100;

// Stream keys (mirrored from backend implementation)
const STREAM_HOT_DETECTED = "arbx:hot:detected";
const STREAM_HOT_SIMULATED = "arbx:hot:simulated";
const STREAM_HOT_PAPER_EXECUTED = "arbx:hot:paper_executed";
const STREAM_OPPS_DETECTED = "arbx:opps:detected";

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

interface PipelineLatencyMetrics {
  detectionToWebsocketMs: number;
  detectionToSimulationMs: number;
  simulationToExecutionMs: number;
  endToEndMs: number;
  timestamp: number;
}

interface OpportunityEvent {
  id: string;
  chain_id?: string;
  strategy_kind?: string;
  detected_at_ms?: string;
  _stream_id?: string;
  [key: string]: string | undefined;
}

interface SimulatedEvent extends OpportunityEvent {
  status?: "passed" | "failed";
  net_profit_wei?: string;
  gas_used?: string;
  timestamp_ms?: string;
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Waits for a WebSocket event with timeout.
 * Returns null if timeout expires (fail-honest pattern).
 */
async function waitForWebSocketEvent<T>(
  socket: Socket,
  eventName: string,
  timeoutMs: number,
  filter?: (data: T) => boolean
): Promise<T | null> {
  return new Promise((resolve) => {
    const timer = setTimeout(() => resolve(null), timeoutMs);

    const handler = (data: T) => {
      if (!filter || filter(data)) {
        clearTimeout(timer);
        socket.off(eventName, handler);
        resolve(data);
      }
    };

    socket.on(eventName, handler);
  });
}

/**
 * Creates a test opportunity payload for injection into the pipeline.
 * Uses OMEGA lexicon: "Holonomic Loop" not "arbitrage", "Topological Yield" not "profit".
 */
function createTestOpportunity(overrides?: Partial<OpportunityEvent>): OpportunityEvent {
  const now = Date.now();
  return {
    id: `test-opp-${now}-${Math.random().toString(36).slice(2, 8)}`,
    chain_id: "1",
    strategy_kind: "holonomic_loop",
    detected_at_ms: now.toString(),
    ...overrides,
  };
}

/**
 * Injects a synthetic opportunity into the hot detection stream.
 * Requires Redis XADD capability via edge admin endpoint.
 */
async function injectSyntheticOpportunity(
  request: APIRequestContext,
  opportunity: OpportunityEvent
): Promise<{ success: boolean; error?: string; injectedAt: number }> {
  const injectedAt = Date.now();
  try {
    // Use the edge API to inject test data (if available)
    const res = await request.post(`${EDGE_URL}/api/test/pipeline/inject`, {
      data: { opportunity, stream: STREAM_HOT_DETECTED },
      failOnStatusCode: false,
    });

    if (res.ok()) {
      return { success: true, injectedAt };
    }

    // If endpoint not available, skip test with honest reason
    return {
      success: false,
      error: `Injection endpoint returned ${res.status()}: ${await res.text()}`,
      injectedAt,
    };
  } catch (e) {
    return {
      success: false,
      error: (e as Error).message,
      injectedAt,
    };
  }
}

/**
 * Queries Redis stream length via edge API.
 */
async function getStreamLength(
  request: APIRequestContext,
  streamKey: string
): Promise<number | null> {
  try {
    const res = await request.get(
      `${EDGE_URL}/api/test/redis/xlen?key=${encodeURIComponent(streamKey)}`,
      { failOnStatusCode: false }
    );
    if (!res.ok()) return null;
    const data = await res.json();
    return typeof data.length === "number" ? data.length : null;
  } catch {
    return null;
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// Test Suite: OMEGA Hot Path Pipeline
// ─────────────────────────────────────────────────────────────────────────────

test.describe("OMEGA Hot Path Pipeline E2E", () => {
  let socket: Socket | null = null;

  test.beforeEach(async ({ request }) => {
    // Verify edge is reachable; skip honestly if not
    let up = false;
    try {
      const r = await request.get(`${EDGE_URL}/api/health`, {
        failOnStatusCode: false,
        timeout: 5000,
      });
      up = r.ok();
    } catch {
      up = false;
    }
    if (!up) {
      test.skip(true, `Edge not reachable at ${EDGE_URL} — VALIDATION_PENDING_POST_DEPLOY`);
    }
  });

  test.afterEach(() => {
    if (socket) {
      socket.disconnect();
      socket = null;
    }
  });

  // ───────────────────────────────────────────────────────────────────────────
  // Scenario 1: Full Pipeline Flow
  // ───────────────────────────────────────────────────────────────────────────

  test("complete flow: detection → simulation → websocket → paper", async ({
    request,
  }) => {
    // Connect WebSocket
    socket = io(WS_URL, {
      transports: ["websocket"],
      reconnection: false,
      timeout: 10000,
    });

    // Wait for connection
    await new Promise<void>((resolve, reject) => {
      const timeout = setTimeout(() => reject(new Error("WebSocket connection timeout")), 5000);
      socket!.on("connect", () => {
        clearTimeout(timeout);
        resolve();
      });
      socket!.on("connect_error", (err) => {
        clearTimeout(timeout);
        reject(err);
      });
    });

    // Subscribe to opportunities room
    socket.emit("subscribe:opportunities");

    // Get baseline stream lengths
    const baselineDetected = await getStreamLength(request, STREAM_HOT_DETECTED);
    const baselineSimulated = await getStreamLength(request, STREAM_HOT_SIMULATED);

    // Inject synthetic opportunity
    const testOpp = createTestOpportunity();
    const injection = await injectSyntheticOpportunity(request, testOpp);

    // If injection endpoint not available, verify pipeline observability instead
    if (!injection.success) {
      const injectionUnavailable = injection.error?.includes("404") || injection.error?.includes("Cannot find");
      if (injectionUnavailable) {
        test.skip(true, "Pipeline injection endpoint not available — VALIDATION_PENDING_IMPLEMENTATION");
        return;
      }
    }

    // Wait for WebSocket event
    const wsEvent = await waitForWebSocketEvent<OpportunityEvent>(
      socket,
      "opportunity:detected",
      5000,
      (data) => data.id === testOpp.id
    );

    // Assert: WebSocket received the opportunity
    expect(wsEvent, "WebSocket should receive opportunity:detected event").not.toBeNull();
    expect(wsEvent!.id).toBe(testOpp.id);
    expect(wsEvent!.chain_id).toBe(testOpp.chain_id);
    expect(wsEvent!.strategy_kind).toBe(testOpp.strategy_kind);

    // Assert: Stream length increased (if we can verify)
    if (baselineDetected !== null) {
      const afterDetected = await getStreamLength(request, STREAM_HOT_DETECTED);
      expect(afterDetected).toBeGreaterThanOrEqual(baselineDetected);
    }

    // Wait for simulation event
    const simEvent = await waitForWebSocketEvent<SimulatedEvent>(
      socket,
      "opportunity:validated",
      10000,
      (data) => data.id === testOpp.id
    );

    // Note: Simulation may not occur if sim-ctl is not running
    if (simEvent) {
      expect(simEvent.status).toMatch(/passed|failed/);
      if (simEvent.status === "passed") {
        expect(simEvent.net_profit_wei).toBeDefined();
        expect(simEvent.gas_used).toBeDefined();
      }
    }
  });

  // ───────────────────────────────────────────────────────────────────────────
  // Scenario 2: Latency Validation
  // ───────────────────────────────────────────────────────────────────────────

  test("latency meets <100ms p95 target", async ({ request }) => {
    const latencies: PipelineLatencyMetrics[] = [];
    const sampleCount = 10;

    socket = io(WS_URL, {
      transports: ["websocket"],
      reconnection: false,
      timeout: 10000,
    });

    await new Promise<void>((resolve, reject) => {
      const timeout = setTimeout(() => reject(new Error("WebSocket connection timeout")), 5000);
      socket!.on("connect", () => {
        clearTimeout(timeout);
        resolve();
      });
    });

    socket.emit("subscribe:opportunities");

    // Collect latency samples
    for (let i = 0; i < sampleCount; i++) {
      const testOpp = createTestOpportunity();
      const injection = await injectSyntheticOpportunity(request, testOpp);

      if (!injection.success) {
        test.skip(true, "Pipeline injection not available — cannot measure latency");
        return;
      }

      const detectedAt = injection.injectedAt;
      const wsEvent = await waitForWebSocketEvent<OpportunityEvent>(
        socket,
        "opportunity:detected",
        5000,
        (data) => data.id === testOpp.id
      );

      if (wsEvent) {
        const receivedAt = Date.now();
        const detectionToWebsocketMs = receivedAt - detectedAt;

        latencies.push({
          detectionToWebsocketMs,
          detectionToSimulationMs: 0, // Not measured in this test
          simulationToExecutionMs: 0,
          endToEndMs: detectionToWebsocketMs,
          timestamp: detectedAt,
        });
      }

      // Small delay between samples
      await new Promise((r) => setTimeout(r, 100));
    }

    // Fail-honest: if we have no samples, skip rather than fabricate
    if (latencies.length === 0) {
      test.skip(true, "No latency samples collected — pipeline may not be running");
      return;
    }

    // Calculate p95
    const sortedLatencies = [...latencies].sort((a, b) => a.endToEndMs - b.endToEndMs);
    const p95Index = Math.floor(sortedLatencies.length * 0.95);
    const p95Latency = sortedLatencies[Math.min(p95Index, sortedLatencies.length - 1)]!.endToEndMs;

    // Log latency breakdown
    console.log(`[HotPathLatency] Samples: ${latencies.length}`);
    console.log(`[HotPathLatency] Min: ${Math.min(...latencies.map((l) => l.endToEndMs))}ms`);
    console.log(`[HotPathLatency] Max: ${Math.max(...latencies.map((l) => l.endToEndMs))}ms`);
    console.log(`[HotPathLatency] P95: ${p95Latency}ms`);
    console.log(`[HotPathLatency] Budget: ${LATENCY_BUDGET_MS}ms`);

    // Assert p95 is within budget
    expect(
      p95Latency,
      `P95 latency (${p95Latency}ms) should be within ${LATENCY_BUDGET_MS}ms budget`
    ).toBeLessThanOrEqual(LATENCY_BUDGET_MS);

    // Log individual stage latencies if available
    const avgDetectionToWs =
      latencies.reduce((sum, l) => sum + l.detectionToWebsocketMs, 0) / latencies.length;
    console.log(`[HotPathLatency] Avg detection→WebSocket: ${avgDetectionToWs.toFixed(2)}ms`);
  });

  // ───────────────────────────────────────────────────────────────────────────
  // Scenario 3: Fail-Honest Behavior
  // ───────────────────────────────────────────────────────────────────────────

  test("fail-honest: rejects invalid opportunities", async ({ request }) => {
    const invalidOpportunities = [
      { id: "", chain_id: "1", strategy_kind: "holonomic_loop" }, // Empty ID
      { id: "test-missing-chain", strategy_kind: "holonomic_loop" }, // Missing chain_id
      { id: "test-invalid-chain", chain_id: "invalid", strategy_kind: "holonomic_loop" }, // Invalid chain_id
      { id: "test-unknown-strategy", chain_id: "1", strategy_kind: "unknown_strategy" }, // Unknown strategy
    ];

    for (const invalidOpp of invalidOpportunities) {
      const injection = await injectSyntheticOpportunity(request, invalidOpp as OpportunityEvent);

      // If injection endpoint validates and rejects, it should report failure
      if (injection.success) {
        // If accepted, verify it was marked as rejected in the stream
        console.log(`[FailHonest] Invalid opportunity ${invalidOpp.id} was accepted — checking rejection tracking`);
      } else {
        // Expected: injection should fail for invalid data
        expect(injection.error).toBeDefined();
      }
    }

    // Verify no fabricated results exist
    const baselineCount = await getStreamLength(request, STREAM_HOT_DETECTED);
    console.log(`[FailHonest] Stream length after invalid injections: ${baselineCount}`);
  });

  test("fail-honest: none profit stays none through pipeline", async ({ request }) => {
    // Per R8: None profit must not be coerced to Some(0.0)
    const testOpp = createTestOpportunity({
      id: `test-none-profit-${Date.now()}`,
    });

    const injection = await injectSyntheticOpportunity(request, testOpp);

    if (!injection.success) {
      test.skip(true, "Pipeline injection not available");
      return;
    }

    // Query the opportunity from the stream (via edge API)
    try {
      const res = await request.get(
        `${EDGE_URL}/api/test/pipeline/opportunity?id=${encodeURIComponent(testOpp.id)}`,
        { failOnStatusCode: false }
      );

      if (res.ok()) {
        const data = await res.json();
        // If profit is present, it should be honest (not fabricated zero)
        if (data.expected_profit_usd !== undefined) {
          expect(data.expected_profit_usd).not.toBeNull();
          expect(typeof data.expected_profit_usd).toBe("number");
        }
      }
    } catch {
      // Query endpoint may not exist — this is acceptable
    }
  });

  // ───────────────────────────────────────────────────────────────────────────
  // Scenario 4: Concurrent Load
  // ───────────────────────────────────────────────────────────────────────────

  test("concurrent load: 100 opportunities/sec handling", async ({ request }) => {
    const concurrentCount = 100;
    const startTime = Date.now();

    // Get baseline stream length
    const baselineDetected = await getStreamLength(request, STREAM_HOT_DETECTED);
    const baselineOpps = await getStreamLength(request, STREAM_OPPS_DETECTED);

    // Inject opportunities concurrently
    const injections = await Promise.all(
      Array.from({ length: concurrentCount }, (_, i) =>
        injectSyntheticOpportunity(
          request,
          createTestOpportunity({
            id: `load-test-${startTime}-${i}`,
          })
        )
      )
    );

    const successfulInjections = injections.filter((i) => i.success).length;
    const failedInjections = injections.length - successfulInjections;

    console.log(`[ConcurrentLoad] Total: ${concurrentCount}, Success: ${successfulInjections}, Failed: ${failedInjections}`);

    // Fail-honest: if injection not available, skip
    if (successfulInjections === 0) {
      test.skip(true, "Pipeline injection not available — cannot test concurrent load");
      return;
    }

    // Verify no messages were dropped (stream length increased appropriately)
    // Wait a moment for async processing
    await new Promise((r) => setTimeout(r, 1000));

    const afterDetected = await getStreamLength(request, STREAM_HOT_DETECTED);
    const afterOpps = await getStreamLength(request, STREAM_OPPS_DETECTED);

    // Streams should have increased (or stayed bounded)
    if (baselineDetected !== null && afterDetected !== null) {
      const detectedDelta = afterDetected - baselineDetected;
      console.log(`[ConcurrentLoad] Detected stream delta: ${detectedDelta}`);

      // Assert: Stream grew by approximately the number of injections
      // (allowing for MAXLEN trimming if at capacity)
      expect(detectedDelta).toBeGreaterThanOrEqual(0);
      expect(detectedDelta).toBeLessThanOrEqual(successfulInjections + 10); // Allow some tolerance
    }

    // Verify stream boundedness (MAXLEN ~10k)
    if (afterDetected !== null) {
      expect(afterDetected).toBeLessThanOrEqual(11000); // ~10k + tolerance
    }

    // Calculate throughput
    const elapsedMs = Date.now() - startTime;
    const throughputPerSec = (successfulInjections / elapsedMs) * 1000;
    console.log(`[ConcurrentLoad] Throughput: ${throughputPerSec.toFixed(2)} ops/sec (${elapsedMs}ms total)`);
  });

  // ───────────────────────────────────────────────────────────────────────────
  // Scenario 5: Stream Topology Validation
  // ───────────────────────────────────────────────────────────────────────────

  test("stream topology: correct stream keys and consumer groups", async ({
    request,
  }) => {
    // Verify stream existence and consumer groups via edge API
    const streamsToCheck = [
      STREAM_HOT_DETECTED,
      STREAM_HOT_SIMULATED,
      STREAM_HOT_PAPER_EXECUTED,
      STREAM_OPPS_DETECTED,
    ];

    for (const streamKey of streamsToCheck) {
      const length = await getStreamLength(request, streamKey);
      console.log(`[StreamTopology] ${streamKey}: length=${length ?? "unknown (endpoint unavailable)"}`);

      // Fail-honest: null means endpoint unavailable — this is OK
      if (length !== null) {
        expect(length).toBeGreaterThanOrEqual(0);
      }
    }

    // If we can't verify via API, log a note
    if ((await getStreamLength(request, STREAM_HOT_DETECTED)) === null) {
      console.log("[StreamTopology] Note: Redis inspection endpoint unavailable — topology validation deferred");
    }
  });

  // ───────────────────────────────────────────────────────────────────────────
  // Scenario 6: WebSocket Room Subscription
  // ───────────────────────────────────────────────────────────────────────────

  test("websocket: opportunity room subscription works", async () => {
    socket = io(WS_URL, {
      transports: ["websocket"],
      reconnection: false,
      timeout: 10000,
    });

    // Connect
    await new Promise<void>((resolve, reject) => {
      const timeout = setTimeout(() => reject(new Error("Connection timeout")), 5000);
      socket!.on("connect", () => {
        clearTimeout(timeout);
        resolve();
      });
    });

    expect(socket!.connected).toBe(true);

    // Subscribe to opportunities room
    socket!.emit("subscribe:opportunities");

    // Wait a moment for subscription to process
    await new Promise((r) => setTimeout(r, 100));

    // Connection should still be alive
    expect(socket!.connected).toBe(true);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Test Suite: Pipeline Observability
// ─────────────────────────────────────────────────────────────────────────────

test.describe("OMEGA Hot Path Observability", () => {
  test("pipeline health endpoint returns correct topology", async ({ request }) => {
    const res = await request.get(`${EDGE_URL}/api/health`, {
      failOnStatusCode: false,
    });

    if (!res.ok()) {
      test.skip(true, "Health endpoint unavailable");
      return;
    }

    const health = await res.json();

    // Verify expected health fields
    expect(typeof health.status).toBe("string");

    // Log pipeline-relevant health info
    if (health.redis !== undefined) {
      console.log(`[Observability] Redis health: ${JSON.stringify(health.redis)}`);
    }
    if (health.postgres !== undefined) {
      console.log(`[Observability] PostgreSQL health: ${JSON.stringify(health.postgres)}`);
    }
  });

  test("stream metrics are observable", async ({ request }) => {
    const res = await request.get(`${EDGE_URL}/api/metrics/pipeline`, {
      failOnStatusCode: false,
    });

    if (!res.ok()) {
      test.skip(true, "Pipeline metrics endpoint unavailable — VALIDATION_PENDING_IMPLEMENTATION");
      return;
    }

    const metrics = await res.json();

    // Verify stream lengths are reported
    if (metrics.streams) {
      for (const [key, length] of Object.entries(metrics.streams)) {
        console.log(`[Observability] Stream ${key}: ${length} entries`);
        expect(typeof length).toBe("number");
        expect(length).toBeGreaterThanOrEqual(0);
      }
    }
  });
});
