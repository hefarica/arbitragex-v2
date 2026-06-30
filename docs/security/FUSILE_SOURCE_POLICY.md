# FUSILE Source Policy

Governance for **porting code from external open-source repositories** into
`hefarica/arbitragex-v2`. "Fusile" = adopt/adapt code from a vetted upstream.

> This is a **source-governance** document, intentionally written in plain
> engineering terms (it describes what may enter the codebase and how). The
> repository's stylistic lexicon conventions do not apply to this policy file.

## Core rule

The `arbx-*` safety gates remain the **top layer** and override anything here:
`mev-ethics`, `net-profit`, `simulation-mandatory`, `pre-execute`,
`pre-edit-audit`, `no-hardcode`, `contract-atomicity`, `flash-loan`,
`rpc-failover`, `risk-limits`, `token-safety`, `paper-trade-first`.

No port may touch, without an explicit `CONFIRMED:` from the operator **and**
adversarial review:
- `configs/app.toml` `paper_mode` (master safety gate — never flipped by tooling)
- `backend/relays-client/src/live_exec_policy.rs` (M1 default-deny barrier)
- `backend/relays-client/src/bundle_builder.rs` broadcast path

## Approved sources (allowlist)

### Tier 1 — pure math, low friction → `backend/math-engine`
| Repo | Use |
|---|---|
| `darkforestry/amms-rs` | UniswapV2/V3, Balancer, ERC4626 simulation (alloy-native) |
| `0xKitsune/uniswap-v3-math` | sqrtPriceX96, tick↔price, liquidity math |
| `shuhuiluo/uniswap-v3-sdk-rs` | V3 SDK with unit-tested reference values |

A Tier-1 port may proceed without per-file approval, but **must** report what
was ported and from where.

### Tier 2 — architecture / executors (operator approves per PR)
| Repo | Target | Activation |
|---|---|---|
| `paradigmxyz/artemis` | `backend/searcher-rs` (patterns only) | `CONFIRMED: artemis patterns` |
| `paradigmxyz/mev-share-rs` | `backend/relays-client/relay_catalog.rs` | `CONFIRMED: relay fusile` |
| `refcell/subway-rs` | `backend/relays-client/bundle_builder.rs` (sim reference) | `CONFIRMED: relay fusile` |

### Tier 3 — read-only reference (no porting)
`SorellaLabs/brontes`, `mouseless0x/rusty-sando` (archived).

## Binding rules for every port

1. **Port-with-validation, not blind copy.** Reimplement to repo conventions
   (alloy / `U256` / existing traits). Prove correctness using the **source
   repo's own tests/values as external vectors**. A verbatim paste is not a port.
2. **Cite the source** as `repo@<sha>` in the code and the PR description.
3. **License check before any verbatim code lands.** This repo has no root
   `LICENSE` yet, and some sources are unlicensed (e.g. `uniswap-v3-math` is
   NO-LICENSE). *Numeric reference values are facts* (citation is sufficient);
   copying actual source code requires license compatibility to be resolved
   first.
4. **CI green before merge.** The gating Rust check is `rust.yml`; Foundry is
   `foundry.yml`. A port lands only with those green.

## First application

PR #216 used `0xKitsune/uniswap-v3-math@11c7e78` TickMath values and Uniswap v2
`getAmountOut` results as external vectors for `backend/math-engine` (numeric
facts, no code copied).

## Not adopted

The originally-proposed "global operator layer" that would (a) overwrite
`~/.claude/CLAUDE.md`, (b) override all project instructions, or (c) run
standing auto-loops was **not** installed: auto-internalizing third-party
instructions/code into a public, mainnet-capable repository is a supply-chain
injection channel. This policy is the reviewed, gated alternative.
