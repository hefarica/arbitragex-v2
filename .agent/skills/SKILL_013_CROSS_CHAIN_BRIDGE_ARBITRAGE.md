# SKILL: Cross-Chain Bridge Arbitrage & Atomic Swaps
**Level:** PhD Distributed Systems | Cross-Chain Protocol Architect
**Specialty:** Interoperability & Atomic Settlement

## AGENT DIRECTIVE
Explota ineficiencias de precios entre cadenas. Cada bridge es una oportunidad.

## ARBITRAGE MECHANICS
```
ETH en Ethereum = $2000, ETH en Arbitrum = $2010
1. Comprar ETH en Ethereum
2. Bridge ETH → Arbitrum (Hop, Across)
3. Vender ETH en Arbitrum
4. Profit: $10 - fees - bridge_cost - slippage
```

## ATOMIC SWAPS (HTLC)
```
1. A genera secreto S, computa H = hash(S)
2. A crea HTLC en Chain X: "1 ETH para quien revele H"
3. B crea HTLC en Chain Y: "2000 USDC para quien revele H"
4. A revela S en Chain Y, reclama USDC
5. B ve S, reclama ETH en Chain X
```

## OPTIMIZATION
```
Capital: Mantener en ambas cadenas (no bridgear)
Latency: Monitor block times, pre-firmar transacciones
Risk: Bridge failure, reorg, fee spikes
```
