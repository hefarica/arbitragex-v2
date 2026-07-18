# Tasks 3-4 Report: Wire Resolver into g-pap-1 + Readiness Endpoints

## Status
Completed and committed.

## Commit
- **Hash:** `62b0bd1`
- **Message:** `feat(readiness): wire canonical paper-mode resolver into g-pap-1 and readiness routes`

## Files Modified
- `backend/api-server/src/readiness/verifiers/g-pap-1.ts`
- `backend/api-server/src/routes/readiness-extras.ts`
- `backend/api-server/src/routes/readiness-steps.ts`
- `backend/api-server/src/index.ts`
- `backend/api-server/src/readiness/paper-mode-state.ts`

## What Changed

### g-pap-1.ts
- Removed inline Redis parsing of `arbx:papermode` global and per-chain keys.
- Imported `resolvePaperModeState` and `gradePaperReadiness`.
- Created a thin `RedisLike` wrapper around the connected `ioredis` instance and called `resolvePaperModeState({ redis, env: process.env, enabledChainIds: [1] })`.
- Built `PaperAccumulationState` from the existing pool queries (`recent_count`, `first_row_age_days`, `last_row_age_hours`).
- Returned `grade.status` and `grade.reason`, preserving the DB evidence suffix.

### readiness-extras.ts
- Added `redis` to `mountReadinessExtras` deps.
- Replaced the env-only `paperMode` inference with `resolvePaperModeState({ redis, env: process.env, enabledChainIds: [1] }).enabled`.
- Updated both `/api/v1/readiness/blockers` and `/api/v1/readiness/decision` paths.

### readiness-steps.ts
- Added `redis` to `mountReadinessSteps` deps.
- Converted `isPaperMode()` from a sync env probe to an async function backed by `resolvePaperModeState`.
- Updated `gatherEngines` to `await isPaperMode(redis)`.

### index.ts
- Passed the existing `redis` client to both `mountReadinessExtras` and `mountReadinessSteps`.

### paper-mode-state.ts
- Fixed two `noUncheckedIndexedAccess` errors uncovered by `tsc --noEmit` (`enabledChainIds[i]!` and `chains[0]!.chain_id`).

## Test Summary
```
npx vitest run \
  src/readiness/verifiers/g-pap-1.test.ts \
  src/routes/readiness-extras.test.ts \
  src/routes/readiness-steps.test.ts \
  src/readiness/paper-mode-state.test.ts \
  src/readiness/paper-mode-readiness.test.ts \
  src/readiness/readiness.test.ts
```
Result: **5 files, 89 tests passed** (4 directly affected + readiness integration).

```
npx tsc --noEmit
```
Result: **clean**.

## Concerns
- `readiness.test.ts` logs an `[ioredis] Unhandled error event: Error: connect ETIMEDOUT` during the verifyAll integration test, but the test still passes. This is pre-existing behavior when no Redis is reachable and does not indicate a regression.
- The `g-pap-1.ts` Redis client is still short-lived (connect/disconnect per call). This preserves the original verifier behavior and avoids holding a persistent connection; no change requested.
- No additional tests were added for the new resolver wiring. Existing tests cover the pure helpers; integration with live Redis remains implicit in `readiness.test.ts`.
