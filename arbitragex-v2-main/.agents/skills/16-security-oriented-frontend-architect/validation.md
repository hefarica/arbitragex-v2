# Validación y Auditoría

## 1. Criterios de Validación
- Auditar el bundle minificado de producción buscando regex de credenciales comunes (`DB_PASS`, `TOKEN`, `SECRET`). Si se encuentra el valor textual, hay un leak grave (Uso de NEXT_PUBLIC o importación en Client Component).
- Lanzar una herramienta de análisis de cabeceras (como Mozilla Observatory) al endpoint en vivo. Debe arrojar nota 'B' o superior (exigiendo cabeceras de seguridad activas).

## 2. Cómo Auditar en ARBITRAGEX
- Revisar `.env` y contrastar contra `docker-compose.dev.yml` y `Dockerfile`. Cualquier variable inyectada al frontend mediante `NEXT_PUBLIC_` debe ser estrictamente inofensiva y de dominio público (ej. URLs externas del WebSocket, IDs de analíticas de terceros).
