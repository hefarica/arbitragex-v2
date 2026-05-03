# API Origin Audit Report

## Phase 1: Total Audit for Hardcoded Localhost

### Diagnosis
The frontend codebase currently defaults to `http://localhost:8787` for its API (Edge URL) and `http://localhost:8080` (or `3000`) for its WebSocket connection if environment variables are missing. This is a **production-breaking** anti-pattern because in a production browser environment, `localhost` resolves to the end-user's device, not the VPS.

### Findings

| File | Line | Found Value | Severity | Required Fix |
|------|------|-------------|----------|--------------|
| `frontend/next.config.js` | 15 | `http://localhost:8787` | Production-breaking | Remove fallback, throw error if prod and missing. |
| `frontend/next.config.js` | 24 | `http://localhost:8080` | Production-breaking | Remove fallback, throw error if prod and missing. |
| `frontend/next.config.js` | 53 | `EDGE_URL \|\| "http://localhost:8787"` | Production-breaking | Force dynamic base URL resolution or strict env. |
| `frontend/next.config.js` | 54 | `WS_URL \|\| "http://localhost:8080"` | Production-breaking | Force dynamic base URL resolution or strict env. |
| `frontend/lib/admin-token.ts` | 23 | `EDGE_URL \|\| "http://localhost:8787"` | Production-breaking | Use centralized `apiClient.getApiBaseUrl()`. |
| `frontend/lib/api-client.ts` | 26 | `process.env... ?? "http://localhost:8787"` | Production-breaking | Refactor to central robust URL getter. |
| `frontend/app/page.tsx` | 42 | `process.env... ?? "http://localhost:8787"` | Production-breaking | Use centralized `apiClient`. |
| `frontend/app/opportunities/page.tsx` | 30 | `process.env... ?? "http://localhost:8787"` | Production-breaking | Use centralized `apiClient`. |
| `frontend/app/opportunities/page.tsx` | 59 | `process.env... ?? "http://localhost:3000"` | Production-breaking | Use centralized WS logic. |
| `frontend/app/onboarding/4-testing/page.tsx` | 7 | `process.env... ?? "http://localhost:8787"` | Production-breaking | Use centralized `apiClient`. |
| `frontend/app/onboarding/5-production/page.tsx`| 7 | `process.env... ?? "http://localhost:8787"` | Production-breaking | Use centralized `apiClient`. |
| `frontend/components/site-header.tsx` | 69 | `process.env... ?? "http://localhost:8787"` | Production-breaking | Use centralized `apiClient`. |
| `frontend/app/onboarding/3-advanced/page.tsx` | 7 | `process.env... ?? "http://localhost:8787"` | Production-breaking | Use centralized `apiClient`. |
| `frontend/app/onboarding/2-connect/page.tsx` | 7 | `process.env... ?? "http://localhost:8787"` | Production-breaking | Use centralized `apiClient`. |
| `frontend/components/paper-mode-toggle.tsx` | 24 | `EDGE_URL \|\| "http://localhost:8787"` | Production-breaking | Use centralized `apiClient`. |

### Conclusion
There are 15 occurrences of hardcoded `localhost` fallbacks spread across `next.config.js`, `lib/*`, `app/*`, and `components/*`. 
This violates the Zero Mocks Doctrine and breaks production routing. A centralized `getApiBaseUrl()` must be built to intercept and properly construct endpoints relative to the current window location or a strictly validated `NEXT_PUBLIC_EDGE_URL`.
