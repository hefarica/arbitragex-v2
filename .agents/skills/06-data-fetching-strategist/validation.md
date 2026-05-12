# Validación y Auditoría

## 1. Criterios de Validación
- Revisar la Network Tab. Si el componente usa Server Components, NO debe haber requests XHR/Fetch visibles hacia el propio dominio cargando JSON inicial (la data debe venir ya incrustada en el HTML/RSC payload).
- En caso de Client fetching, la tabla de requests debe mostrar reintentos (Retries) ordenados y con backoff exponencial, y nunca peticiones canceladas o huérfanas en estado `pending` por más de 5 segundos.

## 2. Cómo Auditar en ARBITRAGEX
- Buscar en el backend de Node/NextJS llamadas `fetch(process.env.NEXT_PUBLIC_EDGE_URL)` hechas dentro de Server Components (`page.tsx`). Si es un recurso externo (Edge API), está permitido. Si es un recurso interno (`/api/readiness` local), debe refactorizarse a la llamada del servicio directamente.
