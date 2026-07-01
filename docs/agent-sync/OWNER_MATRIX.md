# Owner Matrix

> One owner per blocker. Do NOT take a task with an active owner unless the ledger/PR/comment shows it is
> released, abandoned or explicitly transferred.

| Owner | Scope | Blockers | Current PR |
|---|---|---|---|
| **operator (hefarica)** | KMS/HSM decision, secrets, wallet, Sepolia ETH, env approval, allowlist decision, A.9 sign-off, **final mainnet approval** | P0-1, P0-2 (decision), condition 18 | — |
| **S2 — Backend** | API/selector, canary telemetry emission | P1-4 | — |
| **S3 — Rust/execution** | relays-client, searcher-rs, risk layer | P1-1, P1-2, P1-3, P1-5, P2-4, P2-5 | — |
| **S4 — Contracts/Foundry** | UUPS, roles, timelock, deploy scripts, M5 pipeline | P0-5, P2-3, #229 | #229 (draft) |
| **S5 — CI/Security** | workflows, allowlists, no-hardcode, rollback, IP scrub | P0-2 (exec), P0-3, P0-4, P2-1, P2-2 | #226 (IP scrub) |
| **Claude (this session)** | audit, readiness dossier, coordination ledger, read-only verification | — (no fund-path / no CI-gate / no operator-secret work) | #248, this PR |

## Claude's lane (explicit)
Permitted: read-only audit, grounded findings, docs/coordination, evidence packs, state revalidation.
**Not permitted here:** fund-path/contract/CI-gate code (owned by S3/S4/S5), secrets/keys/wallet/faucet
(operator), deploy/broadcast, lifting the code-lock, `--admin`, auto-bumping the allowlist, worktree 17.
