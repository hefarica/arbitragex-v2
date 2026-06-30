# TASK_BOARD — gate status (FASE 0 verified, signal-typed)

| Gate | Status | Signal | Evidence | Owner |
|---|---|---|---|---|
| GATE_REPO_CANONICAL | READY | CHECK | productivo_full, remote github, main b151144 | — |
| GATE_PAPER_CORE | **PARTIAL** | RUNTIME | dormant sinks + dead prefund (BLOCKERS B-PS1/2) | S2/S3 |
| GATE_FRONTEND_OPERATOR | READY* | RUNTIME+CHECK | 8 surfaces real, smoke green; *honest-display no blocking CI test | S1 |
| GATE_BACKEND_API | PARTIAL | RUNTIME | real reads, honest-empty pre-live | S2 |
| GATE_SCANNER_REVM | READY(obs)+PARTIAL(sim) | RUNTIME(static) | observer hard; SIM_SUCCESS unproven | S3 |
| GATE_CROSSDEX_1A | READY(contract) | CHECK | #224 forge tests green | S3/S4 |
| GATE_LAYER_C_PARITY | PARTIAL | CHECK | mechanically sound, not byte-e2e-proven | S3/S4 |
| GATE_ETHICS_CI | READY+EXTENDING | CHECK | #230 merged; E4/script-scan in flight | S5 |
| GATE_NO_CI_SIGNING | READY | CHECK | no `--private-key`/`--broadcast` in workflows (E3 yml-scope; script-scan extending) | S5 |
| GATE_M5_SEPOLIA | PREPARED-GATED | CHECK | #229 dry-run/fail-closed; depends #224 | S4 |
| GATE_LIVE_READINESS_UI | READY | RUNTIME | 17-item gate, flip disabled while blocked | S1 |
| GATE_OPERATOR_APPROVAL | MANUAL | MANUAL | every merge/deploy/broadcast = human GO | operator |

## Done (verified on main)
#232 operator-risk-presets (b151144) · #230 ethics-grep · #223 cargo-audit green.

## In progress (this session)
LOT: frontend/e2e CI wiring + blocking honest-display assertion (fixes orphaned spec) — NEXT.
