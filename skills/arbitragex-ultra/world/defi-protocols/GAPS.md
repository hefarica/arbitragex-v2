# What ArbitrageX is missing vs state of the art

Honest assessment against the repo's own extracted catalog (docs/excel_strategies_extracted.json: 264 strategies across DEX_AMM 53, DEX_STATE 31, PARITY_REDEMPTION 31, CROSS_CHAIN 30, DERIVATIVES 30, LENDING 25, INTENT_AUCTION 20, NFT 18, CEX_DEX 14, PREDICTION 12; nearly all rows Implemented_Source=NO, Initial_Mode=SHADOW):

1. **Uniswap v4 is not a first-class adapter.** No hook-flag awareness (BEFORE_SWAP/AFTER_SWAP gating changes fee and pool behavior), no flash-accounting/ERC-6909 flash path, no dynamic-fee handling. The static-fee marginal prefilter used by the biggest family (spot DEX cycles) is incorrect for v4 hook pools and for Fluid's limit-adjusting curves.

2. **Intent/solver surfaces are defined (20 rows) but have no mechanism client**: no CoW batch-auction solver API integration, no UniswapX Dutch-auction decay model, no 1inch Fusion resolver economics, no uniform-clearing-price/surplus math, no inventory ledger for non-atomic unwinding (the NonAtomic_Type column exists but Execution_Class only implements deterministic atomic).

3. **CEX_DEX (14 rows) is unreachable**: no exchange websocket feeds, no order book state, no colocation/latency tier, no inventory hedging engine. Given the research consensus that CEX-DEX is the dominant extracted-value pool and that 90%+ of arb is private-channel, this is the single largest unrealized family.

4. **Derivatives/perps adapters missing**: no Hyperliquid HyperEVM connector (precompile-readable CLOB), no GMX v2 oracle-price adapter, no dYdX v4 indexer client, no funding-rate time series — required by 30 DERIVATIVES rows plus all funding/basis strategies.

5. **Lending engine is pool-shaped, not market-shaped**: no Morpho isolated-market indexer (event-driven market creation), no Aave V4 hub/spoke credit-line modeling (post-March-2026 the canonical Aave), no Fluid Liquidity Layer adapter (95% LTV, per-block automated limits). Liquidation detection currently assumes monolithic pool oracles.

6. **Yield-tokenization blind spot**: no Pendle PT/YT pricing, no implied-yield-curve construction — needed for the fixed-vs-variable carry class that dominates current "safe" DeFi returns.

7. **Restaking parity is half-covered**: PARITY_REDEMPTION rows exist, but no LRT redemption-queue/delay telemetry, no NAV oracle ingestion, no basis-vs-term modeling; loop leverage math (Aave eMode LST/LRT) absent from SizeOptimizer's strategy templates.

8. **Chain coverage is EVM-Ethereum-centric**: no Move VM (Sui/Aptos), no parallel-EVM (Monad/Sei) adapters, no Hyperliquid. Different MEV physics (Sui: arb/liquidation dominant, sandwich dead) unexploited.

9. **Private orderflow absence**: no MEV-Share hint subscription/bidding, no MEV-Blocker integration — even though project skills reference Flashbots docs, the live searcher has no programmatic-privacy channel, forcing competition in the public/costly channel.

10. **Structural gap**: the system detects instantaneous price dislocations; it has no carry/inventory state (positions over time), no funding accrual accounting, and no solver-bid economics — the three primitives that define 2024-2026 strategy classes beyond atomic cycles.

Mitigating context: the Excel is genuinely broad (it already families intents, prediction markets, NFT, cross-chain), the detection/risk/simulation gates are rigorous, and mode-invariance doctrine is sound. The gap is not conceptual coverage — it is mechanism adapters and non-atomic execution state for the newest surfaces.
