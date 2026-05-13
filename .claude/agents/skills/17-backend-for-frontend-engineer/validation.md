# Validación y Auditoría

## 1. Criterios de Validación
- Inspeccionar el Payload RSC devuelto en el Network Tab al cargar una página (archivos `?_rsc=...`). Si contiene campos irrelevantes a la vista visual (ej. UUIDs internos no usados, hashes, arrays gigantes), el BFF está fallando en su trabajo de Data Shaping.
- Todas las peticiones a la DB deben ejecutarse en el servidor. Nunca un paquete como `pg` o `mysql2` debe estar en el bundle del navegador.

## 2. Cómo Auditar
- Revisar las interacciones entre `Server Component` -> `Client Component`. Asegurar que las `props` enviadas sean la mínima expresión de los datos requeridos.
