# Task 7 Report: Frontend API Client + Hook (Paper-Mode State Authority)

## Status: COMPLETE

## Commits
- `7c44dbe` — feat(paper-mode): add getPaperModeState() client + usePaperModeState() hook (Task 7)

## Files Changed
| Action | File |
|--------|------|
| Modified | `frontend/lib/schemas.ts` |
| Modified | `frontend/lib/api-client.ts` |
| Created | `frontend/hooks/usePaperModeState.ts` |
| Modified | `frontend/components/readiness/GSimSmokeTestCard.tsx` |

## What Was Done

### 1. Schema + Types (`frontend/lib/schemas.ts`)
Added canonical Zod schemas and TypeScript types mirroring the backend `PaperModeState` exactly:
- `PaperModeChainSchema` — per-chain resolution record
- `PaperModeStateSchema` — top-level state with closed `confidence` enum
- `DEFAULT_SAFE_STATE` — fail-safe constant (`enabled=false`, `degraded=true`, `confidence="default_safe"`, `reasons=["endpoint_unavailable"]`)

### 2. API Client (`frontend/lib/api-client.ts`)
Added `getPaperModeState(chainId?: number)` that:
- Uses the existing `getValidated()` pipeline (5s timeout, AbortController, retries on 5xx, Zod validation)
- Returns `Result<PaperModeState>` (`{ ok: true, data } | { ok: false, error }`)
- Builds `/api/paper-mode/state?chain_id=${chainId}` when chainId is provided; falls back to `/api/paper-mode/state`
- Never throws — always resolves to a Result

### 3. React Hook (`frontend/hooks/usePaperModeState.ts`)
Created `usePaperModeState(chainId?: number): UsePaperModeStateResult` with:
- `isLoading` — true on first fetch only
- `isRefreshing` — true on subsequent polls
- `error` — surfaced as `Error | null`; any error reverts `data` to `DEFAULT_SAFE_STATE`
- `refetch` — callable on demand; cancels in-flight requests via AbortController
- Poll interval: 15s via `setInterval`
- Cleanup: clears interval + aborts controller on unmount
- Abort race safety: only the most recent fetch updates loading flags (`abortCtrlRef.current === ctrl` check)

### 4. Typecheck Fix (incidental)
Fixed pre-existing `TS2367` in `GSimSmokeTestCard.tsx` — the `status` variable typed as `"red" | "green"` made the `"yellow"` branch unreachable. Removed the dead branch.

## Typecheck Result
```
> @arbx/frontend@0.1.0 typecheck
> tsc --noEmit

SUCCESS — zero errors, zero new warnings.
```

## Concerns / Notes
1. **Existing code fix**: `GSimSmokeTestCard.tsx` had a pre-existing TS2367 (unreachable yellow branch). This was fixed as a drive-by because it blocked `npm run typecheck`. The fix is purely subtractive (removed dead code).
2. **tsconfig.json `include`**: The `hooks/` directory is NOT listed in `frontend/tsconfig.json` `include`. The hook file is referenced via `@/hooks/usePaperModeState` import paths. TypeScript resolves it correctly through `baseUrl` + `paths`. No `include` change needed.
3. **No component wiring**: Per Task 8 boundary, no existing component files were modified to consume the new hook. The hook is ready for `T8` to import and wire into the UI.
4. **Read-only / paper-shadow**: This code only reads state. No mutations, no capital exposure, no broadcast path.
