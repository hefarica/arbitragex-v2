# Pre-Execute Checklist — 7 Gates in Series

**Doctrine:** `arbx-pre-execute-checklist`
**Owner:** relays-client + sim-ctl
**Reviewed:** 2026-05-10
**Enforced via:** `/api/v1/readiness` gate `G-PEC-1` + runtime checks in
`backend/relays-client/src/submit_engine.rs` and
`backend/relays-client/src/main.rs::execute_handler`.

This document is the **definitive list of in-series gates** that every
opportunity must clear before a transaction is signed and broadcast to a
private relay. The order matters: cheaper gates run first, expensive
gates last. Any gate failing short-circuits the rest and discards the
opportunity with a structured `arbx_execution_total{status="rejected"}`
metric.

A passing checklist does not guarantee profit — it guarantees that the
attempt was not blind.

---

## Gate 1 — Killswitch

- **Source of truth:** Redis key `arbx:killswitch:enabled` (canonical)
  with file fallback `killswitch.json` at boot and config default in
  `configs/app.toml [system].kill_switch_enabled_default`.
- **Behavior:** if `enabled=1`, **abort immediately**. No further gates
  evaluated. Metric `arbx_execution_total{status="killswitched"}`.
- **Why first:** cheapest possible check (single Redis GET); blocks every
  other gate's wasted CPU during incidents.

## Gate 2 — Blacklist

- **Source of truth:** Redis sets `arbx:blacklist:tokens:{chain_id}` per
  chain plus `arbx:blacklist:pools:{chain_id}` for pool-level bans.
- **Behavior:** if any token in the route's path is blacklisted, abort.
  Same for pools. Metric `arbx_execution_total{status="blacklisted"}`.
- **Why second:** still cheap (Redis SISMEMBER × N hops), and blacklist
  hits are common during honeypot bursts — keeps the rest of the pipeline
  from doing wasted work.

## Gate 3 — Net-profit floor

- **Source of truth:** `Opportunity.net_expected_profit_usd` — populated by
  the prioritization-spine evaluator (`calc_net_profit_and_roi`) after deducting
  all **8 cost components**:
  1. Gas cost (`expected_gas_cost_usd`) — EIP-1559 basefee + tip, converted to USD.
  2. LP fees (`lp_fees_usd`) — protocol fees across all route hops.
  3. Slippage / price-impact (`effective_slippage_usd`) — real V2/V3 impact or proxy.
  4. Statistical failure buffer (`failure_cost_usd`) — `p_fail × gas_cost` or flat proxy.
  5. Copy-trade / front-run buffer (`copied_buffer_usd`) — `p_copied × gross_proxy`.
  6. Capital opportunity cost (`capital_cost_usd`) — APR × capital × block_time / year.
  7. Ops overhead (`ops_overhead_usd`) — amortised infra cost per attempt.
  8. **Relay bribe (`relay_fee_usd`)** — Flashbots `coinbaseDiff` EWMA per
     `(chain_id, strategy_kind)`, stored at `arbx:relay_fee_ewma:{chain_id}:{strategy}`.
     Cold-start doctrine floor: `max(gross × 5%, $0.50)`.
     On Ethereum mainnet this is typically 10–50% of gross profit.
     L2 chains (Arbitrum, Base, Optimism, Polygon): always 0.0.

  Component 8 was absent before the C2 fix (audit re-run #2, 2026-05-10),
  causing a systematic 20-40% overstatement of net profit on Ethereum mainnet.

- **Behavior:** `net_expected_profit_usd >= cfg.execution.min_net_profit_usd`. If
  not, abort with `status="below_min_profit"`.

- **Live mode gate (C1 fix):** if `net_expected_profit_usd` is `None` (spine
  has not evaluated the row), the checklist returns `NetProfitUnknown` and the
  opportunity is dropped. Falling back to the gross `expected_profit_usd` is
  **forbidden in live mode** — gross overstates net profit by the relay bribe
  component alone. In paper mode the gross fallback is allowed with a warn log.

- **Why third:** computed earlier in the pipeline — at this gate it's a single
  comparison. Cheaper kill-switch and blacklist gates run first.

## Gate 4 — Simulation

- **Source of truth:** `sim-ctl` (Tier-1) or `simulator-v2` (Tier-2,
  pending A3) returns `pass=true` for the exact bundle that will be
  submitted, including the operator's signer address and current pool
  state.
- **Behavior:** simulation `pass=false` (revert, slippage exceeded,
  insufficient liquidity, oracle deviation) → abort with
  `status="sim_failed"`. Metric
  `arbx_simulation_total{status="fail",reason=…}`.
- **Why fourth:** simulation is the most expensive on-ramp gate
  (RPC round-trip). Cheaper gates above must filter first.

## Gate 5 — RPC health

- **Source of truth:** `arbx_rpc_provider_state` per chain. At least one
  provider must be `Healthy` (state=0) or `Degraded` (state=1); pure
  `Open` (state=2) means broadcast will fail.
- **Behavior:** if no alive provider for the target chain, abort with
  `status="rpc_unavailable"`. Doctrine `arbx-rpc-failover-discipline`
  requires ≥3 providers configured (see `G-RPC-1`).
- **Why fifth:** simulation can hit a different RPC than execution, so
  this gate runs after sim and before signing.

## Gate 6 — Risk limits (`risk_limits`)

- **Source of truth:** `cfg.risk` (per-chain) + Redis KPIs. Includes:
  - `max_position_size_usd` per attempt.
  - `max_concurrent_attempts` per chain (semaphore).
  - `max_drawdown_pct_24h` (auto-killswitch trigger).
  - `cfg.risk.gas_cost_safety_multiplier` (e.g., reject if estimated
    gas > 3× recent median).
- **Behavior:** any limit exceeded → abort with
  `status="risk_limit_exceeded"`. If the drawdown trigger is hit, the
  killswitch arms automatically (Gate 1 will catch the next attempt).
- **Why sixth:** risk evaluation needs the simulated gas + simulated
  output to be sensible, so it follows simulation.

## Gate 7 — Token safety

- **Source of truth:** `selector-api/src/token_safety/` — honeypot
  simulation + sell-tax detection + transfer-tax + liquidity-lock check.
  Doctrine `arbx-token-safety-screen` (see `G-TOK-1`).
- **Behavior:** any token in the route flagged as honeypot, sell-tax
  > 5%, or unlocked liquidity → abort with `status="token_unsafe"`.
- **Why last:** the screen can be slow (RPC simulation per token) and
  is best run only on opportunities that have already passed the cheaper
  gates. Token verdicts are cached per token+chain in
  `token_safety_cache` for 24h.

---

## On a passing run

If all 7 gates clear:

1. The bundle is constructed (`backend/relays-client/src/bundle_builder.rs`).
2. Signed with the configured signer (paper-mode → in-memory only;
   live-mode → submitted to the chosen private relay).
3. `arbx_execution_total{status="submitted"}` increments. The relay
   response (`included` / `dropped`) updates the sibling metric
   `arbx_bundle_included_total{relay,chain_id}`.

In paper-mode (the default), the platform stops at "would have signed".
Nothing reaches the public mempool. Audit row written to `audit_log`
with full per-gate evidence.

---

## On a failing run

Each gate emits a structured rejection with `status` set to the
appropriate label above. The `/recon` page rolls up rejections by reason
and chain, and the `RejectionRateAnomaly` alert fires when any single
reason exceeds a configured rolling threshold.

A gate failure is **not** a bug — it's the system working. The
rejection rate is a deliberate KPI; aim is *high*, not low. A 0%
rejection rate would mean the gates aren't catching anything, which
contradicts every paper-shadow we've ever run.

---

## Why this list and not 8 or 6

Each of the 7 gates corresponds to a category of catastrophic loss the
platform has historically observed in the wild:

| # | Failure mode prevented | Worst observed loss (industry) |
|---|------------------------|--------------------------------|
| 1 | Operating during an active incident | Full bankroll on one bad block |
| 2 | Honeypot / unsellable token | Single-trade total loss |
| 3 | Negative-EV bundle wins the lottery | Slow drain over thousands of trades |
| 4 | Bundle reverts on-chain | Gas wasted, possibly inclusion-fee paid |
| 5 | Submitting via dead RPC | Tx never lands; competitor wins |
| 6 | Position size > tolerable drawdown | Single tail event = stop-loss blown |
| 7 | Tax / lock / honeypot deeper than test | Held position, cannot exit |

Removing any gate widens the loss surface in a category that has
documented historical incidents. Adding more gates beyond these seven
has not produced measurable additional safety in the platforms that have
tried (per ZeroMEV / EigenPhi post-mortems through 2026 Q1).

---

## References

- Skill: `.agents/skills/arbx-pre-execute-checklist/`
- Companion: `docs/policies/mev-ethics.md` (`G-MEV-1`)
- Code: `backend/relays-client/src/submit_engine.rs`,
  `backend/relays-client/src/main.rs::execute_handler`
- Runbooks: `docs/runbooks/relay-degraded.md`,
  `docs/runbooks/killswitch-activated.md`
