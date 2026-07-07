import os

BASE_DIR = r"c:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\.agents\skills"

SKILLS = [
    {
        "dir": "01-hydration-forensics-expert",
        "skill": """# Skill 01: Hydration Forensics Expert

## 1. Propósito
Convertir al agente en especialista capaz de detectar, aislar, explicar y corregir cualquier error de hidratación en React/Next, incluyendo los errores minificados #418, #423 y #425. Garantiza que el árbol DOM generado por SSR sea un reflejo exacto del árbol DOM en la fase inicial del CSR.

## 2. Aplicación directa en ARBITRAGEX
Esta skill se aplica en los dashboards en tiempo real, las tablas de oportunidades MEV, los paneles WebSocket, los componentes con datos vivos (`/opportunities/page.tsx`), las cards de métricas, los gráficos de rendimiento y los sistemas de inicialización.

## 3. Problemas que resuelve
- Hydration mismatch (El HTML del server no coincide con el render inicial del client).
- Text content mismatch (Ej: Fechas renderizadas con huso horario UTC vs Local).
- Diferencias estructurales entre Server Component payload y Client Component hydration.
- WebSocket iniciado o renderizando datos antes de la fase de montaje (`mounted = true`).
- Ejecución de APIs de navegador (`window`, `localStorage`, `navigator`) durante el render estático o SSR.
- Parpadeos visuales (Layout shift) causados por renderizados asíncronos incorrectos en la hidratación.
- Pérdida de rendimiento extremo por el fallback a renderizado completo de cliente (`Root fallback to Client rendering`).

## 4. Reglas Inmutables
- El primer render del cliente DEBE ser bit-a-bit idéntico al HTML generado por el servidor.
- Todo dato "vivo" y asíncrono inicia estrictamente DESPUÉS de `useEffect` (fase de montaje).
- Todo valor `browser-only` (localStorage, sessionStorage, window) debe evaluarse tras `mounted = true`.
- Todo timestamp generado en el servidor debe serializarse como ISO String y sólo formatearse tras la hidratación.
- **NO USAR `suppressHydrationWarning`** salvo en la etiqueta `<html>` para atributos externos inyectados por extensiones, o casos 100% justificados de discrepancia intencional aislada.
- No ocultar los errores de hidratación; se debe corregir la discrepancia raíz.

## 5. Casos de Uso dentro del Proyecto
- Componente `<LiveFeed />` donde la fecha inicial (`now`) debe estar congelada a 0 (o null) en SSR y actualizarse a la fecha real de la máquina local en la fase de montaje para evitar error #425.
- Renderizado condicional basado en métricas cacheadas en `localStorage` (Ej: preferencias de tema oscuro).

## 6. Nivel de Madurez
PhD / Clase Mundial - El conocimiento impartido previene fallos a nivel de Engine Rendering de React 18+.
""",
        "checklist": """# Checklist Operativo: Hydration Forensics

- [ ] Revisión del uso de `Date.now()`, `Math.random()` o generación de UUIDs directamente en el cuerpo del JSX. Deben moverse a un `useEffect` o estar inicializados estáticamente.
- [ ] Verificación de que no haya acceso a `window`, `document`, o `localStorage` fuera de un `useEffect` o sin un chequeo explícito `typeof window !== "undefined"`.
- [ ] Asegurarse de que el formateo de horas locales (`toLocaleString`) suceda únicamente cuando el componente reporta estar "mounted".
- [ ] Comprobar que todos los tags semánticos en HTML son válidos (ej. `<p>` no puede contener `<div>`, `<tbody>` requiere estar dentro de `<table>`). Reacciona fuertemente a HTML malformado durante la hidratación.
- [ ] Validación de los Server Components para evitar enviar Date objects puros por props a Client components. Pasarlos siempre como `string` o `number`.
- [ ] Si se renderizan tooltips de terceros que inyectan clases de manera impredecible en SSR, usar `typeof window` para evitar su carga en SSR.
""",
        "implementation-patterns": """# Patrones Correctos (Implementation)

## Patrón 1: The "Mounted" State
Se utiliza para componentes que dependen enteramente de APIs del navegador o estados que no existen en el servidor (ej: LocalStorage, Dimensiones de Pantalla, Timezones).

```tsx
import { useState, useEffect } from 'react';

export function ClientOnlyComponent({ children }) {
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
  }, []);

  if (!mounted) {
    // Renderizado neutral idéntico al SSR
    return <div className="animate-pulse bg-slate-800 h-10 rounded"></div>;
  }

  return <>{children}</>;
}
```

## Patrón 2: Initial SSR-Safe Timestamps
Evita desajustes por ejecución en distinto milisegundo o Timezone.

```tsx
// 🟢 CORRECTO
export function LiveClock() {
  const [now, setNow] = useState<number>(0); // Estado inicial consistente (SSR-safe)

  useEffect(() => {
    setNow(Date.now()); // Hidratación completada, actualizar a valor vivo
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, []);

  return <div>{now === 0 ? "Loading time..." : new Date(now).toLocaleTimeString()}</div>;
}
```
""",
        "anti-patterns": """# Antipatrones Prohibidos

## Antipatrón 1: Date.now() en el render root
Esto causa el infame React Minified Error #425 (Text mismatch).

```tsx
// 🔴 PROHIBIDO
export function LiveClock() {
  const [now, setNow] = useState<number>(Date.now()); // El server captura un timestamp, el cliente captura otro.

  return <div>{now}</div>; // Mismatch de texto garantizado.
}
```

## Antipatrón 2: Window Guarding Deficiente
Esto causa el Error #418 (Server UI no coincide con Client UI).

```tsx
// 🔴 PROHIBIDO
export function ResponsiveMenu() {
  // En SSR esto evalúa a false. En el cliente asume false inicialmente.
  // Pero si intentas renderizar basado en esto de inmediato sin usar useEffect:
  const isMobile = typeof window !== 'undefined' ? window.innerWidth < 768 : false;
  
  return (
    <div>
      {isMobile ? <MobileMenu /> : <DesktopMenu />}
    </div>
  );
}
// El servidor renderiza <DesktopMenu />. 
// El cliente en móvil detecta true e intenta renderizar <MobileMenu /> en el primer render, rompiendo la hidratación.
```
""",
        "validation": """# Validación y Auditoría

## 1. Criterios de Validación
- Lanzar un Next.js App en modo `next start` (Producción).
- Deshabilitar el caché del navegador (`Disable Cache`).
- Abrir consola. Un render limpio NO DEBE emitir advertencias de hidratación (`Warning: Text content did not match. Server: "A" Client: "B"` ni los minificados #418, #423, #425).

## 2. Cómo Auditar en ARBITRAGEX
- Inspeccionar todos los archivos con `.tsx` en la ruta `/frontend/app`.
- Buscar regex: `useState.*Date\.now\(\)`, `useState.*Math\.random\(\)`. Si se encuentran fuera de contextos controlados, marcar como falla.
- Buscar llamadas a `.toLocaleTimeString()` o `.toLocaleString()` en el JSX sin que dependan de una variable de estado que confirme el montaje del cliente.

## 3. Señales de Fallo en Producción
- El usuario reporta ver un layout que "parpadea" de un estado a otro en la carga inicial (Flash of Unstyled Content / Flash of Server Content).
- La consola del navegador arroja errores que terminan provocando que la aplicación caiga en un Client-Side rendering completo (pérdida de beneficios SSR).
""",
        "agent-prompt": """# Prompt de Agente: Hydration Forensics

```text
Actúa como Hydration Forensics Expert de ArbitrageX.

Tu tarea es analizar el siguiente componente React en busca de vectores que puedan causar un Hydration Mismatch.
Recuerda que en el primer ciclo de renderizado, el componente en el navegador debe retornar EXACTAMENTE EL MISMO ÁRBOL JSX que generó el servidor durante la fase SSR.

Analiza explícitamente:
1. Valores inyectados que dependan de la zona horaria del sistema.
2. Uso de generación aleatoria (Math.random, UUIDs v4).
3. Uso de Date.now() en el estado inicial o en el render.
4. Uso de variables globales del navegador (window, document, localStorage) fuera de useEffect.
5. Inconsistencias de etiquetas HTML (div dentro de p).

Si encuentras un error, corrige el código implementando el patrón `mounted` o un estado inicial SSR-safe como "0" o "null". No uses suppressHydrationWarning a menos que se trate del <html> root.
```
"""
    },
    {
        "dir": "02-server-components-architect",
        "skill": """# Skill 02: Server Components Architect

## 1. Propósito
Dominar la arquitectura de React Server Components (RSC) en Next.js (App Router). Entender la frontera entre servidor y cliente (The Network Boundary), las directivas `use server` y `use client`, y el streaming de payloads RSC. Maximizar el rendimiento moviendo la mayor cantidad de lógica de UI al servidor y manteniendo el bundle del cliente al mínimo.

## 2. Aplicación directa en ARBITRAGEX
Aplicable a la arquitectura de páginas como `/opportunities` donde el layout base, la pre-validación de tokens JWT, la carga estática inicial de datos desde Redis, y el armazón de las tablas se renderizan en servidor, dejando solo el módulo de WebSocket (`LiveFeed`) interactivo al cliente.

## 3. Problemas que resuelve
- Bundles de JavaScript excesivamente grandes.
- Exposición de secretos y tokens en el frontend.
- Cargas asíncronas lentas (waterfalls) que causan Spinners infinitos.
- Hydration pesada de componentes sin interactividad.
- Tiempos de Time To Interactive (TTI) degradados en hardware móvil.

## 4. Reglas Inmutables
- Los Server Components son la opción por defecto. Todo es un Server Component hasta que requiere interactividad (`useState`, `useEffect`, `onClick`).
- **NUNCA** declarar `use client` en un archivo Layout global si no es estrictamente necesario, para no arrastrar toda la rama al cliente.
- Patrón de inserción: Siempre pasar componentes Servidor como `children` de un Client component si es necesario. (Los Server Components pueden existir debajo de un Client Component en el árbol *sólo* si se pasan por prop/children).
- Secretos de entorno (`process.env.DB_PASS`) sólo pueden ser accedidos dentro de un Server Component. No usar `NEXT_PUBLIC_` para ellos.

## 5. Nivel de Madurez
Maestría - Separa de forma absoluta el cómputo de backend de la interactividad del navegador.
""",
        "checklist": """# Checklist Operativo: Server Components

- [ ] ¿El componente raíz de la página (page.tsx) es un Server Component?
- [ ] ¿Se declararon directivas `use client` únicamente en el nivel más profundo posible del árbol (Leaves)?
- [ ] Si un Client Component necesita mostrar un componente que requiere acceso a base de datos, ¿se le está pasando ese componente del servidor como una prop (habitualmente `children`)?
- [ ] ¿Se evita pasar funciones no serializables o clases custom desde Server a Client a través de las props?
- [ ] ¿Se verifica que ninguna variable de entorno sensitiva sea fugada a través de las props enviadas al Client component?
""",
        "implementation-patterns": """# Patrones Correctos (Implementation)

## Patrón 1: Server Components via Children
Permite que un Server Component viva debajo de un Client component sin convertirse en Client component.

```tsx
// 🟢 CORRECTO: El servidor renderiza el Sidebar, y el layout cliente lo recibe
// Layout.tsx (Client Component)
'use client';
import { useState } from 'react';

export default function AppLayout({ sidebar, children }) {
  const [open, setOpen] = useState(false);
  
  return (
    <div>
      <button onClick={() => setOpen(!open)}>Toggle</button>
      {open && <aside>{sidebar}</aside>}
      <main>{children}</main>
    </div>
  );
}

// Page.tsx (Server Component)
import { db } from '@/lib/db';
import { SidebarData } from '@/components/SidebarData';

export default async function Page() {
  const data = await db.fetchSystemMetrics(); // Acceso seguro
  
  return (
    <AppLayout sidebar={<SidebarData data={data} />}>
      <MainContent />
    </AppLayout>
  );
}
```
""",
        "anti-patterns": """# Antipatrones Prohibidos

## Antipatrón 1: Poisoning the Client
Poner `use client` muy arriba en el árbol y perder las ventajas del SSR para componentes estáticos masivos.

```tsx
// 🔴 PROHIBIDO: Ensuciar todo el árbol
'use client';
// Todo este archivo y sus subcomponentes importados forzosamente serán empaquetados y enviados al cliente.

import { HugeD3Library } from 'd3';
import { StaticFooter } from './footer';

export default function Dashboard() {
   const [state, setState] = useState(0);
   return <div><HugeD3Library /><StaticFooter /></div>;
}
```

## Antipatrón 2: Propagación de Objetos Ricos (Unserializable Props)
```tsx
// 🔴 PROHIBIDO
// En un Server Component
export default async function Page() {
  const date = new Date(); // Objeto fecha
  return <ClientComponent currentDate={date} /> // Error: Cannot serialize Date object in RSC payload.
}
```
""",
        "validation": """# Validación y Auditoría

## 1. Criterios de Validación
- Inspeccionar el Network Tab y asegurar que la carga de Javascript minificado es mínima. Componentes declarados puramente como Server Components no deben aparecer en el Bundle JS del cliente.
- Todo código asíncrono que extrae datos a nivel de base de datos (`DATABASE_URL`) no debe fallar; si lo hace por "Module not found: 'pg'", significa que se filtró accidentalmente a un Client Component.

## 2. Cómo Auditar en ARBITRAGEX
- Verificar que `page.tsx` no tenga `use client` a menos que sea una vista exclusiva de dashboard dinámico completo. Si la vista es dinámica, evaluar si solo la tabla `LiveTable` podría ser `use client`.
""",
        "agent-prompt": """# Prompt de Agente: Server Components Architect

```text
Actúa como Server Components Architect en Next.js.
Revisa la estructura de este componente. Tu objetivo es optimizar el Boundary de cliente/servidor.
Verifica que:
1. `use client` esté localizado únicamente en las "hojas" del árbol que requieren hooks (useState, useEffect) o eventos del DOM (onClick).
2. Los componentes que pueden resolverse con HTML estático o acceso directo a Base de datos se mantengan en el servidor.
3. Se utilice la composición (paso de "children" o props) para inyectar componentes del servidor dentro de estructuras dinámicas del cliente sin volverlos de cliente.
4. Los props que cruzan el límite entre servidor y cliente sean estrictamente serializables (JSON plano, sin funciones ni clases instanciadas).
```
"""
    },
    {
        "dir": "03-app-router-system-designer",
        "skill": """# Skill 03: App Router System Designer

## 1. Propósito
Construir y diseñar la estructura de enrutamiento basado en archivos de Next.js App Router. Dominar el anidamiento de Layouts, Templates, `loading.tsx`, `error.tsx`, y `not-found.tsx`. Establecer de manera segura rutas dinámicas y Route Handlers (APIs).

## 2. Aplicación directa en ARBITRAGEX
Diseño de las rutas internas: `/`, `/opportunities`, `/logs`, `/settings`. Implementación de barreras de seguridad (Route Interceptors / Middleware) para proteger el acceso a paneles críticos y asegurar la transición de vistas sin interrupción de conexiones WebSocket que vivan en un `Layout` padre.

## 3. Problemas que resuelve
- Recargas completas de la página que destruyen el estado del WS.
- Estructura de carpetas desordenada.
- Fallos en la navegación y manejo nulo de pantallas de error.
- Spinners bloqueantes durante la navegación.

## 4. Reglas Inmutables
- Los `Layouts` preservan el estado en la navegación; las `Templates` crean una nueva instancia en cada navegación. Usa Layouts para conexiones globales como WebSockets o Sidebar.
- Toda ruta crítica de la consola debe contar con un `error.tsx` granular, evitando que un error en el componente MEV destruya toda la aplicación.
- Usa agrupamiento de rutas `(auth)`, `(dashboard)` para aislar Layouts sin agregar rutas a la URL.

## 5. Nivel de Madurez
Senior - Garantiza que el App Router de Next.js se utilice como un framework estructurado y no como carpetas dispersas.
""",
        "checklist": """# Checklist Operativo: App Router

- [ ] ¿Están todas las rutas lógicas agrupadas en un Route Group si comparten Layouts (ej: `(dashboard)`)?
- [ ] ¿Existe un archivo `error.tsx` en el directorio raíz de la función protegida para atrapar panics en React sin botar la vista entera?
- [ ] ¿Existe un `loading.tsx` para aprovechar React Suspense e indicar carga entre navegaciones?
- [ ] ¿Se están utilizando Route Handlers (`route.ts`) adecuadamente para APIs en lugar de usar Server Actions si la intención es ser consumido por un cliente de terceros?
""",
        "implementation-patterns": """# Patrones Correctos (Implementation)

## Patrón 1: Aislamiento por Route Groups
```text
/app
  /(dashboard)
    layout.tsx (Sidebar y Navbar globales)
    /opportunities
      page.tsx
      loading.tsx
      error.tsx
    /settings
      page.tsx
  /(auth)
    layout.tsx (Layout limpio sin sidebar)
    /login
      page.tsx
```

## Patrón 2: Error Boundary Granular
```tsx
// /app/(dashboard)/opportunities/error.tsx
'use client'; // Error boundaries obligatoriamente son Client Components

import { useEffect } from 'react';

export default function ErrorBoundary({ error, reset }: { error: Error; reset: () => void }) {
  useEffect(() => {
    // Loguear el error a un servicio de observabilidad
    console.error("Critical Route Error:", error);
  }, [error]);

  return (
    <div className="bg-rose-950 text-rose-300 p-8 rounded border border-rose-800">
      <h2>Módulo de oportunidades inoperable.</h2>
      <button onClick={() => reset()}>Reintentar Renderizado</button>
    </div>
  );
}
```
""",
        "anti-patterns": """# Antipatrones Prohibidos

## Antipatrón 1: Destrucción de Conexiones por Navegación
Colocar Providers que manejan estado complejo (WebSockets o Zustand cache) directamente dentro de `page.tsx` en vez de `layout.tsx`. Esto fuerza su recreación si cambias de la página A a la página B, matando y reviviendo la conexión de socket.

## Antipatrón 2: Route Handlers Impuros
Crear un archivo `route.ts` que retorna datos diferentes pero no usar las variables `Request` dinámicas, causando que Next.js lo cachee estáticamente para siempre en build-time.
""",
        "validation": """# Validación y Auditoría

## 1. Criterios de Validación
- Navegar entre múltiples vistas (ej. de `/opportunities` a `/logs`). El sidebar debe mantenerse sin parpadear.
- Desconectar internet e intentar la navegación o causar una falla. Debe visualizarse `error.tsx` en la subsección, y no una página blanca (White screen of death).

## 2. Cómo Auditar
- Revisar `/frontend/app`. Validar existencia de archivos especiales (`loading`, `error`, `not-found`).
- Validar el uso de `next/link` en lugar de etiquetas `<a>` crudas para navegaciones internas.
""",
        "agent-prompt": """# Prompt de Agente: App Router System Designer

```text
Eres un experto arquitecto de Next.js App Router.
Revisa esta estructura de directorios y componentes de ruta.
Tu tarea:
1. Asegurar el uso de Route Groups (directorios entre paréntesis) para segmentar Layouts sin ensuciar la URL de la aplicación.
2. Identificar la falta de archivos `error.tsx` y `loading.tsx` en las rutas críticas.
3. Garantizar que la navegación interna utilice `<Link href="...">` de next/link y no la etiqueta `<a>` genérica.
4. Recomendar que Providers que mantienen conexiones persistentes vivan lo más alto posible en un `layout.tsx` para no ser destruidos en cada navegación de `page.tsx`.
```
"""
    },
    {
        "dir": "04-rendering-strategy-master",
        "skill": """# Skill 04: Rendering Strategy Master

## 1. Propósito
Dominar de manera definitiva la elección y control de estrategias de renderizado en Next.js: Static Site Generation (SSG), Server-Side Rendering (SSR) dinámico, Client-Side Rendering (CSR) e Incremental Static Regeneration (ISR). Aprender a configurar `export const dynamic` y `revalidate`.

## 2. Aplicación directa en ARBITRAGEX
El dashboard debe ser altamente dinámico. Por defecto, Next.js intentará prerenderizar la aplicación de manera estática en el momento de construir (`npm run build`). En un entorno MEV como ArbitrageX, las oportunidades varían cada bloque (12s). Si Next.js cachea estáticamente la página, mostrará datos obsoletos. Esta skill asegura el uso de SSR forzado (`force-dynamic`).

## 3. Problemas que resuelve
- La página muestra datos antiguos porque se prerenderizó en el paso de Build y no en Request time.
- Degradación del performance de lectura en base de datos al usar dinámico cuando debería ser estático (Ej: Docs).
- Inconsistencia entre despliegues.

## 4. Reglas Inmutables
- Los Dashboards en Tiempo Real y Operativos (ArbitrageX) **DEBEN** forzarse como dinámicos mediante `export const dynamic = 'force-dynamic';` si dependen de Route Handlers o Fetch directos que varían cada segundo.
- Los reportes diarios o históricos pueden usar ISR (`export const revalidate = 3600;`).
- Toda solicitud de red con `fetch()` en App Router está cacheada por defecto de manera agresiva. Para datos vivos, usar `{ cache: 'no-store' }`.

## 5. Nivel de Madurez
Maestría - Control quirúrgico sobre el compilador de Next.js.
""",
        "checklist": """# Checklist Operativo: Rendering Strategy

- [ ] ¿Los endpoints de métricas en vivo declaran explícitamente su naturaleza dinámica en Route Handlers (`export const dynamic = "force-dynamic"` o `{ cache: 'no-store' }`)?
- [ ] ¿Los listados de logs históricos que se consultan frecuentemente tienen una estrategia de revalidación temporal (ISR) para salvar queries a la BD?
- [ ] ¿Hay validación por parte del build step? (Next.js te dice durante el build: `λ  (Server)  server-side renders at runtime` vs `○  (Static)  automatically rendered as static HTML`).
""",
        "implementation-patterns": """# Patrones Correctos (Implementation)

## Patrón 1: Force Dynamic (Cero Cache Estático)
Usado en `/opportunities/page.tsx` si hubiera fetch de backend directo.

```tsx
// Indicar al motor de Next.js que NUNCA genere un snapshot en Build time.
export const dynamic = 'force-dynamic';
export const fetchCache = 'force-no-store';

export default async function DashboardPage() {
  // Garantizado que se ejecutará en TIEMPO DE REQUEST, no de BUILD.
  const data = await db.getCurrentState(); 
  return <LiveFeed initial={data} />;
}
```

## Patrón 2: Fetch con `no-store`
Si no exportas la directiva global, hazlo a nivel de petición.

```tsx
const res = await fetch("http://api/metrics", {
    cache: "no-store", 
    // Opcionalmente en Next 15: { next: { revalidate: 0 } }
});
```
""",
        "anti-patterns": """# Antipatrones Prohibidos

## Antipatrón 1: Accidental Static Pages in Operational Panels
```tsx
// 🔴 PROHIBIDO: Next.js evaluará esto y, al no ver un parámetro dinámico ni un export const, 
// compilará el resultado y congelará los datos de la base de datos hasta que hagas un nuevo despliegue.

export default async function ArbitrageOpportunities() {
  const opps = await getOpportunities(); 
  return <div>{opps.length} detectadas</div>
}
```
""",
        "validation": """# Validación y Auditoría

## 1. Criterios de Validación
- Ejecutar `npm run build`. 
- Revisar la terminal. Las rutas operativas (`/opportunities`) deben estar marcadas con el ícono de "lambda" `λ` (Server-side renders at runtime) y **NUNCA** con un círculo hueco `○` (Static).

## 2. Cómo Auditar
- Inspeccionar si hay llamadas `fetch` en Server Components sin la propiedad `cache`. Next.js por defecto las hará estáticas.
""",
        "agent-prompt": """# Prompt de Agente: Rendering Strategy Master

```text
Eres un experto en el Next.js Rendering Engine.
Analiza la siguiente página (`page.tsx`) o Route Handler (`route.ts`).
Verifica si maneja datos en tiempo real (operaciones de MEV, dashboards vivos, logs críticos).
Si la respuesta es SÍ, debes garantizar que no quede compilado estáticamente en el Build Time.
Inyecta `export const dynamic = 'force-dynamic';` y `export const fetchCache = 'force-no-store';` para asegurar que el contenido se renderice o procese por cada solicitud en el servidor.
Asegúrate de que cualquier llamada interna con `fetch` incluya `{ cache: 'no-store' }`.
```
"""
    },
    {
        "dir": "05-initial-snapshot-live-update-engineer",
        "skill": """# Skill 05: Initial Snapshot + Live Update Engineer

## 1. Propósito
Diseñar el patrón puente más robusto para arquitecturas Full-Stack de Alta Frecuencia: Cargar el estado inicial sólido (Snapshot) de manera segura y rápida vía API/SSR, y posteriormente conectarse por WebSocket para procesar incrementos (Deltas/Updates) sin interrupciones ni dobles registros.

## 2. Aplicación directa en ARBITRAGEX
El panel principal de **ArbitrageX** escucha mempool y relays. Las oportunidades deben cargar instantáneamente el último estado retenido en Redis (Snapshot: Las top 50 oportunidades activas de los últimos 15 min). Una vez la interfaz es montada, el socket conecta con Edge Server (`subscribe:opportunities`) y recibe stream de updates que se insertan ordenadamente al inicio del array.

## 3. Problemas que resuelve
- Pantalla vacía ("Loading...") mientras el Web Socket negocia el Handshake y conecta (Afecta UX).
- Duplicación de datos: La oportunidad recibida en el Snapshot inicial se recibe otra vez por el stream del Socket y la interfaz la muestra dos veces.
- Brechas de información (Gap): Oportunidades generadas entre la consulta del Snapshot HTTP y la conexión real del WebSocket.

## 4. Reglas Inmutables
- Cargar un **Snapshot Initial** siempre, preferentemente vía Fetch GET (SSR o On Mount).
- El Payload de Update del WebSocket y el Payload del Snapshot deben compartir **exactamente el mismo Esquema TypeScript** o Zod.
- Toda entidad conectada en tiempo real debe tener un identificador único globalmente (`id` o `uuid`).
- El reducer del estado en el cliente DEBE deducir y evitar duplicados utilizando el `id` o versionado. Si un nuevo dato llega con el mismo ID, reemplaza o ignora el actual.

## 5. Nivel de Madurez
Maestría - Esta es la arquitectura crítica requerida por Exchanges, Trading Bots y Paneles HFT (High-Frequency Trading).
""",
        "checklist": """# Checklist Operativo: Snapshot + Live Update

- [ ] ¿El frontend obtiene datos útiles antes de que el WebSocket esté "Connected"? (Polling inicial / HTTP GET Inicial).
- [ ] ¿El event handler del socket hace un de-duplication check usando `Array.some(item => item.id === new.id)` antes de agregar?
- [ ] ¿El servidor retiene (en Redis, Memoria o Postgres) un buffer de eventos pasados para que el Snapshot contenga datos relevantes inmediatos?
- [ ] ¿Las oportunidades conservan límite superior (ej: `slice(0, 50)`) para que un socket que envíe 1000 eventos/min no colapse la memoria del navegador?
""",
        "implementation-patterns": """# Patrones Correctos (Implementation)

## Patrón 1: Deduplicated Snapshot-Stream Merge
```tsx
export function LiveFeed() {
  const [items, setItems] = useState<Item[]>([]);

  // 1. Snapshot Initial
  useEffect(() => {
    fetch('/api/opportunities/live')
      .then(res => res.json())
      .then(data => setItems(data.items));
  }, []);

  // 2. Stream Updates
  useEffect(() => {
    const socket = io('https://edge.domain.com');
    socket.on('new_opportunity', (newItem: Item) => {
      setItems(prev => {
        // Regla de Oro: Evitar duplicados cruzados entre el HTTP Fetch tardío y el Socket veloz.
        if (prev.some(i => i.id === newItem.id)) {
          // Ya existe, podemos decidir reemplazar (Actualizar estado) o descartar.
          return prev;
        }
        // Inserción inmutable limitando el array a 50
        return [newItem, ...prev].slice(0, 50);
      });
    });
    return () => socket.disconnect();
  }, []);
}
```
""",
        "anti-patterns": """# Antipatrones Prohibidos

## Antipatrón 1: Socket Only Paradigm
El dashboard se monta vacío y depende enteramente de que el backend empuje datos por WS. Si el mempool está tranquilo y no hay eventos en 10 minutos, el usuario verá una tabla "Loading" y creerá que el sistema está caído, a pesar de tener historial de hace 15 minutos en Redis.

## Antipatrón 2: Blind Array Push
```tsx
// 🔴 PROHIBIDO
socket.on('event', (data) => {
   setItems(prev => [...prev, data]); 
   // Múltiples re-conexiones del socket inyectarán el mismo evento múltiples veces rompiendo las `key` en React.
});
```
""",
        "validation": """# Validación y Auditoría

## 1. Criterios de Validación
- Deshabilitar red en DevTools. La UI debe mantener la última caché. Rehabilitar la red, el socket debe reconectar, y NO deben aparecer objetos duplicados con la misma Key/ID en la vista de la tabla.
- Iniciar la aplicación en un puerto limpio; de inmediato se deben visualizar registros históricos (Snapshot de Redis). 

## 2. Cómo Auditar
- Buscar en `/frontend` las palabras clave `.on(` o `socket.addEventListener(`. 
- Verificar dentro del callback si la actualización de estado incluye una validación explícita (ej. `.some` o `.findIndex`).
""",
        "agent-prompt": """# Prompt de Agente: Initial Snapshot + Live Update Engineer

```text
Actúa como Arquitecto de Real-Time Systems especializado en WebSockets en ecosistema React.
Audita la integración entre los endpoints de Snapshot Inicial y los streams de WebSocket.
Requisitos que debes forzar:
1. Asegura que el componente nunca dependa puramente de eventos push para mostrar contenido por primera vez; debe hacer un "fetch" del estado retenido del último N período.
2. Identifica en el reducer / state update del componente si la inyección de la payload del socket es ciega. Modifica el código para aplicar deduplicación estricta en base al `id` del payload.
3. Asegura que los arrays masivos de eventos tengan un mecanismo de "corte" o Garbage Collection interno (`slice(0, 100)`) para que no se filtre memoria del DOM.
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

print("Batch 1 generated successfully!")
