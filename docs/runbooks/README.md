# ArbitrageX v2 — Operator runbooks

Each runbook answers: *"this is the alert / symptom I see. What do I do in the
next two minutes, and then in the next hour?"*

Every runbook follows `_template.md` and lives next to it. Pull requests that
change behaviour affecting an alert MUST update the corresponding runbook in
the same PR.

## Index — by alert or symptom

| Symptom / alert | Runbook | Severity |
|-----------------|---------|----------|
| `KillSwitchActivated` — platform is armed | [killswitch-activated.md](./killswitch-activated.md) | warning |
| `NoOpportunitiesDetectedLongWindow` — detection idle | [rpc-down.md](./rpc-down.md) | warning → critical |
| `HighHTTP5xxRate{service="relays-client"}` — relay errors | [relay-degraded.md](./relay-degraded.md) | warning |
| `RelaySubmitFailuresSpiking` (planned) | [relay-degraded.md](./relay-degraded.md) | warning |
| DB corruption or lost data | [db-restore.md](./db-restore.md) | critical |
| Planned secret rotation or suspected leak | [rotate-secrets.md](./rotate-secrets.md) | info / critical |
| Vault sealed — services won't boot | [vault-sealed.md](./vault-sealed.md) | critical |

## Discovery path

When a page arrives, the routing goes:

```
Slack / PagerDuty alert
   │  contains `alertname`
   ▼
grep docs/runbooks/*.md for the alertname
   │  or consult this index
   ▼
open the runbook, follow "Immediate action" first
```

## Rules

1. **Immediate action must never require diagnosis.** If the first step
   assumes the on-call understands the system, rewrite it.
2. **Every step says what output you expect.** Never "run X and see if it
   works" — "run X; you should see `scanner.subscribed` in ≤ 30 s. If not,
   go to step Y".
3. **Remediation is commands, not prose.** At 3 am, nobody translates intent
   into shell.
4. **Every runbook ends with a Post-incident section.** This is how we close
   the loop — audit log entries, incident writeups, PRs that prevent the
   recurrence.
5. **Walk-throughs are mandatory.** The `Last walked through: YYYY-MM-DD`
   header exists so this stays real. If a runbook hasn't been exercised in
   three months, the quarterly ops review should flag it.
