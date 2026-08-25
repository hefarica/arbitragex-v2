# BETTER THAN EXCEL — What the world does better

> Generated from 5 parallel research agents, 2026-08-19

## defi-protocols
What the 2024-2026 production world does that our 264-row Excel does not capture:

1. **Simulation-through-hooks instead of closed-form fee multiplication.** Our Discovery_Equation assumes static per-leg fees (Σ[−ln((1−fee_e)·rate_e)] < 0 prefilter). In Uniswap v4 the fee is hook code that can depend on the same swap's size, oracle deviation, or detected MEV ("MEV tax" hooks like Detox). State of the art detects through full EVM simulation (revm/Anvil fork) per candidate route, treating the hook...

## mev-practice
What production actors do better than what the 264-strategy Excel encodes (the Excel is strong on taxonomy/topology — 264 families x 31 operators with discovery equations — but the state of the art adds these dimensions the catalog does not capture):

1. **Simulation as a competitive weapon, not just a checkpoint.** Leaders run REVM in-process with bytecode caching (cacache), account mocking, storage-slot mocking, and custom revert-encoding quoters — ~19ms for 100 quotes on a local node (~0.2ms/...

## graph-algorithms
What the world does better than our 264-strategy Excel catalog (which already has the right marginal prefilter Σ_e -ln((1-fee_e)*rate_e) < 0 and golden-section/Newton sizing per route):

1. GLOBAL SPLIT-ROUTE OPTIMIZATION instead of pick-a-path-then-size-it. Our SizeOptimizer sizes ONE discovered path. Angeris/Diamandis/Bancor-MPO jointly optimize route shape AND order splitting across ALL pools of a pair simultaneously (marginal-price equalization lambda*, S(lambda)=Q bisection, or price-vector...

## mev-practice
What the world does better than the 264-strategy Excel catalog (verified against skills/arbitragex-ultra/strategies/ via grep — the Excel is a TAXONOMY of opportunity types; the state of the art is a MECHANICS + ECONOMICS layer that the catalog barely touches):

1. NET-OF-TIP EV COMPUTATION. The Excel computes gross arbitrage; production searchers compute expected value as E[net] = P(inclusion | tip t) x (gross - tip - gas), with empirically-calibrated bid-response curves. The measured reality: ...

## mev-practice
Ground truth for this comparison: I inspected the local 264-strategy catalog (c:\Users\HFRC\Desktop\arbitragex-v2-main (17)\docs\excel_strategies_extracted.json — 264 strategies, families: 36 same-chain DEX spot arb, 31 tx/state-triggered, 31 parity/redemption, 30 cross-chain, 30 derivatives/vol, 25 lending/liquidations, 20 intents/solvers/auctions, 18 NFT, 17 AMM-curve, 14 CEX-DEX, 12 prediction markets; each with Discovery_Equation prose, Primary/Secondary ops from the fixed 31-operator set) a...

