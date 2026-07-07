# Skill 17: Backend-for-Frontend Engineer

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
