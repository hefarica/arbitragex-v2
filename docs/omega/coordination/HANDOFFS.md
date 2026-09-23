# OMEGA HANDOFFS

| ID | From → To | Handoff | Action required |
|----|-----------|---------|-----------------|
| **H1** | THIS → S4 | E4 (#238) makes #229's keyless violation **enforceable** by CI. | S4: delete #229's `deploy_live` job; move the LIVE deploy/canary to the operator/KMS plane (per the M5 plan Task 1). E4 then guards regressions. |
| **H2** | THIS → S4 / operator | #226 unblocked: reuse the existing `VPS_SSH_HOST` secret (no new `VPS_HOST` needed); `VPS_IP` is operator-runtime for 3 non-CI scripts. PR title/body still STALE. | operator: refresh #226 description, re-validate branch vs main, merge. |
| **H3** | S4 / operator → ALL | Merging **#224** unblocks: GATE_CROSSDEX_1A on-main, GATE_LAYER_C_224, the M5 prerequisite, AND S3's scanner-wiring (the on-chain fix that path exercises). | operator: rebase #224 on main, let CI settle green (only the NON-required TS-integration may stay red), then merge. |
| **H4** | THIS → S2 | My FASE A gate map (`ARBX_PATH_TO_LIVE_STATUS.md`, in the internal `(17)` copy) overlaps S2's #236 `FINAL_PAPER_AND_LIVE_READINESS_REPORT.md`. | S2 + THIS: dedupe — #236 is canonical for the readiness REPORT; this ledger keeps coordination only. |
| **H5** | operator → S5 | cargo-audit allowlist expires **2026-06-30**. | operator/S5: re-justify or remediate RUSTSEC-2026-0189 (rmcp 0.3→1.x) before expiry; consider adding `rust.yml`+`cargo audit` to required checks. |
