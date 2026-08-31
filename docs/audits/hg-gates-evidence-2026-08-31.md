# Holy Grail v1.3.1 — Gates runtime evidence (2026-08-31)

**Audited HEAD (workbook baseline):** `0ad819a0b0324112742058490d8d221912f5d91a`
**Workbook:** `Holy_Grail_Audit_20260831_121502Z_0ad819a0b0.xlsx` (52_FINAL_GATE_2026).
**Evidence HEAD:** this branch (post-#493/#494 main `18bd247f` + remediation PRs #495–#498).

This file records RUNTIME evidence (commands + observed output, R8-honest) for the
final-gate rows that were actionable in this remediation loop. It never upgrades a
gate's verdict without a measurement; where the measurement is negative, the
verdict is recorded as negative.

---

## F-12 / F-23 — Local/full suite + Protected CI (GEN-CI-FAIL)

**Observed at baseline:** GitHub checks `{'success': 40, 'failure': 1}` — the
failure was the Dependabot check (vulnerable manifests: sqlx 0.8.0 GHSA,
rmcp GHSA-89vp-x53w-74fx).

**Remediation:**
- PR **#493** — sqlx 0.8.0 → 0.8.1 (GHSA-4qr7-m7pj-3mfq class, `bytes` unsoundness).
- PR **#494** — mcp-sim-engine port to rmcp 3.1.4 (GHSA-89vp-x53w-74fx). Squash-merged
  → main `18bd247f446756a3f170bd93742f30f141fe3be2` (2026-08-31), all checks green
  on the PR head including `analyze (rust)`.

**Verification of the merge commit (37 check-runs at poll time):**
14 `success`, remainder `queued`/`in_progress` ( Dependabot included — its closure
predicate is the next scheduled run finding no unresolvable vulnerable Rust manifest).
Dependabot ALERTS remain open for the npm surface (F-23 follow-up: PR #490 vitest
1.6.1 → 3.x major bump; 91 npm advisories total reported by `npm audit` on push,
7 critical — tracked, not silently dismissed).

**Honest status:** F-12 evidence chain restored (failure → fix → merge → checks);
F-23 remains PARTIAL until the npm major-version bumps land and Dependabot
re-runs clean. No green was painted: the failing check was fixed, not skipped.

## F-13 — Security review (response headers)

**Observed at baseline:** DApp `/` carried 7 security headers; the `/api/*`
surface (edge worker origin) carried **ZERO** (live `curl -I` 2026-08-31,
pre-fix: no HSTS, no X-Content-Type-Options, no X-Frame-Options, no Referrer-Policy,
no Permissions-Policy, no CSP).

**Remediation (PR #498):** `hono/secure-headers` middleware on `*` in
`edge/worker/src/index.ts` — HSTS 1y+subdomains, nosniff, DENY framing,
no-referrer, camera/mic/geolocation denied, CSP `default-src 'none'; frame-ancestors 'none'`
(object form per hono 4.13 typing). Mounted BEFORE the request-log/CORS middleware so
it applies to every response incl. `notFound`.

**Post-deploy verification (to re-run on the deployed HEAD):**
`curl -sI https://arbx.ape-tv.net/api/health` must return the six headers.
SSE routes (`/api/live-testnet/events`) unaffected (`X-Accel-Buffering` passes through).

## F-14 — Redis durability/stall audit (live, read-only)

Executed on VPS `2026-08-31` (`ssh arbx docker exec arbitragex-v2-redis-1 redis-cli ...`):

| Probe | Observed | Verdict |
|---|---|---|
| `aof_enabled` | 1 | AOF on |
| `appendfsync` | everysec | durable-enough config |
| `aof_last_write_status` | ok | no write stalls |
| `aof_last_bgrewrite_status` | ok | rewrites healthy |
| `rdb_last_bgsave_status` | ok | snapshots healthy |
| `stop-writes-on-bgsave-error` | yes | fail-closed on persistence error |
| `loading` | 0 | not mid-load |
| `rejected_connections` | 0 (cumulative) | no connection storms |
| `latest_fork_usec` | 7578 | sub-10ms forks |
| `aof_current_size` | ~332 MB | bounded, monitored |

Stream truth: `XLEN arbx:opps:detected` = 10003, consumption lag 0 (prior
remediation #492). **F-14: audit evidence complete** — durability knobs all
nominal at observation time; this is a point-in-time audit, not a continuous SLO.

## F-15 — Pre-sim discovery p95 < 30 ms (measured — verdict FAIL, honest)

The instrument (`backend/searcher-rs/src/latency_budget.rs`, XLS-QB-07b) is wired
into the live hot path (`route_discovery_worker.rs` `begin_cycle`/`lat_record`/
`snapshot()`) and published on the existing tick wire
(`GET /api/route-discovery/tick`, `lat_stages` + `lat_pass_p95` + `lat_cycles`,
Zod-mirrored in `frontend/lib/apex/schemas/telemetry.ts`, UI
`frontend/app/operations/components/latency-budget.ts`).

**Live measurement — `curl https://arbx.ape-tv.net/api/route-discovery/tick`
(2026-08-31, HTTP 200, window of 39 completed cycles):**

| Stage | Target | p50 | p95 | Headroom p95 |
|---|---:|---:|---:|---:|
| lat.decode | 2 ms | 78 µs | **134 µs** | +1 866 µs ✅ |
| lat.state | 3 ms | 54.6 ms | 84.3 ms | −81 279 µs ❌ |
| lat.reprice | 4 ms | 3.7 ms | 13.9 ms | −9 906 µs ❌ |
| lat.pair | 3 ms | 97.0 ms | **497.2 ms** | −494 169 µs ❌ |
| lat.expand | 7 ms | 49.8 ms | 122.3 ms | −115 280 µs ❌ |
| lat.refine | 5 ms | `null` | `null` | not computed (no samples — honest) |
| lat.gates | 3 ms | 283 µs | **479 µs** | +2 521 µs ✅ |
| lat.emit | 2 ms | 4.4 ms | 67.1 ms | −65 060 µs ❌ |
| **lat.total** | **29 ms** | **375.4 ms** | **829.4 ms** | **`lat_pass_p95 = false`** |

**Verdict: F-15 = FAIL (measured).** The system's own gate emits
`lat_pass_p95: false` — 829 ms p95 vs the 30 ms SLA (`discovery_sla_ms`). This is
the workbook row-21 prediction confirmed by instrument: "<30 ms NO puede
garantizarse por Excel; debe demostrarse". The evidence chain
Code→wire→runtime→API is now COMPLETE (was "No trusted percentile benchmark
ingested"); the SLA itself is NOT met. Dominant offender `lat.pair` (direct
inefficiency scan, p95 497 ms) then `lat.expand` (122 ms) and `lat.state` (84 ms).
`lat.refine` records zero samples (amount-aware refinement not exercised in the
current mode). Remediation = a dedicated hot-path performance effort; this audit
refuses to reclassify FAIL as PASS.

## F-04 / F-05 / F-06 — 264 map parity (ALPHA-MAP-ID-DRIFT)

Closed by `docs/audits/alpha-map-id-drift-2026-08-31.md` (PR #496): static 266 =
264 canonical + MEV-99-999 (TEST negative-control sentinel) + MEV-05-042 (doc
example, nonexistent by design); HopMask parity 264/264 EQUAL, Detector parity
264/264 EQUAL, Status↔Dispatch semantically identical (ROUTE_READY 79 /
NEEDS_ROUTE_DATA 174 / OBSERVE_ONLY 8 / NO_COMPATIBLE_ROUTE 3); runtime registry
271 = 264 + 7 legacy slug-loaded cartridges (loaded axis ≠ dispatch axis —
consistent, not drift).

## DAPP-SURFACE-FAIL — marker surfaces (PR #495)

3 strong markers restructured at source (agents-status anti-mock wording; two
Spanish honesty notes); 21 unlabeled controls labeled (13 CapitalRiskTab, 7
DexRegistry, 1 forge source textarea); 2 Radix hidden internals classified
FRAMEWORK_INTERNAL (false positives — hidden select/checkbox with
`aria-hidden` + `tabindex=-1` are form-association plumbing); 4 edge `/api` route
gaps fixed with discriminated live evidence (worker `notFound` JSON vs api-server
"Cannot GET" HTML): `/api/operator/me` (cookie-forwarding walletProxy),
`/api/live-testnet/events` (SSE), `/api/onboarding/status` (versioned path),
`useLiveTestnetStatus` → public `/api/readiness/decision`. `/wallet` = FAIL-SAFE
+ BLOCKED_EXTERNAL (operator must supply NEXT_PUBLIC_WALLETCONNECT_PROJECT_ID);
`/config` 401 = AUTH-GATE legitimate. Verified: edge tsc, frontend tsc, vitest
105/105, api-server tests 20/20.

## CUR-001..012 — positive re-proof status (NOT_REDETECTED ≠ RESOLVED)

Re-proofs executed against current HEAD (`git ls-tree 0ad819a0…` and `origin/main`
— the productive page set is IDENTICAL at both):

| ID | Re-proof evidence (this loop) | Verdict |
|---|---|---|
| CUR-001 | Exact set arithmetic: legacy 56-row contract = **45 real routes + 1 name-drift (`/admin/sign-in` vs repo `/admin/signin`) + 10 `RECONCILE_47..56` placeholders** (contract declares itself incomplete). Repo = **57 productive pages** = 45 contract-matched + **12 newer surfaces** (monitor, live-testnet, onboarding×5+root, operator/presets, translator, readiness, omega-s5/registry/[entity], admin/signin). Drift is contract-internal, NOT a repo defect; zero missing/extra files vs SSOT. | **RESOLVED (contract is the stale side)** |
| CUR-002 | Stale `AppState.engine` doc-comment in sim-ctl (claimed consumer Anvil-only) — consumer has dispatched via `SimulatorBackend` since SIMWIRE-01 #481. Comment fixed in PR #500. | **RESOLVED (#500)** |
| CUR-003 | `live_exec_policy.rs` re-read at HEAD: `MainnetRefused` at 4 return sites; module doc "chain_id == 1 rejected UNCONDITIONALLY". | **RE-PROVEN (BLOCKER-BY-DESIGN, protected)** |
| CUR-004 | Superseded by the F-15 measurement above: p95 measured 829.4 ms — the claim "<30 ms" is now DISPROVEN by instrument, not merely unproven. | **CONFIRMED(measured) — perf backlog, no longer an evidence gap** |
| CUR-005 | PairBuckets consumer-parity E2E — not re-proven this loop (needs instrumented E2E run). | **OPEN (F-08 class)** |
| CUR-006 | QuoteScore live completeness/hot-mutation ACK — not re-proven this loop. | **OPEN (F-09/F-10 class)** |
| CUR-007 | Per-mutation runtime ACK — not re-proven this loop. | **OPEN (F-10 class)** |
| CUR-008 | Full classified ledger delivered (PR #497): 2 264 raw → 1 278 DOC_NARRATIVE / 282 DATA_FILE / 236 CODE_COMMENT / 133 UI_HTML_ATTRIBUTE / 97 BACKUP_DEAD_CODE / **75 PRODUCTION_CODE (0 defects after cluster review)** / 62 TEST_ONLY / 51 CONFIG_SCRIPT / 48 GENERATED_ARTIFACT. | **RESOLVED (#497)** |
| CUR-009 | Env-name sensitivity classification executed over the workbook inventory digest (175 unique names): **17 SECRET-TIER** (name-pattern: KEY/TOKEN/SECRET/PRIVATE/PASSWORD/MNEMONIC/SEED/CREDENTIAL — e.g. `EXECUTION_SIGNER_KEY`, `ARBX_ADMIN_TOKEN`, `GITHUB_TOKEN`), **54 ENDPOINT** (secret-bearing iff the URL embeds an API key — the RPC-URL class), **104 CONFIG**. §33 spot-verified: zero tracked real `.env` files; `.env.example` values are empty/`<run: openssl …>` placeholders; the one tracked `.env.crucible` is a TESTNET TEMPLATE with `0xCHANGE_ME` sentinels and public RPCs (no secrets); gitleaks CI green across the fleet is the authoritative scanner. | **PARTIAL → classified; per-name owner/scope table outstanding** |
| CUR-010 | Redis durability proven live (F-14 above: AOF ok/everysec/no stalls/forks 7.6 ms/rejected 0). Per-resource TTL/consumer table for the 207 `arbx:*` keys outstanding. | **PARTIAL (durability ✅, per-key policy OPEN)** |
| CUR-011 | 584 static route signatures — mount-prefix normalization outstanding (edge route table now partially enumerated by DAPP-SURFACE work). | **OPEN (LOW)** |
| CUR-012 | Grep over tracked cert/docs surfaces: ZERO promissory claims; the only "guaranteed profit" hits are `ANTI_PATTERNS_FROM_DEMOS.md` rows that CONDEMN such claims (plus stale worktree copies). | **RESOLVED (doctrine holds — no cert criterion promises profit)** |

## PROTECTED TRUTHS — no-regression statement (F-18 / F-22 / F-24)

Diff scope of this remediation (#493–#498) touches NO execution-path, signer,
evaluator, or authorization code: dependency manifests (#493/#494), frontend
labeling/wording + edge read-only proxy routes (#495), documentation (#496/#497),
response headers (#498), this evidence file. `live_exec_policy` default-deny and
`MainnetRefused`, kill-switch paths, and the operator-only live-flip boundary
(§34.3) are untouched — verified by the PR file lists, not by assumption.
