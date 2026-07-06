# Ejemplos seguros

## Creación de un Cliente Resiliente

```typescript
import { createPublicClient, webSocket } from 'viem';
import { mainnet } from 'viem/chains';

// La configuración incluye parámetros específicamente diseñados para lidiar
// con los problemas reportados en GitHub (issues 2563 y 2325)
const publicClient = createPublicClient({
  chain: mainnet,
  transport: webSocket('wss://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY', {
    retryCount: 10,
    retryDelay: 2000,
    keepAlive: true,
  }),
});
```

## Staleness Watchdog (Perro Guardián de Datos)
Monitorea la salud del socket basado en la frecuencia de los bloques recibidos.
```typescript
let lastBlockTime = Date.now();

const unwatch = publicClient.watchBlockNumber({
  onBlockNumber: (blockNumber) => {
    lastBlockTime = Date.now();
    console.log(`New block received: ${blockNumber}`);
  },
  onError: (error) => console.error('Watchdog caught error:', error),
});

// Checkeador periódico
setInterval(() => {
  const timeSinceLastBlock = Date.now() - lastBlockTime;
  if (timeSinceLastBlock > 45000) { // 45 segundos sin bloques (Ethereum emite cada ~12s)
    console.warn("Possible silent disconnect detected! Forcing reconnect logic...");
    // Logic to re-instantiate client or show error UI
  }
}, 10000);
```
