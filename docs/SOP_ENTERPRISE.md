# SOP Enterprise — ArbitrageX v2

## Objetivo

Operar un sistema de detección, validación, simulación y ejecución con disciplina de riesgo fuerte.

## Reglas no negociables

1. Ninguna oportunidad nueva pasa a ejecución sin simulación (S4+).
2. Ningún token nuevo se permite sin filtro de seguridad.
3. Ninguna estrategia degradada sigue activa por costumbre (learning loop desactiva).
4. Ningún endpoint RPC privado se expone al exterior.
5. Ningún resultado de PnL se reporta sin reconciliación real.
6. Honestidad: ningún endpoint devuelve datos sintetizados. Los caminos no implementados devuelven `501` con `requires[]` + `sprint`.

## Flujo obligatorio

Detectar → Validar → **Simular** → Score → Estructurar capital → Ejecutar → Reconciliar → Aprender.

**Simulación ocurre antes de Score** para ejecución. El scoring usa el resultado de sim.

## Kill-switch

Apagar automáticamente cuando ocurra cualquiera:

- latencia p95 > umbral definido
- pérdida neta por hora > umbral
- tasa de revert > umbral
- error de simulación encadenado > umbral
- relay health < umbral mínimo
- sospecha de leak de secreto (ver `configs/secrets.policy.md` §6)

Activación manual: `POST /admin/killswitch` con `X-ArbX-Admin-Token`.

## KPIs de gobierno

- `success_rate_24h`, `net_profit_usd_24h`, `gas_burned_usd_24h`
- `p95_latency_ms`, `p99_latency_ms`
- `simulation_pass_rate`, `execution_variance_pct`
- `relay_acceptance_rate`, `token_safety_rejection_rate`
- `killswitch_events_24h`, `circuit_breaker_trips_24h`

## Backup / Restore

### Backup (rutina; producido por S8 en automación)

```bash
docker compose -f docker/docker-compose.prod-like.yml exec -T postgres \
  pg_dump -U postgres --format=custom --no-owner --compress=9 arbitragex > \
  backups/arbitragex_$(date -u +%Y%m%dT%H%M%SZ).dump
```

- Almacenar cifrado (gpg/age), off-host. Retención: diarios 30 días, semanales 12 semanas, mensuales 12 meses.
- Verificación obligatoria: `pg_restore --list` sobre cada dump.

### Restore

```bash
# 1. Kill-switch ON y detener servicios aplicativos
curl -X POST http://localhost:8080/admin/killswitch \
  -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
  -H "content-type: application/json" \
  -d '{"enabled":true,"reason":"restore_in_progress"}'
./automation/scripts/rollback.sh

# 2. Restaurar
docker compose -f docker/docker-compose.prod-like.yml exec -T postgres \
  pg_restore -U postgres -d arbitragex --clean --if-exists < backups/<file>.dump

# 3. Re-aplicar migraciones por si el dump es viejo
./automation/scripts/migrate.sh

# 4. Validar
./automation/scripts/health-check.sh

# 5. Kill-switch OFF con explicación
# (sólo si la validación pasa)
```

## Runbook — incidente sospecha de ejecución adversarial

1. Kill-switch ON inmediato.
2. Crear fila en `incident_log` (status `investigating`).
3. Exportar últimos 15 min de logs vía Loki: `{service="relays-client"} |~ "submit"`.
4. Revisar `risk_events` por `event_type=circuit_breaker|blacklist_hit`.
5. Congelar snapshot DB (`pg_dump`) antes de cualquier cambio.
6. Postmortem en 72h; actualizar thresholds y `audit_log`.
