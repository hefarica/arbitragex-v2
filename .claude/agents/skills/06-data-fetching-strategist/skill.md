# Skill 06: Data Fetching Strategist

## 1. Propósito
Dominar los patrones de extracción de datos en arquitecturas Next.js App Router (React Server Components + Client Components). Entender la diferencia entre fetch nativo extendido por Next.js, librerías de cliente como React Query (TanStack Query) y llamadas directas a base de datos. Maximizar la velocidad, la re-usabilidad y evitar Waterfalls de solicitudes.

## 2. Aplicación directa en ARBITRAGEX
El sistema consulta constantemente la base de datos PostgreSQL para configuraciones, y a Redis para oportunidades MEV. La extracción del estado inicial debe hacerse a nivel de servidor, sin bloqueo, y la UI interactiva debe hidratarse o usar SWR/TanStack para la revalidación proactiva y el manejo del estado asíncrono.

## 3. Problemas que resuelve
- Sequential Waterfalls (Consultar A, esperar A, luego consultar B).
- Consultas redundantes en Server Components.
- Fugas de conexión a la base de datos (Pool exhaustion) por mal manejo de promesas.
- Exposición innecesaria de Endpoints API (Crear una ruta GET de Next.js solo para consumirla desde el propio servidor).
- Spinners de carga infinitos en conexiones inestables.

## 4. Reglas Inmutables
- En Server Components, **NUNCA** hagas fetch a tu propia API interna (Route Handler `/api/algo`) usando URLs absolutas. Importa la función de base de datos o lógica directamente.
- Agrupa las promesas independientes usando `Promise.all()` para resolverlas en paralelo (Evitar Waterfalls).
- Usa TanStack Query (React Query) en Client Components para datos que requieran paginación infinita, reintentos (retries) y re-enfoque de pestaña (refetchOnWindowFocus), NUNCA `useEffect` crudo para requests complejos.
- Los fetches de Next.js (`fetch`) en Server Components deduplican automáticamente llamadas idénticas dentro del mismo árbol de render. No tengas miedo de llamar `await getUser()` en el Layout y también en el Page.

## 5. Nivel de Madurez
Senior - Diseña tuberías de datos de un extremo a otro, minimizando la latencia.
