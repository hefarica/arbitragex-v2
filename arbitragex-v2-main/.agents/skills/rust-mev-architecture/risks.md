# Riesgos

- **Riesgos técnicos**: Fuga de memoria o canales MPSC bloqueados por picos de spam en la red (ej. ataque DDoS a la blockchain donde entran 100k tx/s al mempool).
- **Riesgos de infraestructura**: Consumo de CPU al 100% matando el contenedor si no hay backpressure (descartar transacciones cuando el canal esté lleno).
- **Riesgos RPC**: Hacer llamadas al RPC desde el bucle principal asíncrono causará cuellos de botella masivos; el RPC siempre debe consultarse concurrentemente.
- **Riesgos de concurrencia**: Mutabilidad compartida sin los bloqueos (Locks/RwLocks) correctos, o mantener bloqueos asíncronos (`tokio::sync::Mutex`) en operaciones bloqueantes síncronas.
- **Riesgos de datos**: Confiar en simulaciones off-chain y obviar validaciones on-chain. El contrato siempre es la barrera final.
- **Riesgos financieros/legales**: Diseñar bots que se dediquen a robar fondos o a abusar del mempool (como un ataque eclipse). Prohibido categóricamente en el proyecto.

## Mitigaciones
- Backpressure: Usar tamaño fijo en canales MPSC y usar `try_send` descartando exceso, prefiriendo perder una tx que matar el bot.
- Validaciones on-chain absolutas ("Revert if not profitable").
