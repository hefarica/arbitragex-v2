# Riesgos

- **Riesgos técnicos**: Archivos de logs infinitos que colapsan el disco del VPS (Logs rotation es obligatorio).
- **Riesgos de seguridad**: Imprimir la `PRIVATE_KEY` en un log de error de viem o ethers-rs. (Error clásico que filtra llaves al enviar telemetría a Datadog/Sentry).
- **Riesgos financieros**: Que el sistema "piense" que está en modo paper-trading, pero use un backend que realmente envía transacciones (falsa sensación de seguridad).

## Mitigaciones
- Usar cuentas diferentes para Paper Trading (una wallet sin fondos reales en absoluto). Si la lógica envía la transacción, fallará en red por falta de fondos, siendo una doble barrera de seguridad.
