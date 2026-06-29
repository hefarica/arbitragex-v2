# ArbitrageX v2 — The 12 Doctrinal Gates

Each gate fires on a trigger and enforces a constraint. They override default behavior; the operator's
explicit instructions still win, but these protect capital, secrets and the live system. Announce
which gates fire when they apply.

## 1. arbx-mev-ethics-gate
- **Fires when**: any design/code mentions sandwich, frontrun, oracle manipulation, JIT-displacement,
  time-bandit/reorg, or "predatory" extraction.
- **Enforces**: only ethical atomic arbitrage. Predatory MEV is blocked by design. For grey-area
  techniques, consult the canonical sources (Flashbots Collective, ethresear.ch, ethereum.org/mev)
  before approving; if it harms a user, block.

## 2. arbx-net-profit-gate
- **Fires when**: opportunity/strategy code lacks an explicit NET-profit (after gas + fees + slippage)
  calculation.
- **Enforces**: no opportunity is "profitable" until net-profit-after-all-costs is computed and
  positive. `non_positive_profit` and `spot_product_le_one` are HONEST rejections — never force them.

## 3. arbx-simulation-mandatory
- **Fires when**: a code path reaches broadcast without a prior fork simulation.
- **Enforces**: `simulation_required_for_new_routes = true`. Anvil fork sim (sim-ctl) must validate a
  route before any live broadcast. Current sim backend = `anvil` (`SIM_BACKEND=anvil`,
  `simulation.provider="anvil"`); REVM backend is a stub.

## 4. arbx-pre-execute-checklist
- **Fires before**: `cast send`, `forge script --broadcast`, relay submit, a prod DB-schema change, a
  prod rebuild/redeploy, or any container recreate on the live VPS.
- **Enforces**: deliberate, operator-OK'd execution. Verify target network, verify it won't clobber
  live config/WIP, confirm reversibility, prefer non-destructive (failed build keeps old image). A
  prod deploy is "hard to reverse / outward-facing" — confirm unless durably authorized.

## 5. arbx-pre-edit-audit
- **Fires before**: the FIRST edit to executor.sol, hot-path Rust, control-plane TS, files >300 lines,
  or anything on the live VPS.
- **Enforces**: read the file fully + check git status BEFORE editing. On the VPS specifically: check
  for uncommitted drift and UNTRACKED operator WIP before any `git pull/reset` — a naive checkout can
  clobber in-progress work (this gate caught the anvil-config drift + untracked `cartridges.ts`).

## 6. arbx-no-hardcode-doctrine (INMUTABLE)
- **Fires when**: a productive literal is about to appear (RPC URL, 0x-address, 0x+64hex key, capital,
  slippage/gas thresholds), or someone says "hardcode for now / put a default".
- **Enforces**: all productive data lives in `.env`/typed config, never in source. Seeds are read from
  the canonical source (migration/ONBOARDING), never invented. Config FLAGS in `.env` (e.g.
  `active`, `shadow`) are config, not hardcoded source literals — those are allowed.

## 7. arbx-contract-atomicity-rules
- **Fires when**: editing executor / arbitrage / flash-loan contracts.
- **Enforces**: profit-floor revert intact, reentrancy guards intact, approvals scoped, atomic
  all-or-nothing execution. Re-run `forge test` after.

## 8. arbx-flash-loan-discipline
- **Fires when**: touching flash-loan callbacks (`executeOperation`, `onFlashLoan`,
  `uniswapV3FlashCallback`).
- **Enforces**: repay-or-revert, the executor signer holds no funds (capital comes from the loan),
  validate the callback initiator.

## 9. arbx-rpc-failover-discipline
- **Fires when**: editing RPC client code, provider config, or hot eth_call loops.
- **Enforces**: ≥2 providers per chain (doctrine G-RPC-1). Current `RPC_HTTP_1` = 3 public providers
  (publicnode/drpc/etc.), `RPC_WS_1` = 2. Single-vendor emits a warn. Anvil forks via
  `ANVIL_FORK_URL` (public node; a dedicated archive RPC is better but rate-limits on free tiers).

## 10. arbx-risk-limits-enforcement
- **Fires when**: strategy/executor lacks daily-loss cap, kill-switch, or per-token/pool limits.
- **Enforces**: caps present and never lowered without explicit operator approval. `max_value_eth`
  hard cap per bundle; `execution_policy` seed uses `max_value_eth=0.0` (fail-closed) in paper mode.

## 11. arbx-token-safety-screen
- **Fires when**: new token allowlist, pool interaction, or address-list logic.
- **Enforces**: external token-safety (GoPlus / Honeypot.is) before trusting a token for LIVE.
  Currently `token_safety.provider="internal_only"` → tolerable for paper, BLOCKS live until
  `GOPLUS_API_KEY` is provided and provider switched.

## 12. arbx-paper-trade-first
- **Fires when**: proposing to put a strategy live without fork/paper evidence.
- **Enforces**: prove in fork sim + accumulate ≥7 days paper-shadow
  (`ARBX_PAPER_SHADOW_MIN_DAYS=7`, `ARBX_CALIBRATION_MIN_SCORED≥100`) before any live flip.

## Canonical external sources (consult in grey areas)
Flashbots Collective (`collective.flashbots.net`), Latest Research (`/c/research/20`),
`ethresear.ch`, Monad Research (`forum.monad.xyz`), `github.com/flashbots/mev-research`,
`github.com/flashbots/pm`, `ethereum.org/developers/docs/mev`. Cite the source for grey-area decisions.
