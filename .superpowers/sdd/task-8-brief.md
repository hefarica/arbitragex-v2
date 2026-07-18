# Task 8: Frontend Components

## Goal
Update paper-mode-toggle, SystemGuardBanner, and site-header to use the canonical usePaperModeState hook.

## Files
- Modify: frontend/components/paper-mode-toggle.tsx
- Modify: frontend/components/SystemGuardBanner.tsx
- Modify: frontend/components/site-header.tsx

## Task 8a: PaperModeToggle

Changes:
- Accept chainId: number as REQUIRED prop (no default)
- POST body must include { enabled, chain_id: chainId }
- Validate chainId is positive integer before POST
- Source initial enabled state from usePaperModeState(chainId).data.enabled
- Block toggle if confidence is "default_safe" or "conflict" or degraded
- Show confidence label next to toggle (EXPLICIT/INFERRED/etc.)

## Task 8b: SystemGuardBanner

Changes:
- Import usePaperModeState()
- Make Paper tile dynamic:
  - value: ON/OFF based on data.enabled
  - tone: safe when enabled && !conflict, danger otherwise
  - subtitle: confidence level (EXPLICIT, INFERRED, etc.)
- Keep Live, Capital, GO live tiles structurally hard-coded-safe
- Show DEGRADED or CONFLICT badges when applicable

## Task 8c: site-header.tsx

Changes:
- Use usePaperModeState() or accept paperMode prop
- Badge text: "PAPER · TLS SHADOW" when enabled
- If conflict or !enabled: show warning state

## Verification
Run: cd frontend && npm run typecheck
Expected: clean

## Report
Write to: .superpowers/sdd/task-8-report.md
