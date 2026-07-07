# RULE 03: NEXT.JS DOCKER BUILD STRICTNESS DOCTRINE

## 1. DEFINICIÓN DE LA REGLA

Esta regla define el protocolo obligatorio para la gestión de variables de entorno públicas (`NEXT_PUBLIC_*`) en contenedores Docker de Next.js dentro del ecosistema ArbitrageX v2. Cualquier desviación resultará en variables "quemadas" (hardcoded) incorrectamente, provocando errores silenciosos de timeout o cross-origin en producción.

## 2. COMPORTAMIENTO ESTÁTICO DE NEXT.JS (STATIC BAKING)

A diferencia de las variables de servidor (Node.js), las variables de cliente prefijadas con `NEXT_PUBLIC_` (ej. `NEXT_PUBLIC_EDGE_URL`, `NEXT_PUBLIC_WS_URL`) son **INYECTADAS Y REEMPLAZADAS ESTÁTICAMENTE (Braked)** en los bundles de JavaScript durante la fase de construcción (`next build`).

- **El Problema**: Si el archivo `.env` del servidor se actualiza *después* de haber construido la imagen Docker, los cambios **NO** tendrán ningún efecto en el frontend, ya que el JS estático mantendrá los valores antiguos o los "fallbacks" definidos en el código (ej. `http://localhost:8787`).
- **Consecuencia Observada**: El navegador del usuario final intenta hacer fetch a `localhost:8787` (su propia máquina local), generando un error de `Timeout (Signal timed out)` al ser bloqueado por firewalls locales, en lugar de un error de red claro.

## 3. PROTOCOLO DE DESPLIEGUE Y RECONSTRUCCIÓN

Está **ESTRICTAMENTE PROHIBIDO** asumir que un reinicio de contenedor (`docker compose restart`) aplicará cambios en variables `NEXT_PUBLIC_`.

- **Regla de Reconstrucción**: Ante cualquier modificación del archivo `.env` en producción/VPS que involucre rutas públicas, el contenedor de frontend **DEBE SER RECONSTRUIDO OBLIGATORIAMENTE SIN CACHÉ**.
- **Comando Obligatorio**:
  ```bash
  docker compose build --no-cache frontend
  docker compose up -d frontend
  ```

## 4. GESTIÓN DE DEPENDENCIAS Y ESTADO DEL REPOSITORIO

Antes de ejecutar cualquier pipeline de Docker Build en el VPS:
1. Asegurarse de que todos los cambios locales (como la instalación de componentes Shadcn o dependencias como `sonner`) han sido formalmente commiteados y pusheados a `origin/main`.
2. El VPS debe ejecutar `git pull` previo a la construcción para evitar que dependencias faltantes rompan silenciosamente el paso `next build` dentro del Dockerfile, lo cual abortaría la actualización de la imagen y dejaría corriendo la versión desactualizada.
