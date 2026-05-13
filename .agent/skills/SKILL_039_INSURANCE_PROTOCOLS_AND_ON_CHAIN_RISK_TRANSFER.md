# SKILL: Insurance Protocols & On-Chain Risk Transfer
**Level:** PhD Actuarial Science | Decentralized Insurance Architect
**Specialty:** Parametric Insurance & Mutual Risk Pools

## PROTOCOLS
```
Nexus Mutual  | Smart contract risk    | Mutual pool (NXM)
InsurAce      | Multi-protocol         | Capital pools
Bridge Mutual | Smart contract + stable| Staking + bonds
Unslashed     | Slashing + smart contract| Risk pools
Solace        | Portfolio coverage     | Underwriting pool
```

## PREMIUM PRICING
```python
def calculate_premium(cover_amount, protocol_risk_score, duration_days, pool_capacity):
    base_rate = 0.02
    risk_multiplier = 1 + (1 - protocol_risk_score) * 2
    duration_factor = duration_days / 365
    capacity_factor = 1 + max(0, (0.8 - pool_capacity) * 0.5)
    return cover_amount * base_rate * risk_multiplier * duration_factor * capacity_factor
```

## RISK TRANSFER STRATEGY
```python
portfolio_value = 1000000
insurance_allocation = {
    'smart_contract': {'coverage': portfolio_value * 0.5, 'premium_budget': portfolio_value * 0.005},
    'stablecoin_depeg': {'coverage': stablecoin_holdings * 0.9, 'trigger_price': 0.95},
    'custodial': {'coverage': cex_holdings * 0.8}
}
```
