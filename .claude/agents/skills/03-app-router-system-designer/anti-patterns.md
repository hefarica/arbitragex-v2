# Antipatrones Prohibidos

## Antipatrón 1: Destrucción de Conexiones por Navegación
Colocar Providers que manejan estado complejo (WebSockets o Zustand cache) directamente dentro de `page.tsx` en vez de `layout.tsx`. Esto fuerza su recreación si cambias de la página A a la página B, matando y reviviendo la conexión de socket.

## Antipatrón 2: Route Handlers Impuros
Crear un archivo `route.ts` que retorna datos diferentes pero no usar las variables `Request` dinámicas, causando que Next.js lo cachee estáticamente para siempre en build-time.
