# Antipatrones Prohibidos

## Antipatrón 1: Funciones anónimas en listas masivas
Si pasas `onClick={() => execute(id)}` en una lista gigante, la Fila no se puede memoizar porque en cada render del padre, se genera una función en memoria diferente, rompiendo `React.memo`.

## Antipatrón 2: Timer Global Masivo
Tener un `useEffect` con `setInterval` en la página principal que hace un `setNow(Date.now())` y forzar que TODO el árbol de React, incluyendo todos los tooltips, modales y charts, se recalcule cada 1000 milisegundos.
