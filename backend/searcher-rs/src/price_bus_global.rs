//! Process-wide `PriceBus` handle (WO-PC7). `main.rs` initializes it once at
//! boot; the Binance WS client (writer: bookTicker/depth), the price worker
//! (writer: Chainlink anchors + fused-price persistence) and the scanner
//! cascade (reader: tier 0) all resolve it through here instead of threading
//! an `Arc<PriceBus>` through every scanner signature.

use shared_rs::price_bus::{PriceBus, PriceBusConfig};
use std::sync::{Arc, OnceLock};

static BUS: OnceLock<Arc<PriceBus>> = OnceLock::new();

/// Initialize (or return the already-initialized) process bus. First call
/// wins; subsequent calls are cheap clones of the same `Arc`.
pub fn init() -> Arc<PriceBus> {
    BUS.get_or_init(|| PriceBus::new(PriceBusConfig::default()))
        .clone()
}

/// Read-side accessor. `None` only when `init()` has not run (unit tests that
/// never boot main.rs) — callers must degrade fail-honest, never fabricate.
pub fn get() -> Option<Arc<PriceBus>> {
    BUS.get().cloned()
}
