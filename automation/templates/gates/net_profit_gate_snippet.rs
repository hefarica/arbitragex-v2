// arbx-net-profit-gate: Fail-fast if net_usd <= 0
let net_usd = gross_usd - gas_cost_usd - flashloan_fee_usd - ops_overhead_usd;
if net_usd <= 0.0 {
    log::warn!("Route rejected: non-positive net profit ({} USD)", net_usd);
    return Ok(None); // RULE 00: fail-honest
}
