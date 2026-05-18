---
title: Run E2E Tests Locally
description: Set up Playwright and execute the full E2E test suite against a local ArbitrageX v2 stack.
tags: [testing, e2e, playwright]
---

# How to Run E2E Tests Locally

This guide covers the complete setup and execution of the end-to-end (E2E) test suite for ArbitrageX v2 using Playwright. The E2E tests verify critical user journeys through the Next.js dashboard against a running local stack.

---

## Prerequisites

| Requirement | Version | Purpose |
|-------------|---------|---------|
| Node.js | 20 LTS | Playwright runtime |
| npm | 10+ | Package management |
| Docker Compose | v2 | Local stack orchestration |
| ArbitrageX v2 | Latest | System under test |

Before running E2E tests, ensure all 21 containers are healthy:

```bash
docker compose ps
# Verify: all 21 containers show (healthy)
```

---

## Step 1: Install Playwright

From the project root, navigate to the E2E test directory and install dependencies:

```bash
cd e2e/
npm install
```

Install Playwright browsers and system dependencies:

```bash
npx playwright install
npx playwright install-deps
```

This installs Chromium, Firefox, and WebKit binaries along with required system libraries. The download may take 2–5 minutes.

### Verify Installation

```bash
npx playwright --version
# Expected: Version 1.40+
```

---

## Step 2: E2E Configuration

The E2E suite uses `playwright.config.ts` in the `e2e/` directory. Key configuration values:

```typescript
// e2e/playwright.config.ts
import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 1,
  workers: process.env.CI ? 1 : 3,
  reporter: [
    ['html', { open: 'never' }],
    ['list']
  ],
  use: {
    baseURL: 'http://localhost:3000',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
    {
      name: 'firefox',
      use: { ...devices['Desktop Firefox'] },
    },
  ],
});
```

| Option | Default | Description |
|--------|---------|-------------|
| `baseURL` | `http://localhost:3000` | Dashboard URL |
| `retries` | `1` (local), `2` (CI) | Retry count for flaky tests |
| `workers` | `3` (local), `1` (CI) | Parallel test workers |
| `trace` | `on-first-retry` | Record traces on first retry failure |

---

## Step 3: Run the Full Suite

Execute all E2E tests:

```bash
cd e2e/
npx playwright test
```

### Expected Output

```
Running 47 tests using 3 workers
  ✓  1 [chromium] › auth/login.spec.ts:12:3 › Login Page › renders login form (1.2s)
  ✓  2 [chromium] › dashboard/overview.spec.ts:8:3 › Dashboard › displays system status (2.1s)
  ✓  3 [chromium] › dashboard/overview.spec.ts:18:3 › Dashboard › shows paper mode banner (1.8s)
  ✓  4 [chromium] › opportunities/stream.spec.ts:9:3 › Opportunity Stream › connects via WebSocket (3.4s)
  ✓  5 [chromium] › opportunities/stream.spec.ts:24:3 › Opportunity Stream › renders opportunity cards (2.7s)
  ...
  ✓  45 [firefox] › trades/history.spec.ts:15:3 › Trade History › paginates results (4.1s)
  ✓  46 [firefox] › settings/mode.spec.ts:8:3 › Settings › toggles paper mode (2.9s)
  ✓  47 [firefox] › api/health.spec.ts:6:3 › API Health › returns healthy status (1.5s)

  47 passed (23.4s)
```

---

## Step 4: Run Specific Test Files

Run a single test file:

```bash
npx playwright test dashboard/overview.spec.ts
```

Run tests matching a pattern:

```bash
npx playwright test --grep "paper mode"
```

Run a specific project (browser):

```bash
npx playwright test --project=chromium
```

Run with headed mode (visible browser):

```bash
npx playwright test --headed
```

---

## Step 5: Run in Debug Mode

For troubleshooting, use the Playwright Inspector:

```bash
npx playwright test --debug
```

Or set the `PWDEBUG` environment variable:

```bash
PWDEBUG=1 npx playwright test dashboard/overview.spec.ts
```

In debug mode:
- Playwright opens a browser window
- Execution pauses at each step
- The Inspector panel shows locator details, DOM snapshots, and network activity
- You can step forward, step back, or resume execution

---

## Step 6: View Test Reports

After each run, an HTML report is generated:

```bash
npx playwright show-report
```

This launches a local web server displaying:

| Section | Content |
|---------|---------|
| Summary | Total passed/failed/skipped/flaky |
| Test List | Each test with status and duration |
| Trace Viewer | Step-by-step execution for failed tests |
| Screenshots | Captured screenshots on failure |
| Video | Recorded video for failed test cases |
| Network | HAR log of all HTTP/WebSocket requests |

---

## Step 7: Test Categories

The E2E suite covers the following test categories:

| Category | Files | Count | Purpose |
|----------|-------|-------|---------|
| Authentication | `auth/*.spec.ts` | 4 | Login, session, logout flows |
| Dashboard | `dashboard/*.spec.ts` | 6 | Overview, status panels, banners |
| Opportunities | `opportunities/*.spec.ts` | 8 | Stream, cards, filtering, detail |
| Trades | `trades/*.spec.ts` | 10 | History, pagination, detail view |
| Paper Trading | `paper/*.spec.ts` | 6 | Execute, verify, history |
| Settings | `settings/*.spec.ts` | 5 | Mode toggle, configuration |
| API | `api/*.spec.ts` | 5 | Direct API endpoint verification |
| WebSocket | `websocket/*.spec.ts` | 3 | Connection, reconnection, messages |

---

## Step 8: Continuous Integration

For CI environments (GitHub Actions, GitLab CI), the test suite runs with stricter settings:

```yaml
# .github/workflows/e2e.yml (excerpt)
- name: Run E2E Tests
  run: |
    cd e2e/
    npx playwright test --reporter=html,junit
  env:
    CI: true
    PLAYWRIGHT_WORKERS: 1
    PLAYWRIGHT_RETRIES: 2
```

CI-specific behavior:
- Workers are limited to 1 to prevent port conflicts
- Retries are set to 2 for flakiness tolerance
- Reports are uploaded as artifacts
- Videos and traces are retained for all failed tests

---

## Troubleshooting

### Tests Fail with `page.goto: net::ERR_CONNECTION_REFUSED`

The dashboard is not running. Start the stack:

```bash
docker compose up -d
curl http://localhost:3000/health  # Verify before running tests
```

### Flaky WebSocket Tests

WebSocket tests may be flaky on slow machines. Increase timeout:

```bash
npx playwright test --grep "WebSocket" --timeout=30000
```

### Browser Download Issues

If browser binaries fail to download, use the system browser:

```bash
PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1
npx playwright test --project=chromium  # Uses system Chromium
```

### Docker Resource Contention

If containers become unhealthy during E2E runs, limit container resources:

```yaml
# docker-compose.override.yml
services:
  ax-evm-exec-1:
    deploy:
      resources:
        limits:
          memory: 1G
```
