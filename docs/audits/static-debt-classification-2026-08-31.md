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

---

## Re-audit extension (2026-08-31, Holy_Grail_Audit_20260831_121502Z_0ad819a0b0 — claim: 561)

The re-audit widened the token set (TODO/FIXME/HACK/STUB/TEMP/**PLACEHOLDER**,
case-sensitive uppercase per sheet 55_TODO_SCAN) and re-raised the finding as
PERSISTS. Its own scan data (2,264 raw occurrences) was classified
per-occurrence; scope reconciliation first (R8):

| Bucket | Count | Reading |
|---|---|---|
| DOC_NARRATIVE (md/txt) | 1278 | plans, burn-downs, skill docs — ledger by design |
| DATA_FILE (json/jsonl/csv/lock) | 282 | registry/catalog data, not executable |
| CODE_COMMENT (`//`, `///`, `//!`, `#`) | 236 | documented notes inside code files |
| UI_HTML_ATTRIBUTE (`placeholder=`/`placeholder:`) | 133 | the HTML input affordance — the re-audit's new token matches React props, not debt |
| BACKUP_DEAD_CODE (app_backup/, *_backup/) | 97 | dead copies of already-classified files |
| **PRODUCTION_CODE (residual)** | **75** | reviewed one by one below |
| TEST_ONLY | 62 | fixtures/negative-controls in test scope (RULE 00 governs productive code) |
| CONFIG_SCRIPT (yml/sh/py) | 51 | dormant alert rules (header-documented), idempotent ops scripts |
| GENERATED_ARTIFACT (ci-artifacts/, playwright-report/, *.min.js) | 48 | committed build outputs — the token lives in minified third-party bundles, not source |

### The 75 production occurrences — clusters (every row read, not pattern-guessed)

* **Spanish determiner "todo" (3):** `config/trading/page.tsx:58` ("todo control…
  muta runtime") — **fixed in the DAPP-SURFACE-FAIL branch** (todo→cada);
  `.cursor/rules/omega_fidelity.mdc:12` ("Todos los Endpoints", editor-rule doc);
  `_recovery/*.patch` (recovery stash artifacts, Spanish inside quoted comments).
* **Anti-placeholder DEFENSES — the token is in the guard, not the hole (4):**
  `credentials/validators.ts:359+364` (PLACEHOLDERS blocklist → `fail("placeholder
  token rejected")`), `shared-ts/middleware/index.ts:57` (SECURE_BOOT refuses known
  placeholder/backdoor values), `agents-status.ts:387` (anti-mock scan evidence
  string — **rephrased in the DAPP-SURFACE-FAIL branch**), `e2e/page_by_page_audit.spec.ts:50`
  (the repo's own FORBIDDEN_WORDS regex).
* **Fail-safe stubs, feature-gated or panic-loud (30):** `prioritization-spine/simulator.rs`
  (6; DEPRECATED v1 stub, fail-closed sentinel, tests assert `must never return PASS`),
  `strategy_applicability.rs`/`sim-ctl/revm_backend.rs` (3; tests assert stub never
  passes/claims engines), `sed-core/allocator` `#[cfg(not(feature = "allocator"))] mod stub`
  (5; compiles out under the real feature), `sed-core` type-level `stub()` constructors
  consumed in tests (6), `shared-rs/price_oracle.rs` test-cascade stubs (8),
  `api-server/routes/stubs.ts:50` (uniform honest 501s), `sed-core/types/kill_switch.rs:65`
  (`todo!()` — documented Phase-1 PANIC so a kill-switch query can never silently
  return a default that would mask an emergency stop), `workers/hft_mempool_listener.rs:25`
  (idle scaffold logs honestly, emits nothing).
* **Honest UI self-declaration (6):** onboarding Phase2–5 `curl-only stub` badges —
  the surface labels itself, the opposite of hidden debt.
* **Documented migration/registry debt (9):** migrations 043/044/046/090 seeds with
  dated source TODOs and `'pending_boot_load:<slug>'` distinct pre-boot placeholders;
  `useRegistry.ts:105` (honest null — already in the section above).
* **UI placeholder props/CSS (10):** multi-select-all, select.tsx, SimulationTab,
  translator `placeholder-zinc-500`, atlas-glass `::placeholder`, theme-toggle
  pre-mount guard, CredentialsClient description text.
* **Docs-as-data (2):** `docs/architecture/frontend-wiring-dashboard.html` — a wiring
  audit artifact whose payload literally encodes `MISSING_CONTRACT` statuses.
* **Display-label debt notes, honest (2):** `backrun_engine.rs:117` /
  `cex_dex_engine.rs:116` — enum label reuse marked "Placeholder, needs specific
  label" (display metadata; no economic/data impact).
* **Cross-branch fix already landed (2):** `config/trading/page.tsx:58` and
  `deploy-pipeline/data.ts:303` — the exact two strong-marker strings remediated in
  the DAPP-SURFACE-FAIL branch (rephrased fail-honest).

### Re-audit verdict

**0 DEFECTs** among the 75 production occurrences; the 1 DEFECT from the first
ledger pass (dex_potential `Some(0.0)`) remains fixed and was NOT re-detected.
The 561-claim reduces to 75 real production-code occurrences, all LEGITIMATE
(dated ledger entries, fail-safe guards, honest self-labels, UI affordances) or
already-remediated cross-branch. STATIC-DEBT-CLASSIFICATION: **RESOLVED-as-ledger
(re-verified at 0ad819a0)**.
