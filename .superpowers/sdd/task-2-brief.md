# Task 2: Readiness Evaluator (Authority + Accumulation)

## Goal
Create gradePaperReadiness() which combines PaperModeState (authority) with PaperAccumulationState (pipeline data) to produce a PaperReadinessGrade (green/yellow/red).

## Files
- Create: backend/api-server/src/readiness/paper-mode-readiness.ts
- Create: backend/api-server/src/readiness/paper-mode-readiness.test.ts

## Interfaces
- Consumes: PaperModeState from paper-mode-state.ts (Task 1)
- Produces: PaperReadinessGrade, PaperAccumulationState

## Rules (exact)

authority.conflict === true                    → RED
authority.enabled === false                    → RED
authority.confidence === "explicit" && accumulation.days >= 7 && accumulation.pipeline_active
                                                → GREEN
authority.confidence === "explicit" && !accumulation.pipeline_active
                                                → YELLOW (pipeline stalled)
authority.confidence === "explicit" && accumulation.days < 7
                                                → YELLOW (not enough days)
authority.confidence === "explicit_legacy"     → YELLOW max (never GREEN)
authority.confidence === "inferred"            → YELLOW max
authority.confidence === "default_safe"        → YELLOW max

## Implementation

Create paper-mode-readiness.ts with:
- PaperAccumulationState { days_accumulated, recent_opportunities, last_opportunity_at, pipeline_active, degraded, reasons }
- PaperReadinessGrade { status, reason, authority_confidence, accumulation_days }
- gradePaperReadiness(authority, accumulation)

Create paper-mode-readiness.test.ts with 5 tests:
1. GREEN when explicit + >=7d + pipeline active
2. YELLOW when explicit + <7d
3. RED when conflict
4. YELLOW when inferred + pipeline active
5. YELLOW when explicit_legacy (never green)

## Verification
Run: cd backend/api-server && npx vitest run src/readiness/paper-mode-readiness.test.ts
Expected: 5/5 PASS

## Report
Write to: .superpowers/sdd/task-2-report.md
