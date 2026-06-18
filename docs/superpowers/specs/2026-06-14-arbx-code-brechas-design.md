# ArbitrageX v2 — Code Brechas (Paper-Shadow) — Design Spec

**Date:** 2026-06-14 · **Branch:** `feat/code-brechas-paper-shadow` (off `main` @ `09dfeac`, PR #167)
**Operator:** HFRC · **Scope:** code-only, paper-safe. **`app.toml` `paper_mode=true` is NOT touched. Zero capital at risk.**

## 0. Provenance & method

Two audit reports (Backend "Brechas Paper-Shadow & Live", Frontend "Wiring/Menú/Lógica") describe HEAD `1b0c1b4` / **PR #171**. This tree is **PR #167** — older. Every claim was re-verified against the actual code via a read-only fan-out before any edit. **The reports were materially stale.**

### Brechas the audits listed that are ALREADY CLOSED in #167 (verified, no work needed)

| Audit claim | Reality in code |
|---|---|
| FASE 3.3 "mount `agents-status.ts`" | Mounted `index.ts:503` (`GET /api/v1/agents/status`) |
| `scoring-status` not wired | Mounted `index.ts:504` (`GET /api/v1/scoring/status`) |
| FASE 4.5 onboarding hub doesn't link phases | `app/onboarding/page.tsx` already links phases 1–5 via `getOnboardingStatus()` |
| 2.1 `GET /api/v1/wallets` → 501 | `wallets.ts` implements it (+balances/allowances), mounted `index.ts:498` **before** stubs → stub inert. Author deliberately does **not** surface `SIM_SIGNER_ADDRESS` (R8: dev sentinel `0x1234…7890` would imply real capital) |
| 2.4 `PUT /api/v1/dexes/:id/active` → 501 | `dexes.ts:204` implements it (toggles `dexes.is_active`), mounted `index.ts:496` → stub inert |

**Why `stubs.ts` looked like 8 live 501s:** Express dispatches the **first** matching route; `mountStubs` is mounted **last** (`index.ts:1327`) as a fallback layer. A real handler registered earlier shadows the stub. The audits counted stub *declarations*, not live behavior.

## 1. Backend — 4 endpoints to build (`backend/api-server/src/routes/`)

House contract (from `pools.ts`): each is a `mountX(app, deps)` fn; injected `pg.Pool | null`; **fail-honest** — `503 {error:"db_unavailable"}` on null pool / `503 {error:"query_failed"}` on throw; **never** synthesize rows (`rule_00`). All mounted in the `index.ts` block (~L495–557) **before** `mountStubs`. Dual-path `app.METHOD(["/api/…","/api/v1/…"], h)` because the FE fetches `/api/…` and the edge maps `/api/v1/…`.

| # | Route | Source | Behavior | Honesty |
|---|---|---|---|---|
| 1 | `GET /api/metrics/paper-shadow` + v1 | NEW `paper-shadow-metrics.ts` | Query `paper_trade_runs`. PnL = `COALESCE(actual_profit_usd, sim_expected_profit_usd, 0)` (**there is no `pnl_usd` column** — audit SQL was wrong). `consecutive_green_days` = walk back `date(created_at)` groups while daily PnL>0. Exact `PaperShadowResponse` shape (incl. `generated_at`). | 503 if pool null; `status:"INACTIVE"` if 0 trades; `COMPLETED` if green-days ≥ target(14) |
| 2 | `GET /api/sim-ctl/fork-status` + v1 | NEW `fork-status.ts` | Proxy `${SIM_URL ?? SIM_CTL_INTERNAL_URL ?? "http://sim-ctl:3003"}/fork-status` (timeout via AbortController). Map to `ForkStatusResponse`. | sim-ctl down/404 → `status:"UNAVAILABLE"`, `block_number:null` — **never fabricated** |
| 3 | `POST /api/v1/opportunities/:id/simulate` | NEW `opportunity-simulate.ts` | **Shadows the public stub.** Validate id; proxy to `${SIM_URL}/simulate` forwarding `{opportunity_id, ...body}`; return sim-ctl's response. | sim-ctl unreachable → `503 {error:"sim_unavailable"}`. Provider `not_implemented` → surfaces sim-ctl's honest error (correct R8, not a bug) |
| 4 | `POST /admin/alertmanager/webhook` | NEW `alertmanager-webhook.ts` | **Shadows the admin-gated stub.** `requireAdminToken(adminToken)` first. Parse `{alerts:[…]}`; for each → `writeAudit("alert.<status>", "alertmanager", "alert", labels.alertname, null, alertJson, ip, traceId, ua)`. Return `{received:true, count}`. | 401 (token) before anything; 503 on db fail; ignores malformed alert entries honestly |

`writeAudit` and `requireAdminToken`/`ARBX_ADMIN_TOKEN` are injected into the alertmanager module (codebase convention from `mountAdminChains`/`trading-config`), not duplicated.

**`stubs.test.ts` stays green:** it mounts `mountStubs` in isolation, so the stubs still return 501/401 there. The real handlers only shadow in the full `index.ts` app. `stubs.ts` is **not modified** (Mirror Law — stubs remain as fallback).

## 2. Frontend (`frontend/`)

| Item | File(s) | Change |
|---|---|---|
| Nav 4-group restructure | `components/nav-items.ts`, `components/app-sidebar.tsx` | `NavItem.group` → `"pipeline"\|"control"\|"setup"\|"omega"`; sidebar renders 4 section headers (Pipeline / Risk & Control / Configuration / Omega S5) + Home pinned; reassign all items per audit §1.2; **credentials first in Setup** |
| Dynamic paper-mode badge | server layout + `components/app-sidebar.tsx` | Server-fetch `/api/v1/config/current` → `execution.paper_mode`, pass as prop; badge green+"paper-mode" / red-pulse+"⚠ LIVE TRADING". Store has **no** `paperMode` field (audit's `useSystemStore(s=>s.paperMode)` was wrong) |
| Home tiles | `app/page.tsx` | Add `live-readiness`, `settings/credentials`, `paper/history` tiles (import `ListChecksIcon`, `KeyRoundIcon`, `FlaskConicalIcon`) |
| Wire dead-code panels | `app/live-readiness/page.tsx` | Render `PaperShadowPanel` + `ForkValidationPanel` (built but never imported) in a grid after Blockers — consume endpoints #1/#2 |
| PaperModeToggle gate | `components/paper-mode-toggle.tsx` | On flip→live (`val===false`): call `getReadiness()` → `flip_blocked`; if blocked, toast top-3 from `getReadinessBlockers()`, revert, return; else `confirm()`. (audit's `getReadinessDecision().flip_blocked`/`.blockers` **don't exist** — corrected) |
| Integration metric | `features/home/ProgressRealCard.tsx` | Bump `FE_INTEGRATION_PCT` from 22 to a verified value reflecting the closed brechas |

## 3. Rust (`backend/prioritization-spine/`)

`bayesian_allocator.rs` won't compile as-is: it reads `signal.n_observations`/`signal.success_rate` and its tests build `AdaptiveSignal{success_rate,n_observations,published_at}`, but the real `feedback.rs::AdaptiveSignal` has `revert_rate: f64`, `sample_count: i64`, `received_at`. Fix:
- `ingest_signal`: `n = sample_count (≤0 → return)`; `success_rate = (1.0 - revert_rate).clamp(0,1)`; `successes = round(n·success_rate)`; `failures = n - successes`; `posterior.update(successes, failures)`.
- Update all 5 `#[cfg(test)]` constructors to the real fields (`revert_rate = 1.0 - sr`, `sample_count = n as i64`, `received_at`).
- Then `lib.rs`: add `pub mod bayesian_allocator;` + `pub use bayesian_allocator::{Allocation, AllocationSource, BayesianAllocator, BetaPosterior};` (grep-checked for name collisions with existing glob re-exports first).

Gate: `cargo check -p prioritization-spine` = 0 errors; `cargo test -p prioritization-spine` green.

## 4. Validation (no VPS)
`frontend`: `tsc --noEmit` + `next build`. `api-server`: `tsc --noEmit` + `vitest run` (338 tests stay green). `cargo check -p prioritization-spine`. Then an adversarial multi-agent review pass; fix findings.

## 5. Out of scope → operator runbook (`docs/omega/OPERATOR_RUNBOOK_PAPER_SHADOW_2026-06.md`)
FASE 1 (RPC inject, token rotation, DexScreener), FASE 2 (`simulation.provider` flip needs `ANVIL_FORK_URL`; `ARBX_SCORING_ARCHIVER_MODE=on` is an env var not an `app.toml` key), FASE 6/7/9, contract deploy, service-control endpoints (operator picks docker-vs-systemd). All require operator secrets / VPS / on-chain actions I cannot and will not fabricate.

## 6. Delivery
Atomic commits per unit on `feat/code-brechas-paper-shadow`. No push/PR unless requested. `paper_mode=true` invariant preserved end-to-end.
