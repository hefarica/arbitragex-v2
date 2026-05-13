# Riesgos

- **Riesgos técnicos**: La desconexión silenciosa es difícil de reproducir y diagnosticar, el socket queda en estado `OPEN` en el navegador pero el servidor dejó de enviar datos.
- **Riesgos financieros**: Un trader ve un precio que no ha cambiado en 2 minutos (porque se desconectó el socket) y envía una orden que fracasa (slippage) costando gas.
- **Riesgos de infraestructura**: Ninguno directo en el backend.

## Mitigaciones
- Watchdog de bloques: Como los bloques de EVM son deterministas en tiempo, usar la ausencia de nuevos bloques como señal infalible de desconexión.
