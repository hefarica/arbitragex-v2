import { defineConfig, devices } from "@playwright/test";

/**
 * Playwright config for ArbitrageX v2 E2E.
 *
 * The tests assume a compose stack is already running. CI brings it up in
 * .github/workflows/e2e.yml; locally you can do:
 *
 *   docker compose -f docker/compose.dev.yml up -d
 *   cd tests/e2e && npm install && npm run install-browsers
 *   ARBX_FRONTEND_URL=http://localhost:5173 npm test
 */

const FRONTEND_URL = process.env["ARBX_FRONTEND_URL"] ?? "http://localhost:5173";

export default defineConfig({
  testDir: ".",
  timeout: 30_000,
  expect: { timeout: 10_000 },
  fullyParallel: false,          // operator console is stateful; serialize
  forbidOnly: Boolean(process.env["CI"]),
  retries: process.env["CI"] ? 1 : 0,
  workers: 1,
  reporter: [
    ["list"],
    ["html", { open: "never", outputFolder: "playwright-report" }],
  ],
  use: {
    baseURL: FRONTEND_URL,
    trace: "on-first-retry",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
    viewport: { width: 1440, height: 900 },
  },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
  ],
});
