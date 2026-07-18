# Task 7: Frontend API Client + Hook

## Goal
Create getPaperModeState() API helper and usePaperModeState() React hook with loading/error/refetch states.

## Files
- Modify: frontend/lib/api-client.ts (add getPaperModeState)
- Create: frontend/hooks/usePaperModeState.ts

## Task 7a: API Client

In frontend/lib/api-client.ts, add:

```typescript
export async function getPaperModeState(chainId?: number): Promise<PaperModeState> {
  const url = chainId != null
    ? `/api/paper-mode/state?chain_id=${chainId}`
    : `/api/paper-mode/state`;
  const res = await fetch(url);
  if (!res.ok) {
    return DEFAULT_SAFE_STATE; // fail-safe
  }
  return res.json();
}
```

Define DEFAULT_SAFE_STATE inline or import from a shared types file.

## Task 7b: React Hook

Create frontend/hooks/usePaperModeState.ts:

```typescript
export interface UsePaperModeStateResult {
  data: PaperModeState;
  isLoading: boolean;
  isRefreshing: boolean;
  error: Error | null;
  refetch: () => Promise<void>;
}

export function usePaperModeState(chainId?: number): UsePaperModeStateResult {
  // Fetch on mount, poll every 15s
  // Cancel fetch on unmount
  // Fail-safe: return DEFAULT_SAFE_STATE on error
}
```

Requirements:
- isLoading=true on first fetch
- isRefreshing=true on subsequent polls
- AbortController to cancel in-flight requests
- clearInterval on unmount
- DEFAULT_SAFE_STATE when endpoint fails

## Types

PaperModeState type should match backend exactly:
```typescript
export interface PaperModeState {
  enabled: boolean;
  chain_id: number | null;
  source: string;
  confidence: "explicit" | "explicit_legacy" | "observed" | "inferred" | "default_safe";
  degraded: boolean;
  conflict: boolean;
  updated_at: string | null;
  reasons: string[];
  chains: Array<{
    chain_id: number;
    enabled: boolean;
    source: string;
    confidence: string;
    conflict: boolean;
    updated_at: string | null;
  }>;
}
```

## Verification
Run: cd frontend && npm run typecheck
Expected: clean (no new errors)

## Report
Write to: .superpowers/sdd/task-7-report.md
