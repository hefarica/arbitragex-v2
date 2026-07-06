# SKILL 101 — React Hydration Forensics & Mounted Snapshot Pattern

## 1. Propósito superior
Erradicar permanentemente la cascada de errores de hidratación en aplicaciones React/Next.js (Errores #425, #418, #423) que rompen la experiencia de usuario y degradan el renderizado del lado del servidor (SSR) a Client-Side Rendering (CSR) forzado. 

Establecer el patrón arquitectónico **"Mounted Snapshot"** como la única doctrina aceptada para renderizar componentes que dependen del tiempo real (relojes, timestamps locales, fechas dinámicas) dentro de aplicaciones Next.js App Router.

## 2. Diagnóstico del Problema (React Error #425)
En la página `/opportunities`, el uso de `"use client"` **no deshabilita el SSR**. El servidor de Next.js ejecuta el componente para generar el HTML inicial.
- **Causa Raíz:** Se utilizó `useState<number>(Date.now())` y `toLocaleTimeString()`.
- **Efecto SSR:** El servidor renderiza el HTML con la hora `T_server` y su respectiva zona horaria.
- **Efecto Cliente (Hidratación):** El navegador descarga el HTML y trata de "hidratarlo" (conectar React). Al ejecutar el componente por primera vez en el cliente, la hora es `T_client` (o la zona horaria del usuario es distinta).
- **Consecuencia:** React detecta que el texto esperado por el servidor (`12:00:05`) no coincide con el texto del cliente (`12:00:08`). Se produce un *Mismatch* (Error #425). La hidratación falla (#418) y React descarta el HTML del servidor para hacer un renderizado completo en el cliente (#423), lo que genera parpadeos y pérdida de performance.

## 3. La Solución: The Mounted Snapshot Pattern
Para garantizar una hidratación determinista y perfecta, el primer render del cliente debe producir **exactamente** el mismo output que el render del servidor.

### Implementación Core:
1. **Inicialización Determinista:** `useState<number>(0)` en lugar de `Date.now()`.
2. **Guardián de Montaje:**
   ```typescript
   const [isMounted, setIsMounted] = useState(false);
   
   useEffect(() => {
     setIsMounted(true);
     setNow(Date.now()); // Iniciar valores dinámicos
   }, []);
   ```
3. **Renderizado Condicionado:**
   ```tsx
   // Si no está montado (SSR y 1er Render del cliente), renderizar un placeholder estático.
   // Si está montado (2do Render del cliente, post-hidratación), renderizar el valor dinámico.
   <span>{isMounted ? formatTime(now) : "--:--:--"}</span>
   ```

## 4. Doctrina de Despliegue Estricto en VPS (Next.js)
El código Next.js empaquetado en el VPS (`195.201.235.70`) **no se actualiza por arte de magia**. Un arreglo local no sirve de nada si no se compila en producción.

1. **Commit y Push:** Los cambios deben subir al repositorio (`git push origin main`).
2. **Pull y Build en Producción:** El VPS debe descargar los cambios y destruir la caché del build de Next.js.
   ```bash
   ssh arbx "cd /opt/arbitragex-v2 && git pull origin main && docker compose build --no-cache frontend && docker compose up -d frontend"
   ```
3. **Verificación Forense:** NUNCA asumir que el despliegue funcionó sin validar:
   - Validar que la imagen es reciente: `docker inspect arbitragex-v2-frontend-1 --format "{{.Created}}"`
   - Validar que el navegador solicita nuevos *Chunks* de JavaScript mediante un *Hard Refresh* (`Ctrl+F5`) o verificando el código fuente: `curl -s http://195.201.235.70:5173/opportunities | grep -oE 'page-[a-f0-9]+\.js'`

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Aplicada en producción: SÍ (Resolvió cascada de errores #425 en VPS).
