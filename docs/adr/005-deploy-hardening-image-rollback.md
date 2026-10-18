# ADR-005: Deploy hardening — one-step image rollback, production environment, and the promotion path

| Field | Value |
|-------|-------|
| **Status** | Proposed |
| **Date** | 2026-10-18 |
| **Author** | Super Agent (Hector's team) |
| **Deciders** | Operator (Hector Fabio Riascos Castro) |
| **Trigger** | Operator: "DEJALO ACTIVO Y APLICA LAS RECOMENDACIONES DE ARREGLOS BARATOS LOS 3 CON REGLA Y HARDENING" |
| **Revert** | one step — `git revert <this commit>`; the workflow keeps working with no rollback capture |

## Context

On 2026-10-18 production was brought back in sync with `main` (`21b093b7`) through
the re-enabled `Auto-Deploy VPS (Post-E2E)` workflow (run `39140818051`, success).
The read-only VPS census that followed (`audit-vps-wiring`, run `39143097918`,
artifact `audit-fase0-3`) measured the deploy pipeline's real state and produced
three findings, in order of value:

1. **No image-level rollback.** `docker images` on the VPS showed **0 tags
   matching `pre-`/`backup`/`rollback`**. The deploy builds every service with
   `docker compose build --no-cache`, which re-points `arbitragex-v2-<svc>:latest`
   at the new image and leaves the previous release *dangling*; both the `[-1/9]`
   disk guard and the `[7a-bis]` post-build prune delete dangling images. The only
   way back was therefore a `git reset --hard` + a full rebuild (~30 min) —
   precisely when the stack may be broken.
2. **The deploy job runs outside any GitHub environment.** `deploy.yml` declares
   `environment: production`; `auto-deploy-vps.yml` did not. So auto-deploys were
   not recorded as deployments, environment-scoped secrets were unusable, and
   there was no single documented place to attach a human approval.
3. **The stack is built on the VPS, not promoted as an artifact.** 11 `build:`
   blocks in `docker/compose.prod.yml`; the CI (`docker-build.yml`) publishes only
   the frontend image to GHCR. Deploys therefore spend ~30 min of VPS CPU/disk per
   release, and the disk incidents (`APT-GPG-02`) live exactly in that window.

The operator asked to apply all three. Items 1 and 2 are cheap and are decided
here. **Item 3 is not a cheap fix** — the evidence is in "Alternatives" below —
so this ADR decides its scope and defers its implementation to a named block.

## Decision

### D1 — G-IMG-ROLLBACK-01: capture a prune-proof rollback point before every build *(implemented)*

Before `[6/9]` (the build), `auto-deploy-vps.yml` now walks the services that are
running, and for each one:

1. retags the **running container's image, by image ID**, as
   `arbx-rollback/<svc>:<gen>` (`gen` = UTC `YYYYmmddTHHMMSSZ`); and
2. `docker create`s a **keeper container** `arbx-rollback-<svc>-<gen>` from that
   image, never started.

The keeper container is the mechanism: `docker image prune -af` removes images
"not used by a container", and a *created* container counts as using it. The
image therefore survives the guard **without weakening the guard** — the guard
still frees exactly the space it freed before. Keeper containers carry no compose
project label, so `up --remove-orphans` never removes them.

Retention is bounded: only the newest `ROLLBACK_KEEP` generations (default 3) are
kept; older keeper containers are removed first, which makes their images
prunable again for the next `[-1/9]` guard.

Restore is one step, scripted in `scripts/deploy/rollback-images.sh`:

```
bash scripts/deploy/rollback-images.sh --list
bash scripts/deploy/rollback-images.sh --gen <GEN> --dry-run
bash scripts/deploy/rollback-images.sh --gen <GEN>
```

It retags `arbx-rollback/<svc>:<GEN>` back to `arbitragex-v2-<svc>:latest` and runs
`docker compose up -d --no-build --force-recreate` over the restored services. It
**never builds** (a build would immediately overwrite the rolled-back image),
**never touches git**, and is fail-closed on a generation that was not captured.
It restores application images only: database migrations remain forward-only, and
a bad schema change is a database restore, not an image rollback.

### D2 — G-DEPLOY-ENV-01: run the deploy job in the `production` environment *(implemented)*

`deploy-to-vps` now declares `environment: { name: production }`. This gives
deployment records (who/when/which SHA, with a rollback trail), the option of
environment-scoped secrets, and one documented place for a human brake.

**The brake is documented and OFF.** Required reviewers on the `production`
environment would make every auto-deploy wait for an approval — which contradicts
the operator's explicit decision that production stays automatic ("déjalo
activo"). Turning the brake on or off is an environment *setting* change, never a
workflow edit. Verified at decision time: the `production` environment has **no
protection rules**, so declaring it changes nothing about today's behaviour.

### D3 — the build-once/promote program is scoped, not smuggled in *(deferred)*

The target is: build each image once in CI, push it to GHCR, and have the VPS
`pull` + recreate, so a deploy is minutes, atomic, and its rollback is a tag
`pull`. This ADR records it as the next block and **does not** implement it —
see why below. Until then, D1 is what makes a fast rollback possible at all.

## Consequences

- **Positive.** A bad release can be rolled back in **one step, without git and
  without a rebuild**; releases are recorded as deployments; the disk guard's
  behaviour is provably unchanged (checked by the gate); the rollback mechanism
  degrades gracefully — a first-ever deploy captures nothing and skips.
- **Cost.** Up to `ROLLBACK_KEEP` × 12 images of extra disk, capped by retention.
  The capture adds a few seconds to a ~30-minute job.
- **Not fixed here.** The ~30-minute VPS build (D3), and the fact that the keeper
  containers are visible in `docker ps -a` as `Created` (a read-only audit that
  counts *running* containers is unaffected).

## Alternatives considered

- **Local tags only, and stop pruning images.** Rejected: `docker image prune -a`
  has no "exclude by tag" filter, and labelling an existing image is impossible
  without rebuilding it — so protecting tags would have meant **weakening the disk
  guard**, the thing that prevents the `APT-GPG-02` disk-100% incidents.
- **Push rollback tags to GHCR on every deploy.** Rejected as the default: it
  moves ~3.5 GB per deploy off the VPS for a capability the local mechanism
  already provides. Revisit if the VPS disk becomes the constraint.
- **Implement D3 now.** Rejected **on evidence**:
  - the frontend image has production values **baked at build time**
    (`NEXT_PUBLIC_EDGE_URL`, `NEXT_PUBLIC_WALLETCONNECT_PROJECT_ID`,
    `ARBX_TLS_ENABLED`, `ARBX_BUILD_SHA`) — a CI-built generic image is not a
    drop-in for the VPS-built one, and getting it wrong silently degrades the
    dapp (WalletConnect missing);
  - `docker-build.yml` publishes only the frontend today, so the other 10 build
    blocks (11 total, most of them Rust) would need a CI matrix whose build-arg
    parity must be proven before it can replace the VPS build;
  - a "pull if present, else build" hybrid is unsafe precisely because of that
    parity: a partially-correct image would be preferred over a correct build.
  Doing this badly is worse than not doing it. It belongs in its own block with
  its own gate, on top of the release manifest D1 now makes identifiable.

## Revert

`git revert <commit>` — one step. The workflow then deploys exactly as it does
today (no capture, no environment key); the rollback script becomes inert (no
generations are produced) and the gate fails loudly if the capture is removed
while the script is still advertised.

**The revert does NOT clean what the mechanism already created.** A revert
restores code; the keepers and `arbx-rollback/*` tags already on the VPS stay
until something removes them. Ownership, stated explicitly:

- **By itself:** the deploy's bounded retention removes them as newer
  generations arrive — but only while the code is still deployed. After a
  revert nothing prunes them, so they would sit there forever.
- **The operator removes them, in one step:**
  `bash scripts/deploy/rollback-images.sh --clean --dry-run` (shows what would
  go) and then `--clean` (removes every container labelled `arbx.rollback=1`,
  every `arbx-rollback/*` tag, and prunes).
- **Disk impact if left alone:** up to `ROLLBACK_KEEP` × 12 images; the next
  `[-1/9]` guard treats them as unused and they become prunable once their
  keeper is gone.

## Gate

`bash scripts/gates/g-img-rollback-01.sh` — asserts the capture runs **before**
the build, in the `arbx-rollback` namespace, with keeper containers and bounded
retention; that the disk guard's prunes are **still present** (not weakened); that
the deploy job declares the `production` environment; and that the restore script
exists, is executable, never builds, and verifies `/api/health`.
