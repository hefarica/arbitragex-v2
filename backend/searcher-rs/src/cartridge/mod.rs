//! FASE OMEGA — Dynamic Strategy Cartridge Runtime.
//!
//! This module implements the "PlayStation MEV" architecture: a sandboxed Rhai
//! scripting engine that allows operators to deploy, update, and remove
//! strategy logic at runtime without recompiling the Rust binary.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                        CartridgeRuntime                              │
//! │  ┌───────────────┐   ┌───────────────┐   ┌───────────────────────┐ │
//! │  │ CartridgeRunner│   │ HostBindings  │   │ CartridgeSubscriber   │ │
//! │  │ (Rhai Engine)  │   │ (Redis/RPC)   │   │ (Redis PubSub)       │ │
//! │  └───────┬───────┘   └───────┬───────┘   └───────────┬───────────┘ │
//! │          │                    │                        │             │
//! │          ▼                    ▼                        ▼             │
//! │  ┌───────────────────────────────────────────────────────────────┐  │
//! │  │              Active Cartridges HashMap<String, CompiledAST>   │  │
//! │  └───────────────────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Universal Cartridge Contract
//!
//! Every `.rhai` cartridge MUST export these functions:
//!
//! | Function                          | Purpose                              |
//! |-----------------------------------|--------------------------------------|
//! | `fn init_strategy() -> Map`       | Metadata: name, version, author      |
//! | `fn evaluate_opportunity(pool_data) -> Map` | Core math logic          |
//! | `fn build_payload(opportunity) -> Map`      | Web3 execution assembly  |
//!
//! ## Host Bindings (Peripherals)
//!
//! Scripts access infrastructure through registered native functions:
//!
//! | Binding                            | Backend                              |
//! |------------------------------------|--------------------------------------|
//! | `get_reserves(pool_addr)`          | Redis cache lookup                   |
//! | `get_token_meta(token_addr)`       | Redis token metadata                 |
//! | `simulate_swap(amount, path)`      | RPC quoter call                      |
//! | `get_base_fee()`                   | Mempool gas oracle                   |
//! | `get_block_number()`               | Latest block from chain client       |
//! | `log_quantum(message)`             | Telemetry → Redis → UI               |
//! | `emit_signal(signal_type, data)`   | Publish to Arteria PubSub            |
//!
//! ## Sandboxing
//!
//! - `max_operations`: 1_000_000 (prevents infinite loops)
//! - `max_modules`: 0 (no external module imports)
//! - `max_string_size`: 65_536 bytes
//! - `max_array_size`: 4_096 elements
//! - `max_map_size`: 1_024 entries
//! - Division by zero → runtime error (caught, not panic)
//! - Stack overflow → runtime error (caught, not panic)

pub mod runner;
pub mod host_bindings;
pub mod subscriber;
pub mod contract;
pub mod types;

pub use runner::CartridgeRunner;
pub use subscriber::CartridgeSubscriber;
pub use types::{CartridgeMetadata, CartridgeState, CartridgeEvalResult};
