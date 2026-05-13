# Checklist Operativo: Backend for Frontend

- [ ] ¿Los Route Handlers (`app/api/`) o Server Actions transforman/agrupan las respuestas crudas del backend antes de enviarlas al cliente (Evitando Over-fetching)?
- [ ] ¿Se eliminó la capa intermedia redundante (Ej: crear una API Next para luego hacer `fetch('/api')` en el mismo `page.tsx` de Next) invocando directamente los servicios de BD?
- [ ] ¿Se manejan de forma centralizada las cookies de sesión/autorización en el Edge o en un middleware de Next.js (`middleware.ts`)?
