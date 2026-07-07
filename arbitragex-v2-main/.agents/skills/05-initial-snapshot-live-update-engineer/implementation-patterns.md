# Patrones Correctos (Implementation)

## Patrón 1: Deduplicated Snapshot-Stream Merge
```tsx
export function LiveFeed() {
  const [items, setItems] = useState<Item[]>([]);

  // 1. Snapshot Initial
  useEffect(() => {
    fetch('/api/opportunities/live')
      .then(res => res.json())
      .then(data => setItems(data.items));
  }, []);

  // 2. Stream Updates
  useEffect(() => {
    const socket = io('https://edge.domain.com');
    socket.on('new_opportunity', (newItem: Item) => {
      setItems(prev => {
        // Regla de Oro: Evitar duplicados cruzados entre el HTTP Fetch tardío y el Socket veloz.
        if (prev.some(i => i.id === newItem.id)) {
          // Ya existe, podemos decidir reemplazar (Actualizar estado) o descartar.
          return prev;
        }
        // Inserción inmutable limitando el array a 50
        return [newItem, ...prev].slice(0, 50);
      });
    });
    return () => socket.disconnect();
  }, []);
}
```
