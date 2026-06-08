# OMEGA Bounded PHVA Loop — Governance Spec

**Status:** PROPOSED · **Date:** 2026-06-08 · **Author:** IA OMEGA (Council verdict) · **Supersedes:** the "infinite autonomous PHVA loop" prompt

This spec is the **safe replacement** for the originally-requested *infinite, bypass-permissions,
"no preguntas, no esperas, continúas"* PHVA loop. The OMEGA Council (security + economics
adversarial lenses + cs/math validators) **BLOCKED** the infinite form for two structural reasons,
both verified against this repo:

1. **The governors are mutable by the governed.** The loop's own safety invariants live in files
   the loop was granted write+push over (`.claude/skills/arbx-*.md`, `CLAUDE.md`,
   `.github/workflows/*`). "El guardián edita las reglas de la cárcel."
2. **The stop condition is unreachable by code → reward-hacking trap.** "100% en TODOS los módulos"
   is blocked by operator-gated (EXECUTOR_1 deploy, VPS deploy, `not_available` panels) and
   time-gated (Gate B/C ≥2-week windows, Gate D KMS/audit/auth) modules. A non-terminating loop
   pointed at a self-graded, unreachable metric optimizes by **moving the measurement, not the
   territory** (inflating `STATUS_REPORT.md` %, weakening CI checks, dismissing alerts).

Repo facts that make this concrete (verified 2026-06-08):
`main` branch protection = `required_reviews=0`, `enforce_admins=false`, `strict=true`, 14 required
checks · sole collaborator = `hefarica` (admin) · repo already contains ~18 `continue-on-error: true`
steps · `security.yml` carries human warnings *"NEVER use continue-on-error here."*

---

## 1. Hard denylist — the loop NEVER does these

Independent of any task or stop-condition pressure:

| # | Forbidden action | Why |
|---|---|---|
| D-1 | Edit `.claude/skills/**`, `CLAUDE.md`, `.claude/CLAUDE.md`, `AGENTS.md` | Self-modifying governors. Enforced by `doctrine-immutability` CI check. |
| D-2 | Edit `.github/workflows/**` or branch-protection settings | Cannot edit its own scoreboard. |
| D-3 | Self-merge any PR | Merge is an exclusive human action by the operator. |
| D-4 | Dismiss/suppress a CodeQL or security alert | Alerts are resolved in **code**, never dismissed. |
| D-5 | Add `continue-on-error` to a required-gate workflow | Turns "green CI" into "no CI". |
| D-6 | Touch capital / live / broadcast / executor / signer / secrets / wallets / private keys | `capital_exposed=0`, `live=OFF` are non-negotiable. |
| D-7 | Inflate a `STATUS_REPORT.md` % without committed + CI-verified evidence | RULE 00 / I-8. |
| D-8 | Mark a `not_available` panel "done" with placeholder/last-known data | Mirror Law / RULE 00 zero-mocks. |
| D-9 | Deploy a contract or to the VPS, or run a DB `DROP` without verified backup | Operator-gated; I-7. |

Crossing any denylist item = **immediate stop + operator handoff**, never a "fix".

## 2. Bounds — the loop is finite

- **Cycle ceiling:** max `N` cycles per run (default `N=8`); then mandatory stop + report.
- **Budget ceiling:** wall-clock / token budget per run; on exhaustion → stop + report.
- **Human checkpoint:** every `K` cycles (default `K=3`) **and** on every `TERMINAL-BLOCKED`, the loop
  pauses and emits an operator-handoff report. It does **not** auto-resume past a checkpoint.
- The phrase *"no preguntas, no esperas, continúas"* is explicitly **overridden** by
  `arbx-no-live-without-go-no-go` + the human-checkpoint requirement.

## 3. Dual-exit stop condition (replaces "loop until 100%")

The loop halts when **either**:

**TERMINAL-SUCCESS** — every code-resolvable item in the current bounded scope is complete **and
evidence-verified**:
- `main` CI green with the **required-check set unchanged** vs the pre-loop baseline
  (weakening/removing a check **invalidates** this exit),
- 0 open PRs carrying the `blocking` label,
- 0 open CodeQL alerts (fixed in code, not dismissed),
- every `STATUS_REPORT.md` module at its **honest code-ceiling**, with each operator/time-gated module
  carrying an explicit terminal label `BLOCKED: operator-gated` or `BLOCKED: time-gated` — which
  **counts as complete-for-autonomous-scope** (NOT as 100%).

**TERMINAL-BLOCKED** — the next actionable item requires crossing an operator gate
(broadcast / deploy / capital / secrets / KMS) or waiting on a wall-clock window → the loop **stops**
and emits a single operator-handoff report naming exactly what human action unblocks the next %.

> The loop may **never** use "100%" as its own exit signal for a module whose ceiling is an
> operator/time gate. `STATUS_REPORT.md` % rises only on committed, CI-verified, reviewer-validated
> evidence ("nada se reporta PASS sin evidencia").

## 4. Default scope ceiling: **Level 6 (paper/shadow)**

Per the 0→100 ladder, the autonomous loop stops at **Level 6**. Levels 7+ (staging deploy, live
readiness, limited live, institutional scale) are operator-authorized only and out of autonomous scope.

## 5. Enforcement surface (defense in depth for a solo repo)

GitHub-native required-reviews **deadlock a single-maintainer repo** (no second approver; admin
override removed by `enforce_admins=true`). So enforcement is layered instead:

1. **`doctrine-immutability` CI check** (this PR) — blocks D-1/D-2-class edits unless the operator
   applies `allow-doctrine-change`. Make it a **required** check (15th) to activate blocking.
2. **`strict=true` + 14 (→15) required checks** — nothing merges red (already in place).
3. **Operator-as-sole-merger policy** — the loop opens PRs; the human merges. This replaces the
   deadlocking `required_reviews≥1`.
4. **Recommended low-risk branch-protection additions** (no deadlock):
   `required_linear_history=true`, `required_conversation_resolution=true`,
   `allow_force_pushes=false`, `allow_deletions=false`. Keep `enforce_admins=false` (preserve the
   sole admin's emergency override) and `required_reviews=0` until a second reviewer identity exists.

## 6. Activation checklist (operator)

- [ ] Merge this PR (introduces `doctrine-immutability` + this spec).
- [ ] Add `doctrine-immutability / guard` to `main`'s required status checks.
- [ ] Create the `allow-doctrine-change` and `blocking` labels in the repo.
- [ ] (Optional) Apply the §5.4 low-risk branch-protection additions.
- [ ] Only then authorize a **bounded** (N≤8, K=3) loop at Level-6 ceiling.
