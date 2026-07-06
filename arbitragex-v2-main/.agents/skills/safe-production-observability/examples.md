# Ejemplos seguros

## Rust - Risk Engine Básico (Validación antes de ejecutar)
Validar rentabilidad asegurando que el simulador devuelve `true` y el beneficio supera el gas.

```rust
pub struct RiskEngine;

impl RiskEngine {
    pub fn validate_execution(
        expected_profit: f64,
        estimated_gas_cost: f64,
        slippage_tolerance: f64
    ) -> Result<(), String> {
        if expected_profit <= estimated_gas_cost {
            return Err("Net profit is negative or zero after gas costs".to_string());
        }
        if slippage_tolerance > 0.05 { // 5% max slippage
            return Err("Slippage tolerance exceeds safety limits".to_string());
        }
        // Validador de Ética (Reglas personalizadas)
        Ok(())
    }
}
```

## Node.js - Ocultar secretos en Logs (Redaction)
Usando Pino logger para censurar variables.
```javascript
const pino = require('pino');

const logger = pino({
  redact: ['req.headers.authorization', 'private_key', 'env.ALCHEMY_API_KEY'],
  level: process.env.LOG_LEVEL || 'info',
});

// Aún si alguien envía esto:
logger.info({ private_key: "0x123..." }, "Bot initialized");
// Saldrá como: {"level":30,"time":... ,"private_key":"[Redacted]","msg":"Bot initialized"}
```
