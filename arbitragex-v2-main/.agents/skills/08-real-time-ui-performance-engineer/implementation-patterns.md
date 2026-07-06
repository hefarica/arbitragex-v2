# Patrones Correctos (Implementation)

## Patrón 1: Fila Memoizada Pura
Evita que toda la tabla se repinte cuando entra una nueva oportunidad, solo se pinta la fila nueva.

```tsx
import { memo } from 'react';

// Subcomponente independiente
const OpportunityRow = memo(({ item, onAction }) => {
  // Solo se re-renderiza si `item` cambia en memoria.
  return (
    <tr>
       <td>{item.pair}</td>
       <td>{item.profit}</td>
       <td><button onClick={() => onAction(item.id)}>Action</button></td>
    </tr>
  );
}, (prev, next) => prev.item.id === next.item.id && prev.item.profit === next.item.profit);

// Componente Padre
export function OppTable({ items }) {
  // onAction DEBE estar memoizado
  const handleAction = useCallback((id) => executeOp(id), []);
  
  return (
    <tbody>
      {items.map(i => <OpportunityRow key={i.id} item={i} onAction={handleAction} />)}
    </tbody>
  )
}
```

## Patrón 2: Time-slicing o Throttle de WS
Si el Socket escupe 500 ops/seg, limita la UI a 5 frames por segundo (cada 200ms).

```ts
// En el servicio WebSocket, empuja a un buffer y actualiza la UI a intervalos.
let buffer = [];
socket.on('data', (d) => buffer.push(d));

setInterval(() => {
  if (buffer.length > 0) {
    flushBufferToReactState(buffer);
    buffer = [];
  }
}, 200); // 5 FPS (Fluidly readable, zero lag)
```
