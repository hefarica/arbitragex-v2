# Skill 03: App Router System Designer

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
