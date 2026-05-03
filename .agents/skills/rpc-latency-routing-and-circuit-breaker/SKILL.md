# RPC Latency Routing and Circuit Breaker

## Propósito
Mantener una conexión websocket/HTTP resiliente con múltiples nodos (Geth, Alchemy, QuickNode). Cortar automáticamente (Circuit Break) si la latencia supera el umbral crítico para MEV (<50ms).
