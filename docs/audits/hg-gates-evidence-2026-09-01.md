# Holy Grail v1.3.1 — P2 request classification + F-gates runtime evidence (2026-09-01)

Workbook baseline: `Holy_Grail_Audit_20260901_064750Z_c719f4e99c.xlsx` (HEAD
c719f4e9, deployed). Sibling of `hg-gates-evidence-2026-08-31.md`; this file
records the NEW-cycle probes executed 2026-09-01 against the deployed c719f4e9.

## P2 (§8) — the aggregate "603 failed requests", classified

The workbook's "Salud de servicios" row aggregates the browser run's failed
requests (63 visited, 603 failures) with PASS predicate "prove health/failover
through runtime endpoints, not page reachability". Method here: normal-pace
Chromium probe over all 57 certified surfaces capturing every `requestfailed`
event (url + failure) and every response ≥400, then classified:

| Class | Count | Explanation |
|---|---|---|
| EXPECTED_RATE_LIMIT (429) | 3341 | nginx `limit_req` tripped by the PROBE's own single-IP aggregate polling across 57 surfaces. Real single-surface user traffic sits far below the zone (control sample 2026-08-31: 0 errors at normal pace). Measurement artifact, same class as the documented 2026-08-31 sweep artifact. |
| EXPECTED_RSC_CANCEL | 2597 | Next.js `?_rsc=` prefetch aborted on navigation — universal across all surfaces including FULL-PASS ones. |
| EXPECTED_NAVIGATION_ABORT | 25 | in-flight requests aborted by navigation (ERR_ABORTED, non-RSC). |
| EXPECTED_AUTH | 8 | 401/403 on admin-gated operator endpoints without a session — the auth-gate working as designed. |
| EXPECTED_HONEST_404_UNKNOWN_ENTITY | 2 | `/omega-s5/registry/0x…0001` (probe sentinel) → 404 document for an entity that does not exist — fail-honest (R8), not a defect. |
| **ACTUAL_FAILURE_HTTP_400** | **8** | `pulse.walletconnect.org/e?projectId=walletconnect_project_id_missing` on /wallet — see disposition below. |

**ACTUAL failure disposition (8×400 WalletConnect telemetry):** root cause is
the absent `NEXT_PUBLIC_WALLETCONNECT_PROJECT_ID`. Our fail-honest placeholder
(`wagmiConfig.ts:71`) keeps injected/EIP-6963 wallets working, but the
transitive SDK (RainbowKit → @reown/appkit html-core) still beacons telemetry
to the third party with the placeholder id → remote 400s. No documented SDK
option suppresses that beacon without a real project id; the honest options are
operator-side:

1. **Set a real WalletConnect/Reown project id** (free, Reown Cloud) in `.env`
   → beacons become valid, WalletConnect fully enabled. Requires the
   §03 `NEXT_PUBLIC_*` rebuild procedure (RULE 03).
2. **Accept the documented degraded state** — the 8 requests stay classified
   ACTUAL with this root cause on record (not silently dismissed).

Neither is taken unilaterally here: the id is operator material (no-hardcode,
RULE 00). Raw ledger: `failed_req_classification.json` (probe artifact).

## F-13 — security headers (live re-verify at c719f4e9)

`curl -sI https://arbx.ape-tv.net/api/health` and `/` → **6/6** on both origins
(HSTS 1y+subdomains, nosniff, DENY, no-referrer, camera/mic/geo denied, CSP).
The headers fire even on a 429 response body — middleware ordering covers the
error path, not just 200s. The workbook's `headers=0.8` counts an 8-header
expectation; the two not sent (COOP/COEP) are absent by design (SSE streaming
+ embedded operator panels would break under COEP) — declared limitation, not
an omission.

## F-14 — Redis durability/stall audit (live, read-only, 2026-09-01)

| Probe | Observed | vs 2026-08-31 |
|---|---|---|
| appendfsync | everysec | same |
| aof_enabled / write / bgrewrite / rdb statuses | 1 / ok / ok / ok | same |
| loading | 0 | same |
| rejected_connections | 0 | same |
| stop-writes-on-bgsave-error | yes (fail-closed) | same |
| aof_current_size | 399 MB | +67 MB/day growth (332 MB yesterday) — bounded, watch item |
| latest_fork_usec | 14 853 | 7 578 → 14.9 ms (AOF rewrite forks on a larger dataset; still sub-15 ms, trending — watch item, threshold 20 ms suggested for the next audit) |

## F-07 / F-09 / F-10 / F-16 / F-17 — runtime E2E evidence (live, read-only)

Probed VPS-internal (localhost:8080, bypassing the edge rate limiter):

- **F-09 (quote inputs/version/ACK)** — `GET /api/quote/anchor` → live
  `quote_version: 36`, per-token component scores (cross_dex / liquidity /
  prior / stability / venues), `quote_score: 68.13` on the anchor pair. The
  quote inputs ARE flowing end-to-end at runtime.
- **F-07 (dynamic scoring core)** — `GET /api/v1/scoring/status` →
  `source: "runtime_evidence"`, `scoring_pipeline_state: "WIRED_RUNTIME_SAMPLE"`,
  bayesian filter / kelly / vpin / archiver all `wired: true`, `a4_state:
  A4_PASSED`, `a5_state: PAPER_SHADOW_WARMING`.
- **F-16 (simulation backend flow)** — `GET /api/v1/sim/pipeline` →
  `strategy_count: 41` with live per-strategy scored/accepted counts
  (dex_arb scored=37 452), `scoring_circuit.posture: observe_only_advisory`,
  and the honest gap kept visible: `calibrated_strategies: 0`,
  prior-writer follow-up pending (matches the calibration-pipeline reality map:
  stage2 κ=20 gated OFF until flips produce labels).
- **F-17 (net economics)** — `GET /api/paper-mode/state` → paper mode enabled
  on all chains, `confidence: "explicit"`, source redis, `degraded: false`,
  `conflict: false`; `GET /api/v1/strategies/runtime-status` → per-strategy
  `engine_loaded/engine_invoked` live with R8 rejection reasons
  (`single_pool_no_spread`) and `paper_viable_1h: 0` — honest zeros, no
  fabricated economics.
- **F-10 (configured vs effective)** — `GET /api/v1/config/canonical-knobs`
  exposes the searcher-rs boot snapshot (57 knobs; superset of the workbook's
  53 — growth documented, not drift). The VPS `.env` contains **zero
  `ARBX_KNOB_*` overrides**, so effective == compile-time defaults for every
  knob: there is no configured value that could have drifted, and the snapshot
  endpoint is the standing ACK surface. Knob template disposition: Configured =
  `(default — no env override set)`, Effective = snapshot value, ACK class =
  read-only until an operator sets an override (then write→persist→consumer→ACK
  applies per the workbook's acceptance column).

## A.1–A.9 — runtime fact-forcing evidence (40_GATES_A1_A9: "runtime proof still missing")

All probes VPS-internal (`localhost:8080`), read-only, executed 2026-09-01
~07:44Z against deployed c719f4e9. The workbook marks A.1–A.8 PARTIAL/FAIL
with blocker "Runtime fact-forcing proof still missing" — this section is that
proof, surface by surface. Where a sub-predicate has no runtime surface yet,
it is recorded as the honest gap (never fabricated — R8).

| Gate | Runtime evidence (endpoint → observed) | Residual honest gap |
|---|---|---|
| **A.1** Identity & Data Quality | `GET /api/pairs` → live TokenKey in canonical form (chain_id + 0x40-hex address + decimals + symbol, e.g. WBTC/8, USDC/6). Provenance visible on every surface: scoring `source:"runtime_evidence"`, fork-status `rpc_url_redacted:"sim-ctl:***"` (secret never leaves), killswitch `triggered_by`+`updated_at`, math evidence `source:"declared_combo"` + `strategy_kind` + `updated_at_ms`. | Outlier-quarantine guard (paper-trade-archiver, PR-tested) fired **0×** in 48h of runtime logs — armed but not proven-by-fire (no invalid event reached it in window). Guard exists; runtime firing not yet observed. |
| **A.2** Freshness & Version | `GET /api/quote/anchor` → ONE atomic snapshot response carrying `chain_id:1, graph_version:0, quote_version:36` + per-token component decomposition. `GET /api/v1/readiness` → all verifier items stamped the same instant (`2026-09-01T07:44:14.492Z`). `GET /status` → 7/7 services ok in the same probe. | Graph builder versions are coarse (`graph_version:0` = never bumped); the admin tokens-resolve surface keeps `graph_version: null` by R8 design until the builder versions rebuilds (index.ts:938–956, documented gap). |
| **A.3** Liquidity / Pair Executability | `GET /api/pairs` → live per-pair venue rows with REAL reserves (`WBTC/USDC UniswapV2 reserves_a=62 887 377 / reserves_b=49 491 455 583`, `fee_bps:30`), freshness `last_reserve_update` ms and honest `dirty` flag; anchor pair liquidity component = 100/100. | Executable-depth-for-amount (differential quote at size) is proven at sim/pipeline level, not as a standalone per-amount runtime endpoint. |
| **A.4** Strategy / Hop / Route | Runtime registry `GET /api/cartridges/runtime` → **271 = 264 canonical + 7 LEGACY_RUNTIME** (verbatim export `alpha-map-parity-runtime.json`, PASS against the 264 SSOT). `GET /api/v1/scoring/status` → `a4_state: "A4_PASSED"`. `GET /api/strategies/catalog` → live MEV-01-001 row (HopMask 2–7, detector R_CLOSED_CYCLE, `execution_class:"DETERMINISTIC_EXECUTABLE"`). | Workbook raw-scan cardinality 266 vs canonical 264 → closed by the alpha-map-parity CI gate (PR #504); re-audit is the counting authority. |
| **A.5** Sizing / Exact Math | `GET /api/math/evidence/all` → `count:28` live operator-evidence items per `strategy_kind` with computed scalars (Golden-Section, Kelly, Newton-Raphson, SVD, GNN) and `source:"declared_combo"` provenance — the 31-operator math engine IS computing at runtime. | Differential/exact-quoter equivalence is still test-harness evidence (repo), not a runtime differential surface. |
| **A.6** Net Economics | `GET /api/v1/risk/circuit-breakers/status` → `mode:"paper_only"`, `capital_exposure_usd:0`, DD tiers computed from the REAL paper ledger (598 877 runs / 1224h, NAV-anchored); overall `WARN` honest. `GET /api/v1/analytics/viable-kpis` → 24h totals `82 849` opportunities, `viable:0` — honest zero, no fabricated economics. `GET /api/v1/recon/summary` → window totals 0 with `avg_pnl_included_usd:null` (None ≠ 0). | Prometheus alert emission from breaker state remains the known partial (readiness blocker `a6_circuit_breakers_partial`); zero viable/24h keeps NetUSD decomposition unfired — honest absence. |
| **A.7** Simulation | `GET /api/v1/sim-ctl/fork-status` → `status:"HEALTHY"`, pinned fork `block_number:25 879 822`, `rpc_latency_ms:1`, `simulations_today:0` (honest), RPC redacted. `GET /api/v1/sim/pipeline` → 41 strategies live scored/accepted counters (dex_arb scored=37 452), posture `observe_only_advisory`, `calibrated_strategies:0` honest. | Determinism is proven by the block-pinned replay design (SIMWIRE-02c) + pipeline counters; no per-replay hash endpoint exists to cite. |
| **A.8** Execution Authorization | Kill-switch live state: `enabled:false` with full provenance (`reason:"re-arm paper pipeline…"`, `triggered_by:"omega-diagnosis-2026-08-29"`, `updated_at:2026-08-29T16:30:15Z`) — and `401 unauthorized` without the `x-arbx-admin-token` header (auth-gate defense verified). `GET /api/capital-gates` → `live_enabled:false, broadcast:false, submit_enabled:false, private_relay_enabled:false, capital_exposed:0`. `GET /api/v1/readiness/decision` → `verdict:"NO_GO"`, `go_live:false`, `capital_exposure_usd:0`. `GET /api/v1/relays` → `count:0` (no relay configured; relays-client stays default-deny — §34.3). `GET /api/v1/go-no-go/status` → `state:"no_ledger"` honest. | None for the paper/testnet posture — by design everything that could broadcast is OFF and evidenced; the flip itself is operator-only. |
| **A.9** Receipt / Recon / PnL | `GET /api/v1/executions/recent` → `count:0`; `GET /api/v1/recon/summary` → totals 0/0/0, null averages. Honest absence of executed receipts in paper mode. | **PENDING by doctrine** — requires observed executed IDs and reconciled realized PnL; never auto-PASS from source. This gate stays with the operator (§27 economic-claims separation). |

## F-15 / F-19 / F-20 — unchanged, honest

- **F-15**: measured 2026-08-31 at p95 829.4 ms vs the 30 ms SLA (`discovery_sla_ms`
  visible in the canonical-knobs snapshot) — honest FAIL, perf backlog
  (dominant `lat.pair` 497 ms). No benchmark ingested since; nothing new to
  certify.
- **F-19**: recon service present; realized PnL deliberately not auto-certified
  (economic-claims separation, §27 doctrine) — unchanged.
- **F-20**: the 2 blocking findings are ALPHA-MAP-ID-DRIFT (gate landed in the
  alpha-map-parity PR) and the surface PARTIALs (closed in the dapp-surface PR)
  — the re-audit after both merges is the counting authority.

## Operator decision items surfaced by this evidence

1. WalletConnect project id (or accept the classified degraded state) — above.
2. nginx `limit_req` burst tuning if a single legit user ever sweeps surfaces
   quickly (today only probes trip it) — backlog, low priority.
3. Redis watch items: AOF +67 MB/day growth, fork 14.9 ms — re-check next audit.
