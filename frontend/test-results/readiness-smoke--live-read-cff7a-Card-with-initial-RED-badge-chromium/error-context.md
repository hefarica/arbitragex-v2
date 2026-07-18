# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: readiness-smoke.spec.ts >> /live-readiness G-SIM-1 smoke card >> renders GSimSmokeTestCard with initial RED badge
- Location: e2e\readiness-smoke.spec.ts:8:7

# Error details

```
Test timeout of 60000ms exceeded while running "beforeEach" hook.
```

```
Error: page.goto: net::ERR_ABORTED; maybe frame was detached?
Call log:
  - navigating to "http://localhost:3000/live-readiness", waiting until "load"

```