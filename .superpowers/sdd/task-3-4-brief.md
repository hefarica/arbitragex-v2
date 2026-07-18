# Tasks 3-4: Wire Resolver into g-pap-1 + Readiness Endpoints

## Goal
Replace inline paper-mode logic in g-pap-1.ts, readiness-extras.ts, and readiness-steps.ts with the canonical resolver from Task 1.

## Files to Modify
- backend/api-server/src/readiness/verifiers/g-pap-1.ts
- backend/api-server/src/routes/readiness-extras.ts
- backend/api-server/src/routes/readiness-steps.ts

## Files Created by Previous Tasks (DO NOT MODIFY)
- backend/api-server/src/readiness/paper-mode-state.ts
- backend/api-server/src/readiness/paper-mode-readiness.ts

## Task 3: g-pap-1.ts
Replace the Redis connection block (lines 47-105 approximately) with:
1. Import resolvePaperModeState from ../paper-mode-state.js
2. Import gradePaperReadiness from ../paper-mode-readiness.js
3. Call resolvePaperModeState({ redis, env: process.env, enabledChainIds: [1] })
4. If authority.conflict or !authority.enabled → return RED
5. Build PaperAccumulationState from existing pool queries
6. Call gradePaperReadiness(authority, accumulation)
7. Return grade.status, grade.reason, with evidence

Keep the existing pool query logic for days_accumulated, recent_opportunities, etc.

## Task 4: readiness-extras.ts + readiness-steps.ts
In readiness-extras.ts:
- Change mountReadinessExtras to accept redis in deps
- In collectBlockers(), call resolvePaperModeState with real redis
- Replace env-only paperMode with state.enabled

In readiness-steps.ts:
- Change mountReadinessSteps to accept redis in deps  
- Replace isPaperMode() function to use resolvePaperModeState with real redis
- gatherEngines() must await the async isPaperMode()

## Verification
Run: cd backend/api-server && npx vitest run src/readiness/verifiers/g-pap-1.test.ts src/routes/readiness-extras.test.ts src/routes/readiness-steps.test.ts
Expected: All existing tests still pass + any new tests pass

If existing tests fail due to async changes or redis dependency, fix them.

## Report
Write to: .superpowers/sdd/task-3-4-report.md
