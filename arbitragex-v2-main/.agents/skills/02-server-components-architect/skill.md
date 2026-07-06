# Skill 02: Server Components Architect

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
