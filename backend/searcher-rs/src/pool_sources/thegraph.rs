//! The Graph pool source — wraps the existing
//! `math_engine::subgraph_client::fetch_top_pools_by_tvl` (V3 primary, V2
//! best-effort) into the common `PoolCandidate` shape. Endpoint comes from
//! `ARBX_SUBGRAPH_URL_<chain>` (no-hardcode); an unconfigured chain contributes
//! nothing (not an error).

use crate::pool_candidate::{PoolCandidate, PoolEnumSource};
use math_engine::subgraph_client::{self, RankedPool, SubgraphPoolKind};

pub async fn fetch(
    chain_id: u64,
    top_n: usize,
    min_tvl_usd: f64,
) -> anyhow::Result<Vec<PoolCandidate>> {
    let mut out = Vec::new();
    // V3 primary. `Ok(None)` = subgraph unconfigured → contribute nothing.
    if let Some(v) =
        subgraph_client::fetch_top_pools_by_tvl(chain_id, SubgraphPoolKind::V3, top_n, min_tvl_usd)
            .await?
    {
        out.extend(v.into_iter().map(map_ranked));
    }
    // V2 best-effort (same endpoint; errors if the configured subgraph is V3-only).
    if let Ok(Some(v)) =
        subgraph_client::fetch_top_pools_by_tvl(chain_id, SubgraphPoolKind::V2, top_n, min_tvl_usd)
            .await
    {
        out.extend(v.into_iter().map(map_ranked));
    }
    Ok(out)
}

fn map_ranked(rp: RankedPool) -> PoolCandidate {
    PoolCandidate {
        address: rp.address,
        token0: Some(rp.token0),
        token1: Some(rp.token1),
        // V3 feeTier is in pips (3000 = 0.30%); bps = pips/100. V2 → None.
        fee_bps: rp.fee_tier.map(|f| f / 100),
        tvl_usd: rp.tvl_usd,
        volume_usd_24h: None,
        source: PoolEnumSource::TheGraph,
    }
}
