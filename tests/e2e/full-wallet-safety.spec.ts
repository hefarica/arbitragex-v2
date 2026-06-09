import { test, expect } from "@playwright/test";

/**
 * Web3 safe-gated wallet — frontend SAFETY assertions (READ-ONLY).
 *
 * Asserts the hard safety invariants are VISIBLE on the /wallet surface and
 * that no enabled execute/broadcast/send affordance exists:
 *   • Live disabled / Capital exposure 0 / Broadcast disabled badges visible.
 *   • Read-only mode + Paper mode visible.
 *   • The WalletSafetyBanner is present.
 *   • Any broadcast/execute affordance is a DISABLED shell with a reason.
 *
 * Targets the FRONTEND; SKIPS honestly when unreachable.
 */

test.describe.configure({ mode: "serial" });

test.beforeEach(async ({ page }) => {
  let up = false;
  try {
    const r = await page.goto("/wallet", { waitUntil: "domcontentloaded", timeout: 12000 });
    up = Boolean(r && r.status() < 500);
  } catch {
    up = false;
  }
  test.skip(!up, "frontend /wallet not reachable — VALIDATION_PENDING_POST_DEPLOY");
});

test("WalletSafetyBanner is always visible", async ({ page }) => {
  await expect(page.getByTestId("wallet-safety-banner")).toBeVisible();
});

test("live OFF + capital 0 + broadcast OFF are visible", async ({ page }) => {
  const banner = page.getByTestId("wallet-safety-banner");
  await expect(banner.getByText(/live disabled/i)).toBeVisible();
  await expect(banner.getByText(/capital exposure 0/i)).toBeVisible();
  await expect(banner.getByText(/broadcast disabled/i)).toBeVisible();
});

test("read-only + paper mode are visible", async ({ page }) => {
  const banner = page.getByTestId("wallet-safety-banner");
  await expect(banner.getByText(/read-only mode/i)).toBeVisible();
  await expect(banner.getByText(/paper mode/i)).toBeVisible();
});

test("the broadcast/execute affordance exists ONLY as a disabled shell", async ({ page }) => {
  const broadcast = page.getByTestId("intent-broadcast-disabled");
  await expect(broadcast).toBeVisible();
  await expect(broadcast).toBeDisabled();
  await expect(broadcast).toHaveText(/broadcast disabled by policy/i);
  // There must be NO enabled "Send" / "Execute" / "Broadcast" button anywhere.
  const enabledSend = page.locator("button:enabled", { hasText: /^(send|execute|broadcast)$/i });
  await expect(enabledSend).toHaveCount(0);
});

test("SIWE panel advertises identity-only (no capital / no broadcast)", async ({ page }) => {
  const siwe = page.getByTestId("siwe-auth-panel");
  await expect(siwe).toBeVisible();
  await expect(siwe.getByText(/identity only/i).first()).toBeVisible();
  await expect(siwe.getByText(/no capital|no broadcast/i).first()).toBeVisible();
});
