# What the world does better than our Excel

What the 2024-2026 production world does that our 264-row Excel does not capture:

1. **Simulation-through-hooks instead of closed-form fee multiplication.** Our Discovery_Equation assumes static per-leg fees (Σ[−ln((1−fee_e)·rate_e)] < 0 prefilter). In Uniswap v4 the fee is hook code that can depend on the same swap's size, oracle deviation, or detected MEV ("MEV tax" hooks like Detox). State of the art detects through full EVM simulation (revm/Anvil fork) per candidate route, treating the hook as an adversary. Our marginal prefilter is provably wrong on hook pools — it will both false-positive (hook eats the margin) and false-negative (hook reduces fee for small sizes).

2. **Flash accounting as an execution primitive.** v4's singleton + net-delta settlement makes multi-pool cycles nearly free in token-movement gas and gives fee-free in-protocol flash loans (ERC-6909). Production bots structure entire cycles inside one `unlock()`; our cartridges model flash loans as an external provider (Aave-style premium), overcosting v4 routes.

3. **Solver/inventory-based execution classes.** CoW (~$200M/day), UniswapX, 1inch Fusion run competitive batch auctions where the arb is captured by solvers WITH inventory + non-atomic hedging, partially refunded as surplus. The world moved from "atomic mempool MEV" to "off-chain auction bidding + inventory risk management". Our Execution_Class taxonomy is DETERMINISTIC_EXECUTABLE-centric; the Excel's INTENT_AUCTION family (20 rows) has no solver-auction client, no batch surplus math (uniform clearing prices, coincidence-of-wants), and no inventory ledger.

4. **CEX-DEX as the primary arb pool with a latency stack.** Empirical work (arXiv 2507.13023) shows CEX-DEX is where the real money is, won via colocation and websocket microstructure, with 90%+ of flow private (Extropy 2025). Our CEX_DEX family (14 rows) has zero CEX connectivity, no orderbook feed ingestion, no lead-lag model.

5. **Carry/funding/basis as a systematic strategy class.** Ethena industrialized the delta-neutral basis (funding + staking − costs) to multi-billion scale; Pendle made yield itself a tradable curve (PT/YT). Our DERIVATIVES family (30 rows) is defined but there is no funding-rate ingestion, no perp venue adapter, no PT/YT pricing model.

6. **Multi-layer yield stacks.** LST loop, LRT looping in eMode, restaking + AVS rewards, Fluid's collateral-as-liquidity dual yield — the frontier is composed carry, not single-cycle spot arb. Requires a position/inventory state machine, per-block rate refresh, and liquidation math — not just route graphs.

7. **Permissionless-market breadth.** Morpho isolated markets (anyone creates: loan asset × collateral × oracle × IRM × LLTV) generate a long tail of mispriced liquidations that simply did not exist when monolithic pools governed listings. Production liquidators index market-creation events and score health factors across thousands of markets.

8. **Non-EVM execution.** Hyperliquid (on-chain CLOB readable from HyperEVM contracts), Sui/Aptos (Move object model, arb+liquidation dominate, sandwich mostly dead), Monad/Sei (parallel EVM, FCFS-ish ordering). Each has different MEV physics; the Excel is EVM-only in data bindings.

9. **Private orderflow access.** MEV-Share programmable privacy (hint → simulate → bid, share rebate with user), MEV-Blocker rebates. Bidding for orderflow rights is cheaper than mempool spam competition — this is the new channel economics.
