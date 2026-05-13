# Observabilidad y Producción Segura

## Nivel
Nivel experto en operaciones (SRE/DevOps).

## Propósito
Otorgar al agente la capacidad de implementar un "Risk Engine", instrumentación profunda y reglas estrictas antes de ejecutar sistemas financieros autónomos en mainnet, garantizando una operación auditable, monitoreada y segura.

## Fuente de aprendizaje
Reglas internas de ética Web3, ingeniería de sistemas y buenas prácticas de producción extraídas de Artemis/Pawel Urbanek.

## Conocimiento interiorizado
- **Risk Engine**: Un módulo evaluador obligatorio que audita cualquier "Transaction Intent" antes de enviarlo. Valida que `Profit > Gas + Slippage`, que no interactúa con contratos bloqueados y que respeta límites éticos.
- **Circuit Breakers**: Mecanismo que detiene todo el trading si se detectan anomalías (ej. RPC latencia > 500ms, balance de la wallet del bot baja bruscamente, errores de gas consecutivos).
- **Paper Trading**: Ejecutar todo el stack tecnológico sin enviar transacciones. El sistema piensa que está en producción, pero un Mock Executor registra la transacción simulada en DB.
- **Secrets Management**: Claves privadas y variables críticas deben inyectarse en memoria, sin tocar el disco ni logs.

## Cuándo activar esta skill
- Antes de mover código de staging a mainnet.
- Al configurar el entorno de ejecución del servidor `api-server` o `searcher-rs`.
- Al escribir logs (`console.log` o `tracing::info`).

## Cuándo no activar esta skill
- Nunca saltarse este paso para despliegues con dinero real.

## Entradas necesarias
- Políticas de riesgo (Max Slippage, Min Profit).
- Herramientas de logging (Winston, Tracing, Datadog, Prometheus).

## Procedimiento paso a paso
1. Reemplazar todos los `println!` o `console.log` puros con un framework de logging estructurado (JSON).
2. Filtrar logs para enmascarar `API_KEY` o `PRIVATE_KEY` (Redaction).
3. Implementar métricas de salud (Health Checks) en `/health` que expongan si el WSS está conectado y la latencia actual.
4. Habilitar la variable de entorno `ARBX_PAPER_TRADE=true` por defecto. Forzar que `false` requiera una revisión humana.
5. Definir la lógica de "Kill Switch" (Botón de pánico) accesible vía API o Panel de Control.

## Salidas esperadas
- Configuración de logging, variables seguras y validación estricta.

## Aplicación al proyecto actual
Aplicable al `api-server` de ArbitrageX para habilitar las compuertas de ética (`arbx-mev-ethics-gate`). Se requiere instrumentación en el frontend para monitorear latencia y estado de conexión en tiempo real.

## Aplicación a futuros proyectos
Sistemas de tesorería, wallets multisig automáticas o cualquier bot que administre capital.

## Buenas prácticas
- Guardar eventos de simulación en base de datos para auditar por qué el bot habría tomado una decisión antes de activarlo.
- Usar identificadores únicos (`request_id` / `correlation_id`) por cada transacción detectada.

## Errores comunes
- Loggear el Payload completo de una transacción incluyendo la clave privada firmada.
- No configurar alertas (Ej. que envíe un Telegram si se apaga el WebSocket).

## Riesgos técnicos
- Logging sincrónico bloqueante: En Rust, imprimir demasiados logs a consola puede bloquear el hilo principal, reduciendo velocidad MEV. Usar Non-blocking File Appender.

## Riesgos legales, éticos o financieros
- Este módulo *es* el encargado de evitar ataques manipulativos y fugas de capital. Sin él, el bot no es auditable ni seguro.

## Controles de seguridad
- Validar `ETH_BALANCE > MINIMUM_BALANCE` antes de intentar ejecutar transacciones que exijan gas.

## Checklist operativo
- [ ] Paper Trading es el modo predeterminado.
- [ ] API de Kill Switch protegida por Token/JWT.
- [ ] Logs estructurados (JSON).
- [ ] Alertas por pérdida de conexión configuradas.

## Ejemplo seguro
Ver `examples.md`.

## Dependencias
- Rust `tracing`, `tracing-subscriber`. Node `winston` o `pino`.

## Métricas de calidad
- Capacidad de reconstruir la historia y decisiones del bot a través de los logs estructurados sin ejecutar el sistema localmente.

## Criterios de finalización
- El bot cuenta con métricas exportables a Prometheus/Grafana y el modo Paper Trading funciona correctamente sin tocar un RPC Mainnet real de escritura.
