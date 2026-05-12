# Skill 04: Rendering Strategy Master

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
