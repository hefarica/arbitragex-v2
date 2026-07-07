# Validación y Auditoría

## 1. Criterios de Validación
- Auditar con Lighthouse en Chrome (Modo incógnito, Simulate Mobile). El Score de Performance debe ser mayor a 90.
- Correr `@next/bundle-analyzer` durante el build. Identificar cualquier chunk único de JavaScript que exceda los 200KB minificado.

## 2. Cómo Auditar en ARBITRAGEX
- Revisar `package.json`. Advertir y marcar reemplazos si se descubren dependencias prohibidas u obsoletas.
- Revisar el código fuente buscando componentes de diagramas o librerías pesadas, y exigir que su importación sea envuelta en `dynamic`.
