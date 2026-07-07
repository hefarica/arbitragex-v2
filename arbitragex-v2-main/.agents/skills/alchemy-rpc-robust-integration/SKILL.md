# Integración Robusta de Alchemy RPC

## Nivel
Nivel experto avanzado, equivalente a formación de maestría e investigación técnica aplicada en su rama.

## Propósito
Otorgar la capacidad de diseñar, integrar y consumir endpoints RPC/WebSocket de Alchemy garantizando resiliencia, alta disponibilidad, evasión de rate limits y recuperación automática ante caídas, asegurando que las operaciones on-chain y lecturas de mempool no se interrumpan.

## Fuente de aprendizaje
https://www.alchemy.com/docs

## Conocimiento interiorizado
Alchemy actúa como la puerta de enlace hacia la blockchain. El conocimiento clave radica en no confiar ciegamente en la conexión:
- **Rate Limits (HTTP 429)**: Alchemy aplica Capacity Units (CUs) por segundo. Superarlos causa rechazo de peticiones.
- **WebSocket Drops**: Las conexiones WSS pueden cerrarse silenciosamente o por timeouts del servidor (ej. balanceo de carga).
- **Consistencia de Nodos**: En configuraciones con balanceadores de carga, leer inmediatamente después de escribir puede arrojar datos "viejos" (stale reads) si el nodo no se ha sincronizado.
- **Filtros Avanzados**: Alchemy provee APIs mejoradas (ej. `alchemy_pendingTransactions`) específicas para MEV y monitoreo de mempool.

## Cuándo activar esta skill
- Al diseñar clientes RPC para bots o frontends.
- Al manejar errores HTTP 429, 502, 503 o 504 en logs.
- Al implementar `searcher` bots que escuchan pending transactions.
- Al observar desconexiones de WebSockets.

## Cuándo no activar esta skill
- No usar para abusar de los rate limits intencionalmente (DDoS).
- No hardcodear API keys de Alchemy bajo ninguna circunstancia.

## Entradas necesarias
- `ALCHEMY_API_KEY` (inyectado por entorno).
- Tipo de red (Mainnet, Goerli, Arbitrum).
- Transporte (HTTP/WebSocket).

## Procedimiento paso a paso
1. Identificar la necesidad (HTTP para llamadas únicas, WSS para streaming).
2. Configurar el cliente base inyectando la URL con el secreto desde Vault/ENV.
3. Envolver las llamadas HTTP en un wrapper con *Exponential Backoff* y *Jitter*.
4. Configurar listeners de WSS para los eventos `close`, `error` y `end`, disparando reconexión automática tras un retraso progresivo.
5. Emplear un proveedor de fallback (RPC redundante) si la reconexión a Alchemy falla N veces.

## Salidas esperadas
- Configuración de transporte RPC/WSS tolerante a fallos.
- Manejo estructurado de reconexiones sin pérdida de estado.

## Aplicación al proyecto actual
Aplicable en `C:\Users\HFRC\Desktop\arbitragex_v2_productivo_full` para estabilizar el `searcher-rs` (Rust) y el `api-server` (TypeScript) cuando leen eventos de la blockchain. Ayuda a evitar que el bot se cuelgue si Alchemy reinicia sus nodos.

## Aplicación a futuros proyectos
Cualquier dApp o bot que requiera lectura estable on-chain y monitoreo de eventos.

## Buenas prácticas
- Usar variables de entorno para endpoints.
- Mantener un health-check paralelo para detectar conexiones WSS zombies (con heartbeat/ping).
- Cachear respuestas estáticas (`eth_chainId`, `eth_getBlockByNumber` pasados).

## Errores comunes
- Asumir que la conexión WSS durará para siempre.
- Ignorar el error HTTP 429 y seguir spameando.
- Hardcodear la API Key.

## Riesgos técnicos
- **Latencia RPC**: Las llamadas HTTP tienen latencia variable. En MEV, la latencia a Alchemy (si no está colocalizado) puede significar la pérdida de una oportunidad.
- **Fuga de memoria**: Acumular event listeners en cada reconexión WSS sin limpiar los anteriores.

## Riesgos legales, éticos o financieros
- Usar planes gratuitos para producción intensiva puede romper TOS de Alchemy.
- Fallar en leer el mempool puede causar envío de transacciones ciegas y pérdida de gas.

## Controles de seguridad
- Validar formato de la URL RPC.
- Rotar API keys periódicamente.
- Circuit breaker: detener el bot si Alchemy falla repetidamente y no hay fallback.

## Checklist operativo
- [ ] API Key inyectada vía `.env`.
- [ ] Backoff exponencial implementado en llamadas HTTP.
- [ ] Heartbeat implementado en WSS.
- [ ] Lógica de reconexión WSS validada.

## Ejemplo seguro
Ver `examples.md`.

## Dependencias
- Rust: `ethers-rs` / `alloy`.
- Node: `viem`, `ethers.js`.

## Métricas de calidad
- <0.1% de llamadas RPC fallidas por rate limit.
- 100% de recuperación ante cortes de WSS.

## Criterios de finalización
- Cliente RPC implementado, testeado forzando un cierre de red (ej. apagando WiFi) y verificando la recuperación automática.
