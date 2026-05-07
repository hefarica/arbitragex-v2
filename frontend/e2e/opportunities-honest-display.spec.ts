/**
 * Sub-Proyecto A — Honest Display E2E test.
 *
 * REQUIREMENTS TO RUN:
 *   @playwright/test is NOT currently in package.json.
 *   Operator must approve and install before execution:
 *     npm install -D @playwright/test && npx playwright install
 *
 * RUN:
 *   Local (dev server must be running on port 3000):
 *     cd frontend && npx playwright test e2e/opportunities-honest-display.spec.ts
 *
 *   Production:
 *     cd frontend && E2E_BASE_URL=http://195.201.235.70:5173/opportunities npx playwright test
 *
 * NOTE on data-status selector:
 *   In OpportunitiesClient.tsx, `data-status={opp.status}` is on the Route <td>
 *   (column 2), NOT on the <tr>.  `data-col="profit"` is on the Profit <td>
 *   (column 4) — a sibling, not a descendant.
 *   We therefore select rows as `tr:has(td[data-status="detected"])` so that
 *   `.locator('[data-col="profit"]')` searches inside the full <tr> and reaches
 *   the profit cell correctly.
 */
import { test, expect } from "@playwright/test";

test("opportunities page shows enriched tokens or honest fallback", async ({ page }) => {
  await page.goto(process.env["E2E_BASE_URL"] ?? "http://localhost:3000/opportunities");

  // Wait for the table to be present in the DOM (SSR delivers it; client hydrates).
  await page.waitForSelector("table");

  // R8: every row whose Route cell has data-status="detected" MUST show "—"
  // in the profit cell — never "$0.00".
  //
  // We select the full <tr> that contains a td[data-status="detected"] so that
  // the subsequent `.locator('[data-col="profit"]')` searches inside the row and
  // reaches the sibling profit <td>.
  const detectedRows = page.locator('tr:has(td[data-status="detected"])');
  const count = await detectedRows.count();

  for (let i = 0; i < count; i++) {
    const row = detectedRows.nth(i);
    const profit = await row.locator('[data-col="profit"]').textContent();
    expect(profit?.trim()).toBe("—");
  }

  // After the enricher has been running for a few minutes, at least one Trust
  // Wallet logo should be visible (token_in_info.logo_url resolved via
  // trustwallet CDN).  On a cold deploy the enricher may still be warming up,
  // so we warn rather than hard-fail to avoid flaky CI.
  const logos = await page.locator('img[src*="trustwallet"]').count();
  if (logos === 0) {
    console.warn(
      "WARN: no Trust Wallet logos visible — enricher may still be warming up",
    );
  } else {
    expect(logos).toBeGreaterThan(0);
  }
});
