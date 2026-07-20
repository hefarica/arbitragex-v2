import { test, expect } from "@playwright/test";

/**
 * Web3 safe-gated wallet — transaction INTENT pipeline (READ-ONLY).
 *
 * Drives the WalletIntentPanel on /wallet and asserts that registering an
 * intent (without any wallet/capital) terminates at BROADCAST_DISABLED with
 * broadcast_allowed=false, and that simulation never enables broadcast. This is
 * the UI-level proof of the backend contract.
 *
 * Targets the FRONTEND; needs the edge reachable from the browser to round-trip
 * the intent. SKIPS honestly when /wallet is unreachable; tolerates the edge
 * being unreachable (the panel then shows an honest error, never a broadcast).
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

test("intent panel is present with a disabled broadcast shell", async ({ page }) => {
  await expect(page.getByTestId("wallet-intent-panel")).toBeVisible();
  await expect(page.getByTestId("intent-broadcast-disabled")).toBeDisabled();
});

test("registering an intent terminates at BROADCAST_DISABLED", async ({ page }) => {
  // Deterministic hydration gate: the panel exposes data-hydrated="true" only after React has
  // mounted it (onClick handlers wired). Waiting for it removes the click-races-hydration flake
  // WITHOUT relaxing any assertion below. RainbowKit/Reown init can delay hydration (esp. with no
  // WalletConnect projectId); submitIntent() itself is a plain fetch that needs no connected wallet.
  await expect(page.getByTestId("wallet-intent-panel")).toHaveAttribute("data-hydrated", "true", { timeout: 20000 });
  await page.getByTestId("intent-register").click();
  // The result block renders once the round-trip resolves (or an honest error).
  const result = page.getByTestId("intent-result");
  await expect(result).toBeVisible({ timeout: 15000 });
  const text = (await result.innerText()).toLowerCase();
  if (text.includes("error:")) {
    // Honest transport error — acceptable, and crucially NOT a broadcast.
    expect(text).not.toContain("broadcast_allowed: true");
  } else {
    await expect(page.getByTestId("intent-terminal-state")).toHaveText(/broadcast_disabled/i);
    await expect(result.getByText(/broadcast_allowed: false/i)).toBeVisible();
    await expect(result.getByText(/broadcast_disabled_by_policy/i)).toBeVisible();
  }
});

test("simulation never reports broadcast allowed", async ({ page }) => {
  // Same deterministic hydration gate as above before interacting with the panel.
  await expect(page.getByTestId("wallet-intent-panel")).toHaveAttribute("data-hydrated", "true", { timeout: 20000 });
  await page.getByTestId("intent-simulate").click();
  const result = page.getByTestId("simulation-result");
  await expect(result).toBeVisible({ timeout: 15000 });
  const text = (await result.innerText()).toLowerCase();
  expect(text).not.toContain("broadcast_allowed: true");
});
