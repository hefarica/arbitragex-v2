# ArbitrageX v2 — End-to-end tests

Playwright against the Docker Compose stack. Three specs:

| Spec | Gating | Purpose |
|------|--------|---------|
| `smoke.spec.ts`       | always | Every operator-console page renders with no "edge unreachable" banner. |
| `killswitch.spec.ts`  | `ARBX_ADMIN_TOKEN` set | Arms, verifies `/status` reflects, disarms; rejects reasonless toggles. |
| `rpc-down.spec.ts`    | `ARBX_ASSUME_NO_RPC=1` | "Never pretend to operate" guard — honest idle state with no RPC. |

## Run locally

```bash
# 1. Bring up the dev stack (dev compose has defaults, prod does not)
docker compose -f docker/compose.dev.yml up -d

# 2. Install Playwright + Chromium
cd tests/e2e
npm install
npm run install-browsers

# 3. Run (smoke-only is fast)
ARBX_FRONTEND_URL=http://localhost:5173 npm run test:smoke

# 4. Full suite (adjust gates per-spec)
ARBX_FRONTEND_URL=http://localhost:5173 \
ARBX_ASSUME_NO_RPC=1 \
ARBX_ADMIN_TOKEN=$(docker compose -f docker/compose.dev.yml exec api-server printenv ARBX_ADMIN_TOKEN) \
  npm test
```

## Doctrine alignment

- `rpc-down.spec.ts` is the executable guardrail for the "idle > fake" rule.
  If a future refactor starts fabricating opportunity rows when RPC is absent,
  this test fails.
- `smoke.spec.ts` specifically forbids the strings "edge unreachable" and
  "edge error:" — our standard error-banner copy. Empty-state strings like
  "No opportunities yet" are allowed.
- `killswitch.spec.ts` asserts the UI refuses a reasonless toggle — matches
  the audit-log requirement.

## Follow-ups

- Add `relays.spec.ts` once `R3 part B` lands (DB-backed relay catalog).
- Add a visual-regression pass (`expect(page).toHaveScreenshot()`) once the
  shadcn refactor stabilizes.
- CI timing: current smoke-only ≈ 30 s; full suite ≈ 2 min. Budget in the CI
  workflow reflects this.
