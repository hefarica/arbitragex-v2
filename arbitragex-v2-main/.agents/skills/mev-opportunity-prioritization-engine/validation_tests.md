# Validation Tests

## Static validation
- Todos los pesos de la configuración de Google Sheets suman 1.0, o se normalizan antes de aplicar.

## Runtime validation
- **Prueba de Invariante de Pérdida:** Crear un test unitario donde `Bribe + Gas > Profit`. El Score debe afirmar `0.0`.
- **Prueba de Degradación (Time-decay):** Una oportunidad cacheada durante 12 segundos (1 bloque Ethereum) debe ver su score penalizado severamente si la liquidez en los DEX es altamente volátil.
