import { defineConfig, devices } from "@playwright/test";

/**
 * Playwright config — LIVE TARGET (read-only by default).
 *
 * Unlike `playwright.config.ts` (which assumes a local docker compose stack),
 * this config points at an ALREADY-RUNNING deployment. Default target is the
 * Cloudflare-fronted edge that serves the operator console SPA.
 *
 * Override the target:
 *   ARBX_FRONTEND_URL=https://edge-arbx.ape-tv.net   (default)
 *   ARBX_FRONTEND_URL=http://195.201.235.70:8787     (raw VPS edge)
 *   ARBX_FRONTEND_URL=http://localhost:5173          (local next dev)
 *
 * Mutating admin specs (admin-mutating-live.spec.ts) are SKIPPED unless
 * ARBX_ADMIN_TOKEN is set — read-only is the doctrine default (§32/§33).
 *
 * Run:
 *   cd tests/e2e
 *   npm install
 *   npx playwright test --config=playwright.live.config.ts
 */

const FRONTEND_URL =
  process.env["ARBX_FRONTEND_URL"] ?? "https://edge-arbx.ape-tv.net";

export default defineConfig({
  testDir: "./live",
  timeout: 45_000,
  expect: { timeout: 15_000 },
  // Operator console is stateful and we hit a shared live edge — serialize.
  fullyParallel: false,
  workers: 1,
  forbidOnly: Boolean(process.env["CI"]),
  retries: process.env["CI"] ? 1 : 0,
  reporter: [
    ["list"],
    ["json", { outputFile: "playwright-report-live/results.json" }],
    ["html", { open: "never", outputFolder: "playwright-report-live" }],
  ],
  use: {
    baseURL: FRONTEND_URL,
    trace: "on-first-retry",
    screenshot: "only-on-failure",
    video: "off",
    viewport: { width: 1440, height: 900 },
    // The live edge is behind Cloudflare; give navigations room.
    navigationTimeout: 30_000,
    actionTimeout: 15_000,
  },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
  ],
});
