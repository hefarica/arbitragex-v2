import { defineConfig, devices } from "@playwright/test";

/**
 * Playwright config — LIVE_TESTNET API surface (api-server direct).
 *
 * These tests exercise the /api/v1/live-testnet/* and /admin/config/live-testnet
 * endpoints. They require a running api-server but NO frontend build and NO live
 * blockchain. The default target is the local docker compose api-server on :8080.
 *
 * Override:
 *   ARBX_API_URL=http://localhost:8080
 *
 * Run:
 *   cd tests/e2e
 *   npm install
 *   npx playwright test --config=playwright.live-testnet.config.ts
 */

const API_URL = process.env["ARBX_API_URL"] ?? "http://localhost:8080";

export default defineConfig({
  testDir: "./live-testnet",
  timeout: 15_000,
  expect: { timeout: 5_000 },
  fullyParallel: false,
  workers: 1,
  forbidOnly: Boolean(process.env["CI"]),
  retries: process.env["CI"] ? 1 : 0,
  reporter: [
    ["list"],
    ["html", { open: "never", outputFolder: "playwright-report-live-testnet" }],
  ],
  use: {
    baseURL: API_URL,
    trace: "on-first-retry",
    screenshot: "only-on-failure",
    video: "off",
  },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
  ],
});
