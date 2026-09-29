# Test Evidence

> Verifiable artifacts backing the phase state + findings. Every claim is `file:line`, a command result,
> a CI run, or a workflow ID.

## PAPER / SHADOW (closed)
- Prod deploy run `28430443285` → `Deploy to VPS (Hetzner) → completed/success` (~16 min, non-destructive).
- VPS read (authorized, read-only): 21/21 containers healthy, `/health` 200, `paper_mode=true` preserved.
- Webapp (Playwright, `https://edge-arbx.ape-tv.net`): 8 operator pages render 200; feed shows real
  opportunities `REJECTED`; SYSTEM GUARD `Live:OFF·Broadcast:OFF·Submit:OFF·Paper:ON·Capital:$0`;
  live-flip `DISABLED` (FLIP BLOCKED).

## Mainnet-readiness audit (dossier #248)
- **Workflow `wu8kkwmv4`** — 7 grounding auditors on `github/main`, ~920k tokens, 206 tool calls. 7/7
  dimensions PARTIAL, all `file:line`.
- **Workflow `wm08i5ex7`** — 3 adversarial agents, ~407k tokens. Both fund-path gaps CONFIRMED_GAP;
  completeness critic surfaced the UPGRADER_ROLE P0 (missed by the 7).

## Independently re-verified (2026-07-01, direct commands)
- `security.yml` on `main` → `gh run list` = **failure** (allowlist expired 2026-06-30; non-required check).
- `lint-no-hardcode.sh` on `eec065b0` → **61 violations, exit 0** (gate neutered).
- Code-lock live: `grep` `live_exec_policy.rs` → `MAINNET_CHAIN_ID`, `MainnetRefused`, `chain_id == 1` at :84.
- P0-5: `DeployMainnet.s.sol:214-236` moves only `DEFAULT_ADMIN_ROLE`; `UPGRADER_ROLE` granted to `admin`
  in all three `initialize()`; `_authorizeUpgrade` gated only on `UPGRADER_ROLE` (`:649`).
- P1-5 nonce: `nonce_manager.rs` `refresh()` has **0 callers** repo-wide.
- P2-5: `bundle_builder.rs:245` `amount_in_to_eth` uses `U256::as_u128()` inside the value-cap guard.
- Env `sepolia-deploy`: required reviewer = `hefarica` (GitHub id `218611253`).

## Collision map (2026-07-01)
- No open PR touches `DeployMainnet.s.sol` / `nonce_manager.rs` / `lint-no-hardcode.sh` / rollback /
  `bundle_builder.rs` / `npm-audit-allowlist.json`. #226 = VPS IP scrub (P2-1) in-flight.
