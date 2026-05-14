//! Paper-Shadow Mode Connectors — Real Data Ingestion Layer
//!
//! Feature-gated under `paper-shadow`. Provides:
//!
//! 1. `mempool_listener` — WS subscription to real pending transactions
//! 2. `reserve_reader` — eth_call to real pool contracts (getReserves/slot0)
//! 3. `gas_oracle` — Real gas price from eth_gasPrice + EIP-1559
//! 4. `flashbots_simulator` — Dry-run via eth_callBundle (NO signing)
//! 5. `price_feed` — Real spot price from on-chain pool contracts
//!
//! ## ZERO-MOCKS COMPLIANCE
//!
//! Every data source in this module connects to REAL infrastructure:
//! - RPC endpoints via `shared_rs::HttpRpcPool` / `shared_rs::WsRpcPool`
//! - No synthetic data, no hardcoded prices, no fabricated transactions
//!
//! ## CAPITAL EXPOSED: STRICTLY 0
//!
//! - No `SecretKey`, `Signer`, `Wallet`, or `PrivateKey` types exist here
//! - No `eth_sendBundle`, `eth_sendRawTransaction`, or `send_transaction`
//! - Only read-only RPC methods: `eth_call`, `eth_gasPrice`, `eth_subscribe`
//! - Flashbots: `eth_callBundle` (simulation) only, NEVER `eth_sendBundle`

pub mod mempool_listener;
pub mod reserve_reader;
pub mod gas_oracle;
pub mod flashbots_simulator;
pub mod price_feed;
pub mod error;
