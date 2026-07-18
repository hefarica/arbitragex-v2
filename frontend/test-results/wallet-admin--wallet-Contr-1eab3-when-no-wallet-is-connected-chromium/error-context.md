# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: wallet-admin.spec.ts >> /wallet ContractAdminPanel >> prompts to connect wallet when no wallet is connected
- Location: e2e\wallet-admin.spec.ts:12:7

# Error details

```
Test timeout of 60000ms exceeded while running "beforeEach" hook.
```

```
Error: page.goto: net::ERR_ABORTED; maybe frame was detached?
Call log:
  - navigating to "http://localhost:3000/wallet", waiting until "load"

```