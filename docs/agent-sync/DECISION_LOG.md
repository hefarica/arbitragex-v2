# Decision Log

| Date | Decision | Rationale |
|---|---|---|
| 2026-07-01 | **Mainnet = gated, NOT prohibited.** | Operator doctrine: permitted only with all 18 gates + KMS/HSM + evidence + explicit hefarica approval. Until then, READINESS not execution. |
| 2026-07-01 | Mainnet stays **code-locked**; do not lift `live_exec_policy` chain-1 refusal. | Must land KMS/HSM + close P0s + sign-off first. Lifting is a deliberate, gated code change. |
| 2026-07-01 | Ground all work on **`github/main`**, never `productivo_full`'s `origin` (=`arbx-git` VPS mirror). | The mirror was 464 commits stale; it made "main" look like it lacked `deploy-vps.yml`. |
| 2026-07-01 | **Do NOT auto-bump the audit allowlist** (P0-2). | The allowlist doc reserves the upgrade-sprint-vs-extend decision for the operator; auto-extending = rubber-stamping (forbidden). |
| 2026-07-01 | **Do NOT naïvely flip `lint-no-hardcode.sh` to `exit 1`** yet (P0-3). | Main has 61 live violations (mostly allow-list false-positives); flipping first reds CI. Triage-then-enforce. |
| 2026-07-01 | Keep the dossier's corrected §3.1. | The original claim that the "atomic handoff" eliminates the deployer-admin window was **false for the upgrade path** (P0-5). Do not restate it. |
| 2026-07-01 | `docs/agent-sync/` is the on-main coordination consolidator; recommend closing redundant ledgers #241/#243/#239/#236. | Four unmerged ledgers = churn; one canonical point of truth. |
| 2026-07-01 | Claude does not take fund-path/CI-gate/secret work. | Owned by S3/S4/S5/operator; taking it = collision + fund-path risk. |
