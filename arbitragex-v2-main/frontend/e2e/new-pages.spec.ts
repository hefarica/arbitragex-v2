/**
 * new-pages.spec.ts — Playwright E2E validation for the 4 new pages.
 *
 * Gate rule: ALL tests must be GREEN before any merge or deploy.
 * Tests are observe-only: no capital, no execution, no broadcast.
 *
 * Pages tested:
 *   1. /agent-insights
 *   2. /routes/discovery
 *   3. /paper/history
 *   4. /opportunities/by-strategy
 */
import { test, expect } from "@playwright/test";

const BASE = process.env.E2E_BASE_URL ?? "http://localhost:3000";

// ─── /agent-insights ─────────────────────────────────────────────────────────
test.describe("/agent-insights", () => {
  test("page loads with HTTP 200 and renders panel", async ({ page }) => {
    const response = await page.goto(`${BASE}/agent-insights`);
    expect(response?.status()).toBe(200);
    // Either the panel or the empty state must be present
    const panel = page.locator('[data-testid="agent-insights-panel"]');
    const empty = page.locator('[data-testid="agent-insights-empty"]');
    await expect(panel.or(empty)).toBeVisible({ timeout: 15_000 });
  });

  test("page title contains ArbitrageX", async ({ page }) => {
    await page.goto(`${BASE}/agent-insights`);
    await expect(page).toHaveTitle(/ArbitrageX/i, { timeout: 15_000 });
  });

  test("nav link to /agent-insights is present in sidebar", async ({ page }) => {
    await page.goto(`${BASE}/`);
    const link = page.locator('a[href="/agent-insights"]');
    await expect(link).toBeVisible({ timeout: 10_000 });
  });
});

// ─── /routes/discovery ───────────────────────────────────────────────────────
test.describe("/routes/discovery", () => {
  test("page loads with HTTP 200 and renders panel", async ({ page }) => {
    const response = await page.goto(`${BASE}/routes/discovery`);
    expect(response?.status()).toBe(200);
    const panel = page.locator('[data-testid="routes-discovery-panel"]');
    const empty = page.locator('[data-testid="routes-empty"]');
    await expect(panel.or(empty)).toBeVisible({ timeout: 15_000 });
  });

  test("page title contains ArbitrageX", async ({ page }) => {
    await page.goto(`${BASE}/routes/discovery`);
    await expect(page).toHaveTitle(/ArbitrageX/i, { timeout: 15_000 });
  });

  test("nav link to /routes/discovery is present in sidebar", async ({ page }) => {
    await page.goto(`${BASE}/`);
    const link = page.locator('a[href="/routes/discovery"]');
    await expect(link).toBeVisible({ timeout: 10_000 });
  });
});

// ─── /paper/history ──────────────────────────────────────────────────────────
test.describe("/paper/history", () => {
  test("page loads with HTTP 200 and renders panel", async ({ page }) => {
    const response = await page.goto(`${BASE}/paper/history`);
    expect(response?.status()).toBe(200);
    const panel = page.locator('[data-testid="paper-history-panel"]');
    const empty = page.locator('[data-testid="paper-history-empty"]');
    await expect(panel.or(empty)).toBeVisible({ timeout: 15_000 });
  });

  test("page title contains ArbitrageX", async ({ page }) => {
    await page.goto(`${BASE}/paper/history`);
    await expect(page).toHaveTitle(/ArbitrageX/i, { timeout: 15_000 });
  });

  test("nav link to /paper/history is present in sidebar", async ({ page }) => {
    await page.goto(`${BASE}/`);
    const link = page.locator('a[href="/paper/history"]');
    await expect(link).toBeVisible({ timeout: 10_000 });
  });
});

// ─── /opportunities/by-strategy ──────────────────────────────────────────────
test.describe("/opportunities/by-strategy", () => {
  test("page loads with HTTP 200 and renders panel", async ({ page }) => {
    const response = await page.goto(`${BASE}/opportunities/by-strategy`);
    expect(response?.status()).toBe(200);
    const panel = page.locator('[data-testid="opportunities-by-strategy-panel"]');
    const empty = page.locator('[data-testid="by-strategy-empty"]');
    await expect(panel.or(empty)).toBeVisible({ timeout: 15_000 });
  });

  test("page title contains ArbitrageX", async ({ page }) => {
    await page.goto(`${BASE}/opportunities/by-strategy`);
    await expect(page).toHaveTitle(/ArbitrageX/i, { timeout: 15_000 });
  });

  test("nav link to /opportunities/by-strategy is present in sidebar", async ({ page }) => {
    await page.goto(`${BASE}/`);
    const link = page.locator('a[href="/opportunities/by-strategy"]');
    await expect(link).toBeVisible({ timeout: 10_000 });
  });

  test("by-strategy summary strip is visible", async ({ page }) => {
    await page.goto(`${BASE}/opportunities/by-strategy`);
    const summary = page.locator('[data-testid="by-strategy-summary"]');
    const empty   = page.locator('[data-testid="by-strategy-empty"]');
    await expect(summary.or(empty)).toBeVisible({ timeout: 15_000 });
  });
});

// ─── Edge proxy routes ────────────────────────────────────────────────────────
test.describe("API proxy routes", () => {
  test("/api/sed/status proxy returns non-500", async ({ request }) => {
    const res = await request.get(`${BASE}/api/sed/status`);
    expect(res.status()).not.toBe(500);
  });

  test("/api/paper/history proxy returns non-500", async ({ request }) => {
    const res = await request.get(`${BASE}/api/paper/history`);
    expect(res.status()).not.toBe(500);
  });

  test("/api/route-discovery/routes proxy returns non-500", async ({ request }) => {
    const res = await request.get(`${BASE}/api/route-discovery/routes`);
    expect(res.status()).not.toBe(500);
  });
});
