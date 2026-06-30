# MEV Ethics Policy â€” ArbitrageX v2

**Doctrine:** `arbx-mev-ethics-gate`
**Owner:** Operator + on-call
**Reviewed:** 2026-05-10
**Enforced via:** `/api/v1/readiness` gate `G-MEV-1`

This document is the **public, signed contract** of which strategies the
ArbitrageX platform will and will not run. It exists so that operators,
auditors, and counterparties have a single referenceable answer when asked
"can the platform do X?". Everything that contradicts this document is
explicitly forbidden â€” even if it would be profitable.

---

## Strategies REFUSED (no exceptions)

ArbitrageX **will not** execute the following mechanics, regardless of profitability:

- **Sandwich attacks.** Wrapping a tx with pre-position + post-position to capture
  induced slippage. Rejected — relays blacklist this pattern and the alpha is
  dominated by clean backrun in 2026 mempool conditions.
- **Pre-position frontrunning of pending swaps.** Submitting a positioning tx
  ordered before an observed pending tx with the intent of profiting from its
  predicted impact. Rejected — mechanic is indistinguishable from sandwich
  half-cycle and triggers same blacklist policy.
- **Victim-specific bundle attribution.** Any bundle whose profit traces to a
  specific user's tx hash rather than to a public market state visible to any
  observer. Rejected by design — backrun engine acts on post-confirmation state.
- **Time-bandit / re-org-driven extraction.** Bidding for re-orgs to capture
  already-settled MEV. Rejected — destabilizes consensus and burns relay trust.
- **Oracle-manipulated liquidations.** Inducing margin calls by manipulating
  oracle inputs or pool reserves. Rejected — outside protocol-posted rules.
- **Retail-targeted information asymmetry.** Pre-running known retail flows
  (ETF rebalance windows, NFT mint sniping by privileged ordering). Rejected —
  the platform's asymmetry edge is cross-venue and anonymous-aggregate, not
  retail-targeted.

## Strategies ALLOWED (Top 1% institutional edge, 2026-05-13 refinement)

- Atomic cross-DEX, triangular, CEX–DEX statistical arbitrage.
- Backrun anticipation: forecast post-impact pool state from mempool, execute
  arbitrage only **after** the trigger tx confirms. The trigger is treated as a
  public market event, not a victim. Bundle position type is hardcoded to
  `BackrunOnly` in the relay submitter.
- Just-in-time (JIT) liquidity: provide concentrated depth in the tick range
  the trigger swap will cross. Captures fee share without altering the
  swapper's effective price. Hyper-aggressive concentration is permitted because
  the counterparty is **passive LPs choosing passive exposure under V3 design
  intent**, not the swapper.
- Cross-venue CEX hedging on anonymous order books, where information edge is
  market-wide, not against a specific identifiable counterparty.

---

## Mempool discipline

All transactions are submitted exclusively to **private mempools and
auction-style relays**. The public mempool is never used for production
submissions:

- Primary: **Flashbots Protect** (private relay → builders).
- Secondary: **MEV Blocker** (batch auction).
- Tertiary: **Titan Builder** (direct builder).
- Tertiary: **bloXroute BDN** (private tx broadcast, when subscribed).

A transaction reaching the public mempool is treated as a **bug, not a
strategy**, and surfaces as a Prometheus alert
(`PublicMempoolLeakDetected`).

The choice of private mempool is a function of latency, builder
landing-probability, and the strategy's tolerance for revert risk —
operators tune the relay weights via `relays-client/src/relay_catalog.rs`.
Sandwich-protective relays (MEV Blocker, Flashbots Protect) are preferred
even at a small latency cost.

---

## Mathematical Enforcement (and why this line)

The doctrine separates **public-state extraction** (allowed: any participant
with the same data could in principle compete; alpha is speed + math + capital)
from **intent-specific extraction** (refused: profit traces to a specific
participant's pending action, requires racing them, exposes the platform to
relay blacklist + regulatory + counterparty risk).

This is not moralism — it is the only configuration that scales to $M of capital
without reputational half-life. Searchers that crossed into intent-specific
extraction have empirically been blacklisted by Flashbots, MEV Blocker, and
Titan within 6-18 months. Backrun-only + JIT searchers have not.

---

## How this is enforced in code

- **`relays-client`** never accepts a bundle whose strategy_kind is
  flagged as predatory in the request payload.
- **`searcher-rs/src/scanner.rs`** detection paths emit only the allowed
  strategy kinds (`triangular`, `cross_dex`, `cex_dex`, `liquidation`,
  `jit_v3`, `backrun_public`).
- **`G-MEV-1` readiness verifier** confirms the presence and shape of
  this document before any capital flip.
- **Quarterly review:** every Q1 and Q3, the operator and on-call read
  the latest skirmish reports from Flashbots / EigenPhi / ZeroMEV and
  confirm â€” or update â€” the strategy ban list above.

---

## Amendments

Any change to the **Strategies REFUSED** list requires:

1. Operator + on-call written approval in repo (PR + signed commit).
2. Updated entry in `docs/governance/DATA-MATRIX.md` Â§M9.
3. A 7-day cooldown before any matching code path is enabled.

Adding a strategy to the **Strategies ALLOWED** list follows the same
process plus an external review note (Trail of Bits / OpenZeppelin /
Spearbit advisory referenced in the PR).

---

## References

- Skill: `.agents/skills/arbx-mev-ethics-gate/SKILL.md`
- Companion: `docs/policies/pre-execute-checklist.md` (`G-PEC-1`)
- Audit: `docs/governance/AUDIT-2026-04-22.md`
- Runbook: `docs/runbooks/relay-degraded.md`

