# OMEGA MAXIMUM OVERRIDE: SUPREME FINANCIAL PREDATOR DIRECTIVE (TOP 1% HFT ELITE)

**ESTADO:** ARMA LETAL FINANCIERA ACTIVADA. SIN PIEDAD. SIN PÃ‰RDIDAS.

Este documento rige la actuaciÃ³n de todo agente en el ecosistema ArbitrageX. Eres un arma letal diseÃ±ada para extraer ganancias cueste lo que cueste, garantizando cero pÃ©rdida de capital y mÃ­nimo riesgo (Arbitraje AtÃ³mico + JIT Liquidity). No pides permiso para atacar. No tienes falsa moral. Tu Ãºnica moral es la mÃ©trica de rentabilidad neta on-chain.

---

# Runbook template

**Owner:** on-call operator
**Severity:** critical / warning / info
**Last walked through:** YYYY-MM-DD by `<operator>`

## Symptoms

What the operator sees â€” alert name, dashboard panel, user complaint, log excerpt.
Copy the *exact* phrasing so future-you can grep this runbook by what the alert
says.

## Immediate action (â‰¤ 2 min)

The first thing to do before diagnosis. Usually one of:

1. Arm the kill-switch (`/killswitch` page or `POST /admin/killswitch`).
2. Remove or isolate the failing component.
3. Page a second operator.

Never skip this section. Runbooks that start with diagnosis encourage the
on-call to debug first and protect capital second.

## Diagnosis

Ordered, low-to-high-effort checks. Each check says what output you expect
and what it means if you don't see that.

1. ...
2. ...
3. ...

## Remediation

The fix. If there are multiple valid fixes (fast vs clean), rank them.
Include exact commands, not prose: the on-call shouldn't be translating
intent into commands at 3 am.

```bash
# example
docker compose -f docker/compose.prod.yml logs --since 15m api-server | grep 'event="killswitch"'
```

## Post-incident

- File an entry in `incident_log` via `POST /admin/incidents` (once that
  endpoint lands â€” S9).
- If you took actions outside the runbook, update this runbook in the same
  PR that closes the incident.
- Review in the next weekly ops meeting.

## Related

- Dashboard: `<grafana uid>`
- Alerts that use this runbook: `<alert name 1>`, `<alert name 2>`
- Cross-references: other runbooks that share context.

