# Ejemplos seguros

## Executor Simulado (LogExecutor)
Este Executor nunca enviará una transacción. Es 100% seguro y vital para Paper Trading, auditoría y monitoreo.

```rust
use async_trait::async_trait;

pub struct LogExecutor;

pub enum Action {
    SubmitArbitrage { expected_profit: f64, path: String },
}

#[async_trait]
impl Executor<Action> for LogExecutor {
    async fn execute(&self, action: Action) -> anyhow::Result<()> {
        match action {
            Action::SubmitArbitrage { expected_profit, path } => {
                // SOLO MONITOREO/LOGGING, SIN RIESGO FINANCIERO
                tracing::info!(
                    "SIMULATED EXECUTION: Arbitrage on path [{}] - Expected Profit: {}", 
                    path, expected_profit
                );
            }
        }
        Ok(())
    }
}
```

## Definición de Eventos Centrales Seguros
Los eventos transmitidos internamente evitan acoplar la red al bot.
```rust
#[derive(Clone, Debug)]
pub enum Event {
    NewBlock(u64),
    NewTransaction(ethers::types::Transaction),
    PriceUpdate { token: String, price: f64 },
}
```
