# WORKSPACE_SYNC — how to coordinate (mechanism, not theater)

There is **no direct inter-session channel**. Coordination is by **repo evidence only**:
- `gh pr list --state open` — who owns what (classify by branch prefix + `gh pr diff --name-only`).
- `git worktree list` / `git branch -a` — active branches.
- CI checks per PR — green/red, required vs non-required.
- This `docs/coordination/` tree — the shared board (once this PR merges, all sessions can read it).

## Rules to avoid collision
1. Before a lot: re-read `TASK_BOARD.md` + `gh pr list`; claim files in `HANDOFFS.md`.
2. One owner per file-glob; cross-lane edits go via a `HANDOFFS.md` flag, not direct edits.
3. Shared chokepoints (guaranteed conflict): `backend/Cargo.lock`, root `package-lock.json`, `database/migrations/NNN` numbering, `.github/workflows/e2e.yml`. Whoever merges first forces others to regen/renumber.
4. No irreversible action (merge/deploy/broadcast/sign) without operator textual GO.

## Current chokepoint warnings
- migration **098** triple-claimed (see BLOCKERS B-X1).
- lockfiles contended by dependabot #181/#175/#220/#233/#144/#145.
- `#224` edits 7 rust sim files — high conflict for any concurrent S3 edit.
