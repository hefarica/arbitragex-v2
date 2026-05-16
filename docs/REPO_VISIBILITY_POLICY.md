# Repo Visibility Policy

**OMEGA-102 Delta Pack · 2026-05-16**
**Status:** REQUIRED reading for every operator + reviewer.
**Enforcement:** `.github/workflows/policy-check.yml` runs hourly and fails if violated.

## Rule

The `hefarica/arbitragex-v2` repository **MUST be private** at all times. No exceptions, no temporary public toggles, no "I'll flip it back in a minute".

```bash
# Verify (anyone can run this):
gh repo view hefarica/arbitragex-v2 --json visibility
# expected: {"visibility":"private"}
```

If the visibility is anything other than `private`, this is incident class **INC-09** per `docs/INCIDENT_RUNBOOK.md`. Treat it like a production breach.

## Why

The repository contains:

1. **Production-leaning MEV strategy code** (`backend/searcher-rs`, `backend/sed-core`, `backend/prioritization-spine`) — surfaces the exact heuristics this operator uses to decide whether a topological asymmetry resolves into a yield-positive trajectory. Public exposure leaks alpha to competing searchers in real time, who replay against the same mempool.
2. **Operator deployment recipes** (`scripts/`, `automation/`, `docs/OPERATOR_RUNBOOK.md`) — describe wallet structure, key custody, RPC endpoints, killswitch wiring. Public exposure provides an attacker the operational map.
3. **Risk policy + threat model** (`docs/RISK_POLICY.md`, `docs/TRUST_POLICY.md`) — knowing the defender's known-unknowns is a footing for targeted attack.
4. **Audit artifacts + incident postmortems** (`audits/`, `docs/auditoria/`) — historical vulnerabilities, including ones believed remediated. Surface for replay attacks against forked deployments or other operator setups with similar architecture.

Public exposure is asymmetric: attackers gain a lot, the operator gains nothing.

## Privatize procedure (Fase 0 of OMEGA-102 rollout)

```bash
gh repo edit hefarica/arbitragex-v2 --visibility private \
  --accept-visibility-change-consequences
```

After privatization:

```bash
# Enable secret scanning + push protection (only available on private repos
# under the appropriate plan, OR on public + advanced security).
gh api -X PATCH repos/hefarica/arbitragex-v2 \
  -f security_and_analysis[secret_scanning][status]=enabled \
  -f security_and_analysis[secret_scanning_push_protection][status]=enabled

# Verify
gh api repos/hefarica/arbitragex-v2 --jq '.security_and_analysis'
```

## Forks

Forks of a private repo:
- Are themselves private by default
- Cannot be made public by the forker
- Are still subject to the parent's permission model

**However**: a collaborator with sufficient permissions on a private fork CAN expose code by other means (copy-paste, screenshot, downloaded ZIP). Visibility policy is necessary but not sufficient. Operator must additionally maintain `docs/TRUST_POLICY.md` to govern who has fork access.

## Monitoring

| Signal | Detector | Action |
|---|---|---|
| Visibility flipped to `public` | `policy-check.yml` (hourly) | Page operator immediately. Flip back. Investigate via audit log (`gh api repos/hefarica/arbitragex-v2/events`). |
| Visibility flipped to `internal` (enterprise only) | `policy-check.yml` | Same as above — `internal` is NOT `private`. |
| Fork created by unknown account | GitHub fork notification | Review `docs/TRUST_POLICY.md`; if forker is not on allowlist, revoke any shared org access. |
| Repo deleted | GitHub email alert | Recovery has a 90-day window via GitHub support. Restore + rotate all secrets that may have leaked via fork. |

## Exceptions

There are no exceptions. If you believe an exception is warranted (e.g., publishing a stripped-down public reference impl), the procedure is:

1. Create a NEW public repository (e.g., `hefarica/arbitragex-public-reference`)
2. Cherry-pick only the sanitized subset there
3. Never mirror or otherwise link `hefarica/arbitragex-v2` directly to the public surface

This policy supersedes any other documentation in the repo that may suggest otherwise.
