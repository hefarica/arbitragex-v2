/**
 * operator-presets.spec.ts — Playwright E2E for /operator/presets.
 *
 * Gate rule: ALL tests GREEN before any merge/deploy. Observe-only: no capital,
 * no execution, no broadcast. Requires the frontend served at E2E_BASE_URL
 * (CI / operator env) — not runnable from a Windows dev box under RULE 01.
 */
import { test, expect } from "@playwright/test";

const BASE = process.env.E2E_BASE_URL ?? "http://localhost:3000";

test.describe("/operator/presets", () => {
  test("loads 200 and renders the panel with all three preset cards", async ({ page }) => {
    const res = await page.goto(`${BASE}/operator/presets`);
    expect(res?.status()).toBe(200);
    await expect(page.locator('[data-testid="operator-presets-panel"]')).toBeVisible({
      timeout: 15_000,
    });
    for (const id of ["conservative", "balanced", "aggressive"]) {
      await expect(page.locator(`[data-testid="preset-card-${id}"]`)).toBeVisible();
    }
  });

  test("page title contains ArbitrageX", async ({ page }) => {
    await page.goto(`${BASE}/operator/presets`);
    await expect(page).toHaveTitle(/ArbitrageX/i, { timeout: 15_000 });
  });

  test("disclaimer is visible (no performance promise)", async ({ page }) => {
    await page.goto(`${BASE}/operator/presets`);
    await expect(page.locator('[data-testid="presets-disclaimer"]')).toBeVisible();
    await expect(page.getByText(/past performance/i)).toBeVisible();
  });

  test("capital <= 0 shows a warning and disables Apply", async ({ page }) => {
    await page.goto(`${BASE}/operator/presets`);
    await page.locator('[data-testid="capital-input"]').fill("0");
    await expect(page.locator('[data-testid="capital-warning"]')).toBeVisible();
    await expect(page.locator('[data-testid="apply-preset"]')).toBeDisabled();
  });

  test("entering capital updates caps in real time", async ({ page }) => {
    await page.goto(`${BASE}/operator/presets`);
    await page.locator('[data-testid="capital-input"]').fill("10000");
    const dailyLoss = page.locator('[data-testid="dim-balanced-daily-loss-value"]');
    await expect(dailyLoss).not.toHaveText("$0.00");
    await expect(page.locator('[data-testid="apply-preset"]')).toBeEnabled();
  });

  test("selection + capital persist across reload (localStorage)", async ({ page }) => {
    await page.goto(`${BASE}/operator/presets`);
    await page.locator('[data-testid="capital-input"]').fill("5000");
    await page.locator('[data-testid="preset-card-aggressive"]').click();
    await expect(page.locator('[data-testid="preset-selected-aggressive"]')).toBeVisible();

    await page.reload();
    await expect(page.locator('[data-testid="preset-selected-aggressive"]')).toBeVisible({
      timeout: 15_000,
    });
    await expect(page.locator('[data-testid="capital-input"]')).toHaveValue("5000");
  });

  test("Apply stages a draft (no trading_config write)", async ({ page }) => {
    await page.goto(`${BASE}/operator/presets`);
    await page.locator('[data-testid="capital-input"]').fill("5000");
    await page.locator('[data-testid="apply-preset"]').click();
    await expect(page.locator('[data-testid="applied-draft"]')).toBeVisible();
  });
});
