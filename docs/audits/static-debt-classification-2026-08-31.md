# Static-Debt Classification — TODO/FIXME/STUB ledger (2026-08-31)

**Workbook item:** STATIC-DEBT-CLASSIFICATION (Holy_Grail_Audit_20260830_223027Z_ac08da8b3f.xlsx,
audited HEAD `ac08da8b3f`).
**Remediation HEAD:** this branch (post-ac08da8b working tree).
**Method:** `git grep -n -E '\bTODO\b|\bFIXME\b|\bSTUB\b|\bStub\b' HEAD` over tracked files,
plus an untracked-scope `rg` pass. Every production-code cluster below was READ, not pattern-guessed.

## Scope reconciliation (R8 — report what was actually measured)

| Scope | Count | Note |
|---|---|---|
| Workbook claim | 560 | Auditor scan; methodology not reproducible from tracked code |
| Tracked files at HEAD (this census) | **185 tokens / 121 files** | Includes docs, plans, skill definitions |
| Code files only (rs/ts/tsx/sol/sql/yml/sh/py) | **98** | The RULE-00-relevant surface |
| Untracked-tree rg (excl. node_modules/target/lock/md/jsonl) | 135 | Working tree incl. untracked |

The 560 figure is not reproducible under any tracked-file methodology; the honest
ledger below classifies the 185 tracked tokens (98 in code).

## Classification

### 1. LEDGER LEGITIMATE — 183 of 185 (documented, dated, honest)

* **Docs/plans/skills/audit text (~81):** `.agents/`, `.agent/`, `docs/superpowers`,
  `docs/architecture`, `audits/`, `.omega/PLACEHOLDER_BURN_DOWN.md` — planning and
  burn-down ledgers by design.
* **Contracts (20):** every adapter/flashloan TODO is a dated, tracked follow-up
  (`TODO(M12, audit 2026-05-10)`, `TODO (follow-up session)`); unwired adapters
  REVERT rather than fake (verified: `DyDxFlashAdapter.getFlashLoanAmount` "Currently
  reverts"). Not wired into `ArbitrageExecutor` = honest absence.
* **monitoring/alerts.rules.yml (11):** header states "Metrics marked TODO are not
  yet emitted; rules become active once emission is wired" — dormant rules, honest.
* **searcher-rs workers:** `jit_v3_worker` logs `scaffold_idle` and emits NOTHING
  (verified live-path: no fabricated Opportunity); `liquidation_bonus_bps_for_asset`
  returns a documented worst-case constant that UNDER-reports profit, never
  over-reports (conservative direction, R8-safe); `rpc_health_worker` TODO documents
  a duplication cleanup; `pool_sync_worker`/`scanner.rs` TODOs are RESOLVED-marker
  comments ("closes the TODO at scanner.rs:350").
* **api-server:** `routes/stubs.ts` serves honest 501s by design (R8 surface);
  `operator-authz.ts` / `bayesian_allocator.rs` / `CurveStableSwapAdapter.sol`
  mention the words only inside the doctrine sentence ("NO usa
  mock/stub/dummy/placeholder/TODO/FIXME").
* **sed-core:** `persistence` stub is feature-gated; `infrastructure.rs:72`
  "assume all listed services unhealthy" fails CLOSED (safe default).
* **frontend:** `useRegistry.ts configHash: null` — honest null with TODO to wire
  when backend supports it. **database/migrations 044 (3):** reseed source notes.
* **token-enricher (2), scripts/automation (5):** operational notes, no fake data.

### 2. DEFECT — 1 (FIXED in this remediation)

* `backend/searcher-rs/src/thermodynamics/adapters/dex_potential.rs::sample` —
  returned a FABRICATED `Some(PotentialGradient { potential_delta_usd: 0.0, ... })`
  while unwired: `Some(0.0)` claims "computed and exactly zero" (R8 violation
  pattern). **Latent only** — grep shows zero call-sites outside the adapters
  module (dead scaffold), so no live surface ever consumed the lie.
  **Fix (this branch):** returns `None` = not computed, matching the sibling
  `liquidation_potential` adapter. `cargo check -p searcher-rs` green.

### 3. BLOCKER — 1 (pre-existing, tracked elsewhere — not new debt)

* `backend/prioritization-spine/src/round_trip_executor.rs:295` —
  "TODO Phase 5: implement using LazyRpcDatabase + revm EVM". This is the KNOWN
  Executor-501 blocker already surfaced by the readiness gates (§IV A-blockers,
  vivid-grove audit) and the §34.3 terminus doctrine. Tracked there; duplicating
  it here would be double-counting, hiding it would be dishonest.

## Verdict

STATIC-DEBT-CLASSIFICATION: **RESOLVED-as-ledger + 1 DEFECT fixed**. The audited
"560 tokens" reduce to 185 tracked (98 in code), of which 183 are legitimate
dated ledger entries, 1 was a latent R8 lie (fixed), and 1 is the already-tracked
executor blocker. No hidden TODO-masked mock serves data in any live path.
