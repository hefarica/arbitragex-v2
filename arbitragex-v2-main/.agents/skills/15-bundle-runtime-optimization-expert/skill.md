# Skill 15: Bundle & Runtime Optimization Expert

## 1. Propósito
Asegurar que el motor de JavaScript, el runtime de Node (Servidor) y el motor V8 (Cliente) corran con la menor fricción posible. Eliminar memory leaks (fugas de memoria) debidas a closures no liberados en React, optimizar algoritmos recursivos o iterativos pesados y garantizar un uso eficiente de Web Workers en casos de cómputo intensivo del lado del cliente.

## 2. Aplicación directa en ARBITRAGEX
ArbitrageX procesa transacciones, cálculos de simulaciones de ganancia, validación de Zod profunda en grandes arreglos de datos cada segundo. Una ineficiencia asintótica `O(N^2)` dentro de la búsqueda de oportunidades colapsaría la pestaña de Chrome u obligaría al servidor Node a entrar en pánico por saturación del Event Loop.

## 3. Problemas que resuelve
- Picos de CPU y cuellos de botella en el Event Loop (Node.js/Edge).
- Pestañas de Chrome que crashean con el mensaje "Out of Memory" o "Aw, Snap!".
- Lentitud progresiva (La aplicación funciona bien el minuto 1, pero al minuto 30 está intocable).
- Tiempos de parseo JSON masivos bloqueando el hilo principal.

## 4. Reglas Inmutables
- Cierre estricto de suscripciones: TODO `setInterval`, `addEventListener`, o conexión de `socket.on` debe tener su limpieza (`clearInterval`, `removeEventListener`, `socket.off`) en el *Cleanup Function* del `useEffect`.
- Si se deben iterar, transformar o filtrar miles de objetos en el front-end frecuentemente, usa un `useMemo` con una key de dependencia exacta. No lo recalcules en el cuerpo de la función.
- Evita mutaciones y clonaciones masivas (`JSON.parse(JSON.stringify(obj))` o `{...giantObject}`) dentro del render loop en secuencias muy rápidas (ej. eventos de scroll o resize).

## 5. Nivel de Madurez
Maestría - Debugging avanzado en Chrome Memory Profiler.
