# Task 1: Types and Pure Resolver

## Goal
Create the canonical resolvePaperModeState() pure resolver with structured PaperModeState, ChainPaperMode, and PaperModeConfidence types. Replace KEYS with MGET for Redis reads.

## Files
- Create: backend/api-server/src/readiness/paper-mode-state.ts
- Create: backend/api-server/src/readiness/paper-mode-state.test.ts

## Global Constraints
- Use MGET only; KEYS is prohibited in production
- Inferred confidence never produces GREEN; only EXPLICIT does
- Global Redis key arbx:papermode is legacy-degraded (confidence explicit_legacy)
- Aggregated confidence = MIN(confidence) across all chains
- No live/capital paths modified

## Implementation

Create paper-mode-state.ts with:
- PaperModeConfidence = "explicit" | "explicit_legacy" | "observed" | "inferred" | "default_safe"
- ChainPaperMode { chain_id, enabled, source, confidence, conflict, updated_at }
- PaperModeState { enabled, chain_id, source, confidence, degraded, conflict, updated_at, reasons, chains }
- resolvePaperModeState({ redis, env, enabledChainIds, chainId?, logger? })

Resolver order:
1. MGET per-chain keys + global key via redis.mget()
2. If per-chain explicit → confidence=explicit
3. If only global → confidence=explicit_legacy, degraded=true
4. If no Redis but archiver env ON → confidence=inferred
5. If no Redis, no archiver → fallback ARBX_TRADE_MODE → confidence=inferred
6. If nothing at all → confidence=default_safe
7. Aggregate: enabled=ALL chains ON, confidence=MIN across chains
8. Conflict detection: any chain OFF while others ON, or explicit OFF + archiver ON

Create paper-mode-state.test.ts with 6 tests:
1. EXPLICIT when per-chain ON
2. INFERRED when Redis empty + archiver ON
3. CONFLICT when per-chain OFF + archiver ON
4. explicit_legacy when only global exists
5. aggregated confidence = minimum
6. default_safe when no data

Use makeRedis() stub with mget() returning arrays.

## Verification
Run: cd backend/api-server && npx vitest run src/readiness/paper-mode-state.test.ts
Expected: 6/6 PASS

## Report
Write report to: .superpowers/sdd/task-1-report.md
Include: status, commits, test summary, any concerns.
