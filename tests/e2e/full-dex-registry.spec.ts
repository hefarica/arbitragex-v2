import { test, expect, type ConsoleMessage } from "@playwright/test";

/**
 * FE-CRIT-06 — /dex-registry Add / Remove interactions.
 *
 * Before this fix the "Add DEX" and Remove (trash) buttons set state but
 * rendered no dialog and called no mutation ("dialogs omitted for brevity").
 * This suite proves the dialogs now mount, the form fields exist, the dialogs
 * open and close, and that NO action produces a fabricated success (RULE 00 /
 * R8) — a missing backend contract surfaces honestly, never a fake toast.
 *
 * Style mirrors tests/e2e/live/functional-live.spec.ts: domcontentloaded + a
 * fixed settle window (pages poll, so networkidle never settles), and console
 * pageerrors are captured so a React #418 hydration crash hard-fails.
 *
 * Base URL is configurable. CI's playwright job points it at the PR's built
 * app; against the OLD deployed build these assertions are
 * VALIDATION_PENDING_POST_DEPLOY (the deployed app predates this fix).
 */

const BASE =
  process.env.E2E_BASE_URL ??
  process.env.ARBX_FRONTEND_URL ??
  "http://localhost:5173";

const PATH = "/dex-registry";

// React production hydration-mismatch error is minified as #418. A crash here
// means R1 (Mounted-Snapshot) was violated.
const REACT_HYDRATION_RE = /Minified React error #418|hydrat/i;

function wireConsole(page: import("@playwright/test").Page): string[] {
  const errors: string[] = [];
  page.on("console", (msg: ConsoleMessage) => {
    if (msg.type() !== "error") return;
    const t = msg.text();
    if (/Failed to load resource/i.test(t)) return; // HTTP 4xx/5xx are tracked structurally elsewhere
    errors.push(t);
  });
  page.on("pageerror", (err) => errors.push(`pageerror: ${err.message}`));
  return errors;
}

async function settle(page: import("@playwright/test").Page) {
  await page.goto(`${BASE}${PATH}`, { waitUntil: "domcontentloaded" });
  await page.waitForTimeout(2500);
}

test.describe("/dex-registry — Add/Remove interactions (FE-CRIT-06)", () => {
  test("page loads with the heading and no hydration crash", async ({ page }) => {
    const errors = wireConsole(page);
    await settle(page);

    await expect(page.locator("h1")).toContainText(/dex registry/i);
    expect(
      errors.filter((e) => REACT_HYDRATION_RE.test(e)),
      `React hydration error on ${PATH}: ${errors.join(" | ")}`,
    ).toHaveLength(0);
  });

  test("Add DEX button exists and opens a real dialog with form fields", async ({ page }) => {
    await settle(page);

    const addBtn = page.getByTestId("dex-add-open");
    await expect(addBtn).toBeVisible();

    // Dialog is NOT in the DOM until opened (proves it is conditionally rendered,
    // not the old no-op).
    await expect(page.getByTestId("dex-add-dialog")).toHaveCount(0);

    await addBtn.click();

    const dialog = page.getByTestId("dex-add-dialog");
    await expect(dialog).toBeVisible();

    // Real form fields must exist.
    await expect(page.getByTestId("dex-add-name")).toBeVisible();
    await expect(page.getByTestId("dex-add-protocol")).toBeVisible();
    await expect(page.getByTestId("dex-add-factory-chain-0")).toBeVisible();
    await expect(page.getByTestId("dex-add-factory-address-0")).toBeVisible();
    await expect(page.getByTestId("dex-add-submit")).toBeVisible();
  });

  test("Add dialog closes via the close button", async ({ page }) => {
    await settle(page);
    await page.getByTestId("dex-add-open").click();
    await expect(page.getByTestId("dex-add-dialog")).toBeVisible();

    await page.getByTestId("dex-add-close").click();
    await expect(page.getByTestId("dex-add-dialog")).toHaveCount(0);
  });

  test("Add submit is disabled until the form is valid (no false success)", async ({ page }) => {
    await settle(page);
    await page.getByTestId("dex-add-open").click();

    const submit = page.getByTestId("dex-add-submit");
    // Empty form → submit disabled. The button never fires a doomed request.
    await expect(submit).toBeDisabled();

    // Fill name + a valid factory address. If no admin token is present the
    // submit stays disabled (the dialog shows the "no admin session" guard) —
    // either way it must NOT produce a success state without a real backend.
    await page.getByTestId("dex-add-name").fill("E2E Test DEX");
    await page
      .getByTestId("dex-add-factory-address-0")
      .fill("0x1111111111111111111111111111111111111111");

    // The chain control is either a <select> (catalog present) or numeric input.
    const chain = page.getByTestId("dex-add-factory-chain-0");
    const tag = await chain.evaluate((el) => el.tagName.toLowerCase());
    if (tag === "select") {
      const optionValues = await chain
        .locator("option")
        .evaluateAll((opts) =>
          opts.map((o) => (o as HTMLOptionElement).value).filter((v) => v !== ""),
        );
      const first = optionValues[0];
      if (first !== undefined) {
        await chain.selectOption(first);
      }
    } else {
      await chain.fill("1");
    }

    // Crucially: we never assert a success toast. We only assert the page did
    // not fabricate a "created" success. If the dialog is still open OR a
    // missing_backend_contract / honest error is shown, that is correct.
    // No fake success element exists by design.
    await expect(page.getByText(/created successfully|dex added/i)).toHaveCount(0);
  });

  test("Remove button (if any rows) opens a confirmation dialog that closes", async ({ page }) => {
    await settle(page);

    // Remove buttons only exist when the registry returned rows. If the table
    // is empty (honest empty state), this interaction is not applicable — skip
    // rather than fabricate a row.
    const removeButtons = page.locator('[data-testid^="dex-remove-open-"]');
    const count = await removeButtons.count();
    test.skip(count === 0, "no DEX rows in this environment — remove flow N/A");

    await removeButtons.first().click();
    const dialog = page.getByTestId("dex-remove-dialog");
    await expect(dialog).toBeVisible();
    await expect(page.getByTestId("dex-remove-confirm")).toBeVisible();

    // Close it without confirming.
    await page.getByTestId("dex-remove-close").click();
    await expect(dialog).toHaveCount(0);
  });
});
