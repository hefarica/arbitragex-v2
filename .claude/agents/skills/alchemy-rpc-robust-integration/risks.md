# Riesgos

- **Riesgos técnicos**: Desconexión silenciosa del WebSocket donde la librería no emite el evento `close`. Requiere latidos (heartbeats).
- **Riesgos de infraestructura**: Alchemy es un punto único de falla (SPOF). Si se cae, el bot queda ciego.
- **Riesgos de red**: Latencia asimétrica (alta variabilidad entre request y request).
- **Riesgos RPC**: Recibir estados inconsistentes debido a balanceadores internos de Alchemy.
- **Riesgos de concurrencia**: Multiplicar llamadas HTTP concurrentes puede agotar la cuota mensual en minutos.
- **Riesgos financieros**: Enviar transacciones ciegas porque el feed de datos estaba pausado.
- **Riesgos legales/éticos**: N/A.

## Mitigaciones
- Implementar Multi-RPC fallback (ej. usar Infura/Quicknode como secundario).
- Usar un sistema local (ej. Reth/Erigon) para MEV avanzado, usando Alchemy solo como respaldo de lectura.
