---
name: arbx-pg-redis-observability
description: Consultar PostgreSQL y Redis con seguridad operativa para métricas runtime.
---
# arbx-pg-redis-observability

## Purpose
Consultar y agregar datos operativos en tiempo real iterando eficientemente PostgreSQL (queries agregadas) y Redis (uso de SCAN), asegurando la salud del servidor bajo carga y honrando los estados nulos reales.

## When to use
Cuando el backend requiera derivar estados en el instante preciso de estrategias, inventarios, o conteos, leyéndolos directamente desde memoria y base transaccional.

## Inputs needed
- Sentencia SQL con intervalos dinámicos.
- Patrones de llaves de caché esperadas (e.g., `arbx:pool_reserves:1:*`).
- Punteros a las instancias activas de Pool y Redis.

## Files usually touched
- `backend/api-server/src/routes/*.ts`

## Commands
- Validación cruzada en bash sobre el VPS:
  `ssh arbx "docker exec -i arbitragex-v2-redis-1 redis-cli XREVRANGE arbx:opps:detected + - COUNT 5"`

## Safety rules
- ABSOLUTAMENTE PROHIBIDO el uso del comando `KEYS *` en Redis. Obligatorio utilizar flujos `SCAN` iterativos con cursores.
- Consultas PostgreSQL deben ser inyectadas de forma paramétrica `($1, $2)`.
- Si las consultas opcionales o Redis fallan/timeouts, debe capturarse la excepción y el objeto de la respuesta debe marcarse como `not_available`.

## Verification steps
1. Prueba directa en psql para la agregación de tiempos.
2. Comprobación del ciclo repetitivo de cursores (SCAN) con logs para iteraciones abundantes en Redis.
3. Validación de fallos desconectando simuladamente Redis para confirmar el comportamiento degradado del endpoint.

## Failure modes
- Bloqueo en Event-Loop de Redis por invocar un `KEYS` que recorre millones de nodos.
- Table scans completos en PostgreSQL debido a falta de índices en columnas relacionales de tiempo (e.g. `detected_at`).

## Golden output
```typescript
async function countRedisKeys(redis: Redis, pattern: string, scanCount = 500): Promise<number> {
  let cursor = "0";
  let total = 0;
  do {
    const [next, keys] = await redis.scan(cursor, "MATCH", pattern, "COUNT", scanCount);
    cursor = next;
    total += keys.length;
  } while (cursor !== "0");
  return total;
}
```

## Anti-patterns
- Solicitar todas las filas a SQL (`SELECT * FROM table`) y realizar conteo en memoria de Node (`.length`).
- Manipular o truncar cursores de Redis prematuramente omitiendo paginación.

## Example prompt
"Con arbx-pg-redis-observability, escribe una función para hacer SCAN de Redis seguro para contar dependencias y sumar el query en Postgres."
