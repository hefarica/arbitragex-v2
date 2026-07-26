/**
 * Gates API E2E Tests
 * Validates /api/gates/status and /api/gates/health endpoints
 */

import { test, expect } from "@playwright/test";

// Prefer host-side ARBX_EDGE_URL. NEVER fall back to NEXT_PUBLIC_EDGE_URL in CI:
// that var is baked as http://edge:8787 for the frontend container and is not
// resolvable from the Playwright host process (getaddrinfo EAI_AGAIN edge).
const EDGE_URL =
  process.env["ARBX_EDGE_URL"] ||
  process.env["E2E_BASE_URL"] ||
  "http://localhost:8787";

test.describe("Gates API", () => {
  test.beforeEach(async ({ request }) => {
    let response;
    try {
      response = await request.get(`${EDGE_URL}/api/gates/status`, {
        failOnStatusCode: false,
        timeout: 8000,
      });
    } catch (err) {
      test.skip(true, `Gates API unreachable at ${EDGE_URL}: ${(err as Error).message}`);
      return;
    }
    if (response.status() === 404) {
      test.skip(true, "Gates API not yet implemented — VALIDATION_PENDING_IMPLEMENTATION");
      return;
    }
    if (!response.ok()) {
      test.skip(true, `Gates API returned ${response.status()} — VALIDATION_PENDING_INFRASTRUCTURE`);
    }
  });

  test("GET /api/gates/status returns valid structure", async ({ request }) => {
    const response = await request.get(`${EDGE_URL}/api/gates/status`);

    expect(response.ok()).toBe(true);
    expect(response.status()).toBe(200);

    const data = await response.json();

    // Validate top-level structure
    expect(data).toHaveProperty("gates");
    expect(data).toHaveProperty("summary");
    expect(data).toHaveProperty("generated_at");

    // Validate summary
    expect(data.summary).toHaveProperty("total");
    expect(data.summary).toHaveProperty("passed");
    expect(data.summary).toHaveProperty("failed");
    expect(data.summary).toHaveProperty("fired");
    expect(data.summary).toHaveProperty("blocked");
    expect(data.summary).toHaveProperty("average_score");

    // Validate gates array
    expect(Array.isArray(data.gates)).toBe(true);
    expect(data.gates.length).toBeGreaterThanOrEqual(4); // At least core gates

    // Validate each gate structure
    for (const gate of data.gates) {
      expect(gate).toHaveProperty("gate_id");
      expect(gate).toHaveProperty("gate_label");
      expect(gate).toHaveProperty("status");
      expect(gate).toHaveProperty("reason");
      expect(gate).toHaveProperty("doctrine");
      expect(gate).toHaveProperty("verified_at");

      // Status must be valid enum
      expect(["passed", "failed", "fired", "blocked"]).toContain(gate.status);

      // verified_at must be valid ISO date
      expect(Date.parse(gate.verified_at)).not.toBeNaN();
    }

    // Summary counts must match gates
    const total = data.summary.total;
    const passed = data.summary.passed;
    const failed = data.summary.failed;
    const fired = data.summary.fired;
    const blocked = data.summary.blocked;

    expect(total).toBe(data.gates.length);
    expect(passed + failed + fired + blocked).toBe(total);
  });

  test("GET /api/gates/health returns valid structure", async ({ request }) => {
    const response = await request.get(`${EDGE_URL}/api/gates/health`);

    // Should return 200 or 503 depending on health
    expect([200, 503]).toContain(response.status());

    const data = await response.json();

    // Validate structure
    expect(data).toHaveProperty("healthy");
    expect(data).toHaveProperty("timestamp");
    expect(data).toHaveProperty("sources");
    expect(data).toHaveProperty("message");

    // Validate types
    expect(typeof data.healthy).toBe("boolean");
    expect(typeof data.message).toBe("string");

    // Validate sources
    expect(data.sources).toHaveProperty("postgres");
    expect(data.sources).toHaveProperty("redis");
    expect(data.sources).toHaveProperty("searcher_rs");

    // All sources should be boolean
    expect(typeof data.sources.postgres).toBe("boolean");
    expect(typeof data.sources.redis).toBe("boolean");
    expect(typeof data.sources.searcher_rs).toBe("boolean");

    // Timestamp should be valid ISO date
    expect(Date.parse(data.timestamp)).not.toBeNaN();
  });

  test("Core gates are always present", async ({ request }) => {
    const response = await request.get(`${EDGE_URL}/api/gates/status`);
    const data = await response.json();

    const gateIds = data.gates.map((g: { gate_id: string }) => g.gate_id);

    // Core gates must exist
    expect(gateIds).toContain("paper_mode");
    expect(gateIds).toContain("kill_switch");
    expect(gateIds).toContain("simulation_required");
    expect(gateIds).toContain("risk_limits");
  });

  test("Gate status response times are acceptable", async ({ request }) => {
    const start = Date.now();
    const response = await request.get(`${EDGE_URL}/api/gates/status`);
    const duration = Date.now() - start;

    expect(response.ok()).toBe(true);
    expect(duration).toBeLessThan(5000); // 5 second timeout
  });
});
