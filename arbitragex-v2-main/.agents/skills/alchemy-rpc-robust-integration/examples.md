# Ejemplos seguros

## TypeScript (viem) - Wrapper de Reconexión Segura para WSS
Este código demuestra cómo monitorear la salud de un transporte WebSocket y reiniciar de forma segura sin filtrar secretos.

```typescript
import { createPublicClient, webSocket } from 'viem';
import { mainnet } from 'viem/chains';

export function createRobustAlchemyClient(rpcUrl: string) {
  // RPC URl is passed safely, not hardcoded.
  const transport = webSocket(rpcUrl, {
    retryCount: 5,
    retryDelay: 1000, // Ms
    keepAlive: true,
  });

  return createPublicClient({
    chain: mainnet,
    transport,
  });
}
```

## Rust (ethers-rs) - Exponential Backoff Middleware
Para simulación de transacciones.
```rust
use ethers::providers::{Provider, Http, RetryClient};

pub fn build_safe_http_provider(rpc_url: &str) -> Provider<RetryClient<Http>> {
    let http = Http::new(rpc_url.parse().expect("Invalid RPC URL"));
    let retry_client = RetryClient::new(
        http,
        Box::new(ethers::providers::HttpRateLimitRetryPolicy),
        5, // retries
        1000, // initial backoff
    );
    Provider::new(retry_client)
}
```
