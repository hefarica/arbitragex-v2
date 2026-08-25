# NEW STRATEGIES — Beyond the 264

> Generated from 5 parallel research agents, 2026-08-19

## defi-protocols
New strategy types NOT in the 264 catalog, with topology and math:

**S-1. Hook-adaptive v4 cycle arbitrage (DEX_AMM family extension).** Topology: closed cycle over mixed v3/v4/hook pools inside one v4 `unlock()`; net-delta flash accounting settlement; ERC-6909 fee-free flash. Math: same cycle prod...

## mev-practice
Concrete strategy families NOT in the 264 catalog (which already covers same-chain spot arb 36, tx-triggered 31, parity/redemption 31, cross-chain 30, derivatives 30, lending/liquidations 25, intents/auctions 20, NFT 18, AMM-curve 17, CEX-DEX 14, prediction markets 12). Derivations with topology and...

## graph-algorithms
Five-plus concrete new strategy types not in the 264 catalog (with topology + math), all read-only/shadow-compatible:

1. SPLIT-ROUTE CLOSED CYCLE (R_SPLIT_CYCLE). Topology: HolonomicLoopResolution where each LEG is a bundle of parallel pools for the same pair. Math: allocate x across parallel pools...

## mev-practice
Seven concrete new strategy cartridges (none in the current 264, grep-verified), mapped to the repo's three canonical topologies (OrthogonalEquilibrium = cross-venue equilibrium capture, DiracImpulseOnly = single atomic impulse following an external event, HolonomicLoopResolution = closed multi-pool...

## mev-practice
Seven concrete new strategy types NOT in the 264 catalog (which I verified contains none of these as such — its CEX-DEX/derivatives/lending families have no LVR-band, auction-bidding, fee-vol, shielded-venue, or certified-adversarial entries), each with topology and math:

1. LVR-BAND CONVERGENCE TR...

