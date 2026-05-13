# Antipatrones Prohibidos

## Antipatrón 1: Array Mutation Infinito (Memory Leak Vivo)
En sistemas de monitoreo vivos, si guardamos todos los logs sin un techo.

```tsx
// 🔴 PROHIBIDO
socket.on('log', (log) => {
   // A las 5 horas, el array "logs" tendrá millones de objetos. Crash garantizado.
   setLogs(prev => [log, ...prev]); 
});

// 🟢 CORRECTO
socket.on('log', (log) => {
   // Garbage collection forzoso por límite (Ej: 1000 logs máx).
   setLogs(prev => [log, ...prev].slice(0, 1000)); 
});
```

## Antipatrón 2: Recreación de constantes u objetos en cada render
```tsx
export function MyComponent() {
  // 🔴 PROHIBIDO: Este objeto y la regex se recrean en memoria cientos de veces por segundo.
  const DEFAULT_CONFIG = { retries: 3, timeout: 5000 }; 
  const regex = /^[a-z0-9]+$/i;
  // ...
}

// 🟢 CORRECTO: Se definen FUERA del componente o dentro de un useMemo.
const DEFAULT_CONFIG = { retries: 3, timeout: 5000 }; 
const REGEX = /^[a-z0-9]+$/i;

export function MyComponent() {
  // ...
}
```
