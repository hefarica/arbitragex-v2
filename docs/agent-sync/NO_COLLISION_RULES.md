# No-Collision Rules

Standing rules for any session/agent acting in this workspace. Violating these = churn or fund-path risk.

1. **One owner per blocker** (see `OWNER_MATRIX.md`). Do not take a task with an active owner unless a
   PR/comment/ledger entry shows it released, abandoned or transferred.
2. **Do not touch another session's file path.** Before editing, check `ACTIVE_WORKSTREAMS.md` + run
   `gh pr diff <n> --name-only` on open PRs to confirm no one else owns that file.
3. **Fund-path is off-limits to non-owners.** `relays-client`, `searcher-rs` execution, contracts,
   signer, `nonce_manager`, `bundle_builder`, `submit_engine` → S3/S4 only, behind review + CI.
4. **Never lift the mainnet code-lock** (`live_exec_policy.rs` chain-1 refusal) without all 18 gates +
   explicit hefarica approval.
5. **Never** touch secrets, keys, wallets, faucets, or run a deploy/broadcast without explicit operator
   authorization for that exact target. No `--admin`. No auto-bumping the audit allowlist.
6. **Do not use `productivo_full`'s `origin` remote** (stale VPS mirror) or the `(17)` clone as source of
   truth or for mutation. Ground on `github/main`.
7. **Do not create more coordination ledgers.** This `docs/agent-sync/` is the consolidator; #241/#243/
   #239/#236 are to be closed by the operator, not multiplied.
8. **No claim without evidence** — `file:line`, command output, CI run, or workflow ID.
9. **Sepolia deploy only via the protected `sepolia-deploy` environment** + manual hefarica approval.
