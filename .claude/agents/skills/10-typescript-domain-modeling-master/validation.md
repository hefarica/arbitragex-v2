# Validación y Auditoría

## 1. Criterios de Validación
- Correr el compilador de TS: `npx tsc --noEmit`. No deben existir errores de type-check a nivel de todo el proyecto.
- Inyectar en el WebSocket un JSON malformado (Ej: En lugar de un `profit_usd: 12.5`, mandar `profit_usd: null`). La UI no debe reventar con excepción blanca; Zod debe interceptar, imprimir un Warning de Schema, y descartar el evento corrupto limpiamente.

## 2. Cómo Auditar
- Ejecutar un grep por ` as ` o `any`. Si aparecen `any` injustificados (ej. `(e: any)` en los catch), forzar su corrección a `(e: unknown)`.
