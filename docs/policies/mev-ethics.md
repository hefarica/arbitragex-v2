# MEV Ethics Policy (G-MEV-1)

**Doctrine:** `arbx-mev-ethics-gate`
**Gate:** G-MEV-1
**Version:** 1.0.0
**Last Updated:** 2026-05-26

---

## Purpose

This document establishes the ethical boundaries for MEV extraction operations conducted by ArbitrageX v2. All strategies, algorithms, and execution paths MUST comply with these rules.

---

## Core Principles

### 1. No Sandwich Attacks

**Definition:** A sandwich attack occurs when an attacker:
1. Sees a pending user transaction in the mempool
2. Places a transaction before (front-run) and after (back-run) the user's transaction
3. Profits from the price impact caused by the user's transaction

**Policy:** ArbitrageX v2 explicitly prohibits sandwich attacks.

**Implementation:**
- The `dex_arb` strategy only targets existing liquidity pool imbalances
- No code path exists to detect and exploit user transactions
- All opportunities are evaluated against pool state, not mempool users

**Violation Consequence:** Immediate termination of the strategy and audit review.

---

### 2. No Frontrunning

**Definition:** Frontrunning occurs when an attacker:
1. Observes a pending transaction that will move the market
2. Places their own transaction ahead of it to profit from the known price movement

**Policy:** ArbitrageX v2 does not frontrun user transactions.

**Implementation:**
- Opportunities are identified from on-chain state, not pending transactions
- No special logic to detect large swaps before they execute
- All execution is based on observable market conditions

**Exception:** Arbitrage opportunities that result from natural market dynamics (not from exploiting specific users) are permitted.

---

### 3. No Backrunning Against Users

**Definition:** Backrunning against users occurs when an attacker:
1. Sees a user's pending transaction
2. Places a transaction immediately after to profit from the price impact

**Policy:** ArbitrageX v2 does not backrun specific user transactions.

**Implementation:**
- No mempool monitoring for user transactions
- Opportunities are generated from pool state analysis
- Execution timing is based on block boundaries, not user transaction ordering

**Exception:** Generalized backrunning that exploits market inefficiencies (not specific users) is permitted.

---

## Permitted MEV Strategies

The following strategies are explicitly permitted under this policy:

### ✅ DEX Arbitrage
- Exploiting price differences between DEXes for the same asset
- No user transaction exploitation required
- Benefits market efficiency

### ✅ Triangular Arbitrage
- Exploiting pricing inefficiencies across multiple trading pairs
- Pure market-making activity
- Improves price discovery

### ✅ Liquidations
- Liquidating undercollateralized positions on lending protocols
- Protocol-intended behavior
- Maintains protocol solvency

### ✅ JIT Liquidity
- Providing liquidity just-in-time for large swaps
- Legitimate market-making
- Earns fees without harming users

---

## Prohibited Strategies

The following strategies are explicitly prohibited:

### ❌ Sandwich Attacks
- Front-running + back-running user transactions
- Direct harm to users
- Violates core principles

### ❌ Oracle Manipulation
- Exploiting price oracle delays
- Harms protocol users
- Market manipulation

### ❌ Time-Bandit Attacks
- Reorging blocks to steal MEV
- Undermines consensus
- Network attack

---

## Compliance Verification

### Self-Audit Checklist

Before deploying any new strategy, verify:

- [ ] Strategy does not require mempool access for user transactions
- [ ] Strategy does not place orders before/after specific users
- [ ] Strategy profits from market inefficiencies, not user harm
- [ ] Strategy improves market efficiency or provides legitimate service

### Code Review Requirements

All new strategies must pass:
1. Internal code review by at least 2 developers
2. Ethics compliance check against this document
3. Simulation testing to verify behavior

---

## Reporting Violations

If a strategy is found to violate this policy:

1. **Immediate Action:** Disable the strategy via kill-switch
2. **Investigation:** Review all recent executions for harm
3. **Remediation:** Refund affected users if possible
4. **Documentation:** Log incident in audit trail
5. **Prevention:** Update code review process to prevent recurrence

---

## Policy Updates

This policy may be updated as:
- New MEV strategies emerge
- Regulatory guidance evolves
- Community standards develop

All updates require approval from operator-lead and must be documented in the audit trail.

---

**Document maintained by:** OMEGA CORTEX
**Next Review:** 2026-06-26
