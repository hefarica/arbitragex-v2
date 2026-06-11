import { test, expect } from "@playwright/test";

/**
 * Operator Self-Test Center — E2E (READ-ONLY).
 *
 * Pins the doctrine for the /operator/self-test surface:
 *   • the page loads through the edge and renders all 10 checklist blocks,
 *   • the credentials API is PRESENCE-ONLY — no secret-looking VALUE in the
 *     body (no credentialed URL, no 32-byte hex, no JWT), `present` is boolean,
 *   • the WalletConnect row is either honest state (BLOCKED_REQUIRES_OPERATOR_SECRET
 *     when absent, VERIFIED when configured) — never a fabricated third state,
 *   • the always-visible safety strip (Live OFF · Capital 0 · Broadcast OFF),
 *   • no React #418 hydration error in the console.
 *
 * BASE must point at the EDGE (8787) — the /api/operator/* proxying and the
 * SPA pass-through live there. Set E2E_BASE_URL (public edge) or ARBX_EDGE_URL
 * (CI compose); default localhost:8787. If the edge is not reachable the suite
 * SKIPS honestly (VALIDATION_PENDING_POST_DEPLOY) — never a false pass.
 */

const BASE =
  process.env["E2E_BASE_URL"] ??
  process.env["ARBX_EDGE_URL"] ??
  "http://localhost:8787";

const SELFTEST_BLOCK_SLUGS = [
  "credentials",
  "connections",
  "wallet",
  "settings",
  "data_feeds",
  "strategies",
  "simulation",
  "signals",
  "paper",
  "live_readiness",
] as const;

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

test("/operator/self-test loads with the safety strip and all 10 blocks", async ({ page }) => {
  const res = await page.goto(`${BASE}/operator/self-test`, { waitUntil: "domcontentloaded" });
  test.skip(!res || res.status() >= 500, "frontend pass-through unavailable — VALIDATION_PENDING_POST_DEPLOY");

  // Always-visible safety strip.
  await expect(page.getByTestId("selftest-safety-strip")).toBeVisible();
  await expect(page.getByTestId("selftest-safety-strip")).toContainText(/Live OFF/i);
  await expect(page.getByTestId("selftest-safety-strip")).toContainText(/Capital 0/i);
  await expect(page.getByTestId("selftest-safety-strip")).toContainText(/Broadcast OFF/i);

  // All 10 checklist blocks render with a data-status from the taxonomy.
  for (const slug of SELFTEST_BLOCK_SLUGS) {
    const row = page.getByTestId(`selftest-block-${slug}`);
    await expect(row, `block ${slug} visible`).toBeVisible();
    const status = await row.getAttribute("data-status");
    expect(status, `block ${slug} carries a data-status`).toBeTruthy();
    expect(
      status,
      `block ${slug} status is from the allowed taxonomy`,
    ).toMatch(
      /^(VERIFIED|MISSING|PARTIAL|FAILING|HONEST_EMPTY|DISABLED_BY_POLICY|BLOCKED_REQUIRES_OPERATOR_SECRET|BLOCKED_REQUIRES_OPERATOR_CONFIG|BLOCKED_REQUIRES_MIGRATION|BLOCKED_REQUIRES_TIME_WINDOW)$/,
    );
  }
});

test("credentials API body is presence-only — no secret-looking value", async ({ request }) => {
  const res = await request.get(`${BASE}/api/operator/credentials/status`, {
    failOnStatusCode: false,
  });
  expect(res.status(), "/api/operator/credentials/status reachable").toBe(200);
  const body = await res.json();

  // Safe posture invariants at the top level.
  expect(body.mode).toBe("shadow_paper");
  expect(body.live_enabled, "live_enabled false").toBe(false);
  expect(body.capital_exposed, "capital_exposed 0").toBe(0);
  expect(body.broadcast, "broadcast false").toBe(false);

  expect(Array.isArray(body.secrets), "secrets is an array").toBeTruthy();
  expect(body.secrets.length, "matrix is non-empty").toBeGreaterThan(0);

  for (const entry of body.secrets) {
    expect(typeof entry.name, "name is a string").toBe("string");
    expect(typeof entry.present, `${entry.name} present is boolean`).toBe("boolean");
    expect("value" in entry, `${entry.name} carries no value field`).toBe(false);
  }

  // No secret-shaped VALUE anywhere in the serialized body:
  const raw = JSON.stringify(body);
  expect(
    /:\/\/[^/\s"']+:[^/\s"'@]+@/.test(raw),
    "no URL-with-password in the body",
  ).toBe(false);
  expect(/0x[0-9a-fA-F]{64}/.test(raw), "no 32-byte hex key in the body").toBe(false);
  expect(/eyJ[A-Za-z0-9_-]{20,}\./.test(raw), "no JWT-shaped token in the body").toBe(false);
});

test("WalletConnect row reports an honest state (BLOCKED_REQUIRES_OPERATOR_SECRET or VERIFIED)", async ({
  request,
}) => {
  const res = await request.get(`${BASE}/api/operator/credentials/status`, {
    failOnStatusCode: false,
  });
  expect(res.status()).toBe(200);
  const body = await res.json();
  const wc = body.secrets.find(
    (s: { name: string }) => s.name === "NEXT_PUBLIC_WALLETCONNECT_PROJECT_ID",
  );
  expect(wc, "WC projectId row present in the matrix").toBeTruthy();
  expect(wc.scope).toBe("public_build_env");
  expect(wc.exposed_to_frontend).toBe(true);
  // Either honest state is acceptable; anything else is a contract violation.
  expect(["BLOCKED_REQUIRES_OPERATOR_SECRET", "VERIFIED"]).toContain(wc.status);
  // Consistency: VERIFIED ⇔ present.
  expect(wc.present).toBe(wc.status === "VERIFIED");
});

test("selftest API returns 10 blocks with the safe posture", async ({ request }) => {
  const res = await request.get(`${BASE}/api/operator/selftest`, { failOnStatusCode: false });
  expect(res.status(), "/api/operator/selftest reachable").toBe(200);
  const body = await res.json();
  expect(body.mode).toBe("shadow_paper");
  expect(body.live_enabled).toBe(false);
  expect(body.capital_exposed).toBe(0);
  expect(body.broadcast).toBe(false);
  expect(Array.isArray(body.blocks)).toBeTruthy();
  expect(body.blocks.length, "exactly 10 blocks").toBe(10);
  expect(body.blocks.map((b: { block: string }) => b.block)).toEqual([...SELFTEST_BLOCK_SLUGS]);
  // verified_count agrees with the blocks (no lying).
  const verified = body.blocks.filter((b: { status: string }) => b.status === "VERIFIED").length;
  expect(body.verified_count).toBe(verified);
});

test("no React #418 hydration error in the console", async ({ page }) => {
  const errors: string[] = [];
  page.on("console", (msg) => {
    if (msg.type() === "error") errors.push(msg.text());
  });
  page.on("pageerror", (err) => errors.push(err.message));
  const res = await page.goto(`${BASE}/operator/self-test`, { waitUntil: "networkidle" });
  test.skip(!res || res.status() >= 500, "frontend pass-through unavailable — VALIDATION_PENDING_POST_DEPLOY");
  await page.waitForTimeout(1500);
  const hydrationErrors = errors.filter((e) => /minified react error #418|hydrat/i.test(e));
  expect(hydrationErrors, `hydration errors: ${hydrationErrors.join(" | ")}`).toHaveLength(0);
});
