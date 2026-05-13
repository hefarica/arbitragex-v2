# REGLA INMUTABLE DE HIDRATACIÓN NEXT/REACT PARA ARBITRAGEX

En toda página SSR/App Router, especialmente /opportunities, /dashboard, /scanner, /chains, /dexes, /assets y paneles live, queda prohibido renderizar en el primer render cualquier valor no determinístico: Date.now(), new Date(), Math.random(), crypto.randomUUID(), window, document, localStorage, sessionStorage, navigator, matchMedia, WebSocket state, medidas de pantalla, estado de conexión, timezone/locale del navegador, precios live sin snapshot, contadores calculados solo en cliente, o datos externos que no vengan serializados desde el servidor.

Toda página SSR debe entregar un snapshot inicial estable desde Server Component. El Client Component debe iniciar exactamente con ese snapshot. Después de hydration, useEffect puede activar WebSocket, polling REST, timers, localStorage, mediciones del browser y actualización live. Ningún dato vivo puede modificar el HTML antes de que React termine de hidratar.

Si un dato debe ser distinto entre servidor y cliente, renderizar placeholder estable en servidor y reemplazarlo solo después de mounted=true. Usar suppressHydrationWarning únicamente como escape controlado para timestamps o textos inevitables, nunca para ocultar errores estructurales.

El WebSocket nunca puede ser dependencia obligatoria del primer render. Debe existir fallback REST funcional, reconexión con backoff, timeout explícito, estado visual seguro y cleanup correcto al desmontar. La UI no puede romperse si socket.io falla.

Todo componente que use datos live debe tener:
1. initialSnapshot recibido por props.
2. estado inicial derivado de initialSnapshot.
3. mounted gate para browser-only APIs.
4. claves estables en listas.
5. cero Math.random/Date.now en render.
6. cero window/localStorage/navigator en render.
7. fallback REST cuando WebSocket falle.
8. ErrorBoundary o estado de degradación controlada.
9. build local en modo development para capturar el componente exacto.
10. validación final con consola limpia: cero #418, cero #423, cero #425.
