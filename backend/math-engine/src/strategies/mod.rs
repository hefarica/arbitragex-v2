//! Módulo de Estrategias — Cartuchos Dinámicos Rhai
//!
//! Las 264 estrategias se cargan dinámicamente desde archivos .rhai
//! en backend/searcher-rs/cartridges/strategies/
//!
//! Doctrina de Aislamiento Topológico:
//! - Cada estrategia es un cartucho Rhai individual (264 archivos)
//! - El frontend ensambla y edita los cartuchos
//! - El backend las carga dinámicamente vía Rhai Engine

pub mod strategy_trait;
pub mod registry;
pub mod classifier;

pub use strategy_trait::{TopologicalStrategy, StrategyOutput, StrategyProfile};
pub use registry::StrategyRegistry;
