---
name: arbx-fail-honest-runtime-status
description: Crear endpoints read-only que agregan estado real desde PG/Redis sin fabricar datos.
---
# arbx-fail-honest-runtime-status

## Purpose
Crear endpoints read-only que agregan estado operativo real (ej. `runtime-status`) desde PG/Redis sin fabricar datos, implementando estricta observancia de la doctrina R8 Fail-Honest.

## When to use
Cuando necesites crear o modificar endpoints en `api-server` que expongan el estado de las estrategias MEV (dex_arb, triangular_arb, flashloan_arb, liquidation) al frontend o Edge Worker.

## Inputs needed
- Archivo de ruta destino (ej. `backend/api-server/src/routes/strategy-runtime-status.ts`).
- Estructura JSON canónica del status esperada por el UI y Edge.

## Files usually touched
- `backend/api-server/src/routes/strategy-runtime-status.ts`
- `backend/api-server/src/index.ts`

## Commands
- `pnpm --filter api-server build`
- `ssh arbx "curl -s http://localhost:8080/api/v1/strategies/runtime-status?chain_id=1 | jq"`

## Safety rules
- R8 Fail-Honest: Null significa "dato no disponible", 0 significa "cero real evaluado". Nunca reemplaces null por 0.
- No uses hardcodes productivos.
- No uses Loki ni parsees logs inyectados. Usa DB transaccional o Redis cache asíncrono.
- Respeta puertos (api-server siempre es 8080 en privado).

## Verification steps
1. Levanta api-server localmente o en VPS.
2. Lanza un curl interno al puerto 8080 para validar el JSON.
3. Verifica que los conteos mostrados correspondan a registros reales en PostgreSQL y llaves en Redis con queries manuales.

## Failure modes
- Redis/Postgres caídos: El endpoint colapsa totalmente con 500 en vez de manejar `not_available` grácilmente por componente.
- Nulls transformados en ceros por "seguridad y complacencia visual de UI".

## Golden output
```json
{
  "source": { "postgres": "ok", "redis": "ok" },
  "strategies": [
    { "strategy_kind": "dex_arb", "candidates_1h": 0, "last_rejection_reason": null }
  ]
}
```

## Anti-patterns
- Crear variables estáticas `let status = { ... }` con fake data en el handler.
- Enmascarar errores críticos de BD devolviendo `[]` u objetos vacíos sin reportar `db_unavailable`.

## Example prompt
"Usa la skill arbx-fail-honest-runtime-status para implementar la ruta de status en api-server usando la query SQL real."
