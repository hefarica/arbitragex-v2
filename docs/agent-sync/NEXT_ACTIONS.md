# Next Actions — per owner (exact)

## operator (hefarica)
1. Decide KMS/HSM vs isolated-signer (P0-1); provision the signer.
2. Decide P0-2: run the dependency-upgrade sprint (merge #181 wagmi-3, #145 rmcp, #175, #144, #220, #233 as they green) **or** re-justify+extend BOTH allowlists with fresh expiry. Do NOT auto-bump.
3. Set Sepolia secrets (`SEPOLIA_RPC_URL`, `DEPLOYER_PRIVATE_KEY`/KMS, `ETHERSCAN_API_KEY`, `M5_GAS_PRICE_WEI`, `RPC_HTTP_11155111`); create isolated deployer wallet; fund Sepolia ETH.
4. When FASE 5 is reached: approve the `sepolia-deploy` environment (manual gate).
5. Consolidate the coordination-ledger proliferation (#241/#243/#239/#236) → close redundant.

## S4 — Contracts
1. **P0-5:** in `DeployMainnet.s.sol`, grant `UPGRADER_ROLE`→timelock + revoke from deployer inside the atomic block; renounce the timelock bootstrap admin; fix `DEPLOY.md §9`.
2. Take **#229** out of draft → READY (M5 Sepolia pipeline). Confirm it declares `environment: sepolia-deploy`.
3. P2-3: port the atomic handoff into `DeployMultichain`; confirm `max_price_impact_pct` unit.

## S5 — CI/Security
1. **P0-3:** triage the 61 no-hardcode violations (fix real, allow-list the `tests/**/*.spec.ts` false-positives), then flip `lint-no-hardcode.sh:141` `exit 0`→`exit 1`.
2. **P0-4:** build `hardened-vps-rollback.yml` (or wire `deploy.yml`'s working rollback into the canonical path); reconcile the K8s/AWS-fictional DR runbooks.
3. Finish **#226** (VPS IP scrub). P2-2: fix ADR-0005 stale claim + runbook drift.

## S3 — Rust/execution
1. P1-1 gas-price ceiling at send · P1-2 aggregate exposure cap (+ make `max_parallel_executions` enforce) · P1-3 wire capital breakers · P1-5 nonce `refresh()` caller · P2-5 checked `U256` conversion · P2-4 authenticate the ValidatedPlan carrier.

## S2 — Backend
1. P1-4: emit `arbx_realized_pnl_usd` / `arbx_actual_profit_usd` / `arbx_sim_predicted_profit_usd` / `arbx_revert_gas_wasted_usd` / `arbx_paper_mode_active` → revives 5 dead alerts.

## Claude (this session)
- **NO-ACTION-AUTONOMOUS on fixes** — every blocker is owned + (in-flight or session-pending). Standing by.
- On the next tick: revalidate #229 (draft→READY?), operator KMS/secrets, and whether any owner PR closed a blocker; update this ledger. No fund-path/CI/secret work.
