import { test, expect } from "@playwright/test";

// The same read-only test can run against CI compose or the authorized public
// domain via ARBX_FRONTEND_URL. No route mocks, toggles, funding or transactions.
test.describe("readiness status panels", () => {
  test("both panels render actual API responses without the five-second abort", async ({ page }) => {
    await page.route("**/*", route => {
      if (!["GET", "HEAD", "OPTIONS"].includes(route.request().method())) {
        return route.abort("blockedbyclient");
      }
      return route.continue();
    });
    const scoringResponse = page.waitForResponse(r =>
      new URL(r.url()).pathname === "/api/scoring/status", { timeout: 12_000 });
    const breakersResponse = page.waitForResponse(r =>
      new URL(r.url()).pathname === "/api/risk/circuit-breakers/status", { timeout: 12_000 });
    const responses = Promise.all([scoringResponse, breakersResponse]);
    const navigation = await page.goto("/live-readiness", { waitUntil: "domcontentloaded" });
    expect(navigation?.status()).toBe(200);
    const [scoring, breakers] = await responses;
    expect(scoring.status()).toBe(200);
    expect(breakers.status()).toBe(200);
    const score = await scoring.json();
    const risk = await breakers.json();
    expect(Number.isInteger(score.recent_scored_count)).toBe(true);
    expect(score.recent_scored_count).toBeGreaterThanOrEqual(0);
    expect(risk.summary.total).toBe(10);
    expect(risk.breakers).toHaveLength(10);
    // A legitimate BLOCKED / NOT_AVAILABLE is not a transport error or PASS.
    expect(risk.breakers.every((b: { state: string }) =>
      ["PASS", "WARN", "PAUSED", "KILLED", "BLOCKED", "NOT_AVAILABLE", "UNKNOWN"].includes(b.state))).toBe(true);
    const scorePanel = page.locator('[data-slot="confidence-scoring-panel"]');
    const riskPanel = page.locator('[data-slot="risk-circuit-panel"]');
    await expect(scorePanel.getByText("Wire coverage", { exact: true })).toBeVisible();
    await expect(riskPanel.getByText(`Overall: ${risk.overall_state}`, { exact: true })).toBeVisible();
    await expect(scorePanel.getByText("Cannot fetch scoring status", { exact: true })).toHaveCount(0);
    await expect(riskPanel.getByText("Cannot fetch circuit breakers", { exact: true })).toHaveCount(0);
  });
});
