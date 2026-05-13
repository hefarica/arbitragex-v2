# Validación y Auditoría

## 1. Criterios de Validación
- Ejecutar `npm run build`. 
- Revisar la terminal. Las rutas operativas (`/opportunities`) deben estar marcadas con el ícono de "lambda" `λ` (Server-side renders at runtime) y **NUNCA** con un círculo hueco `○` (Static).

## 2. Cómo Auditar
- Inspeccionar si hay llamadas `fetch` en Server Components sin la propiedad `cache`. Next.js por defecto las hará estáticas.
