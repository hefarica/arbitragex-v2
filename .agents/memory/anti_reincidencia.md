# Bitácora Anti-Reincidencia

Este archivo actúa como memoria técnica persistente para el agente. Su función es documentar los peores incidentes, los errores cometidos en la fase de resolución y las reglas operativas para prevenir futuros fracasos similares.

---

## Incidente #1: React Hydration Cascade en el Dashboard de Producción

**Fecha del aprendizaje:** 3 de Mayo de 2026

**Qué ocurrió:** 
El dashboard del operador en `http://195.201.235.70:5173/opportunities` presentaba una cascada de errores (React #425, #418, #423), lo que provocaba que la aplicación cayera de Server-Side Rendering a un forzado y costoso Client-Side Rendering completo. La interfaz parpadeaba o se mostraban pantallas de error.

**Qué salió mal (Fallos en la resolución):**
1. **Identificación Incompleta:** Inicialmente se pensó que el problema solo afectaba al archivo `opportunities/page.tsx` por el uso de `Date.now()`.
2. **Error Arquitectónico Oculto:** El componente `SiteHeader` (incluido en el `layout.tsx` general) llamaba a `getApiBaseUrl()`, el cual resolvía a `http://edge:8787` en el servidor y a `https://edge-arbx.ape-tv.net` en el cliente.
3. **Fallo de Despliegue en el VPS:** El contenedor Frontend se reconstruyó sin proveer explícitamente `--env-file .env` al comando de compilación de Docker Compose. Como resultado, Next.js inyectó el valor fallback de desarrollo (`http://localhost:8787`) como `NEXT_PUBLIC_EDGE_URL`, estrellando la aplicación en vivo con el error *"Production API base URL cannot point to localhost"*.

**Causa raíz:**
Violación sistemática del contrato de hidratación SSR-CSR de React/Next.js y una configuración de pipeline de compilación de Docker defectuosa que no pasaba de forma explícita el archivo `.env` a los argumentos de compilación de Next.js (`build args`).

**Regla nueva para prevenirlo:**
1. **Regla Inmutable de Hidratación (Cero Mismatch):** Toda página SSR debe entregar un snapshot inicial estable desde un *Server Component*. El *Client Component* debe iniciar exactamente con ese snapshot, relegando cualquier mutación dinámica (incluyendo WeSockets o lógica de Date) al interior del gancho `useEffect`. Queda absolutamente prohibido renderizar valores no determinísticos en el primer pase del cliente.
2. **Compilación Hermética:** Todo redespliegue de un contenedor React/Next en el VPS debe seguir el protocolo de reconstrucción total inyectando las variables explícitamente:
   `docker compose -f docker/compose.dev.yml --env-file .env build --no-cache frontend`

**Validación obligatoria:**
Después de todo cambio en la interfaz gráfica, el agente DEBE desplegar al VPS de forma asertiva y utilizar un **Subagente de Navegador** (`browser_subagent`) para visitar la URL pública, esperar 5 segundos y corroborar visual y funcionalmente (cero overlays de error y consola limpia) que la hidratación ocurrió satisfactoriamente.

**Archivos o rutas relacionadas:**
- `c:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\frontend\app\opportunities\page.tsx`
- `c:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\frontend\app\opportunities\OpportunitiesClient.tsx`
- `c:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\frontend\components\site-header.tsx`
- `c:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\docker\compose.dev.yml`

**Acción correcta en futuras ocasiones:**
Aplicar la partición estricta Servidor/Cliente (Snapshot en SSR, interactividad y refetching en el Cliente) y **siempre** usar `--no-cache` y `--env-file .env` en las compilaciones de los contenedores web orientados a producción.
