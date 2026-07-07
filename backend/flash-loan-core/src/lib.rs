//! ═══════════════════════════════════════════════════════════════════════════════
//! flash-loan-core — Temporal Liquidity Superposition (TLS) Orchestrator
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Vector 3 de FASE OMEGA: Orquestación de Flash Loans con cálculo de r_flash.
//!
//! ## Uso
//!
//! ```rust
//! use flash_loan_core::orchestrator::{calculate_r_flash, RouteLeg, StepType, SimulationContext};
//!
//! let route = vec![
//!     RouteLeg {
//!         step_type: StepType::FlashLoan,
//!         protocol: "aave_v3".to_string(),
//!         target: "0x...".to_string(),
//!         token_in: "0x...".to_string(),
//!         token_out: "0x...".to_string(),
//!         amount: "1000000000000000000".to_string(),
//!         pool_fee_bps: 30,
//!     },
//!     // ... más legs
//! ];
//!
//! let ctx = SimulationContext {
//!     gas_price_wei: 20_000_000_000,
//!     native_price_usd: 3500.0,
//!     confidence: 0.95,
//! };
//!
//! let result = calculate_r_flash(&route, "1000000000000000000", &ctx, 10.0);
//! ```

// FASE OMEGA — Mismo nivel de rigor que searcher-rs: zero unwrap/expect en producción
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

pub mod orchestrator;

// Re-exports principales para conveniencia
pub use orchestrator::{
    calculate_r_flash,
    FlashOrchestratorError,
    FlashProfitability,
    RouteLeg,
    SimulationContext,
    StepType,
    TlsProvider,
    FLASH_LOAN_FEE_BPS_AAVE,
    FLASH_LOAN_FEE_BPS_BALANCER,
    FLASH_LOAN_FEE_BPS_DYDX,
    MAX_FLASH_FEE_BPS,
};
