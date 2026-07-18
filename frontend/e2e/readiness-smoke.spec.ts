import { test, expect } from "@playwright/test";

const BASE = process.env.E2E_BASE_URL ?? "http://localhost:3000";

test.describe("/live-readiness G-SIM-1 smoke card", () => {
  test("renders GSimSmokeTestCard with initial RED badge", async ({ page }) => {
    const response = await page.goto(`${BASE}/live-readiness`);
    expect(response?.status()).toBe(200);

    const card = page.locator('[data-testid="run-smoke-test"]').locator('xpath=ancestor::div[contains(@class, "Card")][1]');
    await expect(card.getByText("G-SIM-1 (Simulator V2)")).toBeVisible({ timeout: 15_000 });
    await expect(card.getByTestId("run-smoke-test")).toBeVisible({ timeout: 15_000 });
    await expect(card.getByText("RED")).toBeVisible({ timeout: 15_000 });
  });

  test("runs Sepolia smoke test when sim-ctl is healthy", async ({ page, request }) => {
    const health = await request.get(`${BASE}/health`).catch(() => null);
    const simCtlHealthy = health?.ok() ?? false;
    test.skip(!simCtlHealthy, "sim-ctl /health not available; skipping smoke run");

    const response = await page.goto(`${BASE}/live-readiness`);
    expect(response?.status()).toBe(200);

    await page.getByTestId("run-smoke-test").click();

    const resultSection = page.locator('[data-testid="run-smoke-test"]').locator('xpath=ancestor::div[contains(@class, "Card")][1]');
    await expect(
      resultSection.getByText(/SIM_SUCCESS|Failed:/)
    ).toBeVisible({ timeout: 60_000 });
  });
});
