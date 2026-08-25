# What the world does better than our Excel

What the world does better than our 264-strategy Excel catalog (which already has the right marginal prefilter Σ_e -ln((1-fee_e)*rate_e) < 0 and golden-section/Newton sizing per route):

1. GLOBAL SPLIT-ROUTE OPTIMIZATION instead of pick-a-path-then-size-it. Our SizeOptimizer sizes ONE discovered path. Angeris/Diamandis/Bancor-MPO jointly optimize route shape AND order splitting across ALL pools of a pair simultaneously (marginal-price equalization lambda*, S(lambda)=Q bisection, or price-vector root-finding). The 2026 Columbia audit says path-based routing leaves ~2.02 bps/trade ($24M/$120B) on the table vs the split optimum. Our Excel equations Pi_R(x)=Q_R(x)-x-C_R(x) have no split variable - x is a scalar per route, not an allocation vector across parallel pools.

2. EXACT-k GLOBAL CYCLE SEARCH with probabilistic guarantees (RICH): color-coding + Held-Karp DP over color sets, O(2^k*|V|*|E|) per instance, 32.69x faster than the best competitor on 360K-node Uniswap graphs, 0.02-3.9% relative error, escapes the local-optima traps where DFS/greedy plateau for 40+ seconds. Our bounded DFS + 7-leg cap (GATE A) is strictly dominated at k>=4: DFS redundancy explodes (330-2194x slower at k=6) while DP stays quasi-polynomial in k.

3. NON-CYCLE NEGATIVE PATHS as a first-class surface (MMBF line-graph, arXiv:2406.16573): 23,868 profitable paths >$1K vs 19 for cycle-only MBF, path lengths 7-11 vs 3-4. Our catalog's detectors are overwhelmingly cycle/closed-route shaped (R_CLOSED_CYCLE = 25 strategies) plus event/parity detectors; open s-t negative paths (exit via a different token than entry, with inventory management) are essentially absent except implicitly in cross-chain.

4. CONCENTRATED-LIQUIDITY-AWARE OPTIMIZERS: Bancor MPO solves where commercial convex solvers fail to converge on levered curves (200x faster than Clarabel), and Diamandis introduced the "aggregate CFMM" interface - a whole Uniswap V3 pool with all ticks as ONE concave function inside the optimizer. We treat V3 via QuoterV2 dispatch per leg - correct for pricing but not composable as an optimization constraint.

5. MEASURED STALENESS ECONOMICS: the world quantifies freshness loss (1.29-1.78 bps per block of lag) and ranks routes accordingly, discounts edges touched by recent events until re-synced, and attributes heavy-tailed losses to timing mismatch and sandwich attacks. Our pipeline has staleness detection skills but no calibrated bps-decay on opportunity scores.

6. GAS-AWARE POOL ACTIVATION as an optimization model (activation cost iff q_j>0, preserving concavity) - which pools to include per trade is solved inside the router, not by static allowlists.

7. EVENT-DRIVEN STATE LAYER as infrastructure (amms-rs): Sync/Swap-event WebSocket subscriptions updating reserves in place on a persistent graph, plus cycle-edge inverted indexes to re-validate only affected cycles. We rebuild per scan cycle with purge/batching workarounds (FREEZE-01/02 history shows the cost of batch purges).

8. GAME-THEORETIC BIDDING LAYER: equilibrium results showing optimal arb trades use less-than-max size and low-priority gas more often; OFA/backrun bidding as EV*P(win|bid)-bid. Our Kelly op sizes in isolation from competition.
