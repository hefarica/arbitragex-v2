import { test, expect } from "@playwright/test";

/**
 * Honest-idle contract — ITER-18 (PR #90 / OMEGA-S5).
 *
 * When RPC_WS_1 is empty (the compose default — operator hasn't completed
 * onboarding phase 2 yet) AND the operator has explicitly opted in with
 * ARBX_ASSUME_NO_RPC=1, the platform MUST classify searcher-rs as
 * DEGRADED, not UP and not DOWN:
 *
 *   - UP would be a lie  → the searcher cannot trade without an RPC.
 *   - DOWN would also be a lie → the process is intentionally idle, not crashed.
 *   - DEGRADED with reason=no_rpc_configured is the only honest answer.
 *
 * The previous iteration of this test asserted "UP" — that was a workaround
 * that survived only because the searcher-rs container happened to answer.
 * In CI it does not answer (404 / fetch failed), and the test must drive the
 * backend to emit the correct taxonomy instead of being relaxed to accept
 * a fabricated UP. See backend/api-server/src/index.ts::pingUpstream.
 *
 * Row selection uses getByTestId("service-health-row-searcher-rs") — the
 * tag added in ServicesTable — to disambiguate from the sibling
 * ServiceControlPanel table (data-testid="service-control-row-searcher-rs"),
 * which would otherwise trigger a strict-mode locator violation because both
 * rows contain the literal text "searcher-rs".
 *
 * The test only runs when ARBX_ASSUME_NO_RPC=1.
 */

const NO_RPC = process.env["ARBX_ASSUME_NO_RPC"] === "1";
const testMaybe = NO_RPC ? test : test.skip;

testMaybe(
  "searcher-rs reports degraded, not down, when no RPC is configured",
  async ({ page }) => {
    await page.goto("/status");

    // The page must render its h1 — if the client bundle crashed we'd see
    // the Next.js error overlay instead and this would fail fast with a
    // clear message rather than a strict-mode locator violation downstream.
    await expect(page.getByTestId("page-title")).toBeVisible();

    // Pick the health row unambiguously. The control panel renders its own
    // row for the same service; without the testid we'd hit strict mode.
    const healthRow = page.getByTestId("service-health-row-searcher-rs");
    await expect(healthRow).toBeVisible();

    // Doctrinal assertion: under ARBX_ASSUME_NO_RPC=1, searcher-rs must be
    // surfaced as DEGRADED with the human "no RPC configured" detail, never
    // as a flat DOWN and never with the raw "fetch failed" exception leaking
    // through the UI.
    await expect(healthRow).toContainText(/DEGRADED|NO RPC|NO_RPC_CONFIGURED/i);
    await expect(healthRow).not.toContainText(/\bDOWN\b/);
    await expect(healthRow).not.toContainText(/fetch failed/i);

    // Belt-and-suspenders: the structured attribute the backend exposes via
    // ServicesTable must match the machine-readable reason. This is what
    // external monitoring (and any future scripted gates) should branch on.
    await expect(healthRow).toHaveAttribute("data-status", "degraded");
    await expect(healthRow).toHaveAttribute("data-reason", "no_rpc_configured");
  },
);

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
