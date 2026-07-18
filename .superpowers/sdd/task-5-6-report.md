# Tasks 5-6 Report: Paper-Mode State Authority Endpoints

## Status: COMPLETE

| Step | Result |
|---|---|
| Read brief | OK |
| Create `paper-mode-state.ts` | OK |
| Create `paper-mode-reconcile.ts` | OK |
| Mount routes in `index.ts` | OK |
| Typecheck (`tsc --noEmit`) | **CLEAN** (0 errors) |
| Commit | `f153506` on `main` |

---

## Files Changed

### Created
- `backend/api-server/src/routes/paper-mode-state.ts`
- `backend/api-server/src/routes/paper-mode-reconcile.ts`

### Modified
- `backend/api-server/src/index.ts`

---

## Task 5: GET /api/paper-mode/state

**File:** `backend/api-server/src/routes/paper-mode-state.ts`

- Imports `resolvePaperModeState` from `../readiness/paper-mode-state.js`
- Mount function: `mountPaperModeState(app, deps)`
- `deps` requires: `redis`, `env`, `enabledChainIds`, `logger`
- Query param `chain_id` parsed as optional positive integer (400 on invalid)
- Calls `resolvePaperModeState` with `chainId` if provided
- Sets `Cache-Control: no-store` and `Pragma: no-cache`
- Returns 200 JSON with `PaperModeState`
- 503 on resolver failure with logged warning

**Mount location in `index.ts`:** after `mountLiveTestnet`, before RPC registry.
`enabledChainIds` derived from `cfg.chains` filtering `enabled !== false`.

---

## Task 6: POST /admin/paper-mode/reconcile

**File:** `backend/api-server/src/routes/paper-mode-reconcile.ts`

- Lua script atomically: `EXISTS` → `SET NX` → `XADD` audit stream → `PUBLISH` channel
- Gated by `ARBX_PAPER_AUTO_RECONCILE=on` (503 if off)
- Requires admin token via `requireAdminToken(ARBX_ADMIN_TOKEN)`
- Supports `dry_run` mode (returns projected action without writing)
- Body validation: `chain_id` (positive int), `new_state` (non-empty string)
- Optional fields: `reason`, `correlation_id`
- Actor derived from `x-arbx-actor` header (falls back to `"admin"`)
- Returns:
  ```json
  { "dry_run": boolean, "results": [{ "chain_id", "action", "created" }] }
  ```
- Keys used:
  - State: `arbx:papermode:<chain_id>`
  - Audit stream: `arbx:papermode:audit:<chain_id>`
  - Change channel: `arbx:papermode:<chain_id>:changes`

---

## Typecheck Result

```
> tsc --noEmit -p tsconfig.json
(no output)
```

Clean pass. No errors, no warnings.

---

## Concerns / Notes

1. **Redis key alignment:** The existing `index.ts` admin endpoint `POST /admin/config/paper-mode` writes to `arbx:papermode:<chain_id>` (line 974) and publishes on `arbx:papermode:<chain_id>:changes`. The reconcile endpoint uses the SAME key and channel patterns — consistent.

2. **Legacy key read-only:** The resolver (`paper-mode-state.ts`) still reads the legacy global key `arbx:papermode` as fallback (`explicit_legacy` confidence). The reconcile endpoint only writes per-chain keys, never the global key. This is intentional per B0.2 isolation.

3. **`cfg.chains` typing:** `index.ts` already imports `cfg` from `loadAppConfig()`. The chain filter uses an inline type annotation `{ enabled?: boolean; chain_id: number }` to satisfy strict TypeScript without modifying shared types.

4. **`Redis.eval` typing:** ioredis `eval` accepts variadic arguments as `string | number | Buffer`. The Lua script passes 3 keys + 5 args; TypeScript accepts this because the overloads cover `eval(script, numKeys, ...args)`.

5. **No tests added:** The brief did not request unit tests for the route files. The existing `paper-mode-state.test.ts` covers the resolver logic. If desired, route-level tests can be added in a follow-up.

6. **All changes are read-only/paper-shadow:** No live paths, no capital exposure, no broadcast logic modified.
