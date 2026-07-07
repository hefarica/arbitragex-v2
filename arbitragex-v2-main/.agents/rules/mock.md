---
trigger: always_on
---

# RULE: DOCTRINA ZERO MOCKS (PROHIBIDO USAR MOCKS)

## DEFINICIÓN DE LA REGLA

Está **ESTRICTAMENTE PROHIBIDO** inyectar, generar o servir datos falsos, hardcodeados, simulados o "decorativos" (mocks) en cualquier capa del sistema (Frontend, Backend, Base de Datos, Smart Contracts, etc.).

## ALCANCE

1. **Frontend**: Los componentes React deben renderizar exactamente lo que devuelve la API. Si el array de oportunidades o pools está vacío, se debe mostrar vacío o un estado de "Esperando datos de la red".
2. **Backend**: Los endpoints REST y WebSockets deben obtener su información **ÚNICAMENTE** de fuentes veraces (PostgreSQL, Redis, nodos RPC reales o Anvil forks).
3. **Manejo de Errores**: Si un servicio dependiente (ej. Redis, Postgres) está caído, el sistema debe fallar ruidosamente (Fail-Fast) o mostrar el estado degradado real. Nunca se debe ocultar el error retornando datos falsos.

## CONSECUENCIAS

Cualquier intento de "saltarse" esta regla para cumplir requisitos visuales será considerado una violación crítica de la arquitectura institucional de ArbitrageX. La verdad absoluta reside en la Blockchain y en la Base de Datos transaccional.
