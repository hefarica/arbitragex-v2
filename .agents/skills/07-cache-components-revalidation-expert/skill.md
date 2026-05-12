# Skill 07: Cache Components & Revalidation Expert

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
