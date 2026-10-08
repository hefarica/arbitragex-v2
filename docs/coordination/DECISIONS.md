# DECISIONS + retractions (ground-truth over narrative)

## Retractions (my prior claims refuted by ground truth — 2026-06-30)
- **D1** "#232 pending merge" → **MERGED** (`b151144`, 2026-06-30T02:18:13Z).
- **D2** "#232 operator-presets.spec.ts runs in CI" → **ORPHANED/UNRUN** (frontend/e2e wired to zero workflows; no `@playwright/test`). My false-green; I own the fix.
- **D3** "#230 ethics-guard open" → **MERGED** (2026-06-30T01:22:14Z).
- **D4** "next lot = build P0 E4 + E3 script-scan" → already in flight (`omega/ethics-guard-ci-script-scan-20260630`). Do NOT duplicate.
- **D5** (earlier) "cargo-audit red on main too" → main green via #223; #232 was stale-based.
- **D6** "#162 fix clears TS-integration flake" → failed again; insufficient.

## Standing decisions
- Milestone numbering: repo is **ascending** (M1 barrier < M2 encoder < M5 Sepolia); the "M5→M0 descending" framing contradicts terrain → interpreted as "readiness-blocking first" (#224 → M5).
- Autonomy: reversible work proceeds without asking; **hard-gate** merge/deploy/broadcast/signing/secrets/capital.
- Coordination docs live here (`docs/coordination/`) for real cross-session visibility — scratch files sync nothing.

## Next lot (chosen, collision-justified)
Wire `frontend/e2e` into CI (new workflow, not #136's `e2e.yml`) + `@playwright/test` in frontend + blocking honest-display assertion + run the orphaned `operator-presets.spec.ts`. Fixes D2; closes BLOCKERS B-PS4; S1 low-collision.
