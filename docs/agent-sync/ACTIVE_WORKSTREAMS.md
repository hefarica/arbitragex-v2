# Active Workstreams — observed from open PRs (2026-07-01)

> Session→work mapping inferred from open PRs, branch names and titles (observational; no direct channel).
> Sessions are labelled S2–S5 by the dossier's owner assignment; author is `@hefarica` for all human PRs
> (single GitHub account, multiple sessions) — distinguish by branch/topic, not author.

| Workstream | PR(s) | State | Topic |
|---|---|---|---|
| **S4 — Sepolia M5 pipeline** | #229 `feat/m5-sepolia-validation` | DRAFT | manual Sepolia validation CI (fail-closed). Blocks FASE 5. |
| **S5 — VPS IP scrub (P2-1)** | #226 `hardening/p0-ip-scrub-functions` | OPEN, +40/-40, 21 workflow files | env-ref the prod VPS IP in CI. Addresses dossier P2-1. |
| **Dependency upgrades (→ P0-2)** | #181 wagmi 2→3 · #145 rmcp 0.3.2→2.0 · #175 siwe · #144 redis · #220 npm-minor · #233 cargo-minor | OPEN (dependabot) | resolve npm+cargo audit advisories behind the expired allowlist. Operator/S5 sequences the upgrade sprint. |
| **Coordination/readiness docs** | #241 · #243 · #239 · #236 | OPEN/DRAFT | overlapping ledgers — consolidate (see LEDGER). |
| **Mainnet readiness dossier** | #248 `hardening/mainnet-readiness-dossier` | OPEN | this engagement's go/no-go dossier. |
| **Feature/hardening** | #137 governance · #136 dapp live-readiness · #170 cartridge-apex · #127 omega-strategy-pack | OPEN | not on the mainnet critical path; verify no collision before touching. |

## Not yet started as PRs (owned, unclaimed by any open PR — verified 2026-07-01)
No open PR touches `DeployMainnet.s.sol`, `nonce_manager.rs`, `lint-no-hardcode.sh`, a rollback workflow,
`bundle_builder.rs`, or `npm-audit-allowlist.json`. So P0-3, P0-4, P0-5, P1-3/4/5 and P2-5 have **no
in-flight PR** — they await their owner session (S3/S4/S5) or the operator.
