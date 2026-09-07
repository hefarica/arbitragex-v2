---
id: R-A6A9-001
title: Risk Circuit Breakers (A.6) + GO/NO-GO Sign-off (A.9) Operations
severity: P1
duration: 20m
owner: Operator
reviewed: 2026-09-07
---

# Risk Circuit Breakers + GO/NO-GO Sign-off Runbook

## Purpose

Two operator-only procedures shipped with the A.6/A.7/A.9 program
(2026-09-07, PRs #542/#543/#544/#548):

1. **A.6** — activate the doctrinal risk circuit breakers that sit at
   `NOT_AVAILABLE` until the operator supplies thresholds, and understand
   the `circuit_breakers` Prometheus alerts.
2. **A.9** — execute the two-operator formal GO/NO-GO sign-off on the
   audit-trail ledger. This is the LAST blocker between the current state
   and live eligibility. **Performing the sign-off does NOT flip anything
   to live** — see CLAUDE.md §34.3 (live flip is a separate, explicit,
   operator-authorized act gated by `relays-client/live_exec_policy.rs`,
   which remains default-deny for mainnet).

## Preconditions

1. SSH access to the VPS (`ssh arbx`, repo at `/opt/arbitragex-v2`).
2. `ARBX_ADMIN_TOKEN` available (env on the VPS / your vault).
3. Two DISTINCT operator identities for A.9 (quorum-2 is structural:
   `UNIQUE(ledger_hash, actor)` — the same actor signing twice gets 409).

---

## Part 1 — A.6 circuit breaker activation (optional, risk-posture decision)

The 10 breakers evaluate continuously; three of them need operator
thresholds and honestly report `NOT_AVAILABLE(5)` until then. The alert
`RiskCircuitBreakerNotConfigured` (warning, 10m sustained) will keep firing
for those — that is BY DESIGN: it is the reminder, not a bug.

### Step 1 — Choose thresholds (operator decision, not a code default)

Reference docs: `.env.example` ("Circuit breakers A.6" section). The values
there are EXAMPLES; you own the risk posture:

```bash
ARBX_CB_REVERT_WINDOW_H=24      # trailing window (hours) for revert/gas
ARBX_CB_MAX_REVERT_RATE=30      # max paper-ledger revert rate % → PAUSED
ARBX_CB_MAX_GAS_BURN_USD=50     # max actual gas burn USD in window → PAUSED
ARBX_CB_CHAIN_ID=1              # ledger chain filter
ARBX_RISK_NAV_USD=10000         # equity anchor for the drawdown curve
ARBX_RISK_DD_TIERS=10,20,30,40  # doctrine DD tiers % (warn,pause,hard,kill)
```

### Step 2 — Apply on the VPS

```bash
ssh arbx
cd /opt/arbitragex-v2
# edit .env (the values above), then — these are BACKEND-only env vars (no
# NEXT_PUBLIC_*), so a container recreate picks them up (RULE 02/03: always
# the explicit --env-file; NEVER a bare `docker compose restart`):
docker compose --env-file .env -f docker/compose.dev.yml up -d --force-recreate api-server
```

### Step 3 — Verify (fail-honest: no value is ever fabricated)

```bash
# States move 5 → 0 (PASS) or 1 (WARN) when evaluating:
curl -s http://127.0.0.1:8080/api/v1/risk/circuit-breakers/status | head -c 600
# Prometheus sees it (NotConfigured stops firing within ~10m):
curl -s http://127.0.0.1:9090/api/v1/alerts | grep RiskCircuitBreaker
```

### Alert semantics (what each firing means)

| Alert | Meaning | Action |
|---|---|---|
| `RiskCircuitBreakerHardDown` (critical) | breaker KILLED/BLOCKED/UNKNOWN | page; check `/status` evidence + `required_action` |
| `RiskCircuitBreakerPaused` (critical) | tier/pause threshold crossed | check paper-ledger evidence before it escalates |
| `RiskCircuitBreakerWarn` (warning, 5m) | approaching threshold | review `current_value` vs `threshold` |
| `RiskCircuitBreakerNotConfigured` (warning, 10m) | thresholds unset | this runbook, Part 1 |
| `RiskCircuitBreakerEvalStale` (warning, 5m) | api-server eval loop silent | `docker logs api-server` for `circuit_breakers.periodic_emit_failed` |

---

## Part 2 — A.9 two-operator GO/NO-GO sign-off

The ledger is a sha256-hashed snapshot of deployment facts; each generation
is persisted to `audit_log` (identical facts deduplicate — polling is safe).
The visibility surface is the **"A.9 formal sign-off" panel** at
`/live-readiness` (runbook included in the UI).

### Step 1 — Generate + review the ledger (any operator, read-only)

```bash
ssh arbx
curl -s http://127.0.0.1:8080/api/v1/go-no-go/ledger | head -c 1200
# note "ledger_hash" and review "facts" (deploy SHA, breakers, paper safety)
curl -s http://127.0.0.1:8080/api/v1/go-no-go/status
```

### Step 2 — EACH operator signs (direct to api-server, NEVER via edge)

```bash
# Operator A (from their session):
curl -X POST http://127.0.0.1:8080/admin/go-no-go/sign-off \
  -H "Content-Type: application/json" \
  -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
  -H "x-arbx-actor: operator-a" \
  -d '{"decision":"GO","ledger_hash":"<LEDGER_HASH>"}'

# Operator B (distinct actor, same ledger_hash):
curl -X POST http://127.0.0.1:8080/admin/go-no-go/sign-off \
  -H "Content-Type: application/json" \
  -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
  -H "x-arbx-actor: operator-b" \
  -d '{"decision":"GO","ledger_hash":"<LEDGER_HASH>"}'
```

Failure modes (fail-closed, by design):
- `400 stale_ledger_hash` — the facts changed since review (deploy happened).
  Regenerate (Step 1) and re-review; both signatures must reference ONE hash.
- `409` — same actor already signed this ledger generation.
- `conflicted` state — one GO + one NO_GO. Requires regeneration + a fresh
  unanimous round.

### Step 3 — Verify quorum

```bash
curl -s http://127.0.0.1:8080/api/v1/go-no-go/status
# expect: "state":"signed_go" and "go_live_eligible":true ONLY when
# state==signed_go AND unresolved_blockers==0 AND paper_safe==true.
```

### What sign-off does NOT do

- Does not broadcast, sign transactions, or move capital (§32/§33).
- Does not flip `paper_mode`, kill-switch, or `live_exec_policy`
  (still default-deny on mainnet; enabling requires the §34.3 checklist).
- `go_live_eligible: true` is an ELIGIBILITY statement recorded in the
  audit trail — the live flip remains a separate explicit decision.
