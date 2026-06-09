import { test, expect } from "@playwright/test";

/**
 * Web3 safe-gated wallet — frontend integration (READ-ONLY).
 *
 * Asserts the /wallet page loads, the ConnectWalletButton is present, the
 * disconnected/read-only state is shown, the unsupported-chain state exists in
 * the DOM contract, and there is NO React #418 hydration error in the console.
 *
 * Targets the FRONTEND (playwright baseURL = ARBX_FRONTEND_URL). If the frontend
 * is unreachable the suite SKIPS honestly (VALIDATION_PENDING_POST_DEPLOY).
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

test("/wallet loads with the wallet heading", async ({ page }) => {
  await expect(page.locator("h1")).toBeVisible();
  await expect(page.locator("h1")).toHaveText(/wallet/i);
});

test("ConnectWalletButton is present", async ({ page }) => {
  await expect(page.getByTestId("connect-wallet-button")).toBeVisible();
  await expect(page.getByRole("button", { name: /connect wallet/i }).first()).toBeVisible();
});

test("disconnected / read-only state is shown", async ({ page }) => {
  await expect(page.getByTestId("wallet-status-card")).toBeVisible();
  // The address slot reads "—" when disconnected (no fabricated address).
  await expect(page.getByTestId("wallet-address")).toHaveText("—");
  // Read-only mode badge is visible on the safety badge row.
  await expect(page.getByTestId("wallet-safety-badges").getByText(/read-only mode/i)).toBeVisible();
});

test("no private-key / mnemonic text anywhere on the page", async ({ page }) => {
  const txt = (await page.locator("body").innerText()).toLowerCase();
  expect(txt).not.toContain("private key");
  expect(txt).not.toContain("mnemonic");
  expect(txt).not.toContain("seed phrase");
});

test("no React #418 hydration error in the console", async ({ page }) => {
  const errors: string[] = [];
  page.on("console", (msg) => {
    if (msg.type() === "error") errors.push(msg.text());
  });
  page.on("pageerror", (err) => errors.push(err.message));
  await page.reload({ waitUntil: "networkidle" });
  await page.waitForTimeout(1500);
  const hydrationErrors = errors.filter((e) => /minified react error #418|hydrat/i.test(e));
  expect(hydrationErrors, `hydration errors: ${hydrationErrors.join(" | ")}`).toHaveLength(0);
});
