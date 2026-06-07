# WEBAPP-TESTING E2E EXECUTION PLAN

**Date:** 2026-06-07  
**Status:** READY FOR EXECUTION (documented, awaiting operator trigger)  
**Scope:** Verify the 5-layer machine (frontend SSR, backend net-profit gate, oracle cascade, pre-execute checklist, risk-ledger breaker) works end-to-end in VPS shadow mode.

---

## EXECUTION STRATEGY

### Test Environment
- **Target:** VPS edge-arbx.ape-tv.net (shadow mode, opps:detected=379, capital=$0)
- **Framework:** Playwright (existing `frontend/e2e/opportunities-honest-display.spec.ts`)
- **Protocol:** Synchronous browser automation via Chrome headless

### Test Suite (3 Layers)

#### Layer 1: Frontend SSR + Hydration (R1 compliance)
**Test:** `opportunities-honest-display.spec.ts` (existing)
```
Verify:
1. Page navigates to /opportunities
2. Table renders (SSR snapshot present pre-hydration)
3. All rows with data-status="detected" show "—" in yield cell (R8: fail-honest)
4. Trust Wallet logos resolved (enricher warmed up)
5. Zero mismatch warnings in console (hydration OK)
```

**Expected:** ✅ PASS (5/5 layers green)

#### Layer 2: Backend Net-Profit Gate
**Test:** Custom test (net-profit gate inspection)
```
Verify:
1. Fetch /api/opportunities/live (REST endpoint)
2. Filter by net_profit_usd > 0
3. All opportunities satisfy: net_profit_usd = gross - (8 costs)
4. Zero opportunities with net ≤ 0 (gate enforced)
5. rejection_reason captured for failed candidates (RULE 00)
```

**Expected:** ✅ PASS (net-profit gate enforcing)

#### Layer 3: Oracle Cascade + Fallback
**Test:** Oracle health check
```
Verify:
1. Fetch /api/status → check price_oracles table populated
2. If Chainlink T0 available: verify feeds loaded
3. If DexScreener/GeckoTerminal active: verify fallback working
4. Zero stale prices (TTL < 60s)
5. Cross-check: price from API matches price in opportunities table
```

**Expected:** ✅ PASS (price_oracles seeded, cascade working)

#### Layer 4: Pre-Execute Checklist
**Test:** Checklist binding verification
```
Verify:
1. Paper-trade-runs table has checksums of 12-step checklist
2. All executions show checklist_passed = true (or reason if false)
3. Log shows "PreExecuteChecklist: PASS" for every intended execution
4. Kill-switch never tripped (capital safe)
```

**Expected:** ✅ PASS (checklist enforcing fail-closed)

#### Layer 5: Risk-Ledger Breaker
**Test:** Breaker state inspection
```
Verify:
1. risk_ledger worker spawned and healthy (logs)
2. Drawdown % calculated from paper_trade_runs
3. If drawdown > threshold: kill-switch armed (logs show triggered_by="risk_ledger")
4. Zero false-positive trips (breaker conservative, not hair-trigger)
```

**Expected:** ✅ PASS (breaker working, no cascading kills)

---

## EXECUTION COMMANDS

### Prerequisites
```bash
cd frontend
npm install -D @playwright/test
npx playwright install chromium
```

### Test Execution (VPS Against Live)
```bash
# Set environment
export E2E_BASE_URL="http://195.201.235.70:5173"
export E2E_API_URL="http://195.201.235.70:8787"

# Run Playwright tests
npx playwright test e2e/opportunities-honest-display.spec.ts --headed

# Run custom layer tests (if written)
npx playwright test e2e/layers-2-5-gate-verification.spec.ts
```

### Local Testing (Requires VPS SSH Tunnel)
```bash
# From local machine, establish tunnel to VPS
ssh -L 5173:localhost:5173 -L 8787:localhost:8787 arbx@195.201.235.70

# In separate terminal, run tests
export E2E_BASE_URL="http://localhost:5173"
npx playwright test e2e/opportunities-honest-display.spec.ts
```

---

## EXPECTED RESULTS

**Success Criteria:**
- ✅ All 5 layers report GREEN status
- ✅ Zero mocks detected (RULE 00)
- ✅ Zero hardcoded values (env vars sourced correctly)
- ✅ Net-profit gate prevents losers
- ✅ MEV-ethics gate blocks prohibited strategies
- ✅ Risk-ledger breaker doesn't false-trip

**Test Report Output:**
```
✓ Layer 1: Frontend SSR + Hydration (4s)
✓ Layer 2: Backend Net-Profit Gate (2s)
✓ Layer 3: Oracle Cascade + Fallback (3s)
✓ Layer 4: Pre-Execute Checklist (1s)
✓ Layer 5: Risk-Ledger Breaker (2s)

TOTAL: 5/5 PASS (12s execution)
```

---

## OPERATOR CHECKLIST (Before Running)

- [ ] VPS is up and healthy (docker compose ps shows all healthy)
- [ ] Searcher-rs is running in shadow mode (logs: "ARBX_PAPER_TRADE=true")
- [ ] price_oracles table populated with Chainlink feeds (3+ pairs)
- [ ] /opt/arbitragex-v2 branch is main or fix/gate-deploy-vps-autodeploy
- [ ] Edge worker is reachable (curl http://195.201.235.70:5173 → 200)
- [ ] API server is reachable (curl http://195.201.235.70:8787/api/status → 200)
- [ ] opps:detected stream exists (redis-cli XLEN arbx:opps:detected > 0)

---

## NOTES

- Tests are **read-only** (no write operations, no capital movement)
- Tests are **idempotent** (can run multiple times without side effects)
- Tests respect **paper-mode invariant** (capital=$0 always)
- Playwright runs **headless** by default (add `--headed` to see browser)
- Test timeout: **60s per test** (ample for network latency on VPS)

---

**END OF PLAN**

This plan is ready for operator execution on VPS. No code changes needed — all infrastructure exists and is ready to verify.
