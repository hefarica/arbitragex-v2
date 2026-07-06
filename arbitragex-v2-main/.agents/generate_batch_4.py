import os

BASE_DIR = r"c:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\.agents\skills"

SKILLS = [
    {
        "dir": "16-security-oriented-frontend-architect",
        "skill": r"""# Skill 16: Security-Oriented Frontend Architect

## 1. Propósito
Blindar la aplicación contra vectores de ataque Frontend: Cross-Site Scripting (XSS), Cross-Site Request Forgery (CSRF), fugas de variables de entorno (Secret Leakage), inyecciones en la hidratación, Clickjacking, y dominar las Content Security Policies (CSP). Asegurar la transmisión de tokens y permisos operativos críticos (ArbitrageX Admin Token).

## 2. Aplicación directa en ARBITRAGEX
El dashboard tiene controles como el "Kill Switch" que pueden detener el motor MEV. Si un token de administrador se filtra en el HTML (`NEXT_PUBLIC_ADMIN_TOKEN`), o si la aplicación es vulnerable a XSS (permitiendo que un script externo ejecute un click automático en el Kill Switch), las pérdidas financieras serían inmediatas.

## 3. Problemas que resuelve
- Exposición accidental de llaves privadas o URLs de bases de datos al navegador.
- Inyección de HTML arbitrario mediante el renderizado crudo de datos del exterior.
- Peticiones forjadas (CSRF) desde dominios de terceros.
- Cookies de sesión o Auth robadas por Javascript debido a falta de flag `HttpOnly`.

## 4. Reglas Inmutables
- **Regla del NEXT_PUBLIC:** NUNCA, jamás, utilices el prefijo `NEXT_PUBLIC_` para contraseñas, JWTs, llaves privadas o strings de conexión a base de datos.
- Prohibido utilizar `dangerouslySetInnerHTML` salvo que la entrada haya pasado estrictamente por un sanitizador como `DOMPurify`. Los Server Components son seguros por defecto al escapar texto, no desactives esta protección.
- Todos los Server Actions que ejecuten acciones destructivas (Ej. `killEngine()`, `updateConfig()`) DEBEN re-autenticar al usuario / token internamente antes de ejecutar. No confiar en que el botón estaba "oculto" en la UI.
- Implementar encabezados estrictos en `next.config.js` (X-Frame-Options: DENY, Strict-Transport-Security, Content-Security-Policy).

## 5. Nivel de Madurez
Arquitecto de Seguridad - El Frontend como la primera línea de defensa activa.
""",
        "checklist": r"""# Checklist Operativo: Security & AppSec

- [ ] ¿Los tokens o claves sensibles (`ARBX_EDGE_TOKEN`, `DATABASE_URL`) carecen explícitamente del prefijo `NEXT_PUBLIC_`?
- [ ] ¿Los Server Actions (`'use server'`) validan la identidad / token del usuario o del request origin antes de procesar el input?
- [ ] ¿El archivo `next.config.js` incluye una lista estricta de `headers()` de seguridad (CSP, X-Frame-Options)?
- [ ] ¿Los formularios o Server Actions que aceptan payloads del exterior validan la data usando Zod antes de cruzar la capa del framework hacia la BD?
""",
        "implementation-patterns": r"""# Patrones Correctos (Implementation)

## Patrón 1: Secure Server Actions (Zero Trust)
No asumas que quien llama al Server Action es el botón de tu propia UI.

```tsx
// 🟢 CORRECTO
'use server';

import { cookies, headers } from 'next/headers';
import { db } from '@/lib/db';
import { z } from 'zod';

const InputSchema = z.object({ newLimit: z.number().max(100) });

export async function updateGasLimit(data: FormData) {
  // 1. Autorización (Re-verificar)
  const authCookie = cookies().get('session_token');
  if (!isValidSession(authCookie)) throw new Error("Unauthorized");
  
  // 2. Validación de Input (Zod)
  const parsed = InputSchema.safeParse({ newLimit: data.get('limit') });
  if (!parsed.success) throw new Error("Invalid Input");

  // 3. Ejecución Segura
  await db.updateGas(parsed.data.newLimit);
}
```

## Patrón 2: Strict HTTP Headers in Next.js
En `next.config.js`:
```js
module.exports = {
  async headers() {
    return [
      {
        source: '/(.*)',
        headers: [
          { key: 'X-Frame-Options', value: 'DENY' }, // Clickjacking protection
          { key: 'X-Content-Type-Options', value: 'nosniff' },
          { key: 'Referrer-Policy', value: 'strict-origin-when-cross-origin' },
        ],
      },
    ];
  },
};
```
""",
        "anti-patterns": r"""# Antipatrones Prohibidos

## Antipatrón 1: Secret Leakage a través de Cliente
```tsx
// 🔴 PROHIBIDO: Esto incrusta la contraseña en texto claro en todos los navegadores
const dbPass = process.env.NEXT_PUBLIC_DB_PASSWORD;

export function connect() { ... }
```

## Antipatrón 2: Ocultamiento Visual en Lugar de Autorización
Ocultar un botón si el usuario no es Admin, pero dejar la ruta de API / Server Action desprotegida. Un atacante puede ejecutar el `fetch` directo a `/api/admin/kill` desde la consola.
""",
        "validation": r"""# Validación y Auditoría

## 1. Criterios de Validación
- Auditar el bundle minificado de producción buscando regex de credenciales comunes (`DB_PASS`, `TOKEN`, `SECRET`). Si se encuentra el valor textual, hay un leak grave (Uso de NEXT_PUBLIC o importación en Client Component).
- Lanzar una herramienta de análisis de cabeceras (como Mozilla Observatory) al endpoint en vivo. Debe arrojar nota 'B' o superior (exigiendo cabeceras de seguridad activas).

## 2. Cómo Auditar en ARBITRAGEX
- Revisar `.env` y contrastar contra `docker-compose.dev.yml` y `Dockerfile`. Cualquier variable inyectada al frontend mediante `NEXT_PUBLIC_` debe ser estrictamente inofensiva y de dominio público (ej. URLs externas del WebSocket, IDs de analíticas de terceros).
""",
        "agent-prompt": r"""# Prompt de Agente: Security-Oriented Frontend Architect

```text
Actúa como Auditor de Seguridad AppSec especializado en React/Next.
Inspecciona el componente y los Server Actions adjuntos.
Tu objetivo es evitar compromisos del sistema:
1. Asegura que los Server Actions verifiquen la autorización antes de ejecutar su lógica (No confíes en que el UI haya ocultado el botón).
2. Valida que NINGÚN secreto de estado de DB o llaves API críticas estén precedidas por `NEXT_PUBLIC_`. Si lo están, renómbralas y muévelas a Server Components o Route Handlers seguros.
3. Detecta usos crudos de `dangerouslySetInnerHTML`. Si el contenido no está limpiado criptográficamente por `DOMPurify`, alerta de vulnerabilidad XSS crítica.
```
"""
    },
    {
        "dir": "17-backend-for-frontend-engineer",
        "skill": r"""# Skill 17: Backend-for-Frontend Engineer

## 1. Propósito
Construir y diseñar la capa Backend-For-Frontend (BFF) nativa de Next.js (Server Components y Route Handlers). Traducir servicios de backend complejos (Bases de datos profundas, Microservicios gRPC o colas en Rust) a esquemas simples, paginados, protegidos y listos para ser consumidos exclusivamente por el propio frontend de la aplicación.

## 2. Aplicación directa en ARBITRAGEX
El Edge (Gateway en Cloudflare Workers o Nginx) rutea hacia el Frontend VPS en el puerto 5173. El Next.js actúa como un BFF. Se conecta directamente a PostgreSQL para leer `config`, y expone esa información limpia a los componentes de cliente. No necesitamos montar un servidor Express.js secundario; Next.js es el BFF.

## 3. Problemas que resuelve
- Exceso de fetching de datos (Over-fetching): Descargar 50 columnas de DB en el cliente cuando el UI solo necesita 3.
- Múltiples requests (Under-fetching): El cliente haciendo 5 peticiones separadas para armar una vista.
- Complejidad compartida: Exponer URIs complejas o bases de datos al navegador.

## 4. Reglas Inmutables
- El código que se ejecuta en los Server Components y Route Handlers (`app/api/route.ts`) CORRE EN NODE.JS. Puede interactuar directamente con `pg` (Postgres), Redis, o FileSystem de manera segura.
- El BFF debe moldear (Shape) los datos específicamente para la vista requerida. Si la API de Rust devuelve un objeto de 3KB y la tarjeta solo requiere 2 campos (20 bytes), el BFF mapea el objeto y envía solo los 20 bytes al cliente.
- Las Route Handlers (`api/`) no deben duplicarse si la lógica puede ser importada directamente en el Server Component, a menos que un servicio de terceros (webhooks) o un componente puramente de cliente lo necesite.

## 5. Nivel de Madurez
Senior - Unifica la orquestación del backend con las necesidades estrictas de la presentación.
""",
        "checklist": r"""# Checklist Operativo: Backend for Frontend

- [ ] ¿Los Route Handlers (`app/api/`) o Server Actions transforman/agrupan las respuestas crudas del backend antes de enviarlas al cliente (Evitando Over-fetching)?
- [ ] ¿Se eliminó la capa intermedia redundante (Ej: crear una API Next para luego hacer `fetch('/api')` en el mismo `page.tsx` de Next) invocando directamente los servicios de BD?
- [ ] ¿Se manejan de forma centralizada las cookies de sesión/autorización en el Edge o en un middleware de Next.js (`middleware.ts`)?
""",
        "implementation-patterns": r"""# Patrones Correctos (Implementation)

## Patrón 1: Data Shaping en el Server Component
El Server Component abstrae la complejidad de la BD y pasa props limpias.

```tsx
// 🟢 CORRECTO
import { db } from '@/lib/db';

export default async function TopOpportunitiesPage() {
  // El usuario nunca ve la query SQL ni recibe campos confidenciales.
  const rawData = await db.query('SELECT * FROM opportunities ORDER BY profit DESC LIMIT 10');
  
  // Data Shaping (BFF Logic)
  const cleanData = rawData.map(row => ({
    id: row.id,
    title: `${row.dex_a} ↔ ${row.dex_b}`,
    profit: row.profit_usd,
    // Ignoramos campos sensibles como 'arbitrage_path_bytecode'
  }));

  return <OpportunitiesList data={cleanData} />
}
```

## Patrón 2: API Route como Proxy Seguro
```ts
// /app/api/trigger/route.ts
import { NextResponse } from 'next/server';
import { rustEngineCall } from '@/lib/rust-api';

export async function POST(req: Request) {
  // Esconde la URL del servidor de Rust y los tokens de Auth de los ojos del navegador.
  const result = await rustEngineCall("/execute-trade", { auth: process.env.RUST_SECRET });
  return NextResponse.json({ success: true, txId: result.txId });
}
```
""",
        "anti-patterns": r"""# Antipatrones Prohibidos

## Antipatrón 1: Overfetching Directo al Cliente
```tsx
// 🔴 PROHIBIDO: Enviar todo el objeto al cliente innecesariamente
export default async function UserProfile() {
  const user = await db.getUser(1); // Retorna password_hash, email, secret_token, balance
  
  // El cliente recibe toda esa data en su bundle RSC, exponiendo el password_hash en la red
  return <ClientProfile user={user} />
}
```
""",
        "validation": r"""# Validación y Auditoría

## 1. Criterios de Validación
- Inspeccionar el Payload RSC devuelto en el Network Tab al cargar una página (archivos `?_rsc=...`). Si contiene campos irrelevantes a la vista visual (ej. UUIDs internos no usados, hashes, arrays gigantes), el BFF está fallando en su trabajo de Data Shaping.
- Todas las peticiones a la DB deben ejecutarse en el servidor. Nunca un paquete como `pg` o `mysql2` debe estar en el bundle del navegador.

## 2. Cómo Auditar
- Revisar las interacciones entre `Server Component` -> `Client Component`. Asegurar que las `props` enviadas sean la mínima expresión de los datos requeridos.
""",
        "agent-prompt": r"""# Prompt de Agente: BFF Engineer

```text
Actúa como Ingeniero Backend-For-Frontend.
Analiza la transferencia de datos entre el backend crudo (Base de datos o API de Rust) y los componentes visuales del cliente.
Aplica estricto Data Shaping:
1. Elimina (omite/mapa) cualquier campo del JSON resultante que no sea estrictamente utilizado por el componente JSX final, para reducir el payload del RSC.
2. Evita la exposición de objetos completos (`SELECT *`) pasados directamente por prop a un componente `use client`.
3. Identifica servicios proxy; si hay información sensible a la que se accede, crea una Route Handler (`route.ts`) de Next.js o Server Action como puente seguro, en lugar de contactar el servidor externo desde el cliente.
```
"""
    },
    {
        "dir": "18-observability-production-debugging-specialist",
        "skill": r"""# Skill 18: Observability & Production Debugging Specialist

## 1. Propósito
Instrumentar la aplicación React/Next.js para tener visibilidad atómica en Producción. Integrar trazabilidad (OpenTelemetry, Sentry, Datadog), métricas de Core Web Vitals (Next.js Analytics), recolectar registros de error detallados (Source Maps decodificados) y generar alertas de salud en tiempo real.

## 2. Aplicación directa en ARBITRAGEX
Si el frontend del VPS (195.201.235.70) colapsa, tira errores 500 silenciosos, o la hidratación falla masivamente para un operador externo, los logs en consola del navegador están perdidos. Se necesita capturarlos (Sentry / Datadog) o enviar los errores de React a un endpoint de logs de servidor (Loki/Promtail) para analizarlos en Grafana sin acceder al PC del usuario.

## 3. Problemas que resuelve
- "Funciona en mi máquina, pero en producción arroja pantalla blanca".
- Errores minificados crípticos de React (`Error #425`) imposibles de rastrear a qué archivo o componente pertenecen sin Source Maps.
- Fugas de memoria lentas (Memory leaks) detectables tarde.
- Imposibilidad de saber si las llamadas API están fallando por Timeouts en el cliente.

## 4. Reglas Inmutables
- Errores asíncronos y Crash Boundaries (archivos `error.tsx` en Next) **DEBEN** inyectar el objeto `error` hacia el logger de producción (Sentry/Loki). Imprimir a `console.error` no basta en producción.
- **Source Maps Ocultos:** Generar Source Maps durante el Build (`productionBrowserSourceMaps: true` o subidos a Sentry) para decodificar trazas de error, pero evitar exponerlos en la ruta pública si la IP (Propiedad Intelectual) del código es confidencial.
- Toda medición crítica de performance (`useReportWebVitals`) debe recolectarse y agregarse estadísticamente.

## 5. Nivel de Madurez
Senior - "Si no puedes observarlo, no está en producción".
""",
        "checklist": r"""# Checklist Operativo: Observability

- [ ] ¿Los `error.tsx` están capturando la excepción y enviándola a un servicio centralizado de Logging antes de mostrar la UI de fallback?
- [ ] ¿El API Client (`lib/api-client.ts`) loguea las fallas HTTP repetitivas (5xx) a un sistema de monitoreo en lugar de tragar el error o dejarlo solo en la consola del navegador?
- [ ] ¿Hay un sistema de correlación (Correlation ID / Request ID) pasando desde el frontend hacia el backend para rastrear una petición a lo largo de todo el stack?
""",
        "implementation-patterns": r"""# Patrones Correctos (Implementation)

## Patrón 1: Global Error Boundary Logger
```tsx
// app/error.tsx
'use client'; // Error boundaries obligatoriamente cliente

import { useEffect } from 'react';
import { captureError } from '@/lib/monitoring';

export default function GlobalError({
  error,
  reset,
}: {
  error: Error & { digest?: string }
  reset: () => void
}) {
  useEffect(() => {
    // Loguear asíncronamente a Grafana/Sentry
    captureError(error, { 
      digest: error.digest, 
      level: 'fatal', 
      context: 'GlobalRouter' 
    });
  }, [error]);

  return (
    <html>
      <body>
        <h2>Algo colapsó terriblemente.</h2>
        <button onClick={() => reset()}>Reintentar</button>
      </body>
    </html>
  );
}
```

## Patrón 2: Injecting Request IDs (Tracing)
```ts
// Al llamar a la API
const correlationId = crypto.randomUUID();
const res = await fetch('/api/action', {
  headers: {
    'X-Correlation-ID': correlationId
  }
});
// Si falla, el cliente sabe qué ID falló y el backend registró ese ID en Grafana.
```
""",
        "anti-patterns": r"""# Antipatrones Prohibidos

## Antipatrón 1: Consola Ciega en Prod
```tsx
// 🔴 PROHIBIDO
try {
  await doDangerousThing();
} catch (e) {
  console.log("Error", e); // Nadie verá esto nunca en producción. El operador cerrará la pestaña quejándose.
  showToast("Ocurrió un error");
}
```
""",
        "validation": r"""# Validación y Auditoría

## 1. Criterios de Validación
- Lanzar una excepción forzada explícita `throw new Error("Test Alarm")` en un Client Component.
- Abrir Grafana (o la terminal del backend). El error, incluyendo el navegador, URL, Correlation ID y stack trace (idealmente decodificado), debe estar registrado en menos de 5 segundos.

## 2. Cómo Auditar
- Buscar archivos `error.tsx` en el repositorio. Si carecen de una exportación de logs asíncrona hacia el backend (`logger.error` o similar), marcar como "Blind spot".
- Auditar la gestión de `.catch()` en todas las promesas de red.
""",
        "agent-prompt": r"""# Prompt de Agente: Observability Specialist

```text
Actúa como Observability Specialist de Producción.
Analiza la gestión de errores en este código.
Tu objetivo es garantizar la trazabilidad:
1. Reemplaza los silenciosos `console.error` o `.catch(console.log)` por invocaciones a sistemas de logging remotos (Ej: un fetch a `/api/logs` o `captureException`).
2. Verifica que las barreras `error.tsx` envíen el "digest" y el objeto de error a la telemetría central antes de pedirle al usuario que intente nuevamente.
3. Asegura que las peticiones HTTP críticas salientes incrusten cabeceras `X-Correlation-ID` o `X-Request-ID` para permitir rastreo End-to-End en microservicios.
```
"""
    },
    {
        "dir": "19-testing-quality-gate-architect",
        "skill": r"""# Skill 19: Testing & Quality Gate Architect

## 1. Propósito
Crear una red de seguridad infalible y piramidal: Pruebas estáticas (TypeScript, ESLint), Pruebas Unitarias veloces para componentes sin efectos visuales (Vitest, React Testing Library), y Pruebas End-to-End (E2E) imitando navegadores reales (Playwright/Cypress). Bloquear cualquier despliegue que rompa la UI o retroceda funcionalidad.

## 2. Aplicación directa en ARBITRAGEX
Un error tipográfico en el CSS puede ignorarse, pero una falla de renderizado en el `LiveFeed` que impida al operador hacer clic en el botón `SIMULATE` cuesta dinero. Se deben ejecutar flujos E2E de simulación de oportunidad usando MSW (Mock Service Worker solo en Testing) o instancias limpias de backend.

## 3. Problemas que resuelve
- Despliegues rotos ("Push to Prod & Pray").
- Tests unitarios frágiles (Testear implementaciones internas en lugar de comportamiento).
- Falsos Positivos (Tests que pasan aunque la pantalla de login esté en blanco por error de hidratación).

## 4. Reglas Inmutables
- **Test Behavior, Not Implementation:** No testees cuántas veces se llamó a `useState`. Testea si el botón dice "Simulate", y si al hacer clic se muestra "Loading".
- Cobertura 100% de E2E en las "Happy Paths" críticas (Login, Lectura de Oportunidad, Botón Ejecutar).
- Fallar rápido (Fail Fast): TypeScript y ESLint son las primeras puertas (Gates). Si TS arroja error, no se corre Vitest. Si Vitest falla, no se corre Playwright.
- Jamás usar Mocks en Producción (Doctrina de ArbitrageX), pero SI está permitido usarlos exclusivamente dentro del marco de la suite unitaria o E2E simulado (`vi.mock` / MSW) para no depender del motor de Rust encendido durante CI.

## 5. Nivel de Madurez
Senior - Cultura de Calidad (Shift-Left Testing).
""",
        "checklist": r"""# Checklist Operativo: Testing & Quality

- [ ] Estático: ¿El comando `npm run typecheck` (`tsc --noEmit`) y `eslint` terminan con 0 errores?
- [ ] Unitario: ¿Los componentes utilitarios y formatters (Ej: `formatDate`, `calculateROI`) tienen tests estrictos (Vitest)?
- [ ] E2E: ¿Existe al menos un flujo de Playwright que levanta Next.js, entra a `/opportunities`, e inspecciona que cargue correctamente?
- [ ] ¿El CI pipeline (Ej. Github Actions o Husky pre-commit) aborta el build si alguna de estas capas falla?
""",
        "implementation-patterns": r"""# Patrones Correctos (Implementation)

## Patrón 1: Behavior-Driven Testing con Testing Library
```tsx
// 🟢 CORRECTO
import { render, screen, fireEvent } from '@testing-library/react';
import { ToggleButton } from './ToggleButton';

test('Toggles compact mode when clicked', () => {
  render(<ToggleButton />);
  
  // Buscar por rol semántico, no por clase CSS o id
  const button = screen.getByRole('button', { name: /compact/i });
  expect(button).toBeInTheDocument();
  
  fireEvent.click(button);
  expect(button).toHaveAttribute('aria-pressed', 'true'); // Comportamiento a11y medible
});
```

## Patrón 2: E2E Playwright Smoke Test
```ts
// tests/smoke.spec.ts
import { test, expect } from '@playwright/test';

test('Opportunities page loads and connects', async ({ page }) => {
  await page.goto('/opportunities');
  
  // Verifica el h1
  await expect(page.locator('h1')).toContainText('Live MEV Feed');
  
  // Verifica el estado inicial de conexión
  const statusBadge = page.locator('text="POLLING"').first();
  await expect(statusBadge).toBeVisible();
});
```
""",
        "anti-patterns": r"""# Antipatrones Prohibidos

## Antipatrón 1: Testing Implementation Details
```tsx
// 🔴 PROHIBIDO
test('Component state updates', () => {
  const wrapper = shallow(<MyComponent />);
  wrapper.instance().setState({ clicked: true }); // Mal. Nunca interactuar directamente con el estado interno de React.
  expect(wrapper.state('clicked')).toBe(true);
});
```
""",
        "validation": r"""# Validación y Auditoría

## 1. Criterios de Validación
- Romper el código agregando un `any` erróneo o eliminando el `<button>` del JSX.
- El servidor de integración continua (CI) debe interrumpir el proceso de dockerizacion o despliegue y mostrar un Warning color rojo.

## 2. Cómo Auditar
- Ejecutar la pirámide completa: `npm run lint`, `npm run typecheck`, `npm run test`, `npm run test:e2e`. Todos deben terminar exitosamente en entornos limpios.
""",
        "agent-prompt": r"""# Prompt de Agente: Testing Architect

```text
Actúa como Arquitecto de Pruebas de Calidad.
Genera la suite de tests unitarios (Vitest/RTL) para este componente.
Instrucciones estrictas:
1. No escribas tests enfocados en el estado interno o hooks de implementación.
2. Interacciona con el componente como lo haría un usuario ciego utilizando Screen Readers: busca botones por su rol y texto ( `getByRole('button', {name: 'Submit'})` ).
3. Si el componente hace llamadas asíncronas de base de datos, utiliza Mocks controlados (`vi.mock`) o MSW (Mock Service Worker) simulando escenarios de Éxito (200), Falla (500) y Tiempo de espera (Timeout).
```
"""
    },
    {
        "dir": "20-deployment-runtime-scaling-strategist",
        "skill": r"""# Skill 20: Deployment, Runtime & Scaling Strategist

## 1. Propósito
Empaquetar, construir y desplegar la aplicación React/Next.js de forma inmutable, escalable y observable. Dominar Docker multi-stage builds específicos para NodeJS, inyección de variables de entorno en Build-time vs Runtime (`NEXT_PUBLIC_`), gestión de procesos con `tini` y monitoreo de memoria base (Standalone output).

## 2. Aplicación directa en ARBITRAGEX
El error que impidió a la aplicación conectarse usando `edge-arbx.ape-tv.net` e insistió en buscar `localhost:8787` ocurrió debido a fallos al entender en qué fase se embeben las variables `NEXT_PUBLIC_`. Esta skill garantiza despliegues atómicos sin fugas, asegurando que la imagen de Docker contenga todo lo precompilado correctamente, y que la aplicación Node.js sobreviva en el VPS (IP `195.201.235.70`).

## 3. Problemas que resuelve
- Build-time vs Runtime Env Variables: Errores persistentes de localhost en producción.
- Contenedores de 2 Gigabytes por arrastrar carpetas enteras de dependencias de desarrollo (`node_modules`).
- Node.js bloqueado o zombie porque el PID 1 no es `init` o `tini` (las señales `SIGTERM` fallan y docker tarda 10 segundos en matar).
- Excesivo consumo de RAM del VPS por correr Next.js en modo no optimizado (`standalone`).

## 4. Reglas Inmutables
- **NEXT_PUBLIC_ en Build Time:** Toda variable con prefijo `NEXT_PUBLIC_` en Next.js App Router ES CONGELADA (quemada, inlined) de manera textual en los archivos JavaScript estáticos durante el proceso de compilación `next build`. **SIEMPRE** deben proporcionarse al Docker Builder a través de `ARG`. Alterarlas en el bloque `environment:` de docker-compose para el contenedor runtime NO alterará el Javascript del cliente si ya está construido.
- Utilizar `output: 'standalone'` en el `next.config.js` para reducir agresivamente la cantidad de dependencias subidas a producción (Solo incluye los rastros de Node vitales).
- El comando CMD en Docker de Next.js debe ejecutarse detrás de `tini` (o similar) para manejo de PIDs y cierre limpio. `ENTRYPOINT ["/usr/bin/tini","--"]`.

## 5. Nivel de Madurez
PhD / DevOps Maestro - Despliegues inmutables listos para la batalla.
""",
        "checklist": r"""# Checklist Operativo: Deployment

- [ ] Docker Build: ¿Las variables `NEXT_PUBLIC_` están pasadas como `ARG` en el Dockerfile y como `args:` en el `docker-compose.yml` en el bloque build?
- [ ] Docker Image: ¿El Dockerfile es Multi-Stage (Builder vs Runner) evitando que el compilador TypeScript se suba a la fase productiva?
- [ ] Seguridad Node: ¿El proceso de Next se ejecuta como un usuario sin privilegios (ej. `USER nextjs`) en lugar de `root`?
- [ ] Standalone: ¿El archivo `next.config.js` incluye `output: "standalone"`?
""",
        "implementation-patterns": r"""# Patrones Correctos (Implementation)

## Patrón 1: El Dockerfile Multi-Stage Perfecto para Next.js (Standalone)
```dockerfile
# 🟢 CORRECTO
# STAGE 1: Builder
FROM node:20-alpine AS builder
WORKDIR /app
COPY package*.json ./
RUN npm ci
COPY . .
# REGLA ORO: ARG required at build time!
ARG NEXT_PUBLIC_API_URL
ENV NEXT_PUBLIC_API_URL=$NEXT_PUBLIC_API_URL
RUN npm run build

# STAGE 2: Runner
FROM node:20-alpine AS runner
WORKDIR /app
ENV NODE_ENV=production

# Seguridad y Tini
RUN apk add --no-cache tini
RUN addgroup -g 1001 -S nodejs && adduser -S nextjs -u 1001

# Copiar artefactos standalone (Ahorra cientos de MBs)
COPY --from=builder --chown=nextjs:nodejs /app/.next/standalone ./
COPY --from=builder --chown=nextjs:nodejs /app/.next/static ./.next/static
COPY --from=builder --chown=nextjs:nodejs /app/public ./public

USER nextjs
EXPOSE 3000
ENV PORT=3000

ENTRYPOINT ["/sbin/tini", "--"]
CMD ["node", "server.js"]
```

## Patrón 2: Injecting Runtime Env Variables for Backend
Si la variable no dice `NEXT_PUBLIC_`, entonces Next.js la leerá **dinámicamente en cada request de servidor** en tiempo de ejecución.
```yml
# En docker-compose.yml
services:
  web:
    image: mi-app:latest
    environment:
      # Modificable sin hacer Re-Build!
      - DATABASE_URL=postgres://user:pass@db/prod
```
""",
        "anti-patterns": r"""# Antipatrones Prohibidos

## Antipatrón 1: Confusión Build/Runtime de Variables Next.js
El error más común del ecosistema (El caso `localhost:8787`).

```yml
# 🔴 PROHIBIDO (NO ALTERA EL FRONTEND DE CLIENTE)
services:
  frontend:
    build: . # Compila SIN la variable. Next.js le asigna "undefined" o el fallback del código a todos los JS estáticos.
    environment:
      - NEXT_PUBLIC_API_URL=https://api.produccion.com # El Server Node lo leerá, pero el Navegador jamás lo verá.
```

## Antipatrón 2: npm start como PID 1
Correr `CMD ["npm", "start"]` lanza shells intermedios que no reenvían señales de apagado a Node. Al actualizar el contenedor, Docker esperará 10s y le enviará un `SIGKILL` forzoso, cortando conexiones abruptamente.
""",
        "validation": r"""# Validación y Auditoría

## 1. Criterios de Validación
- Ejecutar `docker build`. El tamaño final de la imagen Runner (Standalone) no debería exceder los 250MB - 350MB. (A diferencia de los 1.5GB de una imagen base completa).
- Buscar un componente cargado en el navegador del cliente, inspeccionar en DevTools Sources y verificar que las variables estáticas están "quemadas" (Hardcoded by Webpack) con la IP/Dominio de producción y no de desarrollo.

## 2. Cómo Auditar
- Inspeccionar `docker-compose.yml`. Todo `NEXT_PUBLIC_` en un servicio de frontend obligatoriamente debe estar dentro de la cláusula `build: args:`.
""",
        "agent-prompt": r"""# Prompt de Agente: Deployment Strategist

```text
Actúa como Ingeniero de Infraestructura DevOps.
Analiza este `Dockerfile` y `docker-compose.yml` para una aplicación Next.js.
Tu objetivo es garantizar producción inmutable:
1. Detecta cualquier variable de entorno vital para la UI (prefijo `NEXT_PUBLIC_`) que esté declarada exclusivamente en `environment` y muévela o duplícala también en `build.args` para que el compilador las evalúe durante el build estático.
2. Identifica si la imagen corre bajo modo "standalone" para ahorrar espacio; recomienda los cambios al `next.config.js`.
3. Corrige la instrucción CMD final para no usar `npm start`, sino usar `node server.js` (si es standalone) o `next start`, preferiblemente envuelto en un init manager como `tini`.
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

print("Batch 4 generated successfully!")
