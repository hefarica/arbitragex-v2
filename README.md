# ArbitrageX v2 — Quantum Topological Yield Engine

> **Status:** PAPER / SHADOW (capital $0, broadcast disabled) · **~78% complete** · [Hardening + Roadmap](docs/HARDENING_AND_ROADMAP.md)

A real-time arbitrage detection + paper-shadow simulation system for EVM DEXs. It detects price asymmetries across liquidity venues, simulates execution on REVM/Anvil forks (zero capital), and scores opportunities via a 31-operator mathematical evidence pipeline.

---

## Mathematical Identity

The system models the market as a **Liquidity Manifold** $\mathcal{M}$ where each DEX pool is a point parameterized by reserves $(r_0, r_1)$. Price $p = r_1/r_0$. A closed loop of pools violates no-arbitrage when:

$$\mathcal{A} = \left|\log\prod_{(i,j) \in \text{loop}} \frac{p_j}{p_i}\right| > 0$$

The **Topological Yield** (net profit) is $\mathcal{Y} = \mathcal{A}_{\text{gross}} - \gamma_{\text{gas}} - \delta_{\text{slip}}$.

The 31 mathematical operators form an **observational basis** — each projects the market state into a scalar signal:

$$\mathbf{e} = [O_1, O_2, \dots, O_{31}]^\top \in \mathbb{R}^{31}$$

The evidence vector feeds a **calibrated Bayesian posterior**:

$$\log\frac{\pi}{1-\pi} = \log\frac{\pi_0}{1-\pi_0} + \sum_{k=1}^{31} \log LR_k \cdot e_k$$

Position sizing via **Kelly criterion**: $f^* = \frac{b\hat{p} - q}{b}$, clamped to $[0, 1]$.

**Currently:** the calibration store ($\log LR_k$) is empty → posterior = flat prior → `source_context = 'flat_prior'`. The motor is wired but inert until Stage 2 calibration.

---

## Architecture (C-S-E Canonical)

```
Collector (Rust) ──▶ Strategy Engine (TS) ──▶ Risk Engine ──▶ Executor (paper)
     │                      │                     │
     ▼                      ▼                     ▼
 searcher-rs            Redis Streams         api-server ──▶ edge ──▶ frontend
     │                      │                     │
     ▼                      ▼                     ▼
 math-engine            PostgreSQL            sim-ctl (REVM fork)
 (31 operators)         (opportunities)       (capital $0)
```

**24 services** on the prod VPS (Hetzner). See [HARDENING_AND_ROADMAP.md](docs/HARDENING_AND_ROADMAP.md) §2 for the full list.

---

## The 31 Mathematical Operators

All 31 implemented with real formulas + fail-honest `None` (commit `7f47c5e2`, 107 tests).

| Domain | Operators | Key Scalars |
|---|---|---|
| **Spectral** | PCA, Eigenvalues, Von Neumann entropy | ρ₁, λ_max, S(ρ) |
| **Stochastic** | PDMP, Markov, HMM, Lévy | λ_J, spectral gap, log P(O), α |
| **Filtering** | Kalman | mispricing z-score |
| **Optimization** | Welford, Golden-section, Gradient descent, Newton | σ, f(x*), θ*, break-even |
| **Game Theory / OR** | Queueing, Bundle recon, Path ordering, Shapley | E[W_q], margin, spread, max φ_i |
| **Finance** | Flash Loan (CPMM), JIT Liquidity | optimal x*, decay k |
| **Control** | Pontryagin, Lagrangian | H*, L = T−V |
| **ML** | DRL (PPO) | V(s_t) — gated None (untrained) |

---

## §IV Motor Status

| Stage | Component | Status |
|---|---|---|
| 1 | Evidence primitives + schema + capture | ✅ |
| 2a | Drift-tracker (Y-oracle) + sim-ctl/anvil | ✅ (flagged OFF) |
| — | **Gap 1:** route_metadata per-worker | ❌ Blocks Y-labels |
| — | **Gap 2:** ArbitrageExecutor deploy | ❌ Blocks B2c real-sim |
| 2b | Offline LR calibration | ⏳ Needs Gaps 1+2 |

---

## Quick Start

### Local Dev (no Docker Desktop, no backend services)

```bash
npm install                    # workspace deps
npm run -w api-server typecheck
cd backend && cargo check -p math-engine --lib && cd ..
cd frontend && npm run dev
```

### Deploy (CI/CD auto-deploy)

```bash
git add <files> && git commit -m "..." && git push origin main
```

### VPS Operations

See [SOP.md](docs/SOP.md) + [HARDENING_AND_ROADMAP.md](docs/HARDENING_AND_ROADMAP.md).

---

## Key Documentation

| Doc | Content |
|---|---|
| [HARDENING_AND_ROADMAP.md](docs/HARDENING_AND_ROADMAP.md) | Hardening, §IV gaps, roadmap, checklist |
| [SOP.md](docs/SOP.md) | Operator procedures |
| [THANOS_SETUP.md](docs/operations/THANOS_SETUP.md) | Long-term metrics |
| [VAULT_SETUP.md](docs/operations/VAULT_SETUP.md) | Vault TLS + sealed guide |
| [SECRETS_POLICY.md](docs/operations/SECRETS_POLICY.md) | Secrets T0-T2 |

---

## Security Posture

- **Paper/Shadow:** capital $0, broadcast `false`, no signer
- **Kill-switch:** <10ms fail-closed, armed by default
- **Private relay** (Flashbots/bloXroute): industry-standard MEV infrastructure
- **Zero-Mocks + Fail-Honest:** no fabricated data
- **Audit trail:** partitioned, anonymized

**Path to live:** gated by Crucible 72h ≥95% + institutional security + operator sign-off.

---

© ArbitrageX v2. Private.
