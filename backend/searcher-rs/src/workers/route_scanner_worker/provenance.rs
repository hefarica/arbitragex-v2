//! Source identity checks at the graph-to-intent boundary. A negative marginal
//! weight is only a candidate: sizing and full EVM simulation remain mandatory.
use super::{ProfitableCycle, RouteIntent, TokenGraph};
use crate::impact_index::PoolRef;
use ethers::types::Address;
use std::collections::{HashMap, HashSet};

pub(super) fn valid_cycle(
    graph: &TokenGraph,
    chain_id: u64,
    block_number: u64,
    cycle: &ProfitableCycle,
) -> bool {
    if chain_id == 0
        || block_number == 0
        || !(2..=7).contains(&cycle.edges.len())
        || cycle.hop_count != cycle.edges.len()
        || !cycle.sum_log_weight.is_finite()
        || cycle.sum_log_weight >= 0.0
    {
        return false;
    }
    let mut pools = HashSet::new();
    let mut first = None;
    let mut previous = None;
    let mut sum = 0.0;
    for &index in &cycle.edges {
        let Some(edge) = graph.edges.get(index) else {
            return false;
        };
        let Some(weight) = edge.log_weight else {
            return false;
        };
        if edge.chain_id != chain_id
            || edge.pool.is_zero()
            || edge.token_in.is_zero()
            || edge.token_out.is_zero()
            || edge.token_in == edge.token_out
            || !weight.is_finite()
            || !pools.insert(edge.pool)
            || previous.is_some_and(|token| token != edge.token_in)
        {
            return false;
        }
        first.get_or_insert(edge.token_in);
        previous = Some(edge.token_out);
        sum += weight;
    }
    first == previous && sum.is_finite() && sum < 0.0
}

/// Bind the same snapshot that built the graph, not a global symbol/DEX map.
/// Validate the whole vector before writing any hint (no partial association).
pub(super) fn bind_dexes(
    intent: &mut RouteIntent,
    pools: &HashMap<(u64, Address), &PoolRef>,
) -> Result<(), &'static str> {
    let mut names = Vec::with_capacity(intent.legs.len());
    for leg in &intent.legs {
        let address = leg.pool_hint.ok_or("missing_pool_identity")?;
        let pool = pools
            .get(&(intent.chain_id, address))
            .ok_or("pool_outside_snapshot")?;
        if pool.chain_id != intent.chain_id
            || pool.address != address
            || pool.protocol_type != leg.protocol_type
            || pool.fee_bps != leg.fee_bps
            || !((pool.token0 == leg.token_in && pool.token1 == leg.token_out)
                || (pool.token1 == leg.token_in && pool.token0 == leg.token_out))
        {
            return Err("pool_identity_mismatch");
        }
        let name = pool.dex_name.trim();
        if name.is_empty() || name.eq_ignore_ascii_case("unknown") {
            return Err("missing_dex_identity");
        }
        names.push(name.to_owned());
    }
    for (leg, name) in intent.legs.iter_mut().zip(names) {
        leg.dex_hint = Some(name);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
