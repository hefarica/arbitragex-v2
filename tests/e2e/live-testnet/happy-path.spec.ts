import { test, expect } from "@playwright/test";

test("LT-001: Transaction lifecycle", async ({ page }) => {
  await page.goto("/live-testnet");
  await expect(page.locator("text=LIVE_TESTNET")).toBeVisible();
});
