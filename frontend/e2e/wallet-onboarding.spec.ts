/**
 * wallet-onboarding.spec.ts — E2E reachability for the Wallet Onboarding Guard.
 *
 * Proves the guard is WIRED into /wallet (not dead code): the page loads 200 and the guard
 * container renders after hydration. Observe-only — no wallet is injected, no connect is clicked,
 * no capital, no signing, no broadcast. Interactive connect behaviour (which needs a real injected
 * provider) is out of scope; this asserts topological reachability + the safe idle surface.
 */
import { test, expect } from "@playwright/test";

const BASE = process.env.E2E_BASE_URL ?? "http://localhost:3000";

test.describe("/wallet — onboarding guard reachability", () => {
  test("page loads 200 and the onboarding guard is rendered (not dead code)", async ({ page }) => {
    const response = await page.goto(`${BASE}/wallet`);
    expect(response?.status()).toBe(200);
    await expect(page.locator('[data-testid="wallet-onboarding-guard"]')).toBeVisible({ timeout: 15_000 });
  });

  test("idle state offers the explicit wallet selector (choose-then-connect)", async ({ page }) => {
    await page.goto(`${BASE}/wallet`);
    // Fresh CI has no injected wallet and no prior connection → not connected → the selector is shown.
    // `.or(connected)` tolerates a persisted-cookie connected state without flaking.
    const selector = page.locator('[data-testid="wallet-selector"]');
    const connected = page.locator('[data-testid="wallet-onboarding-connected"]');
    await expect(selector.or(connected)).toBeVisible({ timeout: 15_000 });
  });

  test("the opaque connect-modal button is NOT the disconnected entry point", async ({ page }) => {
    await page.goto(`${BASE}/wallet`);
    await expect(page.locator('[data-testid="wallet-onboarding-guard"]')).toBeVisible({ timeout: 15_000 });
    // The old RainbowKit-modal open button was removed in favour of the explicit guard.
    await expect(page.locator('[data-testid="connect-open"]')).toHaveCount(0);
  });
});
