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

// live/ tests have their own playwright.live.config.ts and require the full
// VPS stack (Chains, RPCs, Pools admin endpoints, topology/snapshot API, etc.).
// They must never run via `npm test` — only via `npm run test:live`.
//
// rpc-down tests require searcher-rs to be running in the compose stack.
// The CI e2e.yml only starts postgres/redis/frontend/edge/api-server, so
// searcher-rs is absent and the UP assertion would always fail. Exclude when
// ARBX_ASSUME_NO_RPC=1 (the CI partial-compose signal).
const NO_RPC = process.env["ARBX_ASSUME_NO_RPC"] === "1";
const ALWAYS_IGNORE = ["**/live/**"];
const CI_IGNORE = NO_RPC
  ? [...ALWAYS_IGNORE, "rpc-down.spec.ts", "**/live-testnet/**"]
  : [...ALWAYS_IGNORE, "**/live-testnet/**"];

export default defineConfig({
  testDir: ".",
  testIgnore: CI_IGNORE,
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
