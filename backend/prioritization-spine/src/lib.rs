pub mod bayesian_allocator;
pub mod config_aware;
pub mod decision;
pub mod erc20_storage;
pub mod errors;
pub mod evidence;
pub mod execute_arbitrage_encoder;
pub mod feedback;
pub mod gates;
pub mod lazy_db;
pub mod round_trip_executor;
pub mod route_plan;
pub mod scoring;
pub mod simulator;
pub mod strategy_config_gate;
pub mod strategy_scores_db;
pub mod swap_encoder;
pub mod types;
pub mod validated_plan;

pub use bayesian_allocator::{Allocation, AllocationSource, BayesianAllocator, BetaPosterior};
pub use config_aware::{ConfigAwareEvaluator, ConfigGateOutcome, NetworkSignals};
pub use decision::*;
pub use errors::*;
pub use evidence::*;
pub use execute_arbitrage_encoder::{
    build_execute_arbitrage_calldata, build_execute_arbitrage_flash_funded_calldata,
    build_flash_funded_broadcast_calldata, build_flash_funded_broadcast_calldata_with_intermediate,
    build_request_flash_loan_calldata, ExecuteArbitrageEncodeError,
    EXECUTE_ARBITRAGE_FLASH_FUNDED_SELECTOR, EXECUTE_ARBITRAGE_SELECTOR, REQUEST_FLASH_LOAN_SELECTOR,
};
pub use feedback::{AdaptiveSignal, FeedbackChannel, SIGNAL_TTL};
pub use gates::*;
pub use route_plan::{RouteLeg, RoutePlan};
pub use scoring::*;
pub use strategy_config_gate::{GateOutcome, StrategyConfigGate};
pub use strategy_scores_db::{StrategyFailRate, StrategyScoresCache};
pub use types::*;
pub use validated_plan::ValidatedPlan;
