# Validación y Auditoría

## 1. Criterios de Validación
- Lanzar una excepción forzada explícita `throw new Error("Test Alarm")` en un Client Component.
- Abrir Grafana (o la terminal del backend). El error, incluyendo el navegador, URL, Correlation ID y stack trace (idealmente decodificado), debe estar registrado en menos de 5 segundos.

## 2. Cómo Auditar
- Buscar archivos `error.tsx` en el repositorio. Si carecen de una exportación de logs asíncrona hacia el backend (`logger.error` o similar), marcar como "Blind spot".
- Auditar la gestión de `.catch()` en todas las promesas de red.
