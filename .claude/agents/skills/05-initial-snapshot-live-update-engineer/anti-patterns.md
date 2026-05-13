# Antipatrones Prohibidos

## Antipatrón 1: Socket Only Paradigm
El dashboard se monta vacío y depende enteramente de que el backend empuje datos por WS. Si el mempool está tranquilo y no hay eventos en 10 minutos, el usuario verá una tabla "Loading" y creerá que el sistema está caído, a pesar de tener historial de hace 15 minutos en Redis.

## Antipatrón 2: Blind Array Push
```tsx
// 🔴 PROHIBIDO
socket.on('event', (data) => {
   setItems(prev => [...prev, data]); 
   // Múltiples re-conexiones del socket inyectarán el mismo evento múltiples veces rompiendo las `key` en React.
});
```
