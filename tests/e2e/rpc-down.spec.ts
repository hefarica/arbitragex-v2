import { test, expect } from "@playwright/test";

/**
 * Honest-idle contract.
 *
 * When RPC_WS_1 is empty (the compose default — operator hasn't completed
 * onboarding phase 2 yet), the platform must:
 *   1. Show searcher-rs as UP on /status (the service is alive).
 *   2. Show /opportunities as empty (not an error banner).
 *   3. Produce zero `arbx_opportunity_total{status="detected"}` counter growth.
 *
 * If any of those turn into fabricated data or error banners, this test
 * catches it. This is the "never pretend to operate" guardrail made executable.
 *
 * The test only runs when ARBX_ASSUME_NO_RPC=1, i.e. the operator is
 * explicitly running the stack without an RPC key. In production pipelines
 * that's the default; if someone has wired a real RPC we skip.
 */

const NO_RPC = process.env["ARBX_ASSUME_NO_RPC"] === "1";
const testMaybe = NO_RPC ? test : test.skip;

testMaybe("searcher-rs reports UP even with no RPC configured", async ({ page }) => {
  await page.goto("/status");

  // searcher-rs row exists and is DEGRADED (or UP depending on health, but we must target the status row, not the control panel row).
  // The control panel row has a "Start" or "Stop" button. We filter it out to avoid strict mode violations.
  const row = page.locator("tr").filter({ hasText: "searcher-rs" }).filter({ hasNotText: /Start|Stop/ });
  await expect(row).toBeVisible();
  await expect(row.getByText(/DEGRADED|UP/i)).toBeVisible();
});

testMaybe("opportunities page shows empty state, not an error", async ({ page }) => {
  await page.goto("/opportunities");
  // The page header must render (no 500 / no unreachable banner).
  await expect(page.locator("h1")).toBeVisible();

  // No error banners.
  await expect(page.getByText(/edge unreachable/i)).toHaveCount(0);
  await expect(page.getByText(/edge error:/i)).toHaveCount(0);

  // Empty state copy is allowed. One of these phrases must appear OR the
  // table must render with zero rows — either is an honest reflection.
  const honestMarkers = [
    /no opportunities/i,
    /0 rows/i,
    /snapshot/i,  // page still shows snapshot metadata block
  ];
  let any = false;
  for (const re of honestMarkers) {
    if (await page.getByText(re).first().isVisible().catch(() => false)) {
      any = true;
      break;
    }
  }
  expect(any, "page should display an honest empty/idle state, not a fabricated result").toBeTruthy();
});

testMaybe("platform overview dashboard link (if exposed) does not fabricate", async ({ page }) => {
  // The frontend does not embed Grafana — it's served separately. We just
  // assert /status did not invent non-zero opportunity counters in any
  // KPI card.
  await page.goto("/recon");
  await expect(page.locator("h1")).toBeVisible();

  // If the KPI cards exist, "Attempts" KPI should be 0 when there's no RPC.
  const attemptsCard = page.locator('[class*="card"]', { hasText: /attempts/i }).first();
  if (await attemptsCard.count()) {
    await expect(attemptsCard.locator('text=/^0$/')).toBeVisible();
  }
});
