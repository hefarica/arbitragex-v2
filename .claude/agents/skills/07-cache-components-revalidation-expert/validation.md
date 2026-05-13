# Validación y Auditoría

## 1. Criterios de Validación
- Hacer una mutación (crear registro, actualizar configuración). Redireccionar o volver a la vista de lista. El cambio debe aparecer en menos de 50ms (por la regeneración inmediata en server).
- Al revisar los logs de acceso al VPS, una vista altamente cacheada no debe mostrar logs de consultas SQL (Postgres logs) por cada navegación del usuario, solo durante la expiración del caché (ISR).

## 2. Cómo Auditar
- Buscar Server Actions (`'use server'`). Cada mutación (INSERT, UPDATE, DELETE) en DB debe ser seguida de un `revalidatePath` o `revalidateTag` antes del `return`.
