---
name: arbitragex-v2-mainnet-live
description: "Roadmap operativo hacia mainnet-live con gates G1-G8 verificables (integridad de datos V3, simulación cíclica 2-5 hops, paper 4-capas PnL, submit engine durable, fork replay 10, Sepolia live, A.9 reforzado, canary mainnet ≤$350). Ejecución LIVE queda subordinada a §32/§33/§34.3 de CLAUDE.md."
---

> ⚖️ **REGISTRO DE GOBERNANZA** *(añadido al guardar por OMEGA, 2026-09-15 — NO destructivo: el documento del operador va íntegro a continuación, sin ediciones)*
>
> 1. Este documento queda registrado como **roadmap operativo hacia mainnet-live**. Sus gates G1-G8 y sus 10 prohibiciones son vinculantes como *checkpoints con evidencia*.
> 2. **Nada de este skill autoriza ejecución LIVE por sí mismo.** El flip a `LIVE_MAINNET` con capital real queda subordinado a §32/§33/§34.3 de `CLAUDE.md`: promoción explícita del modo permanente audit/scaffold/shadow/read-only + skills `arbx-*` PASS + **autorización operativa explícita fuera de chat** (no inferida de flags ni de chat — precedente 2026-09-06). El propio documento lo exige (GATE-7/A.9 con 2 firmas físicas; prohibición #2).
> 3. Los claims fácticos del documento (HEAD `b99c834`, Issue #567 closed/merged, `tx_builder.rs:51-79`, `ethPriceUsd = 3500` hardcode, SSH exit 255) estaban **sin verificar** al momento del registro; verificación en curso (workflow `verify-pipeline-567`).
> 4. Verificado al guardar: los archivos que FIX-1..FIX-6 ordenan modificar existen (`submit_engine.rs`, `paper/executor.ts`, `DeploySepolia.s.sol`, `DeployMainnet.s.sol`, 4 workers, `tx_builder.rs`, `sim_engine.rs`); `canonical_plan_consumer.rs` NO existe (FIX-2 ordena crearlo); los scripts de COMMAND REFERENCE (`00-preflight-checks/`, `01-deploy-contracts/`, `02-execute-canary/`) NO existen aún.

# SKILL: ARBITRAGEX-V2-MAINNET-LIVE
# VERSION: 2.0.0
# OBJECTIVE: Generar ganancias reales en USD mediante arbitraje DEX automatizado
# SAFETY: ZERO-TOLERANCE para pérdida de capital por defecto de software
# MODE: IMPERATIVE - All instructions are mandatory, not suggestive

## CONTEXT

Repository: hefarica/arbitragex-v2
Current HEAD: b99c834 (post-#569, #572 merged)
Status: PIPELINE BROKEN - Critical gaps identified, DO NOT PROCEED to next phase without evidence

## ABSOLUTE GATES (Non-negotiable checkpoints)

GATE-1: DATA-INTEGRITY-V3
- Evidence: Manifest V3 with 318 pools verified on-chain, 90 candidates identified
- Requirement: 2 human signatures on manifest hash + PostgreSQL backup tested + restore verified
- Blocker: SSH access to VPS (exit 255) must be resolved or alternative backup path established
- Output: SQL UPDATE plan generated, signed, ready for execution

GATE-2: SIMULATION-CYCLIC-C2C3
- Evidence: Issue #567 closed with PR merged (codex/567-canonical-plan-simulation)
- Requirement: S4 consumer simulates FULL cycles (2-5 hops), not single-swap probes
- Validation: 10 fork-replay cases with <1% PnL deviation vs on-chain reality
- Blocker: Current tx_builder.rs:51-79 rejects cyclic routes with CyclicRouteNotRepresentable

GATE-3: PAPER-4-LAYERS
- Evidence: 4-layer PnL implemented (predicted → simulated → observed_market → realized)
- Requirement: USD calculations use real decimals, dynamic ETH price (not 3500 fixed)
- Validation: predicted ≠ observed for all trades; realized only for LIVE

GATE-4: SUBMIT-ENGINE-DURABLE
- Evidence: State machine 7-phase operational (CREATED→VALIDATED→AUTHORIZED→SUBMITTING→SUBMITTED→INCLUDED/REVERTED/EXPIRED→RECONCILED)
- Requirement: Idempotency_key deterministic, UNIQUE INDEX PostgreSQL, no 60s window dedup
- Blocker: Duplicate execution prevention must be server-side, not UI-only

GATE-5: FORK-REPLAY-10
- Evidence: 10 historical mainnet blocks replayed with full simulation
- Requirement: <1% deviation in output amounts, gas costs, and net PnL
- Validation: Matched against actual on-chain results

GATE-6: SEPOLIA-LIVE
- Evidence: ArbitrageExecutor.sol deployed, initialized, UUPS proxy verified
- Requirement: Complete operation (detect→quote→simulate→decide→execute→reconcile) with real gas
- Validation: Circuit breaker tested, emergency pause functional

GATE-7: A.9-REINFORCED
- Evidence: Checklist signed by 2 operators (Héctor + Ext-1)
- Requirement: Max notional, max loss, min profit, slippage, gas ceiling, bribe ceiling, expiry blocks, RPC quorum, stale quote protection, reorg protection ALL configured and tested

GATE-8: MAINNET-CANARY
- Evidence: First trade with max_loss parameterized, not seeking "profit" but "correct measurement"
- Requirement: Capital at risk ≤ $350, flash principal 5 WETH, gas reserve separate
- Validation: Reconciliation shows expected vs actual with <1% deviation

## PIPELINE ARCHITECTURE (Must implement exactly)

detect (route_scanner_worker.rs)
  ↓
cartridge_boot.rs [PRESERVE: observed_block_number, dex_hint, RouteMetadata]
  ↓
quote (NSGA-II op_32 + batch_quote) [VALIDATE: V3 fee() from on-chain, not cache]
  ↓
simulate (sim-ctl) [CRITICAL: Must use canonical RoutePlan, recover from Redis arbx:validated_plan:<id>]
  ↓
  └→ If cyclic: Use full EVM simulation (revm/anvil), NOT tx_builder.rs single-swap probe
  
decide (live_risk_ranker)
  ↓
  └→ Requires: evidence_vector (flat_prior if uncalibrated) + simulation_result
  
execute (relays-client submit_engine)
  ↓
  ├→ SHADOW: Simulate only, no broadcast
  ├→ PAPER: Log to PostgreSQL paper_trades, no broadcast, 4-layer PnL
  └→ LIVE: Requires A.9 sign-off, quorum-2, idempotency_key, state machine
  
conciliate (drift_tracker)
  ↓
  └→ ARBX_DRIFT_TRACKER_MODE=on, ARBX_STAGE2_CALIBRATION_MODE=on
  
display (OpportunityTradeCard.tsx)
  ↓
  └→ Show: expected_profit, actual_profit (reconciled), tx_hash, status

## CRITICAL FIXES REQUIRED (In order)

### FIX-1: Gap 1 - Route Metadata Preservation
Files to modify:
- backend/searcher-rs/src/workers/triangular_worker.rs
- backend/searcher-rs/src/workers/flashloan_arb_worker.rs
- backend/searcher-rs/src/workers/liquidation_worker.rs
- backend/searcher-rs/src/workers/cex_dex_worker.rs

Action: Replace insert_opportunity() with insert_opportunity_with_route()
Required fields in RouteMetadata:
- pool_addresses: Vec<Address>
- token_addresses: Vec<Address>
- dex_adapters: Vec<String>
- decimals: Vec<u8>
- hop_count: usize
- route_hash: H256
- router_addresses: Vec<Address>
- fees_bps: Vec<u32>

### FIX-2: Gap 2 / Issue #567 - Cyclic Simulation
Files to modify:
- backend/sim-ctl/src/tx_builder.rs (currently rejects cycles)
- backend/sim-ctl/src/sim_engine.rs

Action: 
1. DO NOT modify tx_builder.rs to accept cycles (it's a single-swap probe by design)
2. CREATE new simulation path: canonical_plan_consumer.rs
3. Recover RoutePlan from Redis keys: arbx:validated_plan:<id>, arbx:validated_economics:<id>
4. Simulate full cycle with revm: 2-5 hops, token[0] == token[N]
5. Calculate PnL: final_amount - initial_amount - gas_cost - flash_fee - protocol_fees

Validation: 0 occurrences of strategy_cyclic_route_not_simulatable_in_s4 in logs

### FIX-3: Paper Mode 4-Layer PnL
File: backend/api-server/src/paper/executor.ts

Replace:
- const ethPriceUsd = 3500; // HARDCODED - FORBIDDEN

With:
- predicted_pnl_wei: from initial simulation
- simulated_pnl_wei: re-simulated at block+1
- observed_market_pnl_wei: calculated with real exit prices post-execution
- realized_pnl_wei: only for LIVE mode (actual on-chain result)

Required context per trade:
- token_decimals: { [symbol]: number }
- gas_used, gas_price_wei, gas_cost_eth, gas_cost_usd
- flash_loan_fee_bps, flash_loan_fee_wei
- protocol_fees_wei
- eth_price_usd, eth_price_source, eth_price_timestamp

### FIX-4: SubmitEngine State Machine
File: backend/relays-client/src/submit_engine.rs

Implement:
enum ExecutionState {
    CREATED, VALIDATED, AUTHORIZED, SUBMITTING, 
    SUBMITTED, INCLUDED, REVERTED, EXPIRED, RECONCILED
}

Idempotency key generation:
keccak256(chain_id + opportunity_id + target_block + route_hash + amount_in + mode)

Database:
CREATE UNIQUE INDEX idx_idempotency ON execution_intents(idempotency_key);

### FIX-5: Environment Configuration
Add to .env:
ARBX_DRIFT_TRACKER_MODE=on
ARBX_STAGE2_CALIBRATION_MODE=on
ARBX_ACCOUNTING_FEEDS_JSON=/path/to/feeds.json
SIM_CALLER_1=<deployed_executor_address>

Fix RPC parsing:
OracleRpc::from_env must handle multiple named entries, not single URL

### FIX-6: Contract Deployment
Execute:
cd contracts
forge script script/DeploySepolia.s.sol --rpc-url $SEPOLIA_RPC --broadcast
forge script script/DeployMainnet.s.sol --rpc-url $MAINNET_RPC --broadcast

Verify:
- UUPS proxy initialized
- Implementation verified on Etherscan
- Routers allowed: UniV2, UniV3, etc.
- Roles configured

## VALIDATION PROCEDURES

### Procedure: Fee Integrity Verification
```bash
# 1. Generate manifest (read-only)
cargo run -p data-integrity --bin verify-v3-fees -- --output manifest.csv

# 2. Human review: 2 signatures on SHA256(manifest.csv)

# 3. Backup PostgreSQL
pg_dump -h $DB_HOST -U $DB_USER $DB_NAME | gzip > backup_pre_fee_fix.sql.gz

# 4. Test restore on local instance
gunzip < backup_pre_fee_fix.sql.gz | psql -h localhost -U test test_db

# 5. Execute updates (90 candidates only)
psql -f fee_updates_90_pools.sql

# 6. Verify: Re-run manifest tool, expect 90 MATCH, 0 MISMATCH
```

### Procedure: Cyclic Simulation Test
```bash
# 1. Start anvil fork
anvil --fork-url $MAINNET_RPC --fork-block-number 20500000

# 2. Run 2-hop cyclic test
cargo test -p sim-ctl --test cyclic_simulation -- --nocapture

# 3. Validate output has:
# - initial_amount, final_amount (token[0] == token[2])
# - intermediate_amounts[hop_1, hop_2]
# - gas_cost, flash_fee, protocol_fees
# - net_profit (calculated, not hardcoded)

# 4. Compare against manual calculation: deviation must be <0.01%
```

### Procedure: Fork Replay Matrix
```bash
# Test 10 historical blocks with known arbitrage opportunities
for block in 20499995 20499996 20499997 20499998 20499999 20500000 20500001 20500002 20500003 20500004; do
    cargo run -p sim-ctl --bin fork_replay -- --block $block --verify-against-etherscan
done

# Expected: 10/10 pass with <1% deviation
```

## PROHIBITED ACTIONS (Will cause immediate abort)

1. NEVER execute UPDATE fee_tier = fee_tier * 100 (mass operation)
2. NEVER enable LIVE mode before A.9 sign-off
3. NEVER use hardcoded USD prices (3500 or any other)
4. NEVER allow predicted_pnl == observed_pnl (must be calculated separately)
5. NEVER proceed with SSH exit 255 unresolved (no blind backups)
6. NEVER merge PR without CI gates passing (including CodeQL, E2E)
7. NEVER use tx_builder.rs for cyclic routes (it's S4 single-swap only)
8. NEVER enable automatic execution without idempotency_key
9. NEVER skip fork replay before Sepolia
10. NEVER use LIVE capital > $350 for canary (first trade limit)

## EVIDENCE REQUIREMENTS

Each gate requires documented evidence:

1. Screenshots/logs of command outputs
2. Signed manifests (GPG or manual signature files)
3. Database dump verification checksums
4. Test result files (JSON output with all PnL layers)
5. Etherscan verification links
6. A.9 checklist with 2 physical signatures

## EMERGENCY PROCEDURES

If capital loss detected:
1. Immediate kill-switch: echo '{"enabled":true,"reason":"emergency_stop"}' > killswitch.json
2. Circuit breaker: cast send $ARBITRAGE_EXECUTOR "pause()" --rpc-url $MAINNET_RPC
3. Notify: 2-operator conference call
4. Preserve state: pg_dump full database, save Redis keys, export logs
5. Root cause analysis before any restart

## SUCCESS CRITERIA

The skill is complete when:

1. First mainnet canary trade executes with:
   - Capital at risk: ≤ $350
   - Flash principal: 5 WETH
   - All 4 PnL layers calculated and logged
   - Reconciliation shows <1% deviation
   - Tx hash visible in OpportunityTradeCard

2. System operates for 7 days with:
   - Zero unauthorized trades
   - Zero duplicate executions
   - All fees calculated correctly (no 100x errors)
   - All cyclic routes simulated fully (no S4 rejection)

3. Monthly profit > gas costs + infrastructure costs

## COMMAND REFERENCE

Generate live mainnet:
./scripts/00-preflight-checks/verify-system-readiness.sh --strict
./scripts/01-deploy-contracts/deploy-mainnet.sh --canary
./scripts/02-execute-canary/run-first-trade.sh --max-loss 350 --verify

Monitor:
curl http://localhost:3000/api/v1/opportunities/live?include_reconciled=true
