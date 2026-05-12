# Patrones Correctos (Implementation)

## Patrón 1: Efectos Libres de Fugas
```tsx
// 🟢 CORRECTO
useEffect(() => {
  const handleScroll = throttle(() => { ... }, 100);
  
  window.addEventListener('scroll', handleScroll);
  
  // Limpieza estricta: Previene que si el componente se desmonta 10 veces,
  // haya 10 oyentes fantasmas robando memoria.
  return () => window.removeEventListener('scroll', handleScroll);
}, []);
```

## Patrón 2: Optimización O(N) Hash Maps
En el frontend de ArbitrageX, al recibir una lista de IDs actualizada desde el backend, cruzarla contra el estado existente de manera rápida.

```tsx
// 🟢 CORRECTO: Búsqueda O(1) usando Set
const highlightIds = useMemo(() => new Set(newUpdates.map(u => u.id)), [newUpdates]);

// O(N) en lugar de O(N^2)
const result = items.map(item => ({
  ...item,
  isUpdated: highlightIds.has(item.id)
}));
```
