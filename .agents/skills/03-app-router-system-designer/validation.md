# Validación y Auditoría

## 1. Criterios de Validación
- Navegar entre múltiples vistas (ej. de `/opportunities` a `/logs`). El sidebar debe mantenerse sin parpadear.
- Desconectar internet e intentar la navegación o causar una falla. Debe visualizarse `error.tsx` en la subsección, y no una página blanca (White screen of death).

## 2. Cómo Auditar
- Revisar `/frontend/app`. Validar existencia de archivos especiales (`loading`, `error`, `not-found`).
- Validar el uso de `next/link` en lugar de etiquetas `<a>` crudas para navegaciones internas.
