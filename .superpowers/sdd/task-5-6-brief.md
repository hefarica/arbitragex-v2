# Tasks 5-6: Endpoint Canonico + Reconciliador Atomico

## Goal
Create GET /api/paper-mode/state endpoint and POST /admin/paper-mode/reconcile with Lua-atomic execution.

## Files
- Create: backend/api-server/src/routes/paper-mode-state.ts
- Create: backend/api-server/src/routes/paper-mode-reconcile.ts
- Modify: backend/api-server/src/index.ts (mount routes)

## Task 5: GET /api/paper-mode/state

Create paper-mode-state.ts route file:
- Import resolvePaperModeState from ../readiness/paper-mode-state.js
- Mount function: mountPaperModeState(app, deps)
- deps needs: redis, env, enabledChainIds, logger
- Parse chain_id query param (optional, positive integer)
- Call resolvePaperModeState with chainId if provided
- Set Cache-Control: no-store, Pragma: no-cache
- Return 200 JSON with PaperModeState

Mount in index.ts alongside other API routes.

## Task 6: POST /admin/paper-mode/reconcile

Create paper-mode-reconcile.ts route file:
- Lua script atomically: check EXISTS, SET NX, XADD audit, PUBLISH
- Only execute if ARBX_PAPER_AUTO_RECONCILE=on
- Support dry_run mode
- Require admin token
- Return: { dry_run, results: [{ chain_id, action, created }] }

Lua script:
```
local key = KEYS[1]
local auditStream = KEYS[2]
local changeChannel = KEYS[3]
local newState = ARGV[1]
local correlationId = ARGV[2]
local actor = ARGV[3]
local reason = ARGV[4]
local chainId = ARGV[5]

if redis.call("EXISTS", key) == 1 then
  return {0, "already_exists"}
end

redis.call("SET", key, newState)
redis.call("XADD", auditStream, "*",
  "correlation_id", correlationId,
  "actor", actor,
  "reason", reason,
  "chain_id", chainId,
  "new_state", newState
)
redis.call("PUBLISH", changeChannel, newState)

return {1, "created"}
```

## Verification
Run: cd backend/api-server && npm run typecheck
Expected: clean

## Report
Write to: .superpowers/sdd/task-5-6-report.md
