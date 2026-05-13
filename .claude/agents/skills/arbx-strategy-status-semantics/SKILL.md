---
name: arbx-strategy-status-semantics
description: Interpretar correctamente estado de estrategias MEV sin usar lenguaje de error falso.
---
# arbx-strategy-status-semantics

## Purpose
Traducir, clasificar y exponer la semántica del estado de un motor MEV (dex, triangular, flashloan, liquidation) para que el frontend o dashboard refleje exactamente la realidad operacional.

## When to use
Cuando tengas que evaluar lógicas condicionales sobre los resultados generados en PG/Redis y mapearlos en el JSON hacia un "Dependency Status" explícito.

## Inputs needed
- Counts operacionales brutos desde DB.
- Estado booleano y comprobación física de existencias operacionales clave (e.g., watchlist arrays en Redis, caches llenas).

## Files usually touched
- `backend/api-server/src/routes/strategy-runtime-status.ts`
- `frontend/src/app/operations/page.tsx` (or components mappings)

## Commands
- N/A - Principalmente conceptual/arquitectural.

## Safety rules
- Flashloan con 0 candidatos porque no existe un base rentable se marca `waiting_for_profitable_base`, NO `failed` ni rojo.
- Liquidaciones sin activos monitoreados se marcan `missing_lending_watchlist`, NO `failed`.
- Triangular sin oportunidad momentánea se marca `armed_waiting_for_impact`.
- NUNCA inventes errores (rojo) si se trata de un escenario nominal de reposo del mercado.

## Verification steps
1. Redactar pruebas lógicas o someter data de ejemplo con conteo = 0.
2. Validar que la cadena final de texto resultante coincida exactamente con lo establecido y exponga honor a la inactividad natural.

## Failure modes
- Generar alarmismo en el Dashboard. El operador suspende las instancias asumiendo errores estructurales cuando en realidad el mercado no tenía liquidez a liquidar o ciclar.

## Golden output
```json
{
  "strategy_kind": "flashloan_arb",
  "candidates_1h": 0,
  "data_dependencies_status": "waiting_for_profitable_base"
}
```

## Anti-patterns
- Convertir una matriz de métricas en un `boolean isHealthy = true/false` matando toda la semántica intermedia.
- Devolver `engine_invoked=true` solo porque el servicio se instanció en PM2, omitiendo corroborar latidos operacionales.

## Example prompt
"Usa arbx-strategy-status-semantics para parsear el resultado de flashloans desde Postgres y asignarles 'waiting_for_profitable_base' si el count es idéntico a cero en lugar de devolver failure."
