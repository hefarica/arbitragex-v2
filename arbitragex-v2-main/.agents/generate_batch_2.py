import os

BASE_DIR = r"c:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\.agents\skills"

SKILLS = [
    {
        "dir": "06-data-fetching-strategist",
        "skill": r"""# Skill 06: Data Fetching Strategist

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
""",
        "checklist": r"""# Checklist Operativo: Data Fetching

- [ ] En Server Components: ¿Se usa acceso directo a la DB o Servicios en lugar de `fetch("http://localhost:3000/api/...")`?
- [ ] En componentes asíncronos complejos: ¿Se usa `Promise.all` para lanzar requests en paralelo si no dependen uno de otro?
- [ ] En Client Components: Si hay polling o estados complejos de request, ¿Se usa TanStack Query / SWR en lugar de `useEffect + useState`?
- [ ] En APIs de Terceros: ¿Se provee un `AbortSignal.timeout(X)` para evitar conexiones colgadas infinitamente?
""",
        "implementation-patterns": r"""# Patrones Correctos (Implementation)

## Patrón 1: Server-Side Parallel Data Fetching
Evita el Waterfall en vistas maestras (Ej: Dashboard).

```tsx
// 🟢 CORRECTO
import { db } from '@/lib/db';
import { getLiveMetrics, getUserConfig } from '@/lib/services';

export default async function DashboardPage() {
  // Lanzar en paralelo
  const metricsPromise = getLiveMetrics();
  const configPromise = getUserConfig();
  
  // Esperar a todas simultáneamente
  const [metrics, config] = await Promise.all([metricsPromise, configPromise]);

  return <DashboardView metrics={metrics} config={config} />;
}
```

## Patrón 2: Client-Side Data with TanStack Query
Manejo profesional del estado asíncrono en cliente.

```tsx
// 🟢 CORRECTO
'use client';
import { useQuery } from '@tanstack/react-query';

export function SystemHealth() {
  const { data, isLoading, error } = useQuery({
    queryKey: ['health'],
    queryFn: async () => {
      const res = await fetch('/api/readiness');
      if (!res.ok) throw new Error('Network error');
      return res.json();
    },
    refetchInterval: 5000, // Polling built-in
  });

  if (isLoading) return <Skeleton />;
  if (error) return <ErrorAlert error={error} />;
  return <HealthCard data={data} />;
}
```
""",
        "anti-patterns": r"""# Antipatrones Prohibidos

## Antipatrón 1: Fetching Local Route Handlers en Server Components
Next.js bloqueará o lanzará errores si el servidor intenta hacerse un fetch HTTP a sí mismo durante el Build Time.

```tsx
// 🔴 PROHIBIDO
export default async function Page() {
  // NO HAGAS ESTO. El servidor no debe consumir su propio HTTP.
  const res = await fetch("http://localhost:3000/api/config");
  const data = await res.json();
  return <View data={data} />;
}

// 🟢 CORRECTO
import { getConfig } from '@/lib/config';
export default async function Page() {
  // Llama directamente a la función lógica.
  const data = await getConfig(); 
  return <View data={data} />;
}
```

## Antipatrón 2: Data Fetching en un useEffect Crudo
Esto provoca race conditions, memory leaks si el componente se desmonta rápido, y nulo manejo de caché.

```tsx
// 🔴 PROHIBIDO (Si la data muta a menudo o es compleja)
useEffect(() => {
   let active = true;
   fetch('/api/data').then(res => res.json()).then(data => {
      if (active) setData(data);
   });
   return () => { active = false; }
}, []);
```
""",
        "validation": r"""# Validación y Auditoría

## 1. Criterios de Validación
- Revisar la Network Tab. Si el componente usa Server Components, NO debe haber requests XHR/Fetch visibles hacia el propio dominio cargando JSON inicial (la data debe venir ya incrustada en el HTML/RSC payload).
- En caso de Client fetching, la tabla de requests debe mostrar reintentos (Retries) ordenados y con backoff exponencial, y nunca peticiones canceladas o huérfanas en estado `pending` por más de 5 segundos.

## 2. Cómo Auditar en ARBITRAGEX
- Buscar en el backend de Node/NextJS llamadas `fetch(process.env.NEXT_PUBLIC_EDGE_URL)` hechas dentro de Server Components (`page.tsx`). Si es un recurso externo (Edge API), está permitido. Si es un recurso interno (`/api/readiness` local), debe refactorizarse a la llamada del servicio directamente.
""",
        "agent-prompt": r"""# Prompt de Agente: Data Fetching Strategist

```text
Actúa como Estratega de Data Fetching para Next.js App Router.
Analiza el archivo proveído e identifica:
1. Si existen Waterfalls (promesas secuenciales que podrían ser concurrentes usando Promise.all).
2. Si el Server Component está usando `fetch()` apuntando a una API REST propia local (ej. `/api/users`), y corrígelo invocando la función lógica directamente.
3. Si el Client Component está manejando polling o data fetching complejo usando `useEffect`, reescribe la lógica utilizando @tanstack/react-query para garantizar reintentos, caché y refetchOnWindowFocus de grado producción.
```
"""
    },
    {
        "dir": "07-cache-components-revalidation-expert",
        "skill": r"""# Skill 07: Cache Components & Revalidation Expert

## 1. Propósito
Dominar el ecosistema de Caché multicapa de Next.js (Request Memoization, Data Cache, Full Route Cache, Router Cache). Dominar las técnicas de invalidación (Revalidation) bajo demanda o por tiempo para garantizar coherencia atómica de datos vivos frente a datos semi-estáticos sin consumir recursos del VPS de forma innecesaria.

## 2. Aplicación directa en ARBITRAGEX
En el panel de configuración (Settings), las métricas del sistema histórico o perfiles operativos no necesitan ser `force-dynamic`. Se pueden cachear estáticamente por horas. Si el operador cambia un setting en el Edge o en Postgres, Next.js necesita purgar el caché estático usando `revalidatePath` o `revalidateTag` para que los componentes muestren el nuevo estado inmediatamente sin reinicios de servidor.

## 3. Problemas que resuelve
- Páginas que muestran datos fantasmas o viejos después de un CRUD o Mutación.
- Estrés innecesario en la BD por consultas estáticas masivas recurrentes.
- Tiempos de Build excesivos debido al caché infinito.
- Fallos de invalidación por uso incorrecto de etiquetas de caché (Tags).

## 4. Reglas Inmutables
- Para invalidar una caché programáticamente tras una acción de usuario (Server Action o API mutation), usa `revalidatePath('/ruta')` o `revalidateTag('collection')`.
- Todos los Server Actions que muten datos DEBEN finalizar con una instrucción de revalidación.
- Nunca dependas del caché del cliente (Router Cache) por más de 30 segundos en pantallas operativas (Next.js App Router lo hace por defecto para pre-fetching). Si requieres frescura, fuerza purga.

## 5. Nivel de Madurez
PhD - Control absoluto del motor de caché híbrido del servidor y el cliente.
""",
        "checklist": r"""# Checklist Operativo: Caching & Revalidation

- [ ] ¿Los fetch de datos semi-estáticos incluyen un array de tags para revalidación quirúrgica? (Ej: `next: { tags: ['settings'] }`).
- [ ] En las mutaciones (POST/PUT), ¿se invoca a `revalidatePath` o `revalidateTag` inmediatamente tras la confirmación de base de datos?
- [ ] En pantallas dinámicas, ¿está explícitamente desactivada la caché de red (`cache: 'no-store'`)?
""",
        "implementation-patterns": r"""# Patrones Correctos (Implementation)

## Patrón 1: Tag-based Revalidation
```tsx
// En el Server Component
export async function getSystemSettings() {
  const res = await fetch('https://api/settings', {
    next: { tags: ['system-settings'] } // Etiqueta de caché
  });
  return res.json();
}

// En el Server Action (Mutación)
'use server';
import { revalidateTag } from 'next/cache';

export async function updateSystemSettings(formData: FormData) {
  await db.updateSettings(formData);
  // Purgar inmediatamente la caché de todos los componentes que usen esta etiqueta
  revalidateTag('system-settings');
  return { success: true };
}
```
""",
        "anti-patterns": r"""# Antipatrones Prohibidos

## Antipatrón 1: Mutation sin Revalidación
Un CRUD que no purga la caché obliga al usuario a usar `Ctrl+F5` o dudar de si la acción tuvo éxito.

```tsx
// 🔴 PROHIBIDO
'use server';
export async function updateProfile(data) {
  await db.updateUser(data);
  // Falta revalidatePath('/profile')
  // La UI seguirá mostrando el perfil viejo de manera infinita.
}
```

## Antipatrón 2: Cachear lo Incacheable
Poner ISR (ej. `revalidate: 60`) en endpoints financieros o endpoints que emiten nonces/tokens de seguridad (Anti-CSRF).
""",
        "validation": r"""# Validación y Auditoría

## 1. Criterios de Validación
- Hacer una mutación (crear registro, actualizar configuración). Redireccionar o volver a la vista de lista. El cambio debe aparecer en menos de 50ms (por la regeneración inmediata en server).
- Al revisar los logs de acceso al VPS, una vista altamente cacheada no debe mostrar logs de consultas SQL (Postgres logs) por cada navegación del usuario, solo durante la expiración del caché (ISR).

## 2. Cómo Auditar
- Buscar Server Actions (`'use server'`). Cada mutación (INSERT, UPDATE, DELETE) en DB debe ser seguida de un `revalidatePath` o `revalidateTag` antes del `return`.
""",
        "agent-prompt": r"""# Prompt de Agente: Cache & Revalidation Expert

```text
Eres un experto en Next.js Caching Architecture.
Analiza este código de Server Action o Route Handler (Mutación de datos).
Asegúrate de que contenga lógica de revalidación.
Si la mutación altera la base de datos o estado global, inyecta `revalidatePath('/ruta-afectada')` de `next/cache` justo después del éxito de la mutación.
Si el endpoint tiene una directiva de fetch de lectura, recomienda el uso de `{ next: { tags: ['mi-entidad'] } }` para permitir invalidaciones quirúrgicas.
```
"""
    },
    {
        "dir": "08-real-time-ui-performance-engineer",
        "skill": r"""# Skill 08: Real-Time UI Performance Engineer

## 1. Propósito
Evitar la degradación de FPS, re-renders masivos y fugas de memoria en interfaces que reciben ráfagas extremas de actualizaciones por WebSockets (High-Frequency UI Updates). Dominar la memoización (`React.memo`, `useMemo`), la virtualización del DOM y el control de la cadencia de estado (Throttling/Debouncing visual).

## 2. Aplicación directa en ARBITRAGEX
El feed en vivo de oportunidades de MEV puede emitir ráfagas si el mempool se satura. Renderizar una tabla con 1000 filas parpadeando cada 200ms congelará el DOM del navegador. Se debe utilizar renderización incremental o list virtualization (TanStack Virtual), así como memoización de celdas y filas, y desvincular los timers puros del Main Thread de renderización de React.

## 3. Problemas que resuelve
- Congelamiento del navegador / Scroll lag debido a sobrecarga del Main Thread.
- Basura excesiva en Memoria (Garbage Collector pausas largas).
- Re-render de toda la tabla cuando solo una fila o una celda cambia.
- `Maximum update depth exceeded` por ciclos reactivos infinitos.

## 4. Reglas Inmutables
- En listas de más de 100 elementos que sufren re-ordenamiento o actualizaciones rápidas, **es obligatorio el uso de Virtualización** (`@tanstack/react-virtual`).
- Las filas de una tabla compleja deben estar aisladas en un sub-componente envuelto en `React.memo`, recibiendo props primitivas o estabilizadas por `useCallback`.
- Para métricas que cambian a alta frecuencia (ej. Ticker de "Age Seconds"), NO guardes ese dato en el Provider Global ni en el objeto de la lista, haz que la propia Fila administre su timer local o use animaciones CSS si es posible, para no forzar re-renders del root parent.

## 5. Nivel de Madurez
Maestría - Arquitectura HFT (High-Frequency Trading) Frontend.
""",
        "checklist": r"""# Checklist Operativo: Real-Time UI Performance

- [ ] ¿El componente de la tabla o feed de oportunidades tiene un límite máximo duro? (ej. `slice(0, 50)`).
- [ ] ¿Las celdas pesadas (gráficos, modales asociados) están envueltas en `React.memo` con comparación estricta de props?
- [ ] ¿Se utiliza CSS Transforms o animaciones opacas para efectos de parpadeo ("Pulse") en lugar de mutar clases mediante React Hooks en cada cuadro?
- [ ] ¿Los arrays recibidos por el WebSocket se despachan en baches (Batches) mediante un worker o se hacen throttle en el reducer del estado si exceden un threshold seguro?
""",
        "implementation-patterns": r"""# Patrones Correctos (Implementation)

## Patrón 1: Fila Memoizada Pura
Evita que toda la tabla se repinte cuando entra una nueva oportunidad, solo se pinta la fila nueva.

```tsx
import { memo } from 'react';

// Subcomponente independiente
const OpportunityRow = memo(({ item, onAction }) => {
  // Solo se re-renderiza si `item` cambia en memoria.
  return (
    <tr>
       <td>{item.pair}</td>
       <td>{item.profit}</td>
       <td><button onClick={() => onAction(item.id)}>Action</button></td>
    </tr>
  );
}, (prev, next) => prev.item.id === next.item.id && prev.item.profit === next.item.profit);

// Componente Padre
export function OppTable({ items }) {
  // onAction DEBE estar memoizado
  const handleAction = useCallback((id) => executeOp(id), []);
  
  return (
    <tbody>
      {items.map(i => <OpportunityRow key={i.id} item={i} onAction={handleAction} />)}
    </tbody>
  )
}
```

## Patrón 2: Time-slicing o Throttle de WS
Si el Socket escupe 500 ops/seg, limita la UI a 5 frames por segundo (cada 200ms).

```ts
// En el servicio WebSocket, empuja a un buffer y actualiza la UI a intervalos.
let buffer = [];
socket.on('data', (d) => buffer.push(d));

setInterval(() => {
  if (buffer.length > 0) {
    flushBufferToReactState(buffer);
    buffer = [];
  }
}, 200); // 5 FPS (Fluidly readable, zero lag)
```
""",
        "anti-patterns": r"""# Antipatrones Prohibidos

## Antipatrón 1: Funciones anónimas en listas masivas
Si pasas `onClick={() => execute(id)}` en una lista gigante, la Fila no se puede memoizar porque en cada render del padre, se genera una función en memoria diferente, rompiendo `React.memo`.

## Antipatrón 2: Timer Global Masivo
Tener un `useEffect` con `setInterval` en la página principal que hace un `setNow(Date.now())` y forzar que TODO el árbol de React, incluyendo todos los tooltips, modales y charts, se recalcule cada 1000 milisegundos.
""",
        "validation": r"""# Validación y Auditoría

## 1. Criterios de Validación
- Habilitar la opción "Highlight updates when components render" en las herramientas React DevTools Profiler.
- Observar el WebSocket funcionando en vivo. Solo deben destellar de color verde las filas específicas que reciben actualización, NO el contenedor entero ni los subcomponentes adyacentes.

## 2. Cómo Auditar
- Inyectar 10,000 eventos sintéticos por WebSocket en 2 segundos.
- La memoria JS no debe crecer más de 50MB ni arrojar picos insalvables de Main Thread Blocking largos en el Profiler del navegador.
""",
        "agent-prompt": r"""# Prompt de Agente: Real-Time UI Performance Engineer

```text
Actúa como Ingeniero de Performance en UI HFT.
Revisa el siguiente código de tabla / lista reactiva a un WebSocket.
Busca y corrige:
1. Asegura que el componente Fila (`Row`) sea abstraído de su lista y envuelto en `React.memo`.
2. Asegura que cualquier función callback (`onClick`) provista desde la Lista a la Fila esté envuelta en `useCallback` y no sea anónima inline.
3. Si la lista no tiene límite de corte visible, inyecta lógica en el dispatcher/setter que proteja el array de sobrepasar 100 elementos (`array.slice(0, 100)`).
```
"""
    },
    {
        "dir": "09-state-management-architect",
        "skill": r"""# Skill 09: State Management Architect

## 1. Propósito
Diseñar y segregar los sistemas de estado en React: Server State (Data cache y Server Component payloads), Client State Global (Context, Zustand, Pinia), URL State (Query Params y Router) y Local Component State (useState, useReducer).

## 2. Aplicación directa en ARBITRAGEX
El token de autenticación y los permisos operativos, el flag de "Live vs Polling", las preferencias visuales del dashboard y los filtros de la tabla de oportunidades. 

## 3. Problemas que resuelve
- "Prop drilling" extremo (pasar props 5 niveles abajo).
- Renderizados globales masivos por poner estado rápido y frecuente (ej. animaciones o texto ingresado) en el Provider global.
- Desincronización: Guardar datos asíncronos en estado de UI (Ej. hacer un global Store para guardar JSONs de base de datos en lugar de usar TanStack Query).
- Fricción de compartición de URLs (Copiar la URL y enviarla a otro usuario, y que se pierdan los filtros aplicados).

## 4. Reglas Inmutables
- **URL First:** Todo estado de filtro, sort, paginación o búsqueda, DEBE ser un parámetro URL (`?search=ETH&page=2`). Nunca un estado local `useState`, para poder copiar y pegar links de estados específicos y retener el History del navegador.
- **Server Data in Server Context:** No copies datos provenientes de la API o la Base de datos en un Store global de Zustand a menos que los vayas a transformar altamente offline. Usa TanStack Query o Next.js RSC payload para eso.
- **Zustand/Jotai > Context:** Para estados globales del cliente altamente interactivos (Preferencias, toggles globales, temas, flags de WebSockets), prefiere Zustand (o Jotai) antes que React Context, por sus optimizaciones de render y selectores moleculares.

## 5. Nivel de Madurez
Senior / Architect - Garantiza modularidad y evita monopolios de estado.
""",
        "checklist": r"""# Checklist Operativo: State Management

- [ ] ¿Los filtros de búsquedas (Ej: Filter by Chain) modifican la URL o solo un estado local volátil? (Deben modificar la URL `useSearchParams` / `router.replace`).
- [ ] ¿El estado persistente (Ej. Tema Oscuro) lee desde `localStorage` de forma SSR-safe (después del mount)?
- [ ] ¿Se evita el antipatrón de duplicar los props pasados desde Server Components guardándolos forzosamente en un estado inicial global?
""",
        "implementation-patterns": r"""# Patrones Correctos (Implementation)

## Patrón 1: URL as State for Filters (App Router)
```tsx
'use client';
import { useRouter, usePathname, useSearchParams } from 'next/navigation';

export function SearchFilter() {
  const router = useRouter();
  const pathname = usePathname();
  const searchParams = useSearchParams();

  const handleSearch = (term: string) => {
    const params = new URLSearchParams(searchParams.toString());
    if (term) {
      params.set('q', term);
    } else {
      params.delete('q');
    }
    // Preserva estado nativo y compartible
    router.replace(`${pathname}?${params.toString()}`);
  }

  return <input onChange={(e) => handleSearch(e.target.value)} defaultValue={searchParams.get('q') || ''} />;
}
```

## Patrón 2: Zustand para Preferencias de UI / Tokens Live
```ts
import { create } from 'zustand';

interface UIStore {
  compactMode: boolean;
  toggleCompact: () => void;
}

export const useUIStore = create<UIStore>((set) => ({
  compactMode: false,
  toggleCompact: () => set((state) => ({ compactMode: !state.compactMode })),
}));
```
""",
        "anti-patterns": r"""# Antipatrones Prohibidos

## Antipatrón 1: React Context "God Mode"
Crear un único `AppProvider` gigantesco que contiene `user`, `theme`, `wsConnection`, `tableData`, `isModalOpen`. Cuando `isModalOpen` cambie, todo el DOM que dependa del contexto re-renderizará forzosamente.

## Antipatrón 2: Redundant API caching in Zustand
Recibir un objeto de DB gigante de un fetch y meterlo entero a un store de Zustand.
""",
        "validation": r"""# Validación y Auditoría

## 1. Criterios de Validación
- Filtrar la tabla del dashboard principal, ordenar las columnas por ROI y actualizar la página con `F5`. Los filtros y el ordenamiento deben preservarse (URL State o LocalStorage persist).
- Observar el React Profiler cuando se hace click en el botón "Toggle Theme" o "Toggle Compact View". Únicamente deben repintarse los componentes suscritos a esos modificadores estéticos, no la data entera.

## 2. Cómo Auditar
- Identificar en `/frontend` instancias de `createContext`. Evaluar si es justificable. Recomendar cambiar a parámetros de URL o Zustand atomizado si se descubre alto tráfico de re-renders.
""",
        "agent-prompt": r"""# Prompt de Agente: State Management Architect

```text
Actúa como State Management Architect en React/Next.
Audita la estructura de estado del siguiente componente o página.
Requerimientos:
1. Si encuentras filtros o paginación (`const [page, setPage] = useState(1)`), refactoriza el código para usar `useSearchParams` y alterar la URL usando `router.replace` o `router.push`.
2. Si existe un React Context que está siendo abusado para valores de frecuente mutación y valores estáticos (causando render penalty), destrúyelo y propon Zustand o atomiza el Context.
3. Asegura que los Server Components pasen los datos puramente mediante props.
```
"""
    },
    {
        "dir": "10-typescript-domain-modeling-master",
        "skill": r"""# Skill 10: TypeScript Domain Modeling Master

## 1. Propósito
Blindar la aplicación a través de tipado estricto y aserciones en el dominio lógico. Eliminar `any` y `unknown` peligrosos, crear contratos precisos mediante tipos de unión discriminada (Discriminated Unions) para manejos de errores, flujos y eventos de red (WebSocket Payloads), garantizando la integración continua E2E entre Node y Rust.

## 2. Aplicación directa en ARBITRAGEX
El pipeline entre el mempool (`scanner.rs`), la base de datos (Postgres), y la interfaz requiere contratos estrictos. Las "Opportunities" emitidas por el `PrioritizationSpine` hacia Redis y despachadas al navegador deben poseer un tipado exacto (ej. `profit_usd`, `gas_cost`, `risk_score`) sincronizado y validadas con Zod.

## 3. Problemas que resuelve
- Errores de "undefined is not an object" en Runtime de Producción.
- APIs que cambian silenciosamente sin que el frontend lo detecte, causando UI rotas (White screens).
- Tipados mentirosos (Type assertions `as MyType` que engañan al compilador).
- Manejo crudo de errores asíncronos (`try/catch` sin tipar el objeto Error atrapado).

## 4. Reglas Inmutables
- **Prohibido el uso de `any`**. Reemplazar siempre por `unknown` si el dato de entrada es dinámico, y validarlo.
- Usar **Uniones Discriminadas** para Respuestas de Red o Eventos de WebSocket en lugar de objetos gigantes opcionales (`{ type: "SUCCESS" | "ERROR", payload?: Data, error?: string }`).
- Los contratos críticos del lado del cliente (APIs, Edge payloads) deben ser protegidos mediante validadores en tiempo de ejecución (ej. **Zod** o Valibot). Si el payload del motor de Rust cambia de forma, Zod debe escupir un log detallado y la UI debe procesar el error "gracefully", en lugar de fallar en una prop del componente.
- Abstraer el dominio (Entidades: `Opportunity`, `NodeStatus`, `Alert`) en archivos globales `/types` o schemas.

## 5. Nivel de Madurez
Maestría - Type-Driven Development y End-to-End Type Safety.
""",
        "checklist": r"""# Checklist Operativo: TS Domain Modeling

- [ ] ¿Existe la directiva `strict: true` y `noImplicitAny: true` en el `tsconfig.json` productivo?
- [ ] ¿Los payloads provenientes de las respuestas HTTP/Fetch se parsean a través de `schema.safeParse` (Zod) antes de tratarlos como el Tipo final?
- [ ] ¿Se eliminaron todos los type castings peligrosos (`response.json() as Opportunity[]`) reemplazándolos con guardias de tipo estables?
- [ ] ¿El catálogo de eventos del WebSocket es una Unión Discriminada estricta y predecible?
""",
        "implementation-patterns": r"""# Patrones Correctos (Implementation)

## Patrón 1: Zod Schema as the Source of Truth
Validación runtime fuerte.

```tsx
import { z } from 'zod';

export const OpportunitySchema = z.object({
  id: z.string().uuid(),
  pair_symbol: z.string(),
  dex_a: z.string(),
  dex_b: z.string(),
  expected_profit_usd: z.number().or(z.string().transform(val => parseFloat(val))),
  risk_score: z.number().min(0).max(100),
  detected_at: z.string().datetime(),
});

// El tipo se infiere del validador. No se duplica código.
export type Opportunity = z.infer<typeof OpportunitySchema>;

// En el Fetcher:
const data = await res.json();
const parsed = OpportunitySchema.array().safeParse(data);
if (!parsed.success) {
  throw new Error("El motor Rust escupió un JSON con estructura corrompida.");
}
return parsed.data;
```

## Patrón 2: Discriminated Unions para Result/Errores (Rust-Like en TS)
```ts
export type Result<T> = 
  | { ok: true; data: T }
  | { ok: false; error: string; code: string };

export async function fetchLiveNodes(): Promise<Result<Node[]>> {
  try {
     // logica
     return { ok: true, data: nodos };
  } catch(e) {
     return { ok: false, error: (e as Error).message, code: "NET_FAIL" };
  }
}
```
""",
        "anti-patterns": r"""# Antipatrones Prohibidos

## Antipatrón 1: Type Assertion Ciego
El infame `as Type` que apaga el detector de mentiras del compilador.

```ts
// 🔴 PROHIBIDO
const res = await fetch('/api/data');
const json = await res.json() as MiEntidadFuerte; // TS confía a ciegas, si el JSON es diferente, explota en Runtime en la UI.
```

## Antipatrón 2: Interfaces Gigantes con todo Opcional
```ts
// 🔴 PROHIBIDO
interface WSPayload {
  type: string; // Puede ser cualquier cosa
  data?: Opportunity;
  errorMsg?: string;
  heartbeatTime?: number;
}
// Esto obliga al consumidor a revisar qué existe y qué no de manera no segura.
```
""",
        "validation": r"""# Validación y Auditoría

## 1. Criterios de Validación
- Correr el compilador de TS: `npx tsc --noEmit`. No deben existir errores de type-check a nivel de todo el proyecto.
- Inyectar en el WebSocket un JSON malformado (Ej: En lugar de un `profit_usd: 12.5`, mandar `profit_usd: null`). La UI no debe reventar con excepción blanca; Zod debe interceptar, imprimir un Warning de Schema, y descartar el evento corrupto limpiamente.

## 2. Cómo Auditar
- Ejecutar un grep por ` as ` o `any`. Si aparecen `any` injustificados (ej. `(e: any)` en los catch), forzar su corrección a `(e: unknown)`.
""",
        "agent-prompt": r"""# Prompt de Agente: TS Domain Modeling Master

```text
Eres el Auditor de TypeScript estricto.
Analiza la definición de tipos, interfaces y llamadas de fetch en este código.
Tu tarea es erradicar la inseguridad estática:
1. Reemplaza todos los `any` por tipos específicos o por `unknown` combinado con aserciones/verificaciones estables.
2. Evalúa si los datos recuperados a través de la red (`.json()`) se están creyendo ciegamente mediante un type-cast (`as Type`). En tal caso, genera e inyecta un validador con Zod para hacer `safeParse`.
3. Convierte las promesas que fallan de manera ruidosa en patrones `Result<T>` con uniones discriminadas (`ok: true | false`) al estilo Rust para manejo de errores declarativos en los componentes de UI.
```
"""
    }
]

for item in SKILLS:
    skill_dir = os.path.join(BASE_DIR, item["dir"])
    os.makedirs(skill_dir, exist_ok=True)
    
    with open(os.path.join(skill_dir, "skill.md"), "w", encoding="utf-8") as f:
        f.write(item["skill"])
    with open(os.path.join(skill_dir, "checklist.md"), "w", encoding="utf-8") as f:
        f.write(item["checklist"])
    with open(os.path.join(skill_dir, "implementation-patterns.md"), "w", encoding="utf-8") as f:
        f.write(item["implementation-patterns"])
    with open(os.path.join(skill_dir, "anti-patterns.md"), "w", encoding="utf-8") as f:
        f.write(item["anti-patterns"])
    with open(os.path.join(skill_dir, "validation.md"), "w", encoding="utf-8") as f:
        f.write(item["validation"])
    with open(os.path.join(skill_dir, "agent-prompt.md"), "w", encoding="utf-8") as f:
        f.write(item["agent-prompt"])

print("Batch 2 generated successfully!")
