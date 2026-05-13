# Plan de aplicación de skills al proyecto arbitragex_v2_productivo_full

## Resumen ejecutivo
ArbitrageX v2 requiere fusionar el conocimiento profundo de infraestructura MEV, manejo de WebSockets en Viem/Alchemy, y arquitecturas modulares en Rust (Artemis) en un producto seguro y predecible. Este plan detalla cómo aplicar las skills recién adquiridas al repositorio actual para llevarlo de su estado de desarrollo hacia un entorno de `Paper Trading` seguro en Mainnet.

## Skills críticas aplicables
- `safe-production-observability`: Para instrumentar el `api-server` y los componentes de TS.
- `alchemy-rpc-robust-integration`: Para garantizar que el bot `searcher-rs` no se caiga cuando Alchemy hace throttling.
- `rust-mev-architecture`: Para simular rentabilidad real de oportunidades en Rust sin gastar gas.

## Arquitectura recomendada
Implementar el paradigma **C-S-E (Collector, Strategy, Executor)** de Artemis dentro del ecosistema actual de ArbitrageX:
1. **Collector (searcher-rs)**: Escucha WebSockets hacia Alchemy. Emite eventos unificados al broker/frontend.
2. **Strategy Engine (api-server + rust)**: Filtra y detecta oportunidades.
3. **Risk Engine (NUEVO)**: Interceptor estricto antes del Executor. Evalúa `arbx-mev-ethics-gate`.
4. **Executor (Mock por ahora)**: Un log de paper-trading que almacena resultados en la base de datos de PostgreSQL en lugar de interactuar con Flashbots.

## Componentes sugeridos
- **Data collectors**: Módulos Rust dedicados solo a ingestar PendingTransactions y BlockHeaders.
- **RPC provider layer**: Un wrapper `RetryClient` para HTTP (Alchemy).
- **WebSocket layer**: Implementación de Heartbeats en la conexión Alchemy-Rust y Alchemy-Viem (Frontend).
- **Strategy engine**: Módulo puro (sin efectos secundarios) para cálculos matemáticos (Slippage, Gas, Ganancia).
- **Simulation engine**: Revm o node fork para ejecutar la transacción en local y garantizar rentabilidad.
- **Risk engine**: Validaciones lógicas y éticas (Circuit breaker financiero).
- **Execution controller**: Switch maestro que elige entre `LogExecutor` (Simulación) y `MempoolExecutor` (Real).
- **Observability layer**: Exportador de latencias al dashboard usando Pino y Tracing.
- **Secrets manager**: Integrar Vault o `.env` estricto con redaction.
- **Testing layer**: Mocks de eventos para inyectar en el Engine sin conectar a internet.

## Riesgos detectados
1. **Manejo de WebSockets en el Frontend**: Si el frontend no tiene el workaround de reconexión, el panel de oportunidades dejará de mostrar datos en vivo, dando falsa seguridad.
2. **Backpressure en Rust**: Si el `searcher-rs` usa canales no limitados y la red tiene un pico de volumen, el VPS se quedará sin RAM.
3. **Falsa Rentabilidad**: Estimar gas con APIs públicas sin simular la ejecución de la EVM llevará a fallos de transacción que consumirán fondos.

## Fases recomendadas
1. **Auditoría inicial**: Revisar el `searcher-rs` para verificar que implementa MPSC channels seguros.
2. **Integración segura de datos**: Actualizar los transportes en `frontend/lib/api-client.ts` con configuración `keepAlive` y reconexión.
3. **Simulación**: Reemplazar lógica de ejecución por un `LogExecutor` puro.
4. **Paper trading**: Inyectar datos reales del mempool y guardar los resultados calculados en PostgreSQL para revisar.
5. **Observabilidad**: Añadir dashboards de latencia.
6. **Risk engine**: Implementar las reglas del Master Prompt en código.
7. **Revisión humana**: Analizar 1 semana de datos de Paper Trading.
8. **Producción controlada**: Solo si el beneficio neto es mayor a cero sistemáticamente, y solo bajo supervisión directa.

## Checklist antes de producción
- [ ] Mocks eliminados de la capa visual (Todo dato viene de la DB o RPC).
- [ ] Latencia del VPS verificada < 5ms contra el RPC de ejecución.
- [ ] El proceso puede sobrevivir a la desconexión del cable de red durante 10 minutos y recuperarse.
- [ ] Variables `.env` inyectadas correctamente en producción, con secrets protegidos.
- [ ] Paper Trading documenta el éxito de un arbitraje en la DB.

## Zonas que deben permanecer en simulación
- **La ejecución final**: El envío real de la transacción firmada debe estar desactivado (`ARBX_PAPER_TRADE=true`) hasta que se complete la Fase 7.
- Operaciones complejas de Sandwich defensivo o Liquidaciones profundas.

## Preguntas pendientes
- ¿El entorno actual de VPS tiene suficientes cores para asignar (CPU Pinning) tareas asíncronas al bot MEV sin interrumpir la DB PostgreSQL?
- ¿El plan actual de Alchemy soporta el volumen de un nodo HFT si se aumentan las CUs por segundo?
