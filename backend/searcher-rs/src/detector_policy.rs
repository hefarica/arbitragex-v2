//! Static detector policy table — workbook QUOTEBASE-264 sheet
//! `13_DETECTOR_POLICY` (ARBX-0026), linked to strategies via the hop map's
//! `Detector_ID` column (264 rows).
//!
//! GENERATED from `docs/quotebase_detector_policy.json` +
//! `docs/quotebase_strategy_hop_map.json` by
//! `py scripts/xls/gen_detector_policy_rs.py` — do not edit rows by hand;
//! regenerate. The generator refuses to emit if the source drifts (60 rows,
//! Strategies == real hop_map counts summing 264, family-envelope
//! Allowed_Hops ⊆ Hop_Use, closed Graph_Policy/Hot_Seed vocabularies,
//! uniform Do_Not_Do, OBSERVE coherence, Execution_Class family-uniform
//! with sheet 11's closed shared 29-class vocabulary, non-empty distinct
//! Required_Data/Exact_Discovery_Criterion contracts).
//!
//! The policy dimensions are consumed GENERICALLY (no per-detector
//! hardcode):
//! - `GraphPolicy` — which graph family/adapter the detector's exact
//!   criterion maps to (annotation; discovery wiring key).
//! - `hop_use` — family hop envelope. Per-strategy bounds stay canonical in
//!   `strategy_hop_mask`; this envelope INTERSECTS them
//!   (`envelope_hop_bounds`) so a strategy can never escape its family.
//! - `Do_Not_Do` — universal guard: detector math is never replaced by a
//!   generic spot-price spread shortcut.
//! - `HotSeed` — hot-seed admission; the telemetry-only mode observes and
//!   emits evidence but never seeds a candidate (matches the 8 OBSERVE_ONLY
//!   strategies).
//! - `execution_class` — execution-precondition ANNOTATION shared verbatim
//!   by every strategy of the family (== sheet 11 col `Execution_Class`,
//!   closed 29-class vocabulary); not a dispatch verdict (same doctrine as
//!   `strategy_execution_class`).
//! - `example_surface` — the workbook's data-domain token (closed 10);
//!   `required_data_gate` maps it to the data classes the runtime can
//!   actually observe per tick (DP-002).
//! - `required_data` — the runtime NEEDS_DATA gate contract: the inputs the
//!   exact criterion needs. Absent input → observe, never approximate (R8).
//! - `exact_discovery_criterion` — the detector's OWN math; together with
//!   `DO_NOT_RULES` it forbids replacing it by a generic spot-price spread.

/// Graph family the detector's exact criterion maps to (12 workbook sentences).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GraphPolicy {
    FamilyCriterionAdapter,
    CrossVenueDepthPair,
    BasisSurfaceInstrumentGraph,
    StateEventDirtySubgraph,
    OrdersIntentsActionGraph,
    PositionLendingLiquidationGraph,
    PredictionClaimPayoutFilter,
    NftVenueSettlementFilter,
    ObserveOnlyNoOpportunity,
    ParityRedemptionValuation,
    DirtyEdgeClosedCycleSearch,
    PerDomainBridgeEdges,
}

impl GraphPolicy {
    /// Full workbook sentence (sheet 13 col `Graph_Policy`).
    pub fn as_str(self) -> &'static str {
        match self {
            GraphPolicy::FamilyCriterionAdapter => {
                "family detector exact criterion → compatible graph adapter"
            }
            GraphPolicy::CrossVenueDepthPair => "cross-venue depth-aware pair comparison",
            GraphPolicy::BasisSurfaceInstrumentGraph => {
                "basis/surface dislocation → instrument action graph"
            }
            GraphPolicy::StateEventDirtySubgraph => {
                "state-event trigger → localized dirty subgraph"
            }
            GraphPolicy::OrdersIntentsActionGraph => {
                "orders/intents → candidate order/action graph"
            }
            GraphPolicy::PositionLendingLiquidationGraph => {
                "position/state trigger → lending/liquidation action graph"
            }
            GraphPolicy::PredictionClaimPayoutFilter => {
                "prediction/claim graph; payout-equivalence filter"
            }
            GraphPolicy::NftVenueSettlementFilter => {
                "NFT/asset venue graph; settlement token filter"
            }
            GraphPolicy::ObserveOnlyNoOpportunity => "OBSERVE_ONLY — no opportunity=true",
            GraphPolicy::ParityRedemptionValuation => {
                "parity/redemption action edges + token valuation"
            }
            GraphPolicy::DirtyEdgeClosedCycleSearch => {
                "dirty pair/edge → closed-cycle/order route search"
            }
            GraphPolicy::PerDomainBridgeEdges => {
                "per-domain graph + supported bridge/preposition edges"
            }
        }
    }
}

/// Hot-seed admission mode (5 workbook modes).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HotSeed {
    DetectorThreshold,
    SpreadDislocation,
    StateEventDelta,
    TelemetryOnly,
    CrossDomainDislocation,
}

impl HotSeed {
    /// Full workbook sentence (sheet 13 col `Hot_Seed`).
    pub fn as_str(self) -> &'static str {
        match self {
            HotSeed::DetectorThreshold => "Detector-specific threshold from exact criterion",
            HotSeed::SpreadDislocation => "Spread/log-alpha/depth dislocation",
            HotSeed::StateEventDelta => "State change / post-event delta",
            HotSeed::TelemetryOnly => "No hot opportunity seed; telemetry evidence only",
            HotSeed::CrossDomainDislocation => "Cross-domain price/settlement dislocation",
        }
    }

    /// `false` only for the telemetry-only mode: the detector observes and
    /// emits evidence but must never seed a hot opportunity candidate.
    pub fn may_seed(self) -> bool {
        !matches!(self, HotSeed::TelemetryOnly)
    }
}

/// Detector's workbook `Example_Surface` token (closed 10-token vocabulary).
/// The RequiredDataGate maps SURFACE → data classes the runtime can actually
/// observe per tick; a surface with no tracked class gates `NotTracked`
/// (honest unknown — never Ready-by-default, R8).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DetectorSurface {
    DexAmm,
    ParityRedemption,
    DexState,
    Derivatives,
    Lending,
    Nft,
    IntentAuction,
    Prediction,
    CrossChain,
    CexDex,
}

impl DetectorSurface {
    /// Workbook token (sheet 13 col `Example_Surface`).
    pub fn as_str(self) -> &'static str {
        match self {
            DetectorSurface::DexAmm => "DEX_AMM",
            DetectorSurface::ParityRedemption => "PARITY_REDEMPTION",
            DetectorSurface::DexState => "DEX_STATE",
            DetectorSurface::Derivatives => "DERIVATIVES",
            DetectorSurface::Lending => "LENDING",
            DetectorSurface::Nft => "NFT",
            DetectorSurface::IntentAuction => "INTENT_AUCTION",
            DetectorSurface::Prediction => "PREDICTION",
            DetectorSurface::CrossChain => "CROSS_CHAIN",
            DetectorSurface::CexDex => "CEX_DEX",
        }
    }
}

/// One detector row of sheet `13_DETECTOR_POLICY`.
pub struct DetectorPolicy {
    pub detector_id: &'static str,
    pub graph_policy: GraphPolicy,
    /// Family hop envelope (strictly ascending, 2..=7).
    pub hop_use: &'static [u8],
    pub hot_seed: HotSeed,
    /// Sheet 13 col `Example_Surface` (closed 10 tokens) — the data-domain
    /// classification the RequiredDataGate keys on (DP-002).
    pub example_surface: DetectorSurface,
    /// Sheet 13 col `Execution_Class` — annotation shared verbatim by every
    /// strategy of the family (== sheet 11 col; closed 29 classes).
    pub execution_class: &'static str,
    /// Sheet 13 col `Required_Data` — inputs the exact criterion needs
    /// (runtime NEEDS_DATA gate contract; never approximated, R8).
    pub required_data: &'static str,
    /// Sheet 13 col `Exact_Discovery_Criterion` — the detector's own math.
    pub exact_discovery_criterion: &'static str,
    /// Strategies linked to this detector (== hop_map real count).
    pub strategy_count: u16,
}

impl DetectorPolicy {
    /// Inclusive hop bounds of the family envelope.
    pub fn hop_bounds(&self) -> (u8, u8) {
        (
            *self.hop_use.first().expect("non-empty hop_use"),
            *self.hop_use.last().expect("non-empty hop_use"),
        )
    }

    /// Whether `hop` is inside the family envelope.
    pub fn allows_hop(&self, hop: u8) -> bool {
        self.hop_use.contains(&hop)
    }
}

/// 60 detector policies, sorted ascending by Detector_ID — binary-searchable.
pub static DETECTOR_POLICIES: [DetectorPolicy; 60] = [
    DetectorPolicy {
        detector_id: "CF_BATCH",
        graph_policy: GraphPolicy::FamilyCriterionAdapter,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::DexAmm,
        execution_class: "DETERMINISTIC_SETTLEMENT",
        required_data: "Batch orders; pool state; clearing constraints; fee model; auction deadline.",
        exact_discovery_criterion: "Solve clearing allocation/price maximizing feasible surplus subject to pool invariant, order limits and conservation; compare settlement value after costs.",
        strategy_count: 2,
    },
    DetectorPolicy {
        detector_id: "CF_BOND",
        graph_policy: GraphPolicy::FamilyCriterionAdapter,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::DexAmm,
        execution_class: "DETERMINISTIC_EXECUTABLE",
        required_data: "Current supply/state; bonding curve parameters; mint/redeem fees; external executable quote.",
        exact_discovery_criterion: "For supply q and cost function C(q), marginal price p(q)=dC/dq; exact acquisition/redeem cost is integral/difference C(q+Δ)-C(q). Compare against executable external value.",
        strategy_count: 1,
    },
    DetectorPolicy {
        detector_id: "CF_CLAMM",
        graph_policy: GraphPolicy::FamilyCriterionAdapter,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::DexAmm,
        execution_class: "DETERMINISTIC_EXECUTABLE",
        required_data: "sqrtPriceX96; active liquidity; initialized ticks/tick bitmap; fee tier; decimals; QuoterV2 or exact local tick traversal.",
        exact_discovery_criterion: "Piecewise quote across initialized ticks using sqrtPrice, active liquidity L and fee tier; route Q_R(x) must traverse ticks exactly or use protocol quoter.",
        strategy_count: 1,
    },
    DetectorPolicy {
        detector_id: "CF_CONSTANT_SUM",
        graph_policy: GraphPolicy::FamilyCriterionAdapter,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::DexAmm,
        execution_class: "DETERMINISTIC_EXECUTABLE",
        required_data: "Reserves; invariant bounds; fees; exact competing route.",
        exact_discovery_criterion: "x+y=k inside the valid inventory region; quote is locally constant-price until a reserve boundary, then infeasible. Detect cross-venue executable deviation net of fees.",
        strategy_count: 1,
    },
    DetectorPolicy {
        detector_id: "CF_CPMM",
        graph_policy: GraphPolicy::FamilyCriterionAdapter,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::DexAmm,
        execution_class: "DETERMINISTIC_EXECUTABLE",
        required_data: "Reserves; token ordering/decimals; pool fee; exact route legs; gas.",
        exact_discovery_criterion: "For reserves (x,y), k=x·y. Exact-in quote with fee γ: Δy = y·(γΔx)/(x+γΔx). Route profit uses exact composition, not spot reserve ratio.",
        strategy_count: 1,
    },
    DetectorPolicy {
        detector_id: "CF_CROSSINV",
        graph_policy: GraphPolicy::FamilyCriterionAdapter,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::DexAmm,
        execution_class: "DETERMINISTIC_EXECUTABLE",
        required_data: "Per-leg protocol adapter/state; exact route; same-block snapshot; gas.",
        exact_discovery_criterion: "Compose protocol-correct quote adapters across heterogeneous invariants. No common spot formula; opportunity iff exact closed-route net profit >0.",
        strategy_count: 1,
    },
    DetectorPolicy {
        detector_id: "CF_DYNAMIC",
        graph_policy: GraphPolicy::FamilyCriterionAdapter,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::DexAmm,
        execution_class: "DETERMINISTIC_IF_ADAPTER",
        required_data: "Protocol-specific dynamic parameters; state snapshot; exact adapter; fees; oracle where protocol requires.",
        exact_discovery_criterion: "Q(x;θ(S_t)) with θ read from current contract state (weights, fees, peg, oracle or hybrid book). Reconstruct exact current invariant before comparing routes.",
        strategy_count: 4,
    },
    DetectorPolicy {
        detector_id: "CF_LB",
        graph_policy: GraphPolicy::FamilyCriterionAdapter,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::DexAmm,
        execution_class: "DETERMINISTIC_EXECUTABLE",
        required_data: "Active bin; bin step; per-bin reserves/liquidity; variable fee state; exact bin traversal.",
        exact_discovery_criterion: "Traverse discrete price bins; each bin uses constant-sum behavior at fixed bin price, with fixed+variable fee. Aggregate fills across bins until amount is satisfied.",
        strategy_count: 1,
    },
    DetectorPolicy {
        detector_id: "CF_PMM",
        graph_policy: GraphPolicy::FamilyCriterionAdapter,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::DexAmm,
        execution_class: "DETERMINISTIC_WITH_ORACLE",
        required_data: "PMM state/inventory; oracle/reference price; k/curve parameters; fees; exact protocol version.",
        exact_discovery_criterion: "Use protocol PMM inventory-aware curve around oracle/reference price; quote exact buy/sell branch and compare executable round-trip net of fees.",
        strategy_count: 1,
    },
    DetectorPolicy {
        detector_id: "CF_STABLESWAP",
        graph_policy: GraphPolicy::FamilyCriterionAdapter,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::DexAmm,
        execution_class: "DETERMINISTIC_EXECUTABLE",
        required_data: "All pool balances; amplification A; fees/admin fees; token rates/decimals; exact invariant version.",
        exact_discovery_criterion: "Use protocol StableSwap invariant D/A and solve post-trade y (Newton iteration as protocol does); compose exact dy across legs and optimize net route profit.",
        strategy_count: 1,
    },
    DetectorPolicy {
        detector_id: "CF_TWAMM",
        graph_policy: GraphPolicy::FamilyCriterionAdapter,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::DexAmm,
        execution_class: "DETERMINISTIC_POST_STATE",
        required_data: "Long-term order state; last virtual execution time; reserves; block timestamp; fees.",
        exact_discovery_criterion: "Advance virtual long-term orders to target block/time, derive post-state reserves, then run exact quote/round-trip on that state.",
        strategy_count: 1,
    },
    DetectorPolicy {
        detector_id: "CF_VAMM",
        graph_policy: GraphPolicy::FamilyCriterionAdapter,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::DexAmm,
        execution_class: "DETERMINISTIC_WITH_DERIVATIVE_STATE",
        required_data: "Virtual reserves; mark/index/oracle; funding; position constraints; fees.",
        exact_discovery_criterion: "Use virtual reserves/invariant plus index/mark/funding rules; opportunity is hedged net basis, not raw virtual reserve spread.",
        strategy_count: 1,
    },
    DetectorPolicy {
        detector_id: "CF_WEIGHTED",
        graph_policy: GraphPolicy::FamilyCriterionAdapter,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::DexAmm,
        execution_class: "DETERMINISTIC_EXECUTABLE",
        required_data: "Balances; normalized weights; swap fee; token scaling factors; exact pool type/version.",
        exact_discovery_criterion: "Balancer invariant V=Π_i B_i^{W_i}; derive exact outGivenIn from balances/weights/fees and evaluate route net profit.",
        strategy_count: 1,
    },
    DetectorPolicy {
        detector_id: "C_CEXDERIV",
        graph_policy: GraphPolicy::CrossVenueDepthPair,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::SpreadDislocation,
        example_surface: DetectorSurface::CexDex,
        execution_class: "EXTERNAL_DATA_REQUIRED",
        required_data: "CEX derivative book; mark/index/funding; DEX exact spot quote; margin/inventory; fees.",
        exact_discovery_criterion: "Hedged net basis/funding: Π=DerivativeLeg+SpotLeg+expected_funding-carry-fees-slippage-settlement risk.",
        strategy_count: 2,
    },
    DetectorPolicy {
        detector_id: "C_CEXDEX",
        graph_policy: GraphPolicy::CrossVenueDepthPair,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::SpreadDislocation,
        example_surface: DetectorSurface::CexDex,
        execution_class: "EXTERNAL_DATA_REQUIRED",
        required_data: "CEX L2/orderbook or firm quotes; DEX exact route quote; fees; inventory; latency; transfer/hedge model.",
        exact_discovery_criterion: "Π(q)=CEX_bid_VWAP(q)-DEX_buy_exact(q)-CEX/DEX fees-transfer/hedge costs, or reverse direction. Require firm depth and settlement/inventory model.",
        strategy_count: 10,
    },
    DetectorPolicy {
        detector_id: "D_BASIS",
        graph_policy: GraphPolicy::BasisSurfaceInstrumentGraph,
        hop_use: &[2, 3, 4, 5, 6],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::Derivatives,
        execution_class: "DERIVATIVE_DATA_REQUIRED",
        required_data: "Spot and derivative firm books; mark/index; rates/carry; funding; expiry; fees; margin.",
        exact_discovery_criterion: "Compare derivative price to carry-adjusted spot/forward replication; for perp include expected funding. Opportunity is hedged net convergence after margin, fees and funding.",
        strategy_count: 13,
    },
    DetectorPolicy {
        detector_id: "D_FUNDING",
        graph_policy: GraphPolicy::BasisSurfaceInstrumentGraph,
        hop_use: &[2, 3, 4, 5, 6],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::Derivatives,
        execution_class: "DERIVATIVE_DATA_REQUIRED",
        required_data: "Funding schedule/rate; perp and spot books; borrow rate; margin; horizon.",
        exact_discovery_criterion: "Expected net over horizon H: funding receipts - spot/perp basis convergence risk - borrow/carry - fees - slippage. Require hedgeable books.",
        strategy_count: 2,
    },
    DetectorPolicy {
        detector_id: "D_OPTIONS_PARITY",
        graph_policy: GraphPolicy::BasisSurfaceInstrumentGraph,
        hop_use: &[2, 3, 4, 5, 6],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::Derivatives,
        execution_class: "DERIVATIVE_DATA_REQUIRED",
        required_data: "Option bid/ask by strike/expiry; spot/forward; rate/dividend/carry; contract multiplier; fees.",
        exact_discovery_criterion: "European no-income core: C-P = S-PV(K). Conversion/reversal/box compare executable legs with same strike/expiry and financing/dividend adjustments.",
        strategy_count: 4,
    },
    DetectorPolicy {
        detector_id: "D_OPTIONS_SURFACE",
        graph_policy: GraphPolicy::BasisSurfaceInstrumentGraph,
        hop_use: &[2, 3, 4, 5, 6],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::Derivatives,
        execution_class: "DERIVATIVE_DATA_REQUIRED",
        required_data: "Full option surface bid/ask; strikes; expiries; spot/forward; rates; depth.",
        exact_discovery_criterion: "Enforce strike monotonicity/convexity and calendar consistency on executable bid/asks; e.g. equally spaced K: C(K1)-2C(K2)+C(K3)≥0. Trade violating butterfly/calendar only if all legs executable.",
        strategy_count: 6,
    },
    DetectorPolicy {
        detector_id: "D_SETTLE",
        graph_policy: GraphPolicy::BasisSurfaceInstrumentGraph,
        hop_use: &[2, 3, 4, 5, 6],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::Derivatives,
        execution_class: "DERIVATIVE_DATA_REQUIRED",
        required_data: "Contract payoff; mark/index/settlement oracle; expiry; position/margin state; executable hedge.",
        exact_discovery_criterion: "Compute deterministic payoff/settlement value under venue rules and compare with executable acquisition/hedge/unwind cost; include liquidation penalty and expiry timing.",
        strategy_count: 5,
    },
    DetectorPolicy {
        detector_id: "E_AUCTION",
        graph_policy: GraphPolicy::StateEventDirtySubgraph,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::StateEventDelta,
        example_surface: DetectorSurface::DexState,
        execution_class: "DETERMINISTIC_AUCTION",
        required_data: "Auction price/curve; remaining lot; deadline; collateral/external value; unwind liquidity.",
        exact_discovery_criterion: "Π(q,t)=FairExecutableValue(q)-AuctionPurchaseCost(q,t)-UnwindCost(q)-Gas; solve feasible q and timing under auction rules.",
        strategy_count: 2,
    },
    DetectorPolicy {
        detector_id: "E_LATENCY",
        graph_policy: GraphPolicy::StateEventDirtySubgraph,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::StateEventDelta,
        example_surface: DetectorSurface::DexState,
        execution_class: "LATENCY_SENSITIVE",
        required_data: "Independent timestamps; fast/slow quotes; volatility estimate; execution latency; firm liquidity.",
        exact_discovery_criterion: "Trigger z=(p_fast-p_slow)/σ only identifies divergence; opportunity requires a firm executable quote/route and Π_net>0 before stale side updates.",
        strategy_count: 6,
    },
    DetectorPolicy {
        detector_id: "E_ORACLE",
        graph_policy: GraphPolicy::StateEventDirtySubgraph,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::StateEventDelta,
        example_surface: DetectorSurface::DexState,
        execution_class: "DETERMINISTIC_POST_ORACLE",
        required_data: "Oracle round/update; protocol state; affected positions/pools; post-update quotes; oracle freshness.",
        exact_discovery_criterion: "Recompute protocol state under confirmed oracle update O_t→O_{t+1}; evaluate resulting executable liquidation/repricing routes. Oracle delta alone is not profit.",
        strategy_count: 2,
    },
    DetectorPolicy {
        detector_id: "E_POST",
        graph_policy: GraphPolicy::StateEventDirtySubgraph,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::StateEventDelta,
        example_surface: DetectorSurface::DexState,
        execution_class: "DETERMINISTIC_POST_STATE",
        required_data: "Event/receipt/log; pre/post state or deterministic transition; affected pools/routes; exact quote adapters; gas.",
        exact_discovery_criterion: "Apply confirmed/public event transition S'=T_e(S), quote all affected routes on S', then Π_R(x;S')=Q_R(x;S')-x-C_R(x). Never infer profit from event size alone.",
        strategy_count: 6,
    },
    DetectorPolicy {
        detector_id: "E_STATE",
        graph_policy: GraphPolicy::StateEventDirtySubgraph,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::StateEventDelta,
        example_surface: DetectorSurface::DexState,
        execution_class: "DETERMINISTIC_POST_STATE",
        required_data: "Protocol event/state variables; before/after block state; affected claims/pools; redemption/execution path.",
        exact_discovery_criterion: "Given deterministic transition e, compute S'=T_e(S); revalue all claims/routes V(S') and compare executable acquisition/unwind cost. Emit only if transition is known and settlement executable.",
        strategy_count: 15,
    },
    DetectorPolicy {
        detector_id: "I_BATCH",
        graph_policy: GraphPolicy::OrdersIntentsActionGraph,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::IntentAuction,
        execution_class: "DETERMINISTIC_SETTLEMENT",
        required_data: "Batch intents; limit prices; token graph; external quotes; settlement constraints.",
        exact_discovery_criterion: "Find feasible matching/clearing maximizing total surplus across orders plus external liquidity, subject to conservation and individual limit-price constraints.",
        strategy_count: 4,
    },
    DetectorPolicy {
        detector_id: "I_DUTCH",
        graph_policy: GraphPolicy::OrdersIntentsActionGraph,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::IntentAuction,
        execution_class: "DETERMINISTIC_AUCTION",
        required_data: "Auction start/end price/time; current timestamp; firm external quote; competition/latency.",
        exact_discovery_criterion: "Given auction price P(t), execute when external executable value V(t)-P(t)-costs is maximized subject to deadline/competition and min surplus.",
        strategy_count: 1,
    },
    DetectorPolicy {
        detector_id: "I_ORDERFLOW",
        graph_policy: GraphPolicy::OrdersIntentsActionGraph,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::IntentAuction,
        execution_class: "AUTHORIZED_FLOW_ONLY",
        required_data: "Explicitly authorized/private order feed; bundle contents; settlement rules; route quotes.",
        exact_discovery_criterion: "Only evaluate flows explicitly available to this searcher; price the authorized order/bundle against alternative settlement and costs. No inference of privileged/private data.",
        strategy_count: 4,
    },
    DetectorPolicy {
        detector_id: "I_ROUTE",
        graph_policy: GraphPolicy::OrdersIntentsActionGraph,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::IntentAuction,
        execution_class: "DETERMINISTIC_SETTLEMENT",
        required_data: "Intent/order constraints; balances/approvals; external liquidity; gas; valid-to; settlement contract.",
        exact_discovery_criterion: "maximize user/solver executable surplus under limit prices, token conservation, balances and settlement constraints; compare AMM/CLOB/RFQ routes.",
        strategy_count: 9,
    },
    DetectorPolicy {
        detector_id: "L_AUCTION",
        graph_policy: GraphPolicy::PositionLendingLiquidationGraph,
        hop_use: &[2, 3, 4],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::Lending,
        execution_class: "DETERMINISTIC_AUCTION",
        required_data: "Auction lot/price curve; collateral; protocol debt; deadline; market liquidity.",
        exact_discovery_criterion: "Π(q,t)=ConservativeCollateralValue(q)-AuctionCost(q,t)-UnwindCost(q)-Gas-capital risk; solve lot size and timing under exact auction rules.",
        strategy_count: 4,
    },
    DetectorPolicy {
        detector_id: "L_COLLATERAL",
        graph_policy: GraphPolicy::PositionLendingLiquidationGraph,
        hop_use: &[2, 3, 4],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::Lending,
        execution_class: "DETERMINISTIC_IF_REDEEMABLE",
        required_data: "Collateral/debt token exchange rates; protocol oracle; repayment/redemption functions; market quotes.",
        exact_discovery_criterion: "Value debt/collateral claims in common numeraire using protocol conversion/redemption, then compare executable acquisition/repayment paths net of costs.",
        strategy_count: 5,
    },
    DetectorPolicy {
        detector_id: "L_LIQ",
        graph_policy: GraphPolicy::PositionLendingLiquidationGraph,
        hop_use: &[2, 3, 4],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::Lending,
        execution_class: "DETERMINISTIC_LIQUIDATION",
        required_data: "Account collateral/debt; liquidation thresholds/bonuses; oracle prices; close factor; firm unwind route; gas/flash fee.",
        exact_discovery_criterion: "Eligibility via protocol health metric (Aave: HF=(collateral value×weighted liquidation threshold)/borrow value; liquidation when HF<1). Π=seized collateral executable value-debt repaid-gas-flash/unwind costs.",
        strategy_count: 7,
    },
    DetectorPolicy {
        detector_id: "L_LOOP",
        graph_policy: GraphPolicy::PositionLendingLiquidationGraph,
        hop_use: &[2, 3, 4],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::Lending,
        execution_class: "DETERMINISTIC_POSITION_STRATEGY",
        required_data: "LTV/liquidation threshold; supply/borrow rates; collateral factor; loop fees; gas.",
        exact_discovery_criterion: "Optimize leverage n / LTV to maximize net APY or finite-horizon P&L subject to health-factor/risk buffer and borrow/supply rate feedback.",
        strategy_count: 1,
    },
    DetectorPolicy {
        detector_id: "L_RATE",
        graph_policy: GraphPolicy::PositionLendingLiquidationGraph,
        hop_use: &[2, 3, 4],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::Lending,
        execution_class: "DETERMINISTIC_IF_POSITIONS",
        required_data: "Supply/borrow indices and rates; utilization; collateral/margin requirements; fees; market hedge.",
        exact_discovery_criterion: "Net carry over horizon H = earned supply/yield - borrow cost - protocol fees - hedge/transaction costs - capital/risk charge.",
        strategy_count: 8,
    },
    DetectorPolicy {
        detector_id: "M_AMM",
        graph_policy: GraphPolicy::PredictionClaimPayoutFilter,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::Prediction,
        execution_class: "DETERMINISTIC_IF_MATCHED_CLAIM",
        required_data: "Prediction AMM invariant; outcome IDs; firm order book; complete-set mechanics; resolution source.",
        exact_discovery_criterion: "Quote conditional-token AMM exactly and compare with firm outcome order-book quotes for identical payout claim; net of split/merge, fees and settlement.",
        strategy_count: 1,
    },
    DetectorPolicy {
        detector_id: "M_COMPLETE",
        graph_policy: GraphPolicy::PredictionClaimPayoutFilter,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::Prediction,
        execution_class: "DETERMINISTIC_IF_COMPLETE_SET",
        required_data: "Outcome partition; collateral denomination; firm bid/ask per outcome; split/merge/redeem availability; fees.",
        exact_discovery_criterion: "For exhaustive mutually exclusive outcomes backed by 1 collateral unit: buy all if Σ asks_i + fees < 1; mint/sell all if Σ bids_i - fees > 1, respecting split/merge/redeem mechanics.",
        strategy_count: 3,
    },
    DetectorPolicy {
        detector_id: "M_CROSS",
        graph_policy: GraphPolicy::PredictionClaimPayoutFilter,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::Prediction,
        execution_class: "EXTERNAL_DATA_REQUIRED",
        required_data: "Canonical event identity; resolution rules; outcome mapping; firm books; fees; settlement horizon.",
        exact_discovery_criterion: "Normalize markets only if they reference the same event, resolution source and payout semantics; then compute firm cross-venue spread after fees/capital lockup.",
        strategy_count: 1,
    },
    DetectorPolicy {
        detector_id: "M_LOGIC",
        graph_policy: GraphPolicy::PredictionClaimPayoutFilter,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::Prediction,
        execution_class: "DETERMINISTIC_IF_PAYOFF_MODEL",
        required_data: "Outcome payoff matrix; logical relation/event mapping; firm quotes; resolution rules.",
        exact_discovery_criterion: "Represent outcome payoffs as linear constraints; solve LP for a portfolio with non-negative state payoff and negative acquisition cost (after fees), or detect violated logical probability bounds.",
        strategy_count: 4,
    },
    DetectorPolicy {
        detector_id: "N_AMM",
        graph_policy: GraphPolicy::NftVenueSettlementFilter,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::Nft,
        execution_class: "DETERMINISTIC_IF_FIRM_EXIT",
        required_data: "NFT AMM curve/state; eligible token set; marketplace firm quotes; royalties; fees.",
        exact_discovery_criterion: "Use exact NFT AMM bonding curve quote for acquisition/unwind and compare against firm marketplace bid/ask for same eligible asset/collection, net of royalties/fees.",
        strategy_count: 2,
    },
    DetectorPolicy {
        detector_id: "N_FLOOR",
        graph_policy: GraphPolicy::NftVenueSettlementFilter,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::Nft,
        execution_class: "SIGNAL_UNLESS_FIRM_EXIT",
        required_data: "Listings; trait metadata; firm bids; sale history; royalty/fees; redemption if applicable.",
        exact_discovery_criterion: "Floor/trait deviation is only a signal. Emit only when a firm buyer/bid or deterministic redemption converts it into Π_net>0; otherwise observe-only.",
        strategy_count: 5,
    },
    DetectorPolicy {
        detector_id: "N_IDENTICAL",
        graph_policy: GraphPolicy::NftVenueSettlementFilter,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::Nft,
        execution_class: "DETERMINISTIC_IF_FIRM_BID",
        required_data: "TokenId identity; executable listing/ask and bid; royalties; marketplace fees; ownership/approval state.",
        exact_discovery_criterion: "For identical tokenId/asset, Π=firm exit bid proceeds-acquisition ask-royalty-marketplace/gas costs; no floor proxy allowed for the exit leg.",
        strategy_count: 6,
    },
    DetectorPolicy {
        detector_id: "N_LIQ",
        graph_policy: GraphPolicy::NftVenueSettlementFilter,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::Nft,
        execution_class: "DETERMINISTIC_IF_FIRM_BID",
        required_data: "Loan state; liquidation rules; oracle/appraisal; firm NFT bid/liquidity; gas.",
        exact_discovery_criterion: "Eligibility from lending protocol; Π=conservative firm-bid value of seized NFT-debt repayment-liquidation/gas/unwind costs. Floor alone cannot be profit.",
        strategy_count: 1,
    },
    DetectorPolicy {
        detector_id: "N_REDEEM",
        graph_policy: GraphPolicy::NftVenueSettlementFilter,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::Nft,
        execution_class: "DETERMINISTIC_IF_REDEEMABLE",
        required_data: "Fractionalization/redemption contract; bundle composition; firm component bids; fees.",
        exact_discovery_criterion: "Π=deterministic redeem/unbundle value-acquisition cost-component unwind costs; inverse path for mint/bundle-and-sell if contract permits.",
        strategy_count: 4,
    },
    DetectorPolicy {
        detector_id: "OBSERVE",
        graph_policy: GraphPolicy::ObserveOnlyNoOpportunity,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::TelemetryOnly,
        example_surface: DetectorSurface::DexState,
        execution_class: "OBSERVE_ONLY",
        required_data: "Telemetry/evidence only; no fabricated private flow, reorg certainty, future resolution or unavailable external data.",
        exact_discovery_criterion: "No is_opportunity=true. Emit structured evidence only until a lawful, deterministic, executable settlement path and required public/authorized data are present.",
        strategy_count: 8,
    },
    DetectorPolicy {
        detector_id: "P_4626",
        graph_policy: GraphPolicy::ParityRedemptionValuation,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::ParityRedemption,
        execution_class: "DETERMINISTIC_IF_REDEEMABLE",
        required_data: "convertToAssets/Shares; previewDeposit/Redeem; maxDeposit/Redeem; vault totalAssets; share/asset market quotes.",
        exact_discovery_criterion: "Use preview/convertToAssets and convertToShares plus actual deposit/redeem limits/fees; compare executable share market price to redeemable asset value.",
        strategy_count: 3,
    },
    DetectorPolicy {
        detector_id: "P_LST",
        graph_policy: GraphPolicy::ParityRedemptionValuation,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::ParityRedemption,
        execution_class: "SETTLEMENT_DELAY_SENSITIVE",
        required_data: "Protocol exchange/redemption rate; withdrawal queue/delay; underlying price; firm LST quotes.",
        exact_discovery_criterion: "Π=RedemptionValue(exchange_rate,t)-MarketAcquisitionCost-queue/carry/fees; cross-LST uses a common underlying redemption unit.",
        strategy_count: 4,
    },
    DetectorPolicy {
        detector_id: "P_NAV",
        graph_policy: GraphPolicy::ParityRedemptionValuation,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::ParityRedemption,
        execution_class: "DETERMINISTIC_IF_REDEEMABLE",
        required_data: "Component weights/holdings; redemption/mint rules; firm component quotes; fees.",
        exact_discovery_criterion: "NAV from exact redeemable components; Π=RedeemValue(token)-MarketCost(token)-unwind/fees, or inverse mint-and-sell path.",
        strategy_count: 5,
    },
    DetectorPolicy {
        detector_id: "P_PEG",
        graph_policy: GraphPolicy::ParityRedemptionValuation,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::ParityRedemption,
        execution_class: "DETERMINISTIC_IF_REDEEMABLE",
        required_data: "Mint/redeem functions/rates; market quotes; fees/caps; oracle if protocol uses one.",
        exact_discovery_criterion: "Compare executable market price with protocol mint/redeem conversion after fees and caps: Π=max(RedeemValue-MarketBuyCost, MarketSellProceeds-MintCost)-costs.",
        strategy_count: 7,
    },
    DetectorPolicy {
        detector_id: "P_PTYT",
        graph_policy: GraphPolicy::ParityRedemptionValuation,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::ParityRedemption,
        execution_class: "DETERMINISTIC_IF_REDEEMABLE",
        required_data: "PT/YT/SY exchange rates; expiry; mint/redeem path; firm PT/YT quotes; underlying APY.",
        exact_discovery_criterion: "Core parity: P_PT+P_YT≈P_underlying (same accounting asset, same maturity). Also implied APY from PT/YT exchange rate; evaluate mint/redeem + market legs net of fees.",
        strategy_count: 3,
    },
    DetectorPolicy {
        detector_id: "P_WRAP",
        graph_policy: GraphPolicy::ParityRedemptionValuation,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::ParityRedemption,
        execution_class: "DETERMINISTIC_IF_CONVERTIBLE",
        required_data: "wrap/unwrap conversion; exchange rate; fees; underlying and wrapper firm quotes; bridge state if bridged.",
        exact_discovery_criterion: "Compare exact wrap→sell and buy→unwrap paths against wrapper conversion ratio, fees and delay. Opportunity iff closed economic value >0.",
        strategy_count: 5,
    },
    DetectorPolicy {
        detector_id: "P_YIELD",
        graph_policy: GraphPolicy::ParityRedemptionValuation,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::DetectorThreshold,
        example_surface: DetectorSurface::ParityRedemption,
        execution_class: "DETERMINISTIC_IF_SETTLEABLE",
        required_data: "Maturities; redemption values; market prices; yield indices/APY; fees; settlement path.",
        exact_discovery_criterion: "Convert each claim to discount factor / implied yield for common underlying and maturity basis; detect executable curve violations after carry, fees and settlement delay.",
        strategy_count: 3,
    },
    DetectorPolicy {
        detector_id: "R_BASKET_NAV",
        graph_policy: GraphPolicy::DirtyEdgeClosedCycleSearch,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::SpreadDislocation,
        example_surface: DetectorSurface::DexAmm,
        execution_class: "DETERMINISTIC_IF_REDEEMABLE",
        required_data: "Basket composition/weights; mint/redeem contract state; exact component quotes; fees; redemption constraints.",
        exact_discovery_criterion: "NAV=Σ_i w_i·V_i (or exact redemption outputs). Π=ExecutableRedeemOrMintValue-BasketAcquisitionCost-all_costs. Emit only if mint/redeem or firm unwind exists.",
        strategy_count: 2,
    },
    DetectorPolicy {
        detector_id: "R_CLOSED_CYCLE",
        graph_policy: GraphPolicy::DirtyEdgeClosedCycleSearch,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::SpreadDislocation,
        example_surface: DetectorSurface::DexAmm,
        execution_class: "DETERMINISTIC_EXECUTABLE",
        required_data: "Full ordered route legs; per-leg protocol adapter; reserves/slot0/ticks/bins; token decimals; pool fees; same-block state; gas; optional flash fee.",
        exact_discovery_criterion: "Q_R(x)=q_n(...q_2(q_1(x))); Π_R(x)=Q_R(x)-x-C_R(x). Opportunity iff max_x Π_R(x)>0. Marginal prefilter: Σ_e[-ln((1-fee_e)·rate_e)]<0.",
        strategy_count: 25,
    },
    DetectorPolicy {
        detector_id: "R_COW",
        graph_policy: GraphPolicy::DirtyEdgeClosedCycleSearch,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::SpreadDislocation,
        example_surface: DetectorSurface::DexAmm,
        execution_class: "DETERMINISTIC_SETTLEMENT",
        required_data: "Orders/intents; limit prices; balances; valid-to; external liquidity quotes; settlement constraints.",
        exact_discovery_criterion: "maximize total executable trader surplus subject to limit prices, token conservation and settlement constraints; compare internal matching vs external liquidity.",
        strategy_count: 1,
    },
    DetectorPolicy {
        detector_id: "R_DIRECT_INDIRECT",
        graph_policy: GraphPolicy::DirtyEdgeClosedCycleSearch,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::SpreadDislocation,
        example_surface: DetectorSurface::DexAmm,
        execution_class: "DETERMINISTIC_EXECUTABLE",
        required_data: "Two competing paths with full legs; exact per-leg quotes; fees; block-consistent pool state; gas delta.",
        exact_discovery_criterion: "Δ(x)=Q_indirect(x)-Q_direct(x)-C_incremental(x); evaluate both executable paths at identical input/state; opportunity iff max_x Δ(x)>0.",
        strategy_count: 2,
    },
    DetectorPolicy {
        detector_id: "R_ORDERBOOK",
        graph_policy: GraphPolicy::DirtyEdgeClosedCycleSearch,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::SpreadDislocation,
        example_surface: DetectorSurface::DexAmm,
        execution_class: "EXTERNAL_DATA_REQUIRED",
        required_data: "Firm bids/asks or RFQs; depth ladder; quote timestamp; venue fees; settlement latency/cost; DEX exact quote where applicable.",
        exact_discovery_criterion: "Π(q)=Proceeds_bid(q)-Cost_ask(q)-taker_fees(q)-settlement_cost(q)-hedge_cost(q). Prices must be VWAP/depth-aware, not top-of-book only.",
        strategy_count: 4,
    },
    DetectorPolicy {
        detector_id: "R_SPLIT",
        graph_policy: GraphPolicy::DirtyEdgeClosedCycleSearch,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::SpreadDislocation,
        example_surface: DetectorSurface::DexAmm,
        execution_class: "DETERMINISTIC_EXECUTABLE",
        required_data: "Parallel executable paths; exact quote functions; gas activation cost per path; shared input amount.",
        exact_discovery_criterion: "maximize Σ_j Q_j(x_j)-C(x_1..x_m), subject to Σ_j x_j=x and x_j≥0; compare optimum against best unsplit route.",
        strategy_count: 2,
    },
    DetectorPolicy {
        detector_id: "X_BRIDGE",
        graph_policy: GraphPolicy::PerDomainBridgeEdges,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::CrossDomainDislocation,
        example_surface: DetectorSurface::CrossChain,
        execution_class: "NONATOMIC_BRIDGE_REQUIRED",
        required_data: "Bridge quote/fee; message/finality state; source/destination liquidity; delay distribution; inventory/capital.",
        exact_discovery_criterion: "E[Π]=DestinationValue-SourceCost-bridge_fee-gas-carry-CVaR(latency/finality/reorg). Must model non-atomic settlement and failure states.",
        strategy_count: 16,
    },
    DetectorPolicy {
        detector_id: "X_ORACLE",
        graph_policy: GraphPolicy::PerDomainBridgeEdges,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::CrossDomainDislocation,
        example_surface: DetectorSurface::CrossChain,
        execution_class: "EXTERNAL_SETTLEMENT_REQUIRED",
        required_data: "Cross-domain oracle/state timestamps; finality; firm routes; bridge/inventory settlement model.",
        exact_discovery_criterion: "Normalize same economic claim across domains; discount for timestamp/finality and require an executable settlement path. Signal deviation alone cannot emit.",
        strategy_count: 2,
    },
    DetectorPolicy {
        detector_id: "X_PREPOS",
        graph_policy: GraphPolicy::PerDomainBridgeEdges,
        hop_use: &[2, 3, 4, 5, 6, 7],
        hot_seed: HotSeed::CrossDomainDislocation,
        example_surface: DetectorSurface::CrossChain,
        execution_class: "NONATOMIC_INVENTORY_REQUIRED",
        required_data: "Per-chain firm quotes; inventory balances; finality/latency; gas; settlement status; accounting FX.",
        exact_discovery_criterion: "No atomic bridge assumption: aggregate independently executable legs using inventory on both domains. Π=Σ proceeds-Σ costs-carry/finality/hedge costs.",
        strategy_count: 12,
    },
];

/// (MEV_ID, Detector_ID) link from the hop map, sorted ascending by MEV_ID.
pub static STRATEGY_DETECTOR: [(&str, &str); 264] = [
    ("MEV-01-001", "R_CLOSED_CYCLE"),
    ("MEV-01-002", "R_CLOSED_CYCLE"),
    ("MEV-01-003", "R_CLOSED_CYCLE"),
    ("MEV-01-004", "R_CLOSED_CYCLE"),
    ("MEV-01-005", "R_CLOSED_CYCLE"),
    ("MEV-01-006", "R_CLOSED_CYCLE"),
    ("MEV-01-007", "R_CLOSED_CYCLE"),
    ("MEV-01-008", "R_CLOSED_CYCLE"),
    ("MEV-01-009", "R_ORDERBOOK"),
    ("MEV-01-010", "R_ORDERBOOK"),
    ("MEV-01-011", "R_ORDERBOOK"),
    ("MEV-01-012", "R_ORDERBOOK"),
    ("MEV-01-013", "R_CLOSED_CYCLE"),
    ("MEV-01-014", "R_CLOSED_CYCLE"),
    ("MEV-01-015", "R_CLOSED_CYCLE"),
    ("MEV-01-016", "R_CLOSED_CYCLE"),
    ("MEV-01-017", "R_CLOSED_CYCLE"),
    ("MEV-01-018", "R_CLOSED_CYCLE"),
    ("MEV-01-019", "R_CLOSED_CYCLE"),
    ("MEV-01-020", "R_CLOSED_CYCLE"),
    ("MEV-01-021", "R_CLOSED_CYCLE"),
    ("MEV-01-022", "R_CLOSED_CYCLE"),
    ("MEV-01-023", "R_DIRECT_INDIRECT"),
    ("MEV-01-024", "R_DIRECT_INDIRECT"),
    ("MEV-01-025", "R_SPLIT"),
    ("MEV-01-026", "R_SPLIT"),
    ("MEV-01-027", "R_BASKET_NAV"),
    ("MEV-01-028", "R_BASKET_NAV"),
    ("MEV-01-029", "R_CLOSED_CYCLE"),
    ("MEV-01-030", "R_COW"),
    ("MEV-01-031", "R_CLOSED_CYCLE"),
    ("MEV-01-032", "R_CLOSED_CYCLE"),
    ("MEV-01-033", "R_CLOSED_CYCLE"),
    ("MEV-01-034", "R_CLOSED_CYCLE"),
    ("MEV-01-035", "R_CLOSED_CYCLE"),
    ("MEV-01-036", "R_CLOSED_CYCLE"),
    ("MEV-02-001", "CF_CPMM"),
    ("MEV-02-002", "CF_CONSTANT_SUM"),
    ("MEV-02-003", "CF_STABLESWAP"),
    ("MEV-02-004", "CF_WEIGHTED"),
    ("MEV-02-005", "CF_CLAMM"),
    ("MEV-02-006", "CF_LB"),
    ("MEV-02-007", "CF_PMM"),
    ("MEV-02-008", "CF_DYNAMIC"),
    ("MEV-02-009", "CF_DYNAMIC"),
    ("MEV-02-010", "CF_BOND"),
    ("MEV-02-011", "CF_DYNAMIC"),
    ("MEV-02-012", "CF_VAMM"),
    ("MEV-02-013", "CF_DYNAMIC"),
    ("MEV-02-014", "CF_TWAMM"),
    ("MEV-02-015", "CF_BATCH"),
    ("MEV-02-016", "CF_BATCH"),
    ("MEV-02-017", "CF_CROSSINV"),
    ("MEV-03-001", "E_POST"),
    ("MEV-03-002", "E_POST"),
    ("MEV-03-003", "E_POST"),
    ("MEV-03-004", "E_POST"),
    ("MEV-03-005", "E_POST"),
    ("MEV-03-006", "E_POST"),
    ("MEV-03-007", "E_LATENCY"),
    ("MEV-03-008", "E_LATENCY"),
    ("MEV-03-009", "E_ORACLE"),
    ("MEV-03-010", "E_ORACLE"),
    ("MEV-03-011", "E_STATE"),
    ("MEV-03-012", "E_STATE"),
    ("MEV-03-013", "E_STATE"),
    ("MEV-03-014", "E_STATE"),
    ("MEV-03-015", "E_STATE"),
    ("MEV-03-016", "E_STATE"),
    ("MEV-03-017", "E_STATE"),
    ("MEV-03-018", "E_STATE"),
    ("MEV-03-019", "E_STATE"),
    ("MEV-03-020", "E_STATE"),
    ("MEV-03-021", "E_STATE"),
    ("MEV-03-022", "E_STATE"),
    ("MEV-03-023", "E_STATE"),
    ("MEV-03-024", "E_STATE"),
    ("MEV-03-025", "E_AUCTION"),
    ("MEV-03-026", "E_AUCTION"),
    ("MEV-03-027", "E_LATENCY"),
    ("MEV-03-028", "E_LATENCY"),
    ("MEV-03-029", "OBSERVE"),
    ("MEV-03-030", "OBSERVE"),
    ("MEV-03-031", "E_STATE"),
    ("MEV-04-001", "P_PEG"),
    ("MEV-04-002", "P_PEG"),
    ("MEV-04-003", "P_PEG"),
    ("MEV-04-004", "P_PEG"),
    ("MEV-04-005", "P_PEG"),
    ("MEV-04-006", "P_WRAP"),
    ("MEV-04-007", "P_WRAP"),
    ("MEV-04-008", "P_WRAP"),
    ("MEV-04-009", "P_WRAP"),
    ("MEV-04-010", "P_4626"),
    ("MEV-04-011", "P_4626"),
    ("MEV-04-012", "P_4626"),
    ("MEV-04-013", "P_NAV"),
    ("MEV-04-014", "P_NAV"),
    ("MEV-04-015", "P_NAV"),
    ("MEV-04-016", "P_NAV"),
    ("MEV-04-017", "P_PEG"),
    ("MEV-04-018", "P_PEG"),
    ("MEV-04-019", "P_LST"),
    ("MEV-04-020", "P_LST"),
    ("MEV-04-021", "P_LST"),
    ("MEV-04-022", "P_LST"),
    ("MEV-04-023", "P_WRAP"),
    ("MEV-04-024", "P_YIELD"),
    ("MEV-04-025", "P_NAV"),
    ("MEV-04-026", "P_PTYT"),
    ("MEV-04-027", "P_PTYT"),
    ("MEV-04-028", "P_PTYT"),
    ("MEV-04-029", "P_YIELD"),
    ("MEV-04-030", "P_YIELD"),
    ("MEV-04-031", "OBSERVE"),
    ("MEV-05-001", "C_CEXDEX"),
    ("MEV-05-002", "C_CEXDEX"),
    ("MEV-05-003", "C_CEXDEX"),
    ("MEV-05-004", "C_CEXDEX"),
    ("MEV-05-005", "C_CEXDEX"),
    ("MEV-05-006", "C_CEXDEX"),
    ("MEV-05-007", "C_CEXDERIV"),
    ("MEV-05-008", "C_CEXDERIV"),
    ("MEV-05-009", "E_LATENCY"),
    ("MEV-05-010", "E_LATENCY"),
    ("MEV-05-011", "C_CEXDEX"),
    ("MEV-05-012", "C_CEXDEX"),
    ("MEV-05-013", "C_CEXDEX"),
    ("MEV-05-014", "C_CEXDEX"),
    ("MEV-06-001", "X_PREPOS"),
    ("MEV-06-002", "X_PREPOS"),
    ("MEV-06-003", "X_PREPOS"),
    ("MEV-06-004", "X_PREPOS"),
    ("MEV-06-005", "X_PREPOS"),
    ("MEV-06-006", "X_PREPOS"),
    ("MEV-06-007", "X_PREPOS"),
    ("MEV-06-008", "X_BRIDGE"),
    ("MEV-06-009", "X_BRIDGE"),
    ("MEV-06-010", "X_PREPOS"),
    ("MEV-06-011", "X_PREPOS"),
    ("MEV-06-012", "X_BRIDGE"),
    ("MEV-06-013", "X_BRIDGE"),
    ("MEV-06-014", "X_BRIDGE"),
    ("MEV-06-015", "X_BRIDGE"),
    ("MEV-06-016", "X_BRIDGE"),
    ("MEV-06-017", "X_BRIDGE"),
    ("MEV-06-018", "X_BRIDGE"),
    ("MEV-06-019", "X_BRIDGE"),
    ("MEV-06-020", "X_BRIDGE"),
    ("MEV-06-021", "X_BRIDGE"),
    ("MEV-06-022", "X_BRIDGE"),
    ("MEV-06-023", "X_PREPOS"),
    ("MEV-06-024", "X_BRIDGE"),
    ("MEV-06-025", "X_BRIDGE"),
    ("MEV-06-026", "X_BRIDGE"),
    ("MEV-06-027", "X_PREPOS"),
    ("MEV-06-028", "X_PREPOS"),
    ("MEV-06-029", "X_ORACLE"),
    ("MEV-06-030", "X_ORACLE"),
    ("MEV-07-001", "D_BASIS"),
    ("MEV-07-002", "D_BASIS"),
    ("MEV-07-003", "D_BASIS"),
    ("MEV-07-004", "D_BASIS"),
    ("MEV-07-005", "D_BASIS"),
    ("MEV-07-006", "D_BASIS"),
    ("MEV-07-007", "D_BASIS"),
    ("MEV-07-008", "D_FUNDING"),
    ("MEV-07-009", "D_FUNDING"),
    ("MEV-07-010", "D_BASIS"),
    ("MEV-07-011", "D_BASIS"),
    ("MEV-07-012", "D_BASIS"),
    ("MEV-07-013", "D_BASIS"),
    ("MEV-07-014", "D_BASIS"),
    ("MEV-07-015", "D_BASIS"),
    ("MEV-07-016", "D_OPTIONS_PARITY"),
    ("MEV-07-017", "D_OPTIONS_PARITY"),
    ("MEV-07-018", "D_OPTIONS_PARITY"),
    ("MEV-07-019", "D_OPTIONS_PARITY"),
    ("MEV-07-020", "D_OPTIONS_SURFACE"),
    ("MEV-07-021", "D_OPTIONS_SURFACE"),
    ("MEV-07-022", "D_OPTIONS_SURFACE"),
    ("MEV-07-023", "D_OPTIONS_SURFACE"),
    ("MEV-07-024", "D_OPTIONS_SURFACE"),
    ("MEV-07-025", "D_OPTIONS_SURFACE"),
    ("MEV-07-026", "D_SETTLE"),
    ("MEV-07-027", "D_SETTLE"),
    ("MEV-07-028", "D_SETTLE"),
    ("MEV-07-029", "D_SETTLE"),
    ("MEV-07-030", "D_SETTLE"),
    ("MEV-08-001", "L_RATE"),
    ("MEV-08-002", "L_RATE"),
    ("MEV-08-003", "L_RATE"),
    ("MEV-08-004", "L_RATE"),
    ("MEV-08-005", "L_RATE"),
    ("MEV-08-006", "L_COLLATERAL"),
    ("MEV-08-007", "L_COLLATERAL"),
    ("MEV-08-008", "L_COLLATERAL"),
    ("MEV-08-009", "L_RATE"),
    ("MEV-08-010", "L_RATE"),
    ("MEV-08-011", "L_LOOP"),
    ("MEV-08-012", "L_LIQ"),
    ("MEV-08-013", "L_LIQ"),
    ("MEV-08-014", "L_LIQ"),
    ("MEV-08-015", "L_LIQ"),
    ("MEV-08-016", "L_LIQ"),
    ("MEV-08-017", "L_LIQ"),
    ("MEV-08-018", "L_AUCTION"),
    ("MEV-08-019", "L_AUCTION"),
    ("MEV-08-020", "L_AUCTION"),
    ("MEV-08-021", "L_AUCTION"),
    ("MEV-08-022", "L_COLLATERAL"),
    ("MEV-08-023", "L_COLLATERAL"),
    ("MEV-08-024", "L_LIQ"),
    ("MEV-08-025", "L_RATE"),
    ("MEV-09-001", "I_ROUTE"),
    ("MEV-09-002", "I_ROUTE"),
    ("MEV-09-003", "I_ROUTE"),
    ("MEV-09-004", "I_ROUTE"),
    ("MEV-09-005", "I_BATCH"),
    ("MEV-09-006", "I_BATCH"),
    ("MEV-09-007", "I_BATCH"),
    ("MEV-09-008", "I_BATCH"),
    ("MEV-09-009", "I_DUTCH"),
    ("MEV-09-010", "I_ORDERFLOW"),
    ("MEV-09-011", "I_ORDERFLOW"),
    ("MEV-09-012", "I_ROUTE"),
    ("MEV-09-013", "I_ROUTE"),
    ("MEV-09-014", "I_ROUTE"),
    ("MEV-09-015", "I_ROUTE"),
    ("MEV-09-016", "I_ROUTE"),
    ("MEV-09-017", "I_ORDERFLOW"),
    ("MEV-09-018", "I_ORDERFLOW"),
    ("MEV-09-019", "OBSERVE"),
    ("MEV-09-020", "OBSERVE"),
    ("MEV-10-001", "N_IDENTICAL"),
    ("MEV-10-002", "N_FLOOR"),
    ("MEV-10-003", "N_FLOOR"),
    ("MEV-10-004", "N_FLOOR"),
    ("MEV-10-005", "N_IDENTICAL"),
    ("MEV-10-006", "N_AMM"),
    ("MEV-10-007", "N_AMM"),
    ("MEV-10-008", "N_IDENTICAL"),
    ("MEV-10-009", "N_FLOOR"),
    ("MEV-10-010", "N_REDEEM"),
    ("MEV-10-011", "N_REDEEM"),
    ("MEV-10-012", "N_LIQ"),
    ("MEV-10-013", "N_FLOOR"),
    ("MEV-10-014", "N_IDENTICAL"),
    ("MEV-10-015", "N_IDENTICAL"),
    ("MEV-10-016", "N_IDENTICAL"),
    ("MEV-10-017", "N_REDEEM"),
    ("MEV-10-018", "N_REDEEM"),
    ("MEV-11-001", "M_COMPLETE"),
    ("MEV-11-002", "M_COMPLETE"),
    ("MEV-11-003", "M_CROSS"),
    ("MEV-11-004", "M_LOGIC"),
    ("MEV-11-005", "M_COMPLETE"),
    ("MEV-11-006", "M_LOGIC"),
    ("MEV-11-007", "M_LOGIC"),
    ("MEV-11-008", "M_LOGIC"),
    ("MEV-11-009", "OBSERVE"),
    ("MEV-11-010", "OBSERVE"),
    ("MEV-11-011", "OBSERVE"),
    ("MEV-11-012", "M_AMM"),
];

/// Workbook policy for a detector; `None` if unknown to the sheet.
pub fn detector_policy(detector_id: &str) -> Option<&'static DetectorPolicy> {
    DETECTOR_POLICIES
        .binary_search_by(|p| p.detector_id.cmp(detector_id))
        .ok()
        .map(|i| &DETECTOR_POLICIES[i])
}

/// Detector linked to a canonical strategy; `None` if the MEV_ID is unknown.
pub fn detector_of_strategy(mev_id: &str) -> Option<&'static str> {
    STRATEGY_DETECTOR
        .binary_search_by(|(id, _)| (*id).cmp(mev_id))
        .ok()
        .map(|i| STRATEGY_DETECTOR[i].1)
}

/// Policy of a canonical strategy via its detector link.
pub fn policy_for_strategy(mev_id: &str) -> Option<&'static DetectorPolicy> {
    detector_of_strategy(mev_id).and_then(detector_policy)
}

/// Universal `Do_Not_Do` guard — uniform across all 60 detectors.
pub static DO_NOT_RULES: [&str; 1] =
    ["Do not replace detector math with generic spot-price spread."];

/// Sheet 13 col `Do_Not_Do`: detector math must never be replaced by a
/// generic spot-price spread shortcut.
pub fn do_not_rules() -> &'static [&'static str] {
    &DO_NOT_RULES
}

/// Intersect per-strategy admissible bounds with the detector family
/// envelope. Unknown strategy or detector → `None` (fail-closed, same
/// doctrine as `strategy_dispatch_status`); empty intersection → `None`.
pub fn envelope_hop_bounds(mev_id: &str, strategy_bounds: Option<(u8, u8)>) -> Option<(u8, u8)> {
    let (smin, smax) = strategy_bounds?;
    let (dmin, dmax) = policy_for_strategy(mev_id)?.hop_bounds();
    let lo = smin.max(dmin);
    let hi = smax.min(dmax);
    (lo <= hi).then_some((lo, hi))
}

/// Per-GraphPolicy detector counts, DERIVED from the table (as_str order).
pub fn graph_policy_counts() -> &'static [(GraphPolicy, usize)] {
    static C: std::sync::OnceLock<Vec<(GraphPolicy, usize)>> = std::sync::OnceLock::new();
    C.get_or_init(|| {
        let mut v: Vec<(GraphPolicy, usize)> = Vec::new();
        for p in &DETECTOR_POLICIES {
            match v.iter_mut().find(|slot| slot.0 == p.graph_policy) {
                Some(slot) => slot.1 += 1,
                None => v.push((p.graph_policy, 1)),
            }
        }
        v.sort_unstable_by_key(|(g, _)| g.as_str());
        v
    })
}

/// Per-HotSeed detector counts, DERIVED from the table (as_str order).
pub fn hot_seed_counts() -> &'static [(HotSeed, usize)] {
    static C: std::sync::OnceLock<Vec<(HotSeed, usize)>> = std::sync::OnceLock::new();
    C.get_or_init(|| {
        let mut v: Vec<(HotSeed, usize)> = Vec::new();
        for p in &DETECTOR_POLICIES {
            match v.iter_mut().find(|slot| slot.0 == p.hot_seed) {
                Some(slot) => slot.1 += 1,
                None => v.push((p.hot_seed, 1)),
            }
        }
        v.sort_unstable_by_key(|(h, _)| h.as_str());
        v
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Differential fixture — generated from the SAME canonical sources by
    /// the SAME script.
    const FIXTURE: &str = include_str!("detector_policy.fixture.json");

    /// (detector, graph sentence, hop_use, seed sentence, strategy_count).
    fn fixture_detectors() -> Vec<(String, String, Vec<u8>, String, usize)> {
        let v: serde_json::Value = serde_json::from_str(FIXTURE).expect("fixture parses");
        v["detectors"]
            .as_array()
            .expect("detectors array")
            .iter()
            .map(|d| {
                (
                    d["d"].as_str().expect("d").to_string(),
                    d["gp"].as_str().expect("gp").to_string(),
                    d["hu"]
                        .as_array()
                        .expect("hu")
                        .iter()
                        .map(|h| h.as_u64().expect("u8") as u8)
                        .collect(),
                    d["hs"].as_str().expect("hs").to_string(),
                    d["sc"].as_u64().expect("sc") as usize,
                )
            })
            .collect()
    }

    /// (MEV_ID, detector, allowed_hops, status).
    fn fixture_strategies() -> Vec<(String, String, Vec<u8>, String)> {
        let v: serde_json::Value = serde_json::from_str(FIXTURE).expect("fixture parses");
        v["strategies"]
            .as_array()
            .expect("strategies array")
            .iter()
            .map(|r| {
                (
                    r["m"].as_str().expect("m").to_string(),
                    r["det"].as_str().expect("det").to_string(),
                    r["ah"]
                        .as_array()
                        .expect("ah")
                        .iter()
                        .map(|h| h.as_u64().expect("u8") as u8)
                        .collect(),
                    r["st"].as_str().expect("st").to_string(),
                )
            })
            .collect()
    }

    fn fixture_do_not() -> String {
        let v: serde_json::Value = serde_json::from_str(FIXTURE).expect("fixture parses");
        v["do_not"].as_str().expect("do_not").to_string()
    }

    /// Full table↔fixture differential: every detector resolves to the exact
    /// workbook policy sentence/hops/seed/count, and every strategy resolves
    /// to its linked detector.
    #[test]
    fn table_matches_workbook_fixture_exactly() {
        let fx = fixture_detectors();
        assert_eq!(fx.len(), 60);
        for (d, gp, hu, hs, sc) in &fx {
            let p = detector_policy(d).expect("detector resolves");
            assert_eq!(p.graph_policy.as_str(), gp.as_str(), "graph drift {}", d);
            assert_eq!(p.hop_use, hu.as_slice(), "hop_use drift {}", d);
            assert_eq!(p.hot_seed.as_str(), hs.as_str(), "seed drift {}", d);
            assert_eq!(p.strategy_count as usize, *sc, "count drift {}", d);
        }
        for (m, det, _, _) in fixture_strategies() {
            assert_eq!(
                detector_of_strategy(&m),
                Some(det.as_str()),
                "link drift {}",
                m
            );
        }
    }

    /// Per-detector strategy_count == real strategies linked in the fixture,
    /// summing 264 (workbook tripwire against silent link drift).
    #[test]
    fn strategy_counts_cover_all_264() {
        let fx = fixture_strategies();
        assert_eq!(fx.len(), 264);
        let mut per_det: Vec<(String, usize)> = Vec::new();
        for (_, det, _, _) in &fx {
            match per_det.iter_mut().find(|slot| slot.0 == *det) {
                Some(slot) => slot.1 += 1,
                None => per_det.push((det.clone(), 1)),
            }
        }
        for (d, _, _, _, sc) in fixture_detectors() {
            let real = per_det
                .iter()
                .find(|slot| slot.0 == d)
                .map(|slot| slot.1)
                .unwrap_or(0);
            assert_eq!(real, sc, "count vs real link mismatch {}", d);
        }
        assert_eq!(
            DETECTOR_POLICIES
                .iter()
                .map(|p| p.strategy_count as usize)
                .sum::<usize>(),
            264,
            "census must cover all 264 strategies"
        );
    }

    /// Graph/seed census: fixture-derived == table-derived, closed
    /// vocabularies (12 graph families, 5 seed modes over 60 detectors).
    #[test]
    fn policy_census_matches_workbook() {
        let fx = fixture_detectors();
        let mut gp_fx: Vec<(String, usize)> = Vec::new();
        let mut hs_fx: Vec<(String, usize)> = Vec::new();
        for (_, gp, _, hs, _) in &fx {
            match gp_fx.iter_mut().find(|slot| slot.0 == *gp) {
                Some(slot) => slot.1 += 1,
                None => gp_fx.push((gp.clone(), 1)),
            }
            match hs_fx.iter_mut().find(|slot| slot.0 == *hs) {
                Some(slot) => slot.1 += 1,
                None => hs_fx.push((hs.clone(), 1)),
            }
        }
        gp_fx.sort();
        hs_fx.sort();
        let gp_t = graph_policy_counts();
        let hs_t = hot_seed_counts();
        assert_eq!(gp_t.len(), 12, "graph family drift");
        assert_eq!(hs_t.len(), 5, "seed mode drift");
        assert_eq!(gp_t.iter().map(|(_, c)| c).sum::<usize>(), 60);
        assert_eq!(hs_t.iter().map(|(_, c)| c).sum::<usize>(), 60);
        for ((gv, gc), (ff, fc)) in gp_t.iter().zip(gp_fx.iter()) {
            assert_eq!(gv.as_str(), ff.as_str(), "graph census name");
            assert_eq!(*gc, *fc, "graph census count {}", ff);
        }
        for ((hv, hc), (ff, fc)) in hs_t.iter().zip(hs_fx.iter()) {
            assert_eq!(hv.as_str(), ff.as_str(), "seed census name");
            assert_eq!(*hc, *fc, "seed census count {}", ff);
        }
    }

    /// Family-envelope invariant: every strategy's Allowed_Hops stays inside
    /// its detector's Hop_Use (data-level pin; the generator re-asserts it
    /// pre-emission, `envelope_hop_bounds` enforces it at runtime).
    #[test]
    fn family_envelope_respected() {
        for (m, det, ah, _) in fixture_strategies() {
            let p = detector_policy(&det).expect("detector resolves");
            for h in ah {
                assert!(
                    p.allows_hop(h),
                    "{}: hop {} escapes family envelope of {}",
                    m,
                    h,
                    det
                );
            }
        }
    }

    /// OBSERVE coherence: the OBSERVE detector is graph OBSERVE_ONLY + seed
    /// telemetry-only, and its strategies are EXACTLY the 8 with Status
    /// OBSERVE_ONLY in the hop map (cross-invariant with
    /// strategy_dispatch_status's census).
    #[test]
    fn observe_detector_coherence() {
        let p = detector_policy("OBSERVE").expect("OBSERVE resolves");
        assert!(p.graph_policy.as_str().starts_with("OBSERVE_ONLY"));
        assert!(!p.hot_seed.may_seed());
        let fx = fixture_strategies();
        let st_obs: Vec<&str> = fx
            .iter()
            .filter(|(_, det, _, _)| det == "OBSERVE")
            .map(|(m, _, _, _)| m.as_str())
            .collect();
        let det_obs: Vec<&str> = fx
            .iter()
            .filter(|(_, _, _, st)| st == "OBSERVE_ONLY")
            .map(|(m, _, _, _)| m.as_str())
            .collect();
        assert_eq!(st_obs, det_obs, "OBSERVE detector != OBSERVE_ONLY status");
        assert_eq!(st_obs.len(), 8);
    }

    /// Do_Not_Do is uniform across the sheet and exposed verbatim.
    #[test]
    fn do_not_rules_uniform() {
        let fx = fixture_do_not();
        assert_eq!(do_not_rules(), &[fx.as_str()]);
        assert!(fx.contains("generic spot-price spread"));
    }

    /// (detector, surface, execution_class, required_data, criterion) — the
    /// DP-001/002 contract columns, fixture side.
    fn fixture_contracts() -> Vec<(String, String, String, String, String)> {
        let v: serde_json::Value = serde_json::from_str(FIXTURE).expect("fixture parses");
        v["detectors"]
            .as_array()
            .expect("detectors array")
            .iter()
            .map(|d| {
                (
                    d["d"].as_str().expect("d").to_string(),
                    d["es"].as_str().expect("es").to_string(),
                    d["ec"].as_str().expect("ec").to_string(),
                    d["rd"].as_str().expect("rd").to_string(),
                    d["edc"].as_str().expect("edc").to_string(),
                )
            })
            .collect()
    }

    /// (MEV_ID, Execution_Class) — strategy-side classes from the hop map.
    fn fixture_strat_classes() -> Vec<(String, String)> {
        let v: serde_json::Value = serde_json::from_str(FIXTURE).expect("fixture parses");
        v["strategies"]
            .as_array()
            .expect("strategies array")
            .iter()
            .map(|r| {
                (
                    r["m"].as_str().expect("m").to_string(),
                    r["ec"].as_str().expect("ec").to_string(),
                )
            })
            .collect()
    }

    /// DP-001/002: the four contract columns ride the table VERBATIM, 60/60
    /// non-empty, and Required_Data/Exact_Discovery_Criterion stay distinct
    /// per family (a duplicate is copy/paste drift).
    #[test]
    fn execution_contracts_match_workbook() {
        let fx = fixture_contracts();
        assert_eq!(fx.len(), 60);
        for (d, es, ec, rd, edc) in &fx {
            let p = detector_policy(d).expect("detector resolves");
            assert_eq!(
                p.example_surface.as_str(),
                es.as_str(),
                "surface drift {}",
                d
            );
            assert_eq!(p.execution_class, ec.as_str(), "class drift {}", d);
            assert_eq!(p.required_data, rd.as_str(), "required_data drift {}", d);
            assert_eq!(
                p.exact_discovery_criterion,
                edc.as_str(),
                "criterion drift {}",
                d
            );
            assert!(!rd.trim().is_empty(), "empty required_data {}", d);
            assert!(!edc.trim().is_empty(), "empty criterion {}", d);
        }
        let mut rd: Vec<&str> = DETECTOR_POLICIES.iter().map(|p| p.required_data).collect();
        let mut edc: Vec<&str> = DETECTOR_POLICIES
            .iter()
            .map(|p| p.exact_discovery_criterion)
            .collect();
        rd.sort_unstable();
        edc.sort_unstable();
        rd.dedup();
        edc.dedup();
        assert_eq!(rd.len(), 60, "required_data duplicates");
        assert_eq!(edc.len(), 60, "criterion duplicates");
    }

    /// DP-002: Example_Surface is a closed 10-token vocabulary over the 60
    /// detectors — the RequiredDataGate's data-domain key.
    #[test]
    fn surface_vocabulary_closed() {
        let mut surfaces: Vec<&str> = DETECTOR_POLICIES
            .iter()
            .map(|p| p.example_surface.as_str())
            .collect();
        surfaces.sort_unstable();
        surfaces.dedup();
        assert_eq!(surfaces.len(), 10, "surface vocabulary drift");
        let expected = [
            "CEX_DEX",
            "CROSS_CHAIN",
            "DERIVATIVES",
            "DEX_AMM",
            "DEX_STATE",
            "INTENT_AUCTION",
            "LENDING",
            "NFT",
            "PARITY_REDEMPTION",
            "PREDICTION",
        ];
        assert_eq!(surfaces, expected);
        // Same census fixture-side (workbook tripwire).
        let fx = fixture_contracts();
        let mut fx_surfaces: Vec<&str> = fx.iter().map(|(_, es, _, _, _)| es.as_str()).collect();
        fx_surfaces.sort_unstable();
        fx_surfaces.dedup();
        assert_eq!(fx_surfaces.len(), 10);
    }

    /// DP-001: Execution_Class is family-uniform — each of the 264
    /// strategies carries exactly its detector's class, and both sheets
    /// share the same closed 29-class vocabulary.
    #[test]
    fn execution_class_family_uniform() {
        let fx = fixture_strat_classes();
        assert_eq!(fx.len(), 264);
        let mut classes: Vec<&str> = Vec::new();
        for (m, ec) in &fx {
            let p = policy_for_strategy(m).expect("strategy resolves");
            assert_eq!(
                p.execution_class,
                ec.as_str(),
                "class not family-uniform {}",
                m
            );
            if !classes.contains(&p.execution_class) {
                classes.push(p.execution_class);
            }
        }
        classes.sort_unstable();
        classes.dedup();
        assert_eq!(classes.len(), 29, "closed vocabulary drift");
        let mut table_classes: Vec<&str> = DETECTOR_POLICIES
            .iter()
            .map(|p| p.execution_class)
            .collect();
        table_classes.sort_unstable();
        table_classes.dedup();
        assert_eq!(classes, table_classes, "sheet13 vocab != sheet11 vocab");
    }

    /// Binary-search preconditions: both static tables sorted and unique.
    #[test]
    fn tables_sorted_unique() {
        let dets: Vec<&str> = DETECTOR_POLICIES.iter().map(|p| p.detector_id).collect();
        let mut sorted = dets.clone();
        sorted.sort_unstable();
        assert_eq!(dets, sorted);
        let ids: Vec<&str> = STRATEGY_DETECTOR.iter().map(|(id, _)| *id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
        assert_eq!(
            ids.iter().collect::<std::collections::BTreeSet<_>>().len(),
            ids.len()
        );
    }

    /// Envelope intersection semantics: identity while the strategy stays
    /// inside its family (the canonical case — 0 violations), clamping and
    /// empty-intersection behavior on synthetic out-of-family bounds, and
    /// fail-closed on unknown strategy.
    #[test]
    fn envelope_intersection_semantics() {
        // Canonical: MEV-01-015 (R_CLOSED_CYCLE, allowed {2}) inside 2..=7.
        assert_eq!(
            envelope_hop_bounds("MEV-01-015", Some((2, 2))),
            Some((2, 2))
        );
        // Clamp: synthetic strategy bounds escaping the family get cut back.
        assert_eq!(
            envelope_hop_bounds("MEV-01-015", Some((2, 8))),
            Some((2, 7))
        );
        // Bounded family from the fixture (first detector with max hop 4).
        let det_fx = fixture_detectors();
        let bounded = det_fx
            .iter()
            .find(|(_, _, hu, _, _)| *hu.last().expect("hu") == 4)
            .expect("bounded family exists");
        let strat_fx = fixture_strategies();
        let member = strat_fx
            .iter()
            .find(|(_, det, _, _)| det == &bounded.0)
            .expect("member strategy exists");
        // Identity inside the envelope…
        let (lo, hi) = member
            .2
            .first()
            .copied()
            .zip(member.2.last().copied())
            .expect("ah");
        assert_eq!(
            envelope_hop_bounds(&member.0, Some((lo, hi))),
            Some((lo, hi)),
            "identity inside family {}",
            member.0
        );
        // …empty intersection beyond it → None.
        let beyond = bounded.2.last().expect("hu") + 1;
        assert_eq!(
            envelope_hop_bounds(&member.0, Some((beyond, beyond + 2))),
            None,
            "empty intersection must forbid expansion {}",
            member.0
        );
        // Fail-closed on unknown strategy / missing bounds.
        assert_eq!(envelope_hop_bounds("MEV-99-999", Some((2, 3))), None);
        assert_eq!(envelope_hop_bounds("MEV-01-015", None), None);
    }
}
