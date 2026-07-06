# Skill 08: Real-Time UI Performance Engineer

## 1. Propósito
Evitar la degradación de FPS, re-renders masivos y fugas de memoria en interfaces que reciben ráfagas extremas de actualizaciones por WebSockets (High-Frequency UI Updates). Dominar la memoización (`React.memo`, `useMemo`), la virtualización del DOM y el control de la cadencia de estado (Throttling/Debouncing visual).

## 2. Aplicación directa en ARBITRAGEX
El feed en vivo de oportunidades de MEV puede emitir ráfagas si el mempool se satura. Renderizar una tabla con 1000 filas parpadeando cada 200ms congelará el DOM del navegador. Se debe utilizar renderización incremental o list virtualization (TanStack Virtual), así como memoización de celdas y filas, y desvincular los timers puros del Main Thread de renderización de React.

## 3. Problemas que resuelve
- Congelamiento del navegador / Scroll lag debido a sobrecarga del Main Thread.
- Basura excesiva en Memoria (Garbage Collector pausas largas).
- Re-render de toda la tabla cuando solo una fila o una celda cambia.
- `Maximum update depth exceeded` por ciclos reactivos infinitos.

## 4. Reglas Inmutables
- En listas de más de 100 elementos que sufren re-ordenamiento o actualizaciones rápidas, **es obligatorio el uso de Virtualización** (`@tanstack/react-virtual`).
- Las filas de una tabla compleja deben estar aisladas en un sub-componente envuelto en `React.memo`, recibiendo props primitivas o estabilizadas por `useCallback`.
- Para métricas que cambian a alta frecuencia (ej. Ticker de "Age Seconds"), NO guardes ese dato en el Provider Global ni en el objeto de la lista, haz que la propia Fila administre su timer local o use animaciones CSS si es posible, para no forzar re-renders del root parent.

## 5. Nivel de Madurez
Maestría - Arquitectura HFT (High-Frequency Trading) Frontend.
