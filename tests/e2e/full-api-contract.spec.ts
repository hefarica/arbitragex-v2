import { test, expect } from "@playwright/test";

/**
 * FE-CRIT-03/04/01 — honest API-contract sweep (READ-ONLY).
 *
 * Pins the doctrine for the contract/capital/crucible read surface and the
 * system-manifest proxies:
 *   • Safety posture is HARDCODED-safe (live_enabled / broadcast / submit false,
 *     capital_exposed 0) regardless of data.
 *   • Empty data is HONEST-EMPTY with an explicit `reason` — never fabricated,
 *     never a 500.
 *   • crucible `ready` is false on empty AND carries the false_green_guard flag.
 *   • /api/system/drift + /feature_manifest respond OR fail honestly (no fake 200).
 *
 * BASE must point at the EDGE (8787) — NOT the frontend (5173) — because the
 * /api/* proxying lives at the edge. Set E2E_BASE_URL (public edge) or
 * ARBX_EDGE_URL (CI compose); default localhost:8787. (ARBX_FRONTEND_URL points
 * at the Next frontend, which has no /api/contracts route — hence not used here.)
 * If the edge is not reachable the suite SKIPS honestly
 * (VALIDATION_PENDING_POST_DEPLOY) — never a false pass.
 */

const BASE =
  process.env["E2E_BASE_URL"] ??
  process.env["ARBX_EDGE_URL"] ??
  "http://localhost:8787";

test.describe.configure({ mode: "serial" });

// Honest skip when the edge isn't reachable (e.g. compose not up) — never a fake green.
test.beforeEach(async ({ request }) => {
  let up = false;
  try {
    const r = await request.get(`${BASE}/api/health`, { failOnStatusCode: false, timeout: 8000 });
    up = r.ok();
  } catch {
    up = false;
  }
  test.skip(!up, `edge not reachable at ${BASE} — VALIDATION_PENDING_POST_DEPLOY`);
});

test("/api/contracts returns the honest contract shape (safe posture)", async ({
  request,
}) => {
  const res = await request.get(`${BASE}/api/contracts`, { failOnStatusCode: false });
  expect(res.status(), "/api/contracts reachable (never a 5xx)").toBe(200);
  const body = await res.json();

  expect(body.status, "status ok").toBe("ok");
  // SAFETY invariants — must always be the safe posture.
  expect(body.live_enabled, "live_enabled false").toBe(false);
  expect(body.broadcast, "broadcast false").toBe(false);
  expect(body.capital_exposed, "capital_exposed 0").toBe(0);

  // DATA honesty — contracts is an array, count matches its length.
  expect(Array.isArray(body.contracts), "contracts is an array").toBeTruthy();
  expect(typeof body.count, "count is a number").toBe("number");
  expect(body.count, "count matches contracts length (no lying)").toBe(
    body.contracts.length,
  );
  // Empty must carry an explicit reason (honest-empty, not silent).
  if (body.count === 0) {
    expect(typeof body.reason, "empty contracts carry a reason").toBe("string");
    expect(body.reason.length, "reason is non-empty").toBeGreaterThan(0);
  }
});

test("/api/capital-gates reflects the zero-capital safety posture", async ({
  request,
}) => {
  const res = await request.get(`${BASE}/api/capital-gates`, { failOnStatusCode: false });
  expect(res.status(), "/api/capital-gates reachable").toBe(200);
  const body = await res.json();

  expect(body.status, "status ok").toBe("ok");
  expect(body.live_enabled, "live_enabled false").toBe(false);
  expect(body.capital_exposed, "capital_exposed 0").toBe(0);
  expect(body.broadcast, "broadcast false").toBe(false);
  expect(body.submit_enabled, "submit_enabled false").toBe(false);
  expect(body.private_relay_enabled, "private_relay_enabled false").toBe(false);

  expect(Array.isArray(body.gates), "gates is an array").toBeTruthy();
  // The exposure-proof gate must always be present and PASS at value 0.
  const exposure = body.gates.find(
    (g: { name?: string }) => g.name === "capital_exposure",
  );
  expect(exposure, "capital_exposure gate present").toBeTruthy();
  expect(exposure.status.toLowerCase(), "capital_exposure PASS").toBe("pass");
  expect(exposure.value, "capital_exposure value 0").toBe(0);
});

test("/api/crucible/status holds the false-green guard (ready=false on empty)", async ({
  request,
}) => {
  const res = await request.get(`${BASE}/api/crucible/status`, { failOnStatusCode: false });
  expect(res.status(), "/api/crucible/status reachable (never a 5xx)").toBe(200);
  const body = await res.json();

  expect(body.status, "status ok").toBe("ok");
  expect(body.false_green_guard, "false_green_guard flag present").toBe(true);
  expect(Array.isArray(body.rows), "rows is an array").toBeTruthy();
  expect(typeof body.count, "count is a number").toBe("number");
  expect(body.count, "count matches rows length").toBe(body.rows.length);

  // FALSE-GREEN GUARD: ready can only be true when there are qualifying rows.
  if (body.count === 0) {
    expect(body.ready, "ready=false on empty crucible").toBe(false);
    expect(typeof body.reason, "empty crucible carries a reason").toBe("string");
  } else {
    // If rows exist, ready must equal "every row qualified" — never a free true.
    const allQualified = body.rows.every(
      (r: { qualified?: boolean }) => r.qualified === true,
    );
    expect(body.ready, "ready agrees with per-row qualification").toBe(allQualified);
  }
});

test("/api/system/drift responds or fails honestly (no fabricated 200)", async ({
  request,
}) => {
  const res = await request.get(`${BASE}/api/system/drift`, { failOnStatusCode: false });
  // Acceptable: 200 with real drift payload, OR an honest upstream/error status.
  // What is NOT acceptable is a 200 with a synthesized/empty-but-claimed-ok body
  // that hides an upstream failure — the api-server returns the real {drift,count}.
  if (res.status() === 200) {
    const body = await res.json();
    expect(
      Array.isArray(body.drift) || body.drift === null || body.error !== undefined,
      "200 drift body is the real shape (drift array) or an explicit error",
    ).toBeTruthy();
  } else {
    // Honest failure surfaced verbatim — must be a 4xx/5xx, not a fake success.
    expect(res.status(), "drift honest non-2xx").toBeGreaterThanOrEqual(400);
  }
});

test("/api/system/feature_manifest responds or fails honestly", async ({
  request,
}) => {
  const res = await request.get(`${BASE}/api/system/feature_manifest`, {
    failOnStatusCode: false,
  });
  if (res.status() === 200) {
    const body = await res.json();
    expect(
      Array.isArray(body.features) || body.error !== undefined,
      "200 manifest body is the real shape (features array) or an explicit error",
    ).toBeTruthy();
  } else {
    expect(res.status(), "manifest honest non-2xx").toBeGreaterThanOrEqual(400);
  }
});
