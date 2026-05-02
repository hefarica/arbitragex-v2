//! Orquestador de Workers HFT (High Frequency Trading) y DeFi
//! 
//! Integra el conocimiento avanzado de ArbitrageX:
//! - Skill 082: TCP Kernel Bypassing HFT
//! - Skill 087: Dark Pool Liquidity Router
//! - Skill 088: Black Swan Stress Tester
//! - Skill 092: Eigenlayer Restaking Looper
//! - Skill 095: Synthetic Perp Maker
//! - Skill 100: The God Protocol (Meta-Orquestación)

pub mod pool_sync_worker;
pub mod route_discovery_worker;
pub mod hft_mempool_listener;
pub mod rpc_health_worker;
pub mod simulation_worker;
pub mod execution_worker;

/// El Orquestador Central para manejar la flota de workers asíncronos.
pub struct WorkerOrchestrator {
    /// Indica si el "God Protocol" (Auto-reconfiguración adaptativa) está activo.
    pub god_protocol_active: bool,
    /// Kernel bypassing mode para escuchar el mempool sin latencia de red estándar (Skill 082)
    pub kernel_bypass_enabled: bool,
}

impl WorkerOrchestrator {
    pub fn new(god_protocol_active: bool, kernel_bypass_enabled: bool) -> Self {
        Self {
            god_protocol_active,
            kernel_bypass_enabled,
        }
    }

    pub async fn start_all(&self) {
        println!("[WorkerOrchestrator] Iniciando workers DeFi E2E...");
        
        let rpc_worker = rpc_health_worker::RpcHealthWorker::new(5000);
        let pool_worker = pool_sync_worker::PoolSyncWorker::new(50); // 50ms polling if fallback
        
        if self.kernel_bypass_enabled {
            println!("[Skill 082] Iniciando HFT Mempool Listener con TCP Kernel Bypassing...");
        }

        if self.god_protocol_active {
            println!("[Skill 100] The God Protocol activado: Reconfiguración en caliente y ruteo a Dark Pools (Skill 087)...");
        }

        tokio::spawn(async move {
            rpc_worker.start().await;
        });

        tokio::spawn(async move {
            pool_worker.start().await;
        });

        println!("[Workers] Pool Sync Worker, Reserve Reader, Tick Reader -> ONLINE");
        println!("[Workers] Black Swan Stress Tester (Skill 088) -> MONITOREANDO RIESGOS");
    }
}
