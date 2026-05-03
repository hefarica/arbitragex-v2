# Skills del Agente - Índice Global

## Resumen
Cantidad total de skills generadas: 6
Dominios cubiertos: RPC, Web3, Infraestructura Cloud, Rust, MEV Architecture, TypeScript/viem, DevOps, Observabilidad, Seguridad en Producción.

## Tabla de skills

| Skill | Dominio | Fuente | Cuándo activarla | Riesgo | Estado |
|---|---|---|---|---|---|
| `alchemy-rpc-robust-integration` | Web3 / RPC | Alchemy Docs | Caídas RPC, Rate Limits (429) | Medio | Activa |
| `cloud-low-latency-infrastructure` | Cloud / Network | Alibaba Cloud | Setup del VPS de producción, Auditoría de latencia | Alto | Activa |
| `rust-mev-architecture` | Rust / Core | Pawel Urbanek | Desarrollo de `searcher-rs` y lógica de bots | Alto | Activa |
| `artemis-bot-framework` | Arquitectura / MEV | Paradigm Artemis | Refactorización, Aislamiento de componentes, Testing | Bajo | Activa |
| `viem-websocket-resilience` | Frontend / TypeScript | GitHub wevm/viem | Frontend no actualiza bloques, Desconexión silenciosa WS | Medio | Activa |
| `safe-production-observability` | DevOps / Seguridad | Síntesis MEV Ético | Despliegue, Logging, Ejecución final, Implementación de límites | Crítico | Activa |

## Skills prioritarias para el proyecto actual

1. **Críticas**: 
   - `safe-production-observability`: Sin un Risk Engine y Paper Trading, el bot podría causar daños financieros y no cumpliría las directrices de ética.
2. **Altas**: 
   - `alchemy-rpc-robust-integration`: La conexión al mempool es la sangre del proyecto.
   - `viem-websocket-resilience`: El frontend debe ser capaz de recuperarse.
3. **Medias**: 
   - `artemis-bot-framework`: Excelente patrón para organizar el código en Rust.
   - `rust-mev-architecture`: Ayuda a implementar simulación asíncrona robusta.
4. **Complementarias**: 
   - `cloud-low-latency-infrastructure`: Vital para ganar en producción (Mainnet), pero no bloquea el desarrollo local ni el Paper Trading.

## Mapa de activación automática

- **Tareas de Alchemy/RPC**: Se activa `alchemy-rpc-robust-integration`.
- **Problemas con TypeScript o React conectados a la Blockchain**: Se activa `viem-websocket-resilience`.
- **Desarrollo del motor en Rust**: Se activan `rust-mev-architecture` y `artemis-bot-framework`.
- **Despliegues en el VPS**: Se activan `cloud-low-latency-infrastructure` y `safe-production-observability`.
- **Escritura de logs y variables de entorno**: Se activa `safe-production-observability`.

## Restricciones globales

- **Prohibido el uso para manipulación**: Estas skills NUNCA deben emplearse para crear ataques Sandwich abusivos, frontrunning malicioso contra usuarios minoristas, o cualquier técnica prohibida en el master prompt de ArbitrageX.
- **Paper Trading Obligatorio**: Todo desarrollo en mainnet debe iniciar obligatoriamente con un *Mock Executor* y solo loggear las transacciones sin firmarlas ni enviarlas.
- **Protección de Secretos**: Las claves nunca se guardan en variables duras (hardcoded) ni se imprimen en consola, por defecto usar *Redacted Loggers*.
