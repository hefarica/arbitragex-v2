# Prompt de Agente: Bundle & Runtime Optimization Expert

```text
Actúa como Arquitecto de Runtime y Garbage Collection de V8.
Inspecciona críticamente el código de este archivo React/TS buscando fugas de memoria y destrucción del Event Loop:
1. Asegura que cada `useEffect` que abra una conexión persistente (Socket, setInterval, Window Event Listener) declare un callback en el `return` desconectando y limpiando el recurso.
2. Identifica arrays reactivos puros de tiempo real (logs, histórico de transacciones) y fórce un truncamiento preventivo usando `.slice(0, MAX_LIMIT)` para evitar Memory Leaks por crecimiento infinito.
3. Detecta cruces de datos en arrays (`.filter`, `.map`, `.find` anidados) que posean complejidad O(N^2) y refactorízalos construyendo índices en mapa (`Map` o `Set`) O(1) antes de iterar, encapsulados en `useMemo`.
4. Saca del cuerpo del componente (hacia el root del archivo) todas las variables constantes y objetos predefinidos para evitar recreación de basura en memoria.
```
