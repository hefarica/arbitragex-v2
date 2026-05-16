# Incident Runbook

**OMEGA-102 Delta Pack · 2026-05-16**
**Owner:** @hefarica
**On-call expected response:** < 15 min during operator-active windows; best-effort otherwise.
**Companion docs:** `ROTATION_POLICY.md`, `REPO_VISIBILITY_POLICY.md`, `OPERATOR_RUNBOOK.md`, `SOP_ENTERPRISE.md`

## How to use this runbook

When an alert fires or you suspect something is wrong:

1. **Triage first** — identify the incident class (INC-01 to INC-10 below).
2. **Apply the playbook** for that class — each one has Detect / Contain / Investigate / Remediate / Postmortem steps.
3. **Tribunal at the end** — every incident closes with an R7 + R8 tribunal review (rigor + fail-honest). No incident is "resolved" until both pass.

If the incident does not match any class, default to **INC-10 (unknown)** which routes to a generic containment + escalation workflow.

## Killswitch — single action that pauses everything

The killswitch is the highest-priority operational action. It instantly halts:

- searcher-rs (no further opportunities scored or broadcast)
- api-server (rejects writes, serves reads only)
- edge worker (returns 503 on POST endpoints)

```bash
# Local trigger (operator workstation, requires VPS SSH access):
ssh arbx 'echo "{\"reason\":\"<your reason>\",\"ts\":\"'"$(date -Iseconds)"'\"}" > /opt/arbitragex-v2/killswitch.json'

# Verify within 5 seconds — all services should report killswitch=true on /status:
curl -fsS https://<edge-url>/api/status | jq '.killswitch'
```

The killswitch state is signed via HMAC (key in `KILLSWITCH_HMAC_KEY` env) and any service that fails to verify the signature treats the killswitch as **engaged** (fail-closed). This is intentional: a missing or unverifiable killswitch must NOT permit operation.

## Incident classes

### INC-01 · Production loss of capital suspected (signer compromise / unauthorized tx)

| | |
|---|---|
| **Severity** | P0 — pager-grade |
| **Trigger** | Unexpected tx from signer address; balance delta unexplained by recon; alert from on-chain monitor |
| **Detect** | `recon` job emits `unexplained_delta` event; on-chain monitor pages |
| **Contain** | (1) Engage killswitch (see above). (2) Revoke signer permissions on any token approvals: run `cast call <signer> 'allowance(...)'` and zero out. (3) Move uncompromised funds to cold custody. |
| **Investigate** | Pull tx history for signer, correlate with operator workstation activity, check VPS audit log. |
| **Remediate** | Rotate signer key per `ROTATION_POLICY.md` (within 2h). Re-deploy executor contracts if compromise is contract-level. |
| **Postmortem** | Required within 72h. Update threat model in `docs/TRUST_POLICY.md`. |

### INC-02 · CI pipeline failing all green checks for > 2h

| | |
|---|---|
| **Severity** | P1 — blocks deploys |
| **Trigger** | All 14 required status checks red on `main`; or 4+ workflows red simultaneously on a PR. |
| **Contain** | Halt all merges to `main` (announce to operator). Do NOT bypass via `--admin`. |
| **Investigate** | (1) Compare last green commit. (2) `git log --since=<last-green-time>` and identify the breaking change. (3) Check provider status pages (GitHub Actions, Alchemy, Codecov). |
| **Remediate** | Revert the breaking commit if behavior is clearly regression, OR open a fix PR. Use `scripts/apply_omega102_fixes.sh`-style idempotent patches for systemic causes. |
| **Postmortem** | Required if root cause is human error in the operator workflow, not provider outage. |

### INC-03 · Production deploy failed mid-flight (cutover broken)

| | |
|---|---|
| **Severity** | P0 if production is degraded; P1 if rollback succeeded cleanly. |
| **Trigger** | `hardened-vps-deploy.yml` workflow fails after gate 4 (snapshot) but before gate 7 (cutover); smoke tests fail post-cutover. |
| **Contain** | The workflow's gate-7 logic should auto-rollback. Verify within 60s: `curl https://<edge>/api/status` returns the previous deployment's version. If it does NOT, manually trigger `scripts/rollback.sh`. |
| **Investigate** | Pull workflow logs, identify which gate failed, inspect VPS docker logs. |
| **Remediate** | Fix the underlying issue in a new PR. Re-deploy when green. Do NOT re-attempt the failed deploy without fixing the root cause. |
| **Postmortem** | Required. Update `DEPLOY_PRODUCTION.md` if the failure exposed a gate-design gap. |

### INC-04 · Leaked credential (PAT, SSH key, API key, signer)

| | |
|---|---|
| **Severity** | P0 if signer/SSH; P1 if read-only PAT/API |
| **Trigger** | Operator notices token in chat, log, screenshot, public PR comment. GitHub secret-scanning push protection blocks a commit. Third-party alert (Have-I-Been-Pwned, provider notification). |
| **Contain** | Revoke at provider within 5 minutes (do not stage a replacement first — revoke first, replace second). |
| **Investigate** | Audit logs for any usage between leak time and revocation. For PAT: `gh api user/audit-log`. For VPS: `journalctl --since=<leak-time>`. |
| **Remediate** | Generate replacement, dual-write per `ROTATION_POLICY.md`. |
| **Postmortem** | Required. If the leak was via this Claude Code session, propose a memory update + tool hardening (e.g., redact patterns in transcripts). |

### INC-05 · Repo visibility flipped to public/internal

| | |
|---|---|
| **Severity** | P0 |
| **Trigger** | `policy-check.yml` hourly run fails. Or operator notices via GitHub UI. |
| **Contain** | Flip back to private immediately: `gh repo edit hefarica/arbitragex-v2 --visibility private --accept-visibility-change-consequences`. |
| **Investigate** | Audit log: `gh api repos/hefarica/arbitragex-v2/events` — identify who flipped and when. Assume the repo was scraped during the public window. |
| **Remediate** | Rotate ALL secrets visible in repo or workflow logs from that window. Treat as INC-04 for each. |
| **Postmortem** | Required. Review `REPO_VISIBILITY_POLICY.md`. |

### INC-06 · Killswitch fails to engage

| | |
|---|---|
| **Severity** | P0 — direct safety violation |
| **Trigger** | Operator writes killswitch.json; services do NOT report `killswitch=true` within 5s. |
| **Contain** | Manual stop of services: `ssh arbx 'docker compose -f /opt/arbitragex-v2/docker-compose.prod.yml stop searcher-rs api-server'`. |
| **Investigate** | Check HMAC key consistency across services. Check file watcher health. |
| **Remediate** | Restore killswitch path. Re-verify with a drill. |
| **Postmortem** | Required. The killswitch is a load-bearing safety control. Any failure is a critical bug. |

### INC-07 · Database / Redis exposed publicly

| | |
|---|---|
| **Severity** | P0 |
| **Trigger** | Port scan or external researcher reports open 5432/6379. Cloud provider firewall alert. |
| **Contain** | Block external access immediately at the VPS firewall: `ssh arbx 'ufw deny 5432 && ufw deny 6379'`. |
| **Investigate** | Identify how exposure happened (misconfigured nginx, docker bind, security group). |
| **Remediate** | Rotate DB + Redis credentials (INC-04 procedure). Dump and review audit logs for unauthorized queries. |
| **Postmortem** | Required. |

### INC-08 · Mempool toxicity / sandwich anomaly (op-state)

| | |
|---|---|
| **Severity** | P1 — operational, not safety |
| **Trigger** | Searcher score variance spikes; observed slippage exceeds modelled bounds by > 3σ. |
| **Contain** | (Not the killswitch — that's overkill.) Toggle `ARBX_PAPER_TRADE=true` for the affected strategy. |
| **Investigate** | Pull recent opportunity logs, run `mev-inspect-py` over the affected block range. |
| **Remediate** | Tune detection thresholds; add to filter list per `docs/RISK_POLICY.md`. |
| **Postmortem** | Optional unless capital loss. |

### INC-09 · Branch protection bypassed

| | |
|---|---|
| **Severity** | P1 |
| **Trigger** | Commit on `main` that did NOT go through a PR. `--admin` flag observed in audit log. |
| **Contain** | Identify the commit, run `git revert` via a NEW PR (do NOT force-push). |
| **Investigate** | Audit log: who, why, what was the urgency. |
| **Remediate** | Re-verify branch protection settings via `scripts/setup_branch_protection.sh --enforce-required-checks`. |
| **Postmortem** | Required if the bypass introduced regressions; optional if it was a quick test that was reverted. The policy IS no bypasses. |

### INC-10 · Unknown / unclassified

| | |
|---|---|
| **Severity** | Default P1 until reclassified |
| **Contain** | Engage killswitch if any uncertainty about capital safety. |
| **Investigate** | Operator decides — there is no playbook. |
| **Remediate** | Whatever fits. |
| **Postmortem** | Required. The output of an INC-10 should always be (a) an actual root cause and (b) a new INC class added to this runbook so the next occurrence has a playbook. |

## Tribunal R7 + R8 — close every incident

Every incident closes with two reviews:

- **Tribunal R7 (rigor):** Was the root cause identified to engineering depth, or only to "we restarted it and it works now"? If the latter, the incident is NOT closed.
- **Tribunal R8 (fail-honest):** Were the operator's assumptions during the incident WRITTEN DOWN, and were any of them wrong? Failure to admit a wrong assumption is itself an INC-10.

Both tribunals are operator self-review. They are not bureaucratic theater — they are a forcing function to prevent the same incident from recurring under a different mask.
