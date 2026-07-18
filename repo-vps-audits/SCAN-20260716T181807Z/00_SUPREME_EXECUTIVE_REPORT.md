# Supreme Repository + VPS Architecture Audit

## Verdict

```text
NO-GO
```

- Scan ID: `SCAN-20260716T181807Z`
- Repository SHA: `aa28fee4ff421bc907b16358418cb1f5505887c4`
- VPS SHA: `UNKNOWN`
- Weighted maturity: **64.2%**
- Previous maturity: **64.2%**
- Delta: **+0.0%**
- Duration: `31.93s`
- Read-only safety: **ENFORCED**

## Conformity

| VERIFIED | DRIFT | MISSING | BROKEN | BLOCKED | UNKNOWN | EXTRA |
|---|---|---|---|---|---|---|
| 16 | 448 | 9 | 1 | 0 | 1 | 1 |

## First actions by architectural unlock

| Priority | Node | Status | Unlock | Blocked by | Recommendation |
|---|---|---|---|---|---|
| P0 | api-server | MISSING | 16.0 |  | Create or restore the missing repository/compose/runtime component. |
| P0 | edge | MISSING | 12.0 | api-server | Create or restore the missing repository/compose/runtime component. |
| P0 | relays-client | MISSING | 8.0 |  | Create or restore the missing repository/compose/runtime component. |
| P0 | searcher-rs | MISSING | 8.0 |  | Create or restore the missing repository/compose/runtime component. |
| P0 | selector-api | MISSING | 8.0 |  | Create or restore the missing repository/compose/runtime component. |
| P0 | sim-ctl | MISSING | 8.0 |  | Create or restore the missing repository/compose/runtime component. |
| P0 | frontend | MISSING | 6.0 | edge | Create or restore the missing repository/compose/runtime component. |
| P0 | recon | MISSING | 5.0 |  | Create or restore the missing repository/compose/runtime component. |
| P0 | token-enricher | MISSING | 5.0 |  | Create or restore the missing repository/compose/runtime component. |
| P1 | thanos-store | BROKEN | 6.0 |  | Restore or start the service only through the approved deployment workflow. |
| P2 | HOST:repo-parity | UNKNOWN | 10 |  |  |
| P2 | FILE:docker/compose.prod.yml | DRIFT | 8.0 |  | Deploy the canonical file version and verify SHA-256 parity. |
| P2 | FILE:.claude/worktrees/agent-a7a72813e0989b73b/.devcontainer/Dockerfile | DRIFT | 4.0 |  | Deploy the canonical file version and verify SHA-256 parity. |
| P2 | FILE:.claude/worktrees/agent-a7a72813e0989b73b/.devcontainer/docker-compose.yml | DRIFT | 4.0 |  | Deploy the canonical file version and verify SHA-256 parity. |
| P2 | FILE:.claude/worktrees/agent-a7a72813e0989b73b/arbitragex-v2-main/.devcontainer/Dockerfile | DRIFT | 4.0 |  | Deploy the canonical file version and verify SHA-256 parity. |
| P2 | FILE:.claude/worktrees/agent-a7a72813e0989b73b/arbitragex-v2-main/.devcontainer/docker-compose.yml | DRIFT | 4.0 |  | Deploy the canonical file version and verify SHA-256 parity. |
| P2 | FILE:.claude/worktrees/agent-a7a72813e0989b73b/arbitragex-v2-main/backend/Cargo.lock | DRIFT | 4.0 |  | Deploy the canonical file version and verify SHA-256 parity. |
| P2 | FILE:.claude/worktrees/agent-a7a72813e0989b73b/arbitragex-v2-main/backend/api-server/Dockerfile | DRIFT | 4.0 |  | Deploy the canonical file version and verify SHA-256 parity. |
| P2 | FILE:.claude/worktrees/agent-a7a72813e0989b73b/arbitragex-v2-main/backend/api-server/package.json | DRIFT | 4.0 |  | Deploy the canonical file version and verify SHA-256 parity. |
| P2 | FILE:.claude/worktrees/agent-a7a72813e0989b73b/arbitragex-v2-main/backend/math-engine/Cargo.toml | DRIFT | 4.0 |  | Deploy the canonical file version and verify SHA-256 parity. |
| P2 | FILE:.claude/worktrees/agent-a7a72813e0989b73b/arbitragex-v2-main/backend/mcp-sim-engine/Cargo.toml | DRIFT | 4.0 |  | Deploy the canonical file version and verify SHA-256 parity. |
| P2 | FILE:.claude/worktrees/agent-a7a72813e0989b73b/arbitragex-v2-main/backend/prioritization-spine/Cargo.toml | DRIFT | 4.0 |  | Deploy the canonical file version and verify SHA-256 parity. |
| P2 | FILE:.claude/worktrees/agent-a7a72813e0989b73b/arbitragex-v2-main/backend/recon/Cargo.toml | DRIFT | 4.0 |  | Deploy the canonical file version and verify SHA-256 parity. |
| P2 | FILE:.claude/worktrees/agent-a7a72813e0989b73b/arbitragex-v2-main/backend/recon/Dockerfile | DRIFT | 4.0 |  | Deploy the canonical file version and verify SHA-256 parity. |
| P2 | FILE:.claude/worktrees/agent-a7a72813e0989b73b/arbitragex-v2-main/backend/relays-client/Cargo.toml | DRIFT | 4.0 |  | Deploy the canonical file version and verify SHA-256 parity. |

## Trust boundary

The audit did not modify the repository, VPS, Docker, database, Redis, services,
CI/CD, secrets, firewall or deployment state. Repository URLs are cloned only
into an isolated audit workspace. Existing local repositories are opened without
checkout, reset, pull, clean or file writes.
