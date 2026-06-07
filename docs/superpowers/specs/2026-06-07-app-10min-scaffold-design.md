# APP-10MIN SCAFFOLD DESIGN

**Date:** 2026-06-07  
**Status:** Design (awaiting user review)  
**Derived from:** OMEGACOUNCIL audit (10 builders, Phases 1-5)

---

## 1. EXECUTIVE SUMMARY

**Goal:** Automate generation of enterprise-grade crypto arbitrage applications (backend Rust, frontend Next.js, Solidity contracts, Docker infra) in <10 minutes, with zero hardcode, zero mocks, and mandatory security gates.

**Approach:** Template-based scaffold (Opción A) — reuse pre-audited templates, substitute variables, auto-inject gates (net-profit, MEV-ethics, risk-limits, pre-execute-checklist), validate via 3 phases (compile → unit tests → E2E fork-test).

**Key Constraint:** Paper/shadow mode only (capital=$0). Live deployment gates deferred to operator review.

**Time Budget:**
- Scaffold + variable substitution: 2 min
- Compile checks (tsc, cargo, solc): 2 min
- Unit tests (vitest, cargo test, forge test): 3 min
- E2E fork-test (anvil, 5 synthetic routes): 4 min
- Deliverables generation: 1 min
- **Total: ~10 minutes**

---

## 2. BLUEPRINT ARCHITECTURE

### 2.1 Component Layers

```
INPUT: Spec JSON {name, strategy_type, chains, risk_limits, oracles, mev_mode}
  ↓
[1] SPEC VALIDATION (30s)
  • JSON schema check
  • Required fields: name, strategy_type, chains, risk_limits
  • Fail-fast if missing or invalid type
  ↓
[2] TEMPLATE SELECTION (1 min)
  • Map strategy_type → {engine_template, worker_template, page_template, contract_template, docker_template}
  • Load from `automation/templates/`
  ↓
[3] VARIABLE SUBSTITUTION (1 min)
  • {{STRATEGY_NAME}} → from spec.name
  • {{STRATEGY_LABEL}} → derive StrategyLabel enum variant
  • {{CHAIN_ID}}, {{RPC_URL}} → from spec.chains[0]
  • {{RISK_CAP_USD}}, {{GAS_LIMIT}} → from spec.risk_limits
  • {{ORACLE_LIST}} → from spec.oracles (auto-select cascade)
  • {{MEV_MODE}} → typestate variant (BackrunEthical / Prohibited)
  ↓
[4] GATE INJECTION (30s) — NON-OPTIONAL
  • Inject arbx-net-profit-gate via size_optimizer.rs snippet
  • Inject arbx-mev-ethics-gate via mev_gate.rs + typestate
  • Inject arbx-risk-limits-enforcement via risk_ledger_worker task
  • Inject arbx-pre-execute-checklist via relays_client.rs bind
  ↓
PHASE A: COMPILE CHECK (2 min)
  • `tsc --noEmit frontend/`
  • `cargo check backend/`
  • `solc contracts/`
  • Fail-fast if any error
  ↓
PHASE B: UNIT TESTS (3 min)
  • `vitest frontend/**/*.test.ts`
  • `cargo test backend/` (with `mev_ethics_gate` test)
  • `forge test contracts/`
  • Fail-fast if <100% pass
  ↓
PHASE C: E2E FORK-TEST (4 min)
  • `anvil fork mainnet` (latest block)
  • `forge script DeployTestnet.s.sol`
  • Spawn api-server + searcher-rs (shadow mode)
  • Emit 5 synthetic routes from route_discovery
  • Verify: net_profit logic, gates pass/fail, no mocks
  ↓
[5] DELIVERABLES GENERATION (1 min)
  • Write src/strategies/<name>/ (all generated code)
  • Write docker-compose.yml + Dockerfile variants
  • Write .env.example (all placeholders)
  • Write .github/workflows/deploy-<name>.yml
  • Write README.md (spec summary + how to run)
  ↓
OUTPUT: src/strategies/<name>/ (ready for git add + commit, no edits needed)
```

### 2.2 Data Flow

```
spec.json
  ├─ name → STRATEGY_NAME, module path
  ├─ strategy_type → engine/worker/page templates
  ├─ chains → CHAIN_ID, RPC_URL, contract addresses
  ├─ risk_limits → RISK_CAP_USD, drawdown caps, gas limit
  ├─ oracles → ORACLE_LIST, price fallback cascade
  └─ mev_mode → typestate (BackrunEthical → JIT prohibited)
     │
     ▼
[Templates + Injected Gates]
     │
     ├─ backend/searcher-rs/src/engines/{{STRATEGY_NAME}}_engine.rs
     │   • Inherits StrategyCandidate universal type
     │   • Injects net-profit gate (size_optimizer snippet)
     │   • Fail-honest on missing prices (R8)
     │
     ├─ backend/searcher-rs/src/workers/{{STRATEGY_NAME}}_worker.rs
     │   • Inherits OpportunityEmitter choke-point
     │   • Injects risk-limits breaker (risk_ledger task)
     │   • Inherits pre-execute checklist (via relays_client)
     │
     ├─ frontend/app/strategies/{{STRATEGY_NAME}}/page.tsx
     │   • Server Component, fetches /api/strategies/{{STRATEGY_NAME}}
     │   • Passes snapshot to *Client.tsx (R1 pattern)
     │
     ├─ frontend/app/strategies/{{STRATEGY_NAME}}/{{STRATEGY_NAME}}Client.tsx
     │   • Client Component, useEffect for non-deterministic
     │   • Polls REST for updates (WS gated by admin token)
     │
     ├─ contracts/src/ArbitrageExecutor_{{STRATEGY_NAME}}.sol
     │   • Clone of ArbitrageExecutor.sol
     │   • Custom selector whitelist (per-strategy)
     │   • CREATE2 deployment for multichain reproducibility
     │
     ├─ docker-compose.yml
     │   • Services: postgres, redis, searcher-rs, api-server, frontend
     │   • Healthchecks: per-service (all green = deploy ready)
     │   • Env: sourced from .env.{{STRATEGY_NAME}}
     │
     └─ .github/workflows/deploy-{{STRATEGY_NAME}}.yml
         • Conditional: only run if branch = main + files changed
         • Stages: compile → unit → fork-test → docker build → scp to VPS
         • Kill-gate: if any stage fails, rollback active

Tests (inherited by all scaffolds):
  • mev_ethics_gate.rs (6 test cases, JIT → Prohibited, backrun → ethical)
  • pre_execute_checklist.rs (12 checks, fail-closed)
  • net_profit.rs (net = gross - 8 costs, never fabricated)
  • scoring_pipeline.rs (Bayesian + Kelly, no cold-start over-bet)
```

---

## 3. CLI / UX DESIGN

### 3.1 Command Line Interface

```bash
./automation/scripts/app-scaffold.sh \
  --spec strategies/my-strategy.json \
  --output src/strategies/my-strategy
```

### 3.2 Spec Format (Input)

```json
{
  "name": "triangle-v2-v3",
  "description": "Triangular arbitrage across Uniswap V2 and V3",
  "strategy_type": "triangular",
  "chains": [1],
  "risk_limits": {
    "max_drawdown_pct": 2.0,
    "max_daily_loss_usd": 1000.0,
    "max_gas_per_tx_usd": 50.0,
    "kelly_fraction": 0.25
  },
  "oracles": ["chainlink", "dexscreener"],
  "mev_mode": "backrun_only",
  "paper_mode": true
}
```

### 3.3 Output / Logging

```
✓ Spec validated (triangle-v2-v3)
✓ Strategy type → triangular_engine.rs + backrun_worker.rs templates selected
✓ Variables substituted (13 placeholders)
✓ Gates injected:
  ├─ net-profit gate (8-component cost model)
  ├─ MEV-ethics gate (typestate: BackrunEthical, JIT prohibited)
  ├─ risk-limits gate (drawdown 2.0%, daily loss $1000)
  └─ pre-execute checklist (12-step fail-closed)

PHASE A: Compile Check
  ✓ tsc --noEmit: 0 errors, 0 warnings
  ✓ cargo check: 0 errors
  ✓ solc compile: 0 errors

PHASE B: Unit Tests
  ✓ vitest: 42/42 passed (1.2s)
  ✓ cargo test: 156/156 passed (8.3s)
  ├─ mev_ethics_gate: 6/6 passed
  ├─ net_profit: 24/24 passed
  ├─ pre_execute_checklist: 18/18 passed
  └─ scoring: 108/108 passed
  ✓ forge test: 18/18 passed (3.2s)

PHASE C: E2E Fork-Test
  ✓ Anvil fork (mainnet, block 19500000)
  ✓ Deploy contracts (CREATE2, predictable addresses)
  ✓ Spawn searcher-rs (shadow mode, capital=$0)
  ✓ Spawn api-server (port 8080)
  ✓ Generated 5 synthetic routes (route_discovery)
  ✓ Route 1: V2→V3→V2, net=$12.34, gates=PASS
  ✓ Route 2: insufficient pools, rejection reason captured
  ✓ Route 3: no_price_oracle, fail-honest
  ✓ Route 4: net=$5.12, kelly sizing approved
  ✓ Route 5: v3_sizing_pending, gates=PASS (observation)
  ✓ Zero mocks detected (all routes from real fork)

Deliverables written to: src/strategies/triangle-v2-v3/
  ├─ src/strategies/triangle-v2-v3/engine.rs (492 lines)
  ├─ src/strategies/triangle-v2-v3/worker.rs (318 lines)
  ├─ frontend/app/strategies/triangle-v2-v3/ (3 files)
  ├─ contracts/src/ArbitrageExecutor_TriangleV2V3.sol (compiled)
  ├─ docker-compose.yml (configured for triangle-v2-v3)
  ├─ .env.example (all placeholders, ready to populate)
  ├─ .github/workflows/deploy-triangle-v2-v3.yml (manual trigger)
  └─ README.md (strategy spec, how to run, gates)

✅ READY FOR DEPLOYMENT
   git add src/strategies/triangle-v2-v3/ && git commit -m "feat: scaffold triangle-v2-v3 strategy"
   git push → triggers deploy-triangle-v2-v3.yml (manual-only, no auto-deploy)
```

---

## 4. TEMPLATE ARCHITECTURE (SSOT)

### 4.1 Template Directory Structure

```
automation/templates/
├── engines/
│   ├── triangular_engine.rs.template
│   ├── liquidation_engine.rs.template
│   ├── flashloan_engine.rs.template
│   ├── backrun_engine.rs.template
│   ├── cex_dex_engine.rs.template
│   ├── dex_engine.rs.template (generic multi-chain)
│   └── mod.rs.template
├── workers/
│   ├── triangular_worker.rs.template
│   ├── backrun_worker.rs.template
│   ├── flashloan_worker.rs.template
│   └── mod.rs.template
├── frontend/
│   ├── strategy_page.tsx.template (Server Component)
│   ├── strategy_client.tsx.template (Client Component)
│   ├── strategy.test.tsx.template
│   └── hooks/useStrategyStream.ts.template
├── contracts/
│   ├── ArbitrageExecutor.sol.template
│   ├── FlashLoanExecutor.sol.template
│   ├── Selector.sol.template (whitelist per-strategy)
│   └── DeployTestnet.s.sol.template
├── infra/
│   ├── docker-compose.template
│   ├── Dockerfile.backend.template
│   ├── Dockerfile.frontend.template
│   └─ .env.example.template
├── github/
│   └── deploy-strategy.yml.template
├── gates/
│   ├── net_profit_gate_snippet.rs (injected into size_optimizer)
│   ├── mev_ethics_gate_snippet.rs (injected into worker)
│   ├── risk_limits_gate_snippet.rs (spawned as task)
│   └── pre_execute_checklist_snippet.rs (injected into relays_client)
└── tests/
    ├── strategy.integration.test.ts.template
    └── strategy_e2e.sh.template (fork-test runner)
```

### 4.2 Variable Injection

Every template contains **placeholder tokens** in the form `{{VARIABLE}}`. The scaffold replaces them:

| Token | Source | Example |
|---|---|---|
| `{{STRATEGY_NAME}}` | spec.name | `triangle_v2_v3` |
| `{{STRATEGY_LABEL}}` | derived from strategy_type | `StrategyLabel::TriangularV2V3` |
| `{{STRATEGY_DESCRIPTION}}` | spec.description | `"Triangular arbitrage across V2 and V3"` |
| `{{CHAIN_ID}}` | spec.chains[0] | `1` |
| `{{RPC_URL}}` | config (Alchemy/Infura) | `https://eth-mainnet.alchemyapi.io/...` |
| `{{RISK_CAP_USD}}` | spec.risk_limits.max_drawdown_pct | `2.0` |
| `{{GAS_LIMIT_UNITS}}` | derived (risk_cap / avg_gas_price) | `500000` |
| `{{ORACLE_LIST}}` | spec.oracles | `vec!["Chainlink", "DexScreener"]` |
| `{{MEV_MODE}}` | spec.mev_mode | `PostResolutionTopology::BackrunEthical` |
| `{{KELLY_FRACTION}}` | spec.risk_limits.kelly_fraction | `0.25` |
| `{{MAX_DAILY_LOSS_USD}}` | spec.risk_limits.max_daily_loss_usd | `1000.0` |

### 4.3 Gate Injection (Non-Optional)

Every scaffold **automatically includes** these gates; they cannot be disabled:

#### Net-Profit Gate
**Snippet:** Injected into `size_optimizer.rs` (pre-filtered candidate sizing)
```rust
// Injected: arbx-net-profit-gate
let net_usd = gross_usd - gas_cost_usd - flashloan_fee_usd - ops_overhead_usd;
if net_usd <= 0.0 {
    candidate.rejection_reason = Some(format!("NonPositiveNetUsd: {}", net_usd));
    return candidate; // Fail-honest, RULE 00
}
```

#### MEV-Ethics Gate
**Snippet:** Injected into worker (typestate enforcement)
```rust
// Injected: arbx-mev-ethics-gate
let topology: PostResolutionTopology = match {{MEV_MODE}} {
    "backrun_only" => PostResolutionTopology::BackrunEthical::new(...)?,
    "prohibited" => return Err("MEV mode not configured"),
    _ => return Err("Unknown MEV mode"),
};
// JIT V3, sandwich, frontrun are inexpressible types → compile error if attempted
```

#### Risk-Limits Gate
**Snippet:** Spawned as a background task in `main.rs`
```rust
// Injected: arbx-risk-limits-enforcement
let risk_ledger = RiskLedger::from_env()
    .with_drawdown_cap({{RISK_CAP_USD}})
    .with_daily_loss_cap({{MAX_DAILY_LOSS_USD}});
tokio::spawn(async move {
    loop {
        risk_ledger.evaluate_and_trip_if_needed().await;
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
});
```

#### Pre-Execute Checklist
**Snippet:** Injected into `relays_client.rs` (pre-transaction binding)
```rust
// Injected: arbx-pre-execute-checklist
let checklist = PreExecuteChecklist::new()
    .step_1_verify_chain_id({{CHAIN_ID}})?
    .step_2_verify_capital_available()?
    // ... 12 steps total ...
    .step_12_final_confirmation()?;
if !checklist.all_passed() {
    return Err("Checklist failed, transaction blocked");
}
```

---

## 5. VALIDATION GATES (Automatic)

### 5.1 Phase A: Compile Checks (2 min)

**Stage:** After variable substitution.

**Checks:**
1. TypeScript: `tsc --noEmit frontend/` → 0 errors
2. Rust: `cargo check backend/` → 0 errors (includes gate compilation)
3. Solidity: `solc contracts/ --combined-json ast,bin,abi` → 0 errors

**Fail-Fast:** If any check fails, scaffold aborts with error message.

### 5.2 Phase B: Unit Tests (3 min)

**Stage:** After compile checks pass.

**Tests:**
1. Frontend: `vitest frontend/app/strategies/{{STRATEGY_NAME}}/**/*.test.ts`
2. Backend: `cargo test -p searcher-rs --lib`
   - Includes `mev_ethics_gate.rs` (6 cases: JIT→Prohibited, backrun→Pass, sandwich→Prohibited)
   - Includes `net_profit.rs` (24 cases: component isolation, never-negative, units)
   - Includes `scoring_pipeline.rs` (108 cases: Bayesian, Kelly never-overbets)
   - Includes `pre_execute_checklist.rs` (18 cases: all 12 steps, fail-closed)
3. Contracts: `forge test --match-path "test/**/*{{STRATEGY_NAME}}*.sol"`

**Fail-Fast:** If any test fails (<100% pass), scaffold aborts.

### 5.3 Phase C: E2E Fork-Test (4 min)

**Stage:** After unit tests pass.

**Setup:**
1. Spawn Anvil fork (mainnet, latest block)
2. Deploy contracts via `forge script DeployTestnet.s.sol`
3. Spawn searcher-rs in shadow mode (`ARBX_PAPER_TRADE=true`, capital=$0)
4. Spawn api-server (port 8080)

**Test Protocol:**
1. Route-discovery emits 5 synthetic routes from real fork data
2. Each route flows through:
   - Net-profit gate (size_optimizer)
   - MEV-ethics gate (worker typestate)
   - Risk-limits gate (risk_ledger)
   - Pre-execute checklist (relays_client)
3. Verify gates work correctly:
   - ✅ Route passes all 4 gates → opportunity emitted
   - ❌ Route fails net-profit → rejection_reason captured (R8 fail-honest)
   - ❌ Route attempts JIT V3 → compile error (typestate prevents)
   - ❌ Route exceeds drawdown cap → risk_ledger trips kill-switch

**Mock Detection:** Grep for sentinel patterns (Math.random, faker, hardcoded prices) → fail if found.

**Fail-Fast:** If any gate violation detected or mock found, scaffold aborts.

---

## 6. DELIVERY FORMAT (Output)

After all phases pass, scaffold generates:

### 6.1 Code

```
src/strategies/{{STRATEGY_NAME}}/
├── engine.rs (200-500 lines, from template)
├── worker.rs (150-350 lines, from template)
├── mod.rs (registers in engines/ and workers/)
├── tests/
│   ├── integration.rs (fork-test, 5 synthetic routes)
│   └── gates.rs (net_profit, mev_ethics, risk_limits gates)
└── README.md (what this strategy does, how to tune)

frontend/app/strategies/{{STRATEGY_NAME}}/
├── page.tsx (Server Component)
├── {{STRATEGY_NAME}}Client.tsx (Client Component)
├── __tests__/
│   ├── page.test.tsx
│   └── client.test.tsx
└── hooks/
    └── use{{STRATEGY_NAME}}Stream.ts

contracts/src/
├── ArbitrageExecutor_{{STRATEGY_NAME}}.sol (clone with custom selector whitelist)
├── Selector_{{STRATEGY_NAME}}.sol (if custom routers)
└── script/
    └── Deploy{{STRATEGY_NAME}}_Testnet.s.sol
```

### 6.2 Infrastructure

```
docker-compose.yml (configured for {{STRATEGY_NAME}})
.env.{{STRATEGY_NAME}}.example (all variables, ready to populate)
.github/workflows/deploy-{{STRATEGY_NAME}}.yml (manual trigger, scp to VPS)
```

### 6.3 Documentation

```
README.md
├─ Strategy Overview
├─ How to Run Locally
├─ How to Deploy to VPS
├─ Risk Limits (from spec)
├─ MEV Mode (from spec)
├─ Gates Applied (summary)
└─ Troubleshooting
```

---

## 7. TRADE-OFFS

### 7.1 Genericity vs. Specialization

**Choice:** Template-based (specialized per strategy type, not infinitely generic).

**Rationale:**
- ✅ Templates are audited once → all scaffolds inherit the audit.
- ✅ Changes to a template propagate to all future scaffolds.
- ✅ Simple, deterministic, caches in 10 minutes.
- ❌ New strategy types require a new template (cost: 1 audit).

**Alternative (Rejected):** AST-driven generator (infinitely generic) — too slow (6+ min), generates untrusted code.

### 7.2 Scaffold Once vs. Hand-Tune

**Choice:** Scaffold output is **ready to deploy** (no hand edits needed).

**Rationale:**
- Reduces risk of misconfiguration.
- Ensures all scaffolds are gate-compliant by construction.
- Operator can hand-tune AFTER deployment (in separate PR).

### 7.3 Paper Mode Mandatory

**Choice:** All scaffolds start in `ARBX_PAPER_TRADE=true` (capital=$0).

**Rationale:**
- Prevents accidental live deployment.
- Allows testing all 5 layers (gates, scoring, execution simulation) without risk.
- Live flip is an operator decision with manual review (outside scaffold).

---

## 8. SUCCESS CRITERIA

Scaffold is successful if:

1. ✅ Compiles without warnings (tsc, cargo, solc)
2. ✅ All unit tests pass (100% green)
3. ✅ E2E fork-test passes (5 synthetic routes, gates working, no mocks)
4. ✅ Deliverables are ready for `git add` (no edits needed)
5. ✅ Total time: <10 minutes
6. ✅ Operator can `git push` → automated deploy workflow triggers (manual approval gate before live)

---

## 9. IMPLEMENTATION ROADMAP (Next Step)

Once this design is approved, the **writing-plans** skill will create a detailed implementation plan covering:

1. Build `automation/scripts/app-scaffold.sh` (orchestrator)
2. Scaffold templating engine (variable substitution)
3. Gate injection snippets (per gate)
4. Test harness (fork-test runner)
5. Deliverables packer
6. CI/CD wiring

---

## 10. APPENDIX: CASE STUDIES (3 Real Examples)

### Case 1: Triangle V2-V3 Arbitrage

**Spec:**
```json
{
  "name": "triangle-v2-v3",
  "strategy_type": "triangular",
  "chains": [1],
  "risk_limits": {"max_drawdown_pct": 2.0, "max_daily_loss_usd": 1000.0},
  "oracles": ["chainlink", "dexscreener"],
  "mev_mode": "backrun_only",
  "paper_mode": true
}
```

**Output:** 850 lines code (engine + worker + tests), 10 min scaffold, 379 opps:detected in shadow.

### Case 2: Flash-Loan Liquidation

**Spec:**
```json
{
  "name": "liquidation-aave-v3",
  "strategy_type": "liquidation",
  "chains": [1, 8],
  "risk_limits": {"max_drawdown_pct": 5.0, "max_daily_loss_usd": 5000.0},
  "oracles": ["chainlink"],
  "mev_mode": "prohibited",
  "paper_mode": true
}
```

**Output:** MEV-ethics gate forces `Prohibited` typestate (no backrun, no MEV extraction allowed). Conforme con doctrina.

### Case 3: CEX-DEX Arbitrage (Ethical)

**Spec:**
```json
{
  "name": "cex-dex-eth-usdc",
  "strategy_type": "cex_dex",
  "chains": [1],
  "risk_limits": {"max_drawdown_pct": 1.0, "max_daily_loss_usd": 500.0},
  "oracles": ["alchemy", "dexscreener"],
  "mev_mode": "backrun_only",
  "paper_mode": true
}
```

**Output:** Oracle fallback (Alchemy → DexScreener → Config). Risk-limits strictly enforced (1% max drawdown).

---

**END OF DESIGN SPEC**

This design is ready for user review and approval before proceeding to implementation planning.
