# Especificaciones originales por familia (no certificados de ejecución)

## CF_BATCH — fila 2
- Nombre: Batch/auction-managed AMM
- Ecuación original: Solve clearing allocation/price maximizing feasible surplus subject to pool invariant, order limits and conservation; compare settlement value after costs.
- Datos originales: Batch orders; pool state; clearing constraints; fee model; auction deadline.
- Parámetros propuestos: min_surplus_usd; max_auction_age_ms; solver timeout; min_price_improvement_bps.

## CF_BOND — fila 3
- Nombre: Bonding-curve AMM
- Ecuación original: For supply q and cost function C(q), marginal price p(q)=dC/dq; exact acquisition/redeem cost is integral/difference C(q+Δ)-C(q). Compare against executable external value.
- Datos originales: Current supply/state; bonding curve parameters; mint/redeem fees; external executable quote.
- Parámetros propuestos: max_curve_fraction; min_profit_usd; max_price_impact_pct.

## CF_CLAMM — fila 4
- Nombre: Concentrated-liquidity AMM
- Ecuación original: Piecewise quote across initialized ticks using sqrtPrice, active liquidity L and fee tier; route Q_R(x) must traverse ticks exactly or use protocol quoter.
- Datos originales: sqrtPriceX96; active liquidity; initialized ticks/tick bitmap; fee tier; decimals; QuoterV2 or exact local tick traversal.
- Parámetros propuestos: max_ticks_crossed; max_quote_age_blocks; min_profit_usd; max_slippage_pct; max_price_impact_pct.

## CF_CONSTANT_SUM — fila 5
- Nombre: Constant-sum AMM
- Ecuación original: x+y=k inside the valid inventory region; quote is locally constant-price until a reserve boundary, then infeasible. Detect cross-venue executable deviation net of fees.
- Datos originales: Reserves; invariant bounds; fees; exact competing route.
- Parámetros propuestos: min_profit_usd; reserve safety floor; max trade fraction.

## CF_CPMM — fila 6
- Nombre: Constant-product AMM
- Ecuación original: For reserves (x,y), k=x·y. Exact-in quote with fee γ: Δy = y·(γΔx)/(x+γΔx). Route profit uses exact composition, not spot reserve ratio.
- Datos originales: Reserves; token ordering/decimals; pool fee; exact route legs; gas.
- Parámetros propuestos: min_profit_usd; probe/simulation capital; max_slippage_pct; max_price_impact_pct; reserve freshness.

## CF_CROSSINV — fila 7
- Nombre: Cross-invariant routing
- Ecuación original: Compose protocol-correct quote adapters across heterogeneous invariants. No common spot formula; opportunity iff exact closed-route net profit >0.
- Datos originales: Per-leg protocol adapter/state; exact route; same-block snapshot; gas.
- Parámetros propuestos: allowed_protocol_mix; min_profit_usd; max_legs; quote/simulation timeout.

## CF_DYNAMIC — fila 8
- Nombre: Dynamic parameter / hybrid AMM
- Ecuación original: Q(x;θ(S_t)) with θ read from current contract state (weights, fees, peg, oracle or hybrid book). Reconstruct exact current invariant before comparing routes.
- Datos originales: Protocol-specific dynamic parameters; state snapshot; exact adapter; fees; oracle where protocol requires.
- Parámetros propuestos: max_state_age_blocks; min_profit_usd; max_dynamic_fee_bps; max_slippage_pct.

## CF_LB — fila 9
- Nombre: Liquidity Book / bin AMM
- Ecuación original: Traverse discrete price bins; each bin uses constant-sum behavior at fixed bin price, with fixed+variable fee. Aggregate fills across bins until amount is satisfied.
- Datos originales: Active bin; bin step; per-bin reserves/liquidity; variable fee state; exact bin traversal.
- Parámetros propuestos: max_bins_crossed; max_variable_fee_bps; min_profit_usd; max_quote_age_blocks.

## CF_PMM — fila 10
- Nombre: Proactive Market Maker
- Ecuación original: Use protocol PMM inventory-aware curve around oracle/reference price; quote exact buy/sell branch and compare executable round-trip net of fees.
- Datos originales: PMM state/inventory; oracle/reference price; k/curve parameters; fees; exact protocol version.
- Parámetros propuestos: max_oracle_age; min_profit_usd; inventory deviation cap; max_price_impact_pct.

## CF_STABLESWAP — fila 11
- Nombre: Curve StableSwap invariant
- Ecuación original: Use protocol StableSwap invariant D/A and solve post-trade y (Newton iteration as protocol does); compose exact dy across legs and optimize net route profit.
- Datos originales: All pool balances; amplification A; fees/admin fees; token rates/decimals; exact invariant version.
- Parámetros propuestos: min_profit_usd; max_iterations; numerical_tolerance; max_slippage_pct; same-block state.

## CF_TWAMM — fila 12
- Nombre: TWAMM post-virtual-order state
- Ecuación original: Advance virtual long-term orders to target block/time, derive post-state reserves, then run exact quote/round-trip on that state.
- Datos originales: Long-term order state; last virtual execution time; reserves; block timestamp; fees.
- Parámetros propuestos: max_state_age_blocks; min_profit_usd; event horizon.

## CF_VAMM — fila 13
- Nombre: Virtual AMM / derivative curve
- Ecuación original: Use virtual reserves/invariant plus index/mark/funding rules; opportunity is hedged net basis, not raw virtual reserve spread.
- Datos originales: Virtual reserves; mark/index/oracle; funding; position constraints; fees.
- Parámetros propuestos: max_basis_bps; max_oracle_age; min_profit_usd; hedge requirement.

## CF_WEIGHTED — fila 14
- Nombre: Weighted geometric-mean AMM
- Ecuación original: Balancer invariant V=Π_i B_i^{W_i}; derive exact outGivenIn from balances/weights/fees and evaluate route net profit.
- Datos originales: Balances; normalized weights; swap fee; token scaling factors; exact pool type/version.
- Parámetros propuestos: min_profit_usd; max_in_ratio; max_out_ratio; max_price_impact_pct.

## C_CEXDERIV — fila 15
- Nombre: CEX derivative vs DEX spot basis
- Ecuación original: Hedged net basis/funding: Π=DerivativeLeg+SpotLeg+expected_funding-carry-fees-slippage-settlement risk.
- Datos originales: CEX derivative book; mark/index/funding; DEX exact spot quote; margin/inventory; fees.
- Parámetros propuestos: max_basis_bps; funding_horizon_hours; max_latency_ms; min_depth_usd; min_profit_usd.

## C_CEXDEX — fila 16
- Nombre: CEX/DEX executable VWAP arbitrage
- Ecuación original: Π(q)=CEX_bid_VWAP(q)-DEX_buy_exact(q)-CEX/DEX fees-transfer/hedge costs, or reverse direction. Require firm depth and settlement/inventory model.
- Datos originales: CEX L2/orderbook or firm quotes; DEX exact route quote; fees; inventory; latency; transfer/hedge model.
- Parámetros propuestos: max_cex_quote_age_ms; min_cex_depth_usd; max_latency_ms; inventory_mode; fee schedule; min_profit_usd.

## D_BASIS — fila 17
- Nombre: Spot/future/perpetual basis no-arbitrage
- Ecuación original: Compare derivative price to carry-adjusted spot/forward replication; for perp include expected funding. Opportunity is hedged net convergence after margin, fees and funding.
- Datos originales: Spot and derivative firm books; mark/index; rates/carry; funding; expiry; fees; margin.
- Parámetros propuestos: max_basis_bps; funding_horizon_hours; max_margin_utilization_pct; min_profit_usd.

## D_FUNDING — fila 18
- Nombre: Funding-rate carry arbitrage
- Ecuación original: Expected net over horizon H: funding receipts - spot/perp basis convergence risk - borrow/carry - fees - slippage. Require hedgeable books.
- Datos originales: Funding schedule/rate; perp and spot books; borrow rate; margin; horizon.
- Parámetros propuestos: funding_horizon_hours; min_annualized_edge_bps; max_basis_bps; min_profit_usd.

## D_OPTIONS_PARITY — fila 19
- Nombre: Options static no-arbitrage parity
- Ecuación original: European no-income core: C-P = S-PV(K). Conversion/reversal/box compare executable legs with same strike/expiry and financing/dividend adjustments.
- Datos originales: Option bid/ask by strike/expiry; spot/forward; rate/dividend/carry; contract multiplier; fees.
- Parámetros propuestos: max_quote_age_ms; min_parity_edge_usd; min_open_interest/depth; min_profit_usd.

## D_OPTIONS_SURFACE — fila 20
- Nombre: Options surface static-arbitrage constraints
- Ecuación original: Enforce strike monotonicity/convexity and calendar consistency on executable bid/asks; e.g. equally spaced K: C(K1)-2C(K2)+C(K3)≥0. Trade violating butterfly/calendar only if all legs executable.
- Datos originales: Full option surface bid/ask; strikes; expiries; spot/forward; rates; depth.
- Parámetros propuestos: min_surface_violation_usd; max_quote_age_ms; min_depth; max_legs; min_profit_usd.

## D_SETTLE — fila 21
- Nombre: Derivative settlement / expiry / liquidation convergence
- Ecuación original: Compute deterministic payoff/settlement value under venue rules and compare with executable acquisition/hedge/unwind cost; include liquidation penalty and expiry timing.
- Datos originales: Contract payoff; mark/index/settlement oracle; expiry; position/margin state; executable hedge.
- Parámetros propuestos: max_settlement_delay_sec; max_oracle_age_sec; min_profit_usd; min_margin_buffer.

## E_AUCTION — fila 22
- Nombre: Auction clearing / decay opportunity
- Ecuación original: Π(q,t)=FairExecutableValue(q)-AuctionPurchaseCost(q,t)-UnwindCost(q)-Gas; solve feasible q and timing under auction rules.
- Datos originales: Auction price/curve; remaining lot; deadline; collateral/external value; unwind liquidity.
- Parámetros propuestos: min_discount_bps; max_auction_age_ms; max_position_usd; min_profit_usd.

## E_LATENCY — fila 23
- Nombre: Latency / stale-state discrepancy
- Ecuación original: Trigger z=(p_fast-p_slow)/σ only identifies divergence; opportunity requires a firm executable quote/route and Π_net>0 before stale side updates.
- Datos originales: Independent timestamps; fast/slow quotes; volatility estimate; execution latency; firm liquidity.
- Parámetros propuestos: max_quote_age_ms; min_zscore; max_end_to_end_latency_ms; min_profit_usd.

## E_ORACLE — fila 24
- Nombre: Oracle/OEV state transition
- Ecuación original: Recompute protocol state under confirmed oracle update O_t→O_{t+1}; evaluate resulting executable liquidation/repricing routes. Oracle delta alone is not profit.
- Datos originales: Oracle round/update; protocol state; affected positions/pools; post-update quotes; oracle freshness.
- Parámetros propuestos: max_oracle_age_sec; min_deviation_bps; min_profit_usd; confirmed_update_only.

## E_POST — fila 25
- Nombre: Deterministic post-event / backrun repricing
- Ecuación original: Apply confirmed/public event transition S'=T_e(S), quote all affected routes on S', then Π_R(x;S')=Q_R(x;S')-x-C_R(x). Never infer profit from event size alone.
- Datos originales: Event/receipt/log; pre/post state or deterministic transition; affected pools/routes; exact quote adapters; gas.
- Parámetros propuestos: max_event_age_blocks; min_profit_usd; post_state_confirmation; private route preference.

## E_STATE — fila 26
- Nombre: Protocol state-transition arbitrage
- Ecuación original: Given deterministic transition e, compute S'=T_e(S); revalue all claims/routes V(S') and compare executable acquisition/unwind cost. Emit only if transition is known and settlement executable.
- Datos originales: Protocol event/state variables; before/after block state; affected claims/pools; redemption/execution path.
- Parámetros propuestos: event allowlist; max_state_age_blocks; min_profit_usd; settlement timeout.

## I_BATCH — fila 27
- Nombre: Batch/combinatorial intent matching
- Ecuación original: Find feasible matching/clearing maximizing total surplus across orders plus external liquidity, subject to conservation and individual limit-price constraints.
- Datos originales: Batch intents; limit prices; token graph; external quotes; settlement constraints.
- Parámetros propuestos: batch_size_cap; solver_timeout_ms; min_surplus_usd; min_matched_volume_usd.

## I_DUTCH — fila 28
- Nombre: Dutch auction decay
- Ecuación original: Given auction price P(t), execute when external executable value V(t)-P(t)-costs is maximized subject to deadline/competition and min surplus.
- Datos originales: Auction start/end price/time; current timestamp; firm external quote; competition/latency.
- Parámetros propuestos: min_discount_bps; max_wait_ms; min_profit_usd; competition buffer.

## I_ORDERFLOW — fila 29
- Nombre: Authorized/private order-flow strategy
- Ecuación original: Only evaluate flows explicitly available to this searcher; price the authorized order/bundle against alternative settlement and costs. No inference of privileged/private data.
- Datos originales: Explicitly authorized/private order feed; bundle contents; settlement rules; route quotes.
- Parámetros propuestos: allowed_orderflow_sources; min_surplus_usd; max_bundle_age_ms; private_route_required.

## I_ROUTE — fila 30
- Nombre: Intent/solver optimal settlement
- Ecuación original: maximize user/solver executable surplus under limit prices, token conservation, balances and settlement constraints; compare AMM/CLOB/RFQ routes.
- Datos originales: Intent/order constraints; balances/approvals; external liquidity; gas; valid-to; settlement contract.
- Parámetros propuestos: min_surplus_usd; min_price_improvement_bps; max_solver_latency_ms; max_auction_age_ms.

## L_AUCTION — fila 31
- Nombre: Credit / bad-debt auction
- Ecuación original: Π(q,t)=ConservativeCollateralValue(q)-AuctionCost(q,t)-UnwindCost(q)-Gas-capital risk; solve lot size and timing under exact auction rules.
- Datos originales: Auction lot/price curve; collateral; protocol debt; deadline; market liquidity.
- Parámetros propuestos: min_discount_bps; max_auction_age_sec; max_position_usd; min_profit_usd.

## L_COLLATERAL — fila 32
- Nombre: Collateral/debt/wrapper parity
- Ecuación original: Value debt/collateral claims in common numeraire using protocol conversion/redemption, then compare executable acquisition/repayment paths net of costs.
- Datos originales: Collateral/debt token exchange rates; protocol oracle; repayment/redemption functions; market quotes.
- Parámetros propuestos: max_oracle_age_sec; min_parity_deviation_bps; min_profit_usd.

## L_LIQ — fila 33
- Nombre: Lending liquidation
- Ecuación original: Eligibility via protocol health metric (Aave: HF=(collateral value×weighted liquidation threshold)/borrow value; liquidation when HF<1). Π=seized collateral executable value-debt repaid-gas-flash/unwind costs.
- Datos originales: Account collateral/debt; liquidation thresholds/bonuses; oracle prices; close factor; firm unwind route; gas/flash fee.
- Parámetros propuestos: max_health_factor; min_liquidation_bonus_bps; max_oracle_age_sec; min_profit_usd; max_position_usd.

## L_LOOP — fila 34
- Nombre: Recursive leverage-loop carry
- Ecuación original: Optimize leverage n / LTV to maximize net APY or finite-horizon P&L subject to health-factor/risk buffer and borrow/supply rate feedback.
- Datos originales: LTV/liquidation threshold; supply/borrow rates; collateral factor; loop fees; gas.
- Parámetros propuestos: max_loops; target_ltv_pct; min_health_factor; min_net_apy_bps; capital cap.

## L_RATE — fila 35
- Nombre: Lending/borrow carry spread
- Ecuación original: Net carry over horizon H = earned supply/yield - borrow cost - protocol fees - hedge/transaction costs - capital/risk charge.
- Datos originales: Supply/borrow indices and rates; utilization; collateral/margin requirements; fees; market hedge.
- Parámetros propuestos: horizon_hours; min_net_apy_bps; max_utilization_pct; min_profit_usd.

## M_AMM — fila 36
- Nombre: Prediction AMM vs order-book
- Ecuación original: Quote conditional-token AMM exactly and compare with firm outcome order-book quotes for identical payout claim; net of split/merge, fees and settlement.
- Datos originales: Prediction AMM invariant; outcome IDs; firm order book; complete-set mechanics; resolution source.
- Parámetros propuestos: max_quote_age_ms; min_depth_usd; min_profit_usd; resolution_source_allowlist.

## M_COMPLETE — fila 37
- Nombre: Prediction complete-set arbitrage
- Ecuación original: For exhaustive mutually exclusive outcomes backed by 1 collateral unit: buy all if Σ asks_i + fees < 1; mint/sell all if Σ bids_i - fees > 1, respecting split/merge/redeem mechanics.
- Datos originales: Outcome partition; collateral denomination; firm bid/ask per outcome; split/merge/redeem availability; fees.
- Parámetros propuestos: min_complete_set_edge_bps; max_quote_age_ms; min_depth_usd; min_profit_usd.

## M_CROSS — fila 38
- Nombre: Cross-platform prediction-market parity
- Ecuación original: Normalize markets only if they reference the same event, resolution source and payout semantics; then compute firm cross-venue spread after fees/capital lockup.
- Datos originales: Canonical event identity; resolution rules; outcome mapping; firm books; fees; settlement horizon.
- Parámetros propuestos: resolution_source_allowlist; max_settlement_days; min_probability_gap_bps; min_profit_usd.

## M_LOGIC — fila 39
- Nombre: Logical/payoff constraint arbitrage
- Ecuación original: Represent outcome payoffs as linear constraints; solve LP for a portfolio with non-negative state payoff and negative acquisition cost (after fees), or detect violated logical probability bounds.
- Datos originales: Outcome payoff matrix; logical relation/event mapping; firm quotes; resolution rules.
- Parámetros propuestos: min_guaranteed_edge_usd; max_portfolio_legs; resolution_source_allowlist.

## N_AMM — fila 40
- Nombre: NFT AMM / marketplace convergence
- Ecuación original: Use exact NFT AMM bonding curve quote for acquisition/unwind and compare against firm marketplace bid/ask for same eligible asset/collection, net of royalties/fees.
- Datos originales: NFT AMM curve/state; eligible token set; marketplace firm quotes; royalties; fees.
- Parámetros propuestos: max_curve_impact_pct; max_listing_age_sec; min_profit_usd.

## N_FLOOR — fila 41
- Nombre: NFT floor/relative-value signal
- Ecuación original: Floor/trait deviation is only a signal. Emit only when a firm buyer/bid or deterministic redemption converts it into Π_net>0; otherwise observe-only.
- Datos originales: Listings; trait metadata; firm bids; sale history; royalty/fees; redemption if applicable.
- Parámetros propuestos: max_floor_age_sec; min_floor_discount_bps; require_firm_exit_bid; min_bid_depth.

## N_IDENTICAL — fila 42
- Nombre: NFT identical-asset cross-market spread
- Ecuación original: For identical tokenId/asset, Π=firm exit bid proceeds-acquisition ask-royalty-marketplace/gas costs; no floor proxy allowed for the exit leg.
- Datos originales: TokenId identity; executable listing/ask and bid; royalties; marketplace fees; ownership/approval state.
- Parámetros propuestos: max_listing_age_sec; require_firm_exit_bid=true; max_royalty_bps; min_profit_usd.

## N_LIQ — fila 43
- Nombre: NFT-backed loan liquidation
- Ecuación original: Eligibility from lending protocol; Π=conservative firm-bid value of seized NFT-debt repayment-liquidation/gas/unwind costs. Floor alone cannot be profit.
- Datos originales: Loan state; liquidation rules; oracle/appraisal; firm NFT bid/liquidity; gas.
- Parámetros propuestos: max_appraisal_age_sec; min_liquidation_discount_bps; require_firm_exit_bid; min_profit_usd.

## N_REDEEM — fila 44
- Nombre: NFT fractional/bundle redemption NAV
- Ecuación original: Π=deterministic redeem/unbundle value-acquisition cost-component unwind costs; inverse path for mint/bundle-and-sell if contract permits.
- Datos originales: Fractionalization/redemption contract; bundle composition; firm component bids; fees.
- Parámetros propuestos: min_nav_discount_bps; max_component_count; min_profit_usd.

## OBSERVE — fila 45
- Nombre: Observe-only / non-deterministic or non-authorized
- Ecuación original: No is_opportunity=true. Emit structured evidence only until a lawful, deterministic, executable settlement path and required public/authorized data are present.
- Datos originales: Telemetry/evidence only; no fabricated private flow, reorg certainty, future resolution or unavailable external data.
- Parámetros propuestos: telemetry_enabled; alert_threshold; evidence_retention.

## P_4626 — fila 46
- Nombre: ERC-4626 share/asset parity
- Ecuación original: Use preview/convertToAssets and convertToShares plus actual deposit/redeem limits/fees; compare executable share market price to redeemable asset value.
- Datos originales: convertToAssets/Shares; previewDeposit/Redeem; maxDeposit/Redeem; vault totalAssets; share/asset market quotes.
- Parámetros propuestos: min_share_discount_bps; max_vault_fee_bps; max_redeem_delay; min_profit_usd.

## P_LST — fila 47
- Nombre: LST/LRT redemption and market parity
- Ecuación original: Π=RedemptionValue(exchange_rate,t)-MarketAcquisitionCost-queue/carry/fees; cross-LST uses a common underlying redemption unit.
- Datos originales: Protocol exchange/redemption rate; withdrawal queue/delay; underlying price; firm LST quotes.
- Parámetros propuestos: max_redemption_delay_sec; annual_carry_pct; min_discount_bps; min_profit_usd.

## P_NAV — fila 48
- Nombre: Tokenized basket / NAV parity
- Ecuación original: NAV from exact redeemable components; Π=RedeemValue(token)-MarketCost(token)-unwind/fees, or inverse mint-and-sell path.
- Datos originales: Component weights/holdings; redemption/mint rules; firm component quotes; fees.
- Parámetros propuestos: min_nav_deviation_bps; max_component_slippage_pct; min_profit_usd.

## P_PEG — fila 49
- Nombre: Mint/redeem peg parity
- Ecuación original: Compare executable market price with protocol mint/redeem conversion after fees and caps: Π=max(RedeemValue-MarketBuyCost, MarketSellProceeds-MintCost)-costs.
- Datos originales: Mint/redeem functions/rates; market quotes; fees/caps; oracle if protocol uses one.
- Parámetros propuestos: min_parity_deviation_bps; max_redemption_delay_sec; max_fee_bps; min_profit_usd.

## P_PTYT — fila 50
- Nombre: PT/YT no-arbitrage parity
- Ecuación original: Core parity: P_PT+P_YT≈P_underlying (same accounting asset, same maturity). Also implied APY from PT/YT exchange rate; evaluate mint/redeem + market legs net of fees.
- Datos originales: PT/YT/SY exchange rates; expiry; mint/redeem path; firm PT/YT quotes; underlying APY.
- Parámetros propuestos: min_parity_deviation_bps; days_to_expiry floor; max_slippage_pct; min_profit_usd.

## P_WRAP — fila 51
- Nombre: Wrapper / representation parity
- Ecuación original: Compare exact wrap→sell and buy→unwrap paths against wrapper conversion ratio, fees and delay. Opportunity iff closed economic value >0.
- Datos originales: wrap/unwrap conversion; exchange rate; fees; underlying and wrapper firm quotes; bridge state if bridged.
- Parámetros propuestos: max_conversion_delay_sec; max_fee_bps; min_profit_usd; representation allowlist.

## P_YIELD — fila 52
- Nombre: Yield-curve / maturity inconsistency
- Ecuación original: Convert each claim to discount factor / implied yield for common underlying and maturity basis; detect executable curve violations after carry, fees and settlement delay.
- Datos originales: Maturities; redemption values; market prices; yield indices/APY; fees; settlement path.
- Parámetros propuestos: min_implied_yield_gap_bps; max_duration_days; min_profit_usd.

## R_BASKET_NAV — fila 53
- Nombre: Basket/index NAV convergence
- Ecuación original: NAV=Σ_i w_i·V_i (or exact redemption outputs). Π=ExecutableRedeemOrMintValue-BasketAcquisitionCost-all_costs. Emit only if mint/redeem or firm unwind exists.
- Datos originales: Basket composition/weights; mint/redeem contract state; exact component quotes; fees; redemption constraints.
- Parámetros propuestos: min_nav_deviation_bps; max_component_slippage_pct; min_profit_usd; basket component allowlist.

## R_CLOSED_CYCLE — fila 54
- Nombre: Exact closed-route arbitrage
- Ecuación original: Q_R(x)=q_n(...q_2(q_1(x))); Π_R(x)=Q_R(x)-x-C_R(x). Opportunity iff max_x Π_R(x)>0. Marginal prefilter: Σ_e[-ln((1-fee_e)·rate_e)]<0.
- Datos originales: Full ordered route legs; per-leg protocol adapter; reserves/slot0/ticks/bins; token decimals; pool fees; same-block state; gas; optional flash fee.
- Parámetros propuestos: enabled; min_profit_usd; min_roi_pct; simulation_capital_usd; min/max legs from cartridge; require_same_block=true; max_slippage_pct; max_price_impact_pct; max_gas_usd; allowed base/quote tokens; pool/DEX/protocol allowlists.

## R_COW — fila 55
- Nombre: Coincidence-of-Wants / combinatorial clearing
- Ecuación original: maximize total executable trader surplus subject to limit prices, token conservation and settlement constraints; compare internal matching vs external liquidity.
- Datos originales: Orders/intents; limit prices; balances; valid-to; external liquidity quotes; settlement constraints.
- Parámetros propuestos: min_surplus_usd; max_auction_age_ms; max_external_liquidity_share; min_price_improvement_bps.

## R_DIRECT_INDIRECT — fila 56
- Nombre: Direct-vs-indirect path inconsistency
- Ecuación original: Δ(x)=Q_indirect(x)-Q_direct(x)-C_incremental(x); evaluate both executable paths at identical input/state; opportunity iff max_x Δ(x)>0.
- Datos originales: Two competing paths with full legs; exact per-leg quotes; fees; block-consistent pool state; gas delta.
- Parámetros propuestos: min_path_edge_usd; min_profit_usd; max_legs; max_slippage_pct; max_gas_usd; require_same_block.

## R_ORDERBOOK — fila 57
- Nombre: Executable order-book / RFQ cross-market spread
- Ecuación original: Π(q)=Proceeds_bid(q)-Cost_ask(q)-taker_fees(q)-settlement_cost(q)-hedge_cost(q). Prices must be VWAP/depth-aware, not top-of-book only.
- Datos originales: Firm bids/asks or RFQs; depth ladder; quote timestamp; venue fees; settlement latency/cost; DEX exact quote where applicable.
- Parámetros propuestos: max_quote_age_ms; min_depth_usd; max_settlement_ms; fee_bps overrides; min_profit_usd; inventory cap.

## R_SPLIT — fila 58
- Nombre: Split/parallel optimal routing
- Ecuación original: maximize Σ_j Q_j(x_j)-C(x_1..x_m), subject to Σ_j x_j=x and x_j≥0; compare optimum against best unsplit route.
- Datos originales: Parallel executable paths; exact quote functions; gas activation cost per path; shared input amount.
- Parámetros propuestos: max_parallel_paths; min_split_improvement_bps; min_profit_usd; gas activation cost; max_price_impact_pct.

## X_BRIDGE — fila 59
- Nombre: Bridge-in-the-loop expected-value arbitrage
- Ecuación original: E[Π]=DestinationValue-SourceCost-bridge_fee-gas-carry-CVaR(latency/finality/reorg). Must model non-atomic settlement and failure states.
- Datos originales: Bridge quote/fee; message/finality state; source/destination liquidity; delay distribution; inventory/capital.
- Parámetros propuestos: allowed_bridges; max_bridge_fee_bps; max_finality_sec; settlement_risk_haircut_bps; min_expected_profit_usd.

## X_ORACLE — fila 60
- Nombre: Cross-domain oracle / state discrepancy
- Ecuación original: Normalize same economic claim across domains; discount for timestamp/finality and require an executable settlement path. Signal deviation alone cannot emit.
- Datos originales: Cross-domain oracle/state timestamps; finality; firm routes; bridge/inventory settlement model.
- Parámetros propuestos: max_source_age_sec; max_finality_sec; min_deviation_bps; settlement mode.

## X_PREPOS — fila 61
- Nombre: Cross-domain pre-positioned inventory convergence
- Ecuación original: No atomic bridge assumption: aggregate independently executable legs using inventory on both domains. Π=Σ proceeds-Σ costs-carry/finality/hedge costs.
- Datos originales: Per-chain firm quotes; inventory balances; finality/latency; gas; settlement status; accounting FX.
- Parámetros propuestos: inventory_mode=prepositioned; max_finality_sec; max_latency_sec; per-chain capital caps; min_profit_usd.
