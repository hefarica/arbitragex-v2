# Pre-Execute Checklist (G-PEC-1)

**Doctrine:** `arbx-pre-execute-checklist`
**Gate:** G-PEC-1
**Version:** 1.0.1
**Last Updated:** 2026-05-26

---

## Purpose

This document defines the7 mandatory gates that MUST pass before any capital-flipping operation is executed. No bundle may be submitted to Flashbots/MEV relays until all gates are GREEN.

---

## The 7 Pre-Execute Gates

The following gates are checked in series before any execution:
- **G-RPC-1**: RPC Connectivity Verified
- **G-SIM-1**: Simulation Passed
- **G-NET-1**: Net Profit Gate
- **G-TOK-1**: Token Safety Screen (risk_limits, token_safety)
- **G-FL-1**: Flash Loan Discipline
- **G-RIS-1**: Risk Limits Enforced (risk_limits)
- **G-PAP-1**: Paper Mode Duration Met

### Gate 1: RPC Connectivity Verified

**Check:** At least3 RPC providers are healthy and responding.

```bash
curl -s http://localhost:8787/api/readiness | jq '.items[] | select(.id == "G-RPC-1")'
```

**Pass Criteria:** `status: "green"`

**Failure Action:** Do not execute. Wait for RPC recovery or add fallback providers.

---

### Gate 2: Simulation Passed

**Check:** The opportunity has been simulated via `sim-ctl` with `revm` or `eth_call+stateOverride`.

```bash
curl -s http://localhost:8787/api/readiness | jq '.items[] | select(.id == "G-SIM-1")'
```

**Pass Criteria:** `status: "green"` OR simulation result shows positive net profit after gas.

**Failure Action:** Do not execute. Re-simulate with updated state.

---

### Gate 3: Net Profit Gate

**Check:** Expected net profit > 3× gas cost after all deductions.

**Formula:**
```
net_profit = gross_profit - gas_cost - slippage_estimate - relay_fee - p_fail_penalty
```

**Pass Criteria:** `net_profit > 0` AND `net_profit >= 3 * gas_cost`

**Failure Action:** Skip opportunity. Do not execute.

---

### Gate 4: Token Safety Screen

**Check:** All tokens in the path have passed the safety screen (no honeypot, no excessive tax, not blacklisted).

```bash
curl -s http://localhost:8787/api/readiness | jq '.items[] | select(.id == "G-TOK-1")'
```

**Pass Criteria:** `status: "green"`

**Failure Action:** Skip opportunity. Add token to blacklist if suspicious.

---

### Gate 5: Flash Loan Discipline

**Check:** Flash loan callback adheres to the7 discipline rules (see `arbx-flash-loan-discipline`).

```bash
curl -s http://localhost:8787/api/readiness | jq '.items[] | select(.id == "G-FL-1")'
```

**Pass Criteria:** `status: "green"` OR manual review approved.

**Failure Action:** Do not execute. Review callback logic.

---

### Gate 6: Kill-Switch Disengaged

**Check:** Global kill-switch is NOT armed.

```bash
curl -s http://localhost:8787/api/killswitch/status
```

**Pass Criteria:** `enabled: false`

**Failure Action:** Do not execute. Disengage kill-switch via admin panel.

---

### Gate 7: Paper-Mode Duration Met

**Check:** Paper-mode has been running for≥7 days with continuous data accumulation.

```bash
curl -s http://localhost:8787/api/readiness | jq '.items[] | select(.id == "G-PAP-1")'
```

**Pass Criteria:** `status: "green"` OR operator override with documented risk acceptance.

**Failure Action:** Continue paper trading. Do not flip to live.

---

## Execution Protocol

1. **Pre-Flight:** Run all 7 gates via `/api/readiness` endpoint.
2. **Gate Evaluation:** All gates must be GREEN or have documented exceptions.
3. **Exception Log:** Any gate override must be logged in audit trail with operator justification.
4. **Final Check:** Re-verify kill-switch status immediately before bundle submission.

---

## Emergency Override

In exceptional circumstances, the operator may override a RED gate with:

1. Documented justification in audit log
2. Risk acceptance signed by operator-lead
3. Time-limited override (max 1 hour)

**Override Command:**
```bash
curl -X POST http://localhost:8787/admin/gate-override \
  -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
  -H "x-arbx-actor: operator-lead" \
  -d '{"gate": "G-XXX-1", "reason": "Documented justification", "duration_minutes": 60}'
```

---

**Document maintained by:** OMEGA CORTEX
**Next Review:** 2026-06-26
