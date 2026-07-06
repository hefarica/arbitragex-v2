# Skill 101: React Hydration Forensics & Mounted Snapshot Pattern

## Descripción General
Esta skill define el protocolo forense y arquitectónico para resolver los errores de hidratación más críticos en Next.js App Router (React #425, #418, #423), específicamente cuando ocurren discrepancias de estado entre el renderizado del servidor (SSR) y el cliente (CSR).

## Síntomas Clásicos
Cuando Next.js renderiza un componente en el servidor y luego lo hidrata en el navegador, React compara el árbol HTML inicial con el primer renderizado del cliente. Si hay diferencias, se desencadena una cascada de errores:
1. **Error #425**: "Text content does not match server-rendered HTML" (La raíz del problema).
2. **Error #418**: "Hydration failed because the initial UI does not match what was rendered on the server" (La consecuencia directa).
3. **Error #423**: "There was an error while hydrating. Because the error happened outside of a Suspense boundary, the entire root will switch to client rendering." (El fallback destructivo que degrada el rendimiento).

## Orígenes Frecuentes
- `Date.now()`, `new Date()`, o `Math.random()` evaluados en el renderizado inicial.
- Variables de entorno (`process.env`) evaluadas de forma distinta en servidor (ej. `INTERNAL_EDGE_URL`) vs cliente (ej. `NEXT_PUBLIC_EDGE_URL`).
- Accesos directos a `window.localStorage` o dimensiones de pantalla (`window.innerWidth`) durante el primer renderizado.

## El Patrón: "Mounted Snapshot" (isMounted)
Para componentes `use client` que dependen de datos asíncronos o específicos del cliente, el estado inicial DEBE ser perfectamente determinista y coincidir al 100% con lo que el servidor es capaz de pre-renderizar.

### Implementación Correcta
```tsx
"use client";
import { useState, useEffect } from "react";

export function TimeDependentComponent() {
  // 1. El estado inicial debe ser determinista (0, null, false, o un string fijo)
  const [now, setNow] = useState(0);
  
  // 2. Usar un gate booleano para retrasar el renderizado específico del cliente
  const [isMounted, setIsMounted] = useState(false);

  useEffect(() => {
    // 3. Este efecto SOLO corre en el cliente, DESPUÉS del primer render (hidratación).
    setIsMounted(true);
    setNow(Date.now());
    
    const interval = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(interval);
  }, []);

  return (
    <div>
      {/* 4. Proveer un fallback estático (ej. "--:--") durante la fase de hidratación */}
      <time>
        {isMounted ? new Date(now).toLocaleTimeString() : "--:--:--"}
      </time>
    </div>
  );
}
```

## Reglas Estrictas de Prevención (Zero Hydration Mismatch)
1. **Regla de Determinismo SSR:** El primer renderizado de un `Client Component` debe ser idéntico sin importar si se ejecuta en Node.js o en el Navegador.
2. **Regla del Fallback Visual:** Nunca renderices un `spinner` o un texto de carga diferente al HTML del servidor a menos que esté envuelto en un estado `isMounted`. Usa placeholders (`--`, `Cargando...`) o skeletons estructurales que se apliquen en ambos entornos.
3. **Manejo de Variables de Entorno:** Si un helper (como `getApiBaseUrl()`) devuelve un valor distinto en SSR (Docker interno) vs CSR (Dominio público), su invocación en la UI debe retrasarse hasta que `isMounted === true`.

## Diagnóstico y Auditoría Forense
Si el error #425 aparece en producción:
1. Revisa los logs del cliente y ubica el componente exacto (`at ComponentName`).
2. Haz `grep` en el código de ese componente buscando: `window`, `Date`, `Math`, `localStorage`, y helpers de entorno.
3. Aplica el patrón `Mounted Snapshot`.
4. Importante: ¡Reconstruye la imagen Docker sin caché (`--no-cache`)! Si no se invalida la caché de Next.js, el servidor y el cliente seguirán sirviendo los chunks `.js` y archivos `layout` antiguos, causando un loop infinito de #423.
