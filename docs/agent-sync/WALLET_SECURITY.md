# Wallet Command Center — security report + LIVE-blockers matrix

Feature branch `feat/wallet-command-center`. Surgical extension of the existing read-only wallet
surface. Grounded against `github/main` by a 6-agent map (anti-duplication) before any code.

## What already existed (preserved, NOT rebuilt)
The wallet surface was already read-only / fail-honest with **3-layer defense**: backend compile-time
`as const` SAFE_POSTURE (`api-server/src/routes/wallet.ts`), client-side re-pinning of
live_enabled/capital_exposed/broadcast (`useWalletStatus`/`useWalletSafety`), and a permanently-disabled
broadcast shell (`WalletIntentPanel`). No signer key, no private key, WalletConnect degrades fail-honest.
These were left untouched.

## The real delta (added)
| File | Purpose | Verified |
|---|---|---|
| `frontend/lib/web3/modes.ts` | 4-mode enum + deny-by-default resolver (READ_ONLY default; MANUAL/INTENT/AUTOMATION gated) | 5 unit tests |
| `frontend/lib/web3/intent.ts` | `TransactionIntent` (ARBX_INTENT_V1) EIP-712 type + legible preview + calldata hashing (pure, no signing) | 7 unit tests |
| `frontend/lib/web3/policy.ts` | 19-gate PURE evaluator, deny-by-default; reuses `SafetyGate` shape | 21 unit tests |
| `frontend/lib/web3/approval-risk.ts` | approval classifier — blocks unlimited/setApprovalForAll/Permit2-unlimited by default | 8 unit tests |
| `frontend/lib/web3/security.test.ts` | static scan of the wallet surface for HARD-invariant violations | runs in CI |
| `frontend/components/wallet/WalletModeBadge.tsx` | displays the resolved mode (READ_ONLY today) + AUTOMATION LOCKED | display-only |
| `frontend/components/wallet/TransactionIntentPreview.tsx` | legible EIP-712 preview + per-gate policy verdict; sign is a disabled shell | display-only |
| `automation/tools/wallet-security-gate.sh` + `.github/workflows/wallet-security.yml` | static CI gate (ethics-guard excludes `frontend/**`) | gate runs, exits 1 on violation |

**41/41 unit tests pass locally** (vitest+viem). One real bug was caught and fixed by the tests
(approval display order for setApprovalForAll). No mocks, no `test.skip`, no fake-green.

## HARD invariants — how each is enforced
| Invariant | Enforcement |
|---|---|
| No seed phrase / no private key in FE | no key-import primitives (`privateKeyToAccount`/`mnemonicToAccount`) — CI gate + `security.test` |
| No signer in FE / no FE broadcast | no `sendTransaction`/`writeContract`/`eth_sendTransaction` — CI gate + test; broadcast is a disabled shell |
| No blind signing | only SIWE `personal_sign` of a legible message + the EIP-712 legible intent; `eth_sign` banned by the gate |
| No unlimited approvals default | `approval-risk` blocks unlimited/setApprovalForAll/Permit2-unlimited; `policy` `approval_within_cap` denies unlimited |
| No wallet auto-click | gate bans `autoConnect:true` and `openConnectModal()` in effects; all actions are explicit onClicks |
| Deny-by-default | `resolveAllowedMode` → READ_ONLY unless posture opens; `evaluatePolicy` allow=false unless all 19 gates pass |

## LIVE-blockers matrix — what stays BLOCKED (by design)
| Capability | State | Unblocks only when |
|---|---|---|
| MANUAL_SIGN / INTENT_SIGN affordance | LOCKED (mode resolves READ_ONLY) | backend posture opens `live_enabled` + kill-switch-off (+ readiness/sim for INTENT_SIGN) |
| AUTOMATION_LOCKED (automated execution) | LOCKED | explicit live-canary human sign-off (M5/A.9) + full green — never in current posture |
| `signTypedData` wiring | NOT BUILT (preview only) | a future gated increment; requires policy `allow=true` which is impossible today |
| Policy `simulation_passed` / `calldata_hash_matches_sim` | FAIL (backend `/api/wallet/simulate` = `runtime_not_configured`) | the wallet simulation runtime is wired |
| Policy `live_gate_open` / `readiness_green` | FAIL (readiness NO-GO, live disabled) | mainnet readiness dossier (#248) P0s close + hefarica sign-off |
| Approval issuance | NOT BUILT (analyzer only classifies) | a gated approval flow with exact-amount + expiry |

**Net:** the Command Center adds the *safe scaffolding* (modes, legible EIP-712 intent, 19-gate policy,
approval analyzer) with every execution path **denied by default** in the current posture. Nothing signs,
broadcasts, approves, or moves capital. Enabling any signing is a separate, explicitly-gated step.

## Security confirmation
No private key · No seed phrase · No signer in frontend · No auto-click · No blind signing · No unlimited
approvals · No frontend broadcast · No mainnet · No capital · No live flip · No fake-green · No mocks ·
No `test.skip` · Existing read-only surface not degraded.
