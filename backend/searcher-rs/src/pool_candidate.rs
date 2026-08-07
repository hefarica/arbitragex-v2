//! Common candidate type produced by every pool-enumeration source adapter
//! (The Graph / DeFiLlama / GeckoTerminal) and consumed by the
//! `PoolEnumerationWorker`.
//!
//! `token0`/`token1` are HINTS only — the authoritative token0()/token1() come
//! from the on-chain read inside `PoolDiscoveryService::hydrate_and_persist_pool`,
//! which rejects a pool whose on-chain tokens don't match the hints
//! (anti-spoofing). The worker needs the hints to (a) run the token-safety
//! screen and (b) feed the on-chain pair-match guard; a candidate without both
//! token hints is skipped fail-honestly.

/// Which live data source produced a candidate (for provenance / `enum_source`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PoolEnumSource {
    /// On-chain truth anchor — pools enumerated from factory creation events via
    /// the searcher's own RPC (Alchemy). Never depends on a third-party indexer.
    Alchemy,
    TheGraph,
    DeFiLlama,
    /// High-fidelity free (no-key) indexer: on-chain pair addresses with
    /// liquidity + 24h volume. Enumerates via pivot tokens and enriches anchor
    /// pools whose TVL is otherwise unknown.
    DexScreener,
    GeckoTerminal,
}

impl PoolEnumSource {
    pub fn as_str(self) -> &'static str {
        match self {
            PoolEnumSource::Alchemy => "alchemy",
            PoolEnumSource::TheGraph => "thegraph",
            PoolEnumSource::DeFiLlama => "defillama",
            PoolEnumSource::DexScreener => "dexscreener",
            PoolEnumSource::GeckoTerminal => "geckoterminal",
        }
    }

    /// Parse a source name (case-insensitive) from `ARBX_POOL_ENUM_SOURCES`.
    #[allow(clippy::should_implement_trait)] // returns Option, not the std FromStr contract
    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "alchemy" | "onchain" | "on_chain" | "rpc" => Some(PoolEnumSource::Alchemy),
            "thegraph" | "the_graph" | "graph" => Some(PoolEnumSource::TheGraph),
            "defillama" | "llama" => Some(PoolEnumSource::DeFiLlama),
            "dexscreener" | "dexs" => Some(PoolEnumSource::DexScreener),
            "geckoterminal" | "gecko" => Some(PoolEnumSource::GeckoTerminal),
            _ => None,
        }
    }
}

/// A single pool candidate from a live source — TVL-ranked, pre-validation.
#[derive(Debug, Clone)]
pub struct PoolCandidate {
    /// Pool/pair address (lowercase `0x` hex).
    pub address: String,
    /// token0 address hint (lowercase `0x` hex), if the source exposed it.
    pub token0: Option<String>,
    /// token1 address hint (lowercase `0x` hex), if the source exposed it.
    pub token1: Option<String>,
    /// Fee in basis points (already converted from the source's native unit), or
    /// `None` (resolved on-chain during hydration).
    pub fee_bps: Option<u32>,
    /// Total value locked (USD) — ranking/threshold only, never profit math.
    pub tvl_usd: f64,
    /// 24h volume (USD), if the source exposed it (metadata only).
    pub volume_usd_24h: Option<f64>,
    /// Which source produced this candidate.
    pub source: PoolEnumSource,
}

impl PoolCandidate {
    /// Lowercase the address + token hints in place (sources vary in casing).
    pub fn normalise(&mut self) {
        self.address = self.address.trim().to_ascii_lowercase();
        if let Some(t) = self.token0.as_mut() {
            *t = t.trim().to_ascii_lowercase();
        }
        if let Some(t) = self.token1.as_mut() {
            *t = t.trim().to_ascii_lowercase();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_roundtrip() {
        for s in [
            PoolEnumSource::Alchemy,
            PoolEnumSource::TheGraph,
            PoolEnumSource::DeFiLlama,
            PoolEnumSource::DexScreener,
            PoolEnumSource::GeckoTerminal,
        ] {
            assert_eq!(PoolEnumSource::from_str(s.as_str()), Some(s));
        }
        assert_eq!(
            PoolEnumSource::from_str("GECKO"),
            Some(PoolEnumSource::GeckoTerminal)
        );
        assert_eq!(PoolEnumSource::from_str("onchain"), Some(PoolEnumSource::Alchemy));
        assert_eq!(PoolEnumSource::from_str("dexs"), Some(PoolEnumSource::DexScreener));
        assert_eq!(PoolEnumSource::from_str("nope"), None);
    }

    #[test]
    fn normalise_lowercases() {
        let mut c = PoolCandidate {
            address: "0xABC".into(),
            token0: Some("0xDEF".into()),
            token1: None,
            fee_bps: Some(30),
            tvl_usd: 1.0,
            volume_usd_24h: None,
            source: PoolEnumSource::DeFiLlama,
        };
        c.normalise();
        assert_eq!(c.address, "0xabc");
        assert_eq!(c.token0.as_deref(), Some("0xdef"));
    }
}
