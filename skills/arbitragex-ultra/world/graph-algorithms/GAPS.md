# What ArbitrageX is missing vs state of the art

Honest assessment of ArbitrageX vs this state of the art (grounded in repo artifacts and memory):

1. ROUTE DISCOVERY ALGORITHM: we run bounded DFS-style route-graph-engine with a 7-leg cap (GATE A) and static router catalog lookups (memory: find_router=catálogo estático). SOTA: RICH color-coding DP (O(2^k*m), 32.69x faster, near-optimal with guarantees, escapes local traps) and line-graph MMBF for open paths. At k>=4 our approach is exponentially dominated; and we cannot express hop-exact layered strategies.

2. NO SPLIT ROUTING: SizeOptimizer sizes a single path. No marginal-price equalization across parallel pools per leg, no aggregate-CFMM (V3 ticks-as-one-curve) optimizer, no gas-aware pool activation model. The Excel equations themselves (Pi_R(x) scalar x) encode this limitation - the catalog's math needs an allocation-vector variant for legs.

3. NO NON-CYCLE NEGATIVE-PATH DETECTOR: candidates are cycles + event/parity triggers; the MMBF result (23,868 vs 19 profitable paths) says we are blind to the majority of the profitable open-path surface (only implicitly covered cross-chain).

4. STATE FRESHNESS ARCHITECTURE: per-scan rebuild + purge batching (FREEZE-01/02 incidents) instead of event-driven reserve updates on a persistent graph with a cycle-edge inverted index. No amms-rs-style Sync/Swap WebSocket subscription layer documented in the detection path. The 1.29-1.78 bps/block staleness cost is unbilled anywhere in our scoring.

5. RANKING: opportunities are ranked by deterministic Pi + score thresholds (min_accept_score) without (a) staleness decay, (b) adverse-selection/sandwich probability term, (c) competition/win-probability adjustment for auction or gas-priority contexts. Kelly advisory is flat_prior-blocked (memory: motor §IV blockers - flat_prior means 8,184 relations without signal), so sizing is uncalibrated vs the equilibrium literature's less-than-max-size result.

6. PRUNING DISCIPLINE: watchlist-based with pools gated (P3) rather than quantified TVL-floor + degree-1 peel + (k>=2)-core + hub-pivot reduction with measured TVL coverage (~90% target in the literature). k-core decomposition appears in NEITHER our codebase NOR the literature - a genuine open lane we could own.

7. EVALUATION HARNESS: no SCO/FVO/G-FVO-style benchmark suite to measure OUR OWN routing sub-optimality in shadow mode - we can't quantify how much yield our single-path sizing leaves on the table. The Columbia methodology (bisection optimum as ground truth, staleness ablations) is directly portable to PAPER_SHADOW.

8. CALIBRATION FEEDBACK LOOP: detectors emit is_opp=false floods and 0-viable markets (memory: detection pipeline realities); SOTA systems validate detector recall against exact solvers (RICH's 0.02-3.9% relative-error methodology). We have no exact-solver ground truth to measure detector recall against.

What we already have right (not missing): the log-transform marginal prefilter (matches RICH exactly), golden-section/Newton optimal sizing per route, fee-adjusted weights, same-block state requirement, mode-invariant math doctrine, and honest Fail-Fast observation semantics (RULE 00/R8) which is better discipline than most public repos. The gap is concentrated in SEARCH (color-coding/DP/optimizers) and SCORING (staleness/competition), not in the per-route economics.
