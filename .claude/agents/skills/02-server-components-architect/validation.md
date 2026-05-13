# Validación y Auditoría

## 1. Criterios de Validación
- Inspeccionar el Network Tab y asegurar que la carga de Javascript minificado es mínima. Componentes declarados puramente como Server Components no deben aparecer en el Bundle JS del cliente.
- Todo código asíncrono que extrae datos a nivel de base de datos (`DATABASE_URL`) no debe fallar; si lo hace por "Module not found: 'pg'", significa que se filtró accidentalmente a un Client Component.

## 2. Cómo Auditar en ARBITRAGEX
- Verificar que `page.tsx` no tenga `use client` a menos que sea una vista exclusiva de dashboard dinámico completo. Si la vista es dinámica, evaluar si solo la tabla `LiveTable` podría ser `use client`.
