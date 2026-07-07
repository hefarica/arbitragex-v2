# Ejemplos seguros

## Filtro inicial de transacciones (Simulación defensiva)
Este fragmento muestra cómo descartar transacciones que no interactúan con los routers que nos interesan, para evitar gastar CPU.

```rust
use ethers::types::{Transaction, Address};
use std::collections::HashSet;

pub fn is_transaction_relevant(tx: &Transaction, target_routers: &HashSet<Address>) -> bool {
    // Solo si tiene datos ("To" address existe) y coincide con routers objetivo
    if let Some(to) = tx.to {
        if target_routers.contains(&to) {
            // El bot podría auditar si la transacción afecta nuestros pools.
            return true;
        }
    }
    false
}
```

## Arquitectura de Canales (MPSC) Módulo Aislado
Un pipeline seguro para conectar la escucha del mempool con el análisis.

```rust
use tokio::sync::mpsc;
use ethers::types::Transaction;

pub async fn start_pipeline() {
    let (tx, mut rx) = mpsc::channel::<Transaction>(10_000);

    // Tarea colectora (simulada)
    tokio::spawn(async move {
        // En la vida real esto viene de una subscripción al mempool
        // tx.send(tx_data).await.unwrap();
    });

    // Tarea analizadora
    tokio::spawn(async move {
        while let Some(transaction) = rx.recv().await {
            // Analizar la transacción para detectar riesgos o simular su estado
            // process_transaction(transaction).await;
        }
    });
}
```
