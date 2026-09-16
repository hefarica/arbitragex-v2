# OMEGA DECISIONS (durable; V3 §7 — ground truth over narrative)

| ID | Decision | Rationale / evidence |
|----|----------|----------------------|
| **D1** | CI is **KEYLESS**. No `--private-key`/`--broadcast` in CI — neither in workflows (ethics-guard E3) nor in CI-invoked scripts (ethics-guard E4, #238). Deploy/canary/signing = operator / KMS / HSM / YubiKey plane only. | OMEGA anti-CI-signing doctrine; the M5 spec Q2 decision. |
| **D2** | **#224 is the canonical** cross-DEX-fix + sim↔broadcast-parity carrier. Do NOT re-implement it. | A duplicate PR #234 was opened on a stale-ref error and CLOSED; #224 also carries an `AliasedTwoLegRoute` guard the dup lacked. |
| **D3** | M5 canary = **self-funded seeded-pools** (Approach A) with `MockConstantProductRouter`, broadcast by the operator; CI does dry-run + REAL fork-validation only (no keys). | M5 spec/plan; reuse `DeployTestnet.s.sol` + `run_a4_fork_validation.sh` + `multistep_fork.rs`. |
| **D4** | `paper_mode=true` (`configs/app.toml`) is **NEVER agent-flipped**. `max_value_eth=1.0` on main (paper exposure suppression comes from `paper_mode`, NOT a 0-cap). | Master safety gate; FASE A corrected the prior "0.0" assumption. |
| **D5** | **Ground truth (repo / PRs / CI) > handoffs / memory.** Verify branch/PR state via `gh pr view/diff/checks`, never a stale local `git diff origin/main...origin/<branch>`. | The #234 duplicate + several FASE A stale-state corrections. |
| **D6** | The agent does **NOT** merge / deploy / broadcast / touch absent secrets / move capital / use real signers. It prepares + proves; the operator gates all outward-facing irreversibles. | OMEGA human-gate doctrine (V3 §14). |
