//! Simulation Worker
//!
//! Recibe oportunidades detectadas (brutas) por el Event Bus.
//! Ejecuta `eth_call` usando Alloy/Ethers simulando la ejecución on-chain
//! real contra el contrato ArbitrageExecutor para verificar Profit final y Gas.

use tokio::time::{sleep, Duration};
use tracing::{info, debug};

pub struct SimulationWorker {
    pub enabled: bool,
}

impl SimulationWorker {
    pub fn new() -> Self {
        Self { enabled: true }
    }

    pub async fn start(&self) {
        info!("[SimulationWorker] Motor de Simulación Exacta (eth_call) iniciado...");
        
        loop {
            // 1. Escuchar canal MPSC o Redis Pub/Sub por `Opportunity`
            // 2. Construir la data (payload) para ArbitrageExecutor.executeArbitrage
            // 3. Hacer `eth_call` al RPC con un block overrides si es posible
            // 4. Si `eth_call` revierte o da net_profit negativo, descartar.
            // 5. Si es exitosa, mandar a Transaction Builder / Execution Worker.
            
            sleep(Duration::from_millis(100)).await;
        }
    }
}
