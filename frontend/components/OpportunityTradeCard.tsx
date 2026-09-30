/**
 * OpportunityTradeCard — enriched trading card for a detected resolution route.
 *
 * Revives the "card v2" dashboard pattern (commit e9bcadd) and extends it per
 * the operator's execution spec:
 *
 *   1. STEP LADDER — the capital path rendered as a vertical ledger:
 *        Flash loan in → buy A → buy B → buy C → repay → fees → gas → net.
 *      Each step shows its running USD total so the operator reads, at a
 *      glance, where value is gained or lost at every hop. All figures come
 *      from the live snapshot (gross spread + simulated cost breakdown +
 *      simulated net) — never fabricated (RULE 00); unknown cells show "—".
 *
 *   2. APPLIED STRATEGY CONFIG — the /strategies-applied gate the route was
 *      sized against (min net USD, min ROI %, binding floor, suggested
 *      borrow), read from simulated_target. Honest "—" when not run.
 *
 *   3. DETECTION TIME / AGE / VIGENCY — detected_at timestamp, live age in
 *      seconds, and a vigency pill (VIGENTE while the snapshot is fresh,
 *      STALE past the freshness window). The card updates in place (stable
 *      React key), never duplicates, and disappears when the route drops.
 *
 *   4. EXECUTE (shadow) — POST /api/v1/opportunities/:id/simulate against the
 *      sim-ctl + Anvil fork. Read-only, capital = 0; returns pass/fail +
 *      gas estimate + trace id as verifiable evidence. Surfaces the result
 *      inline; never promises live execution it cannot deliver.
 *
 *   5. ⓘ INSPECT — the previous full-width "Inspect details" button is now a
 *      discreet circled-i affordance in the card's top-right corner.
 *
 * R8 fail-honest throughout: null → "—", never 0 dressed as a computed value.
 */
"use client";

import React, { useState } from "react";
// MEM-RENDER-01: framer-motion removed from the live card grid. 200 motion.div
// nodes churning at prod feed rates (up to ~2 mounts/s) retained animation
// state/drivers in the renderer until the tab reached multi-GB heaps. The enter
// animation is now CSS-only (.arbx-card-enter in globals.css) — zero JS state.
// Same discipline as the memory-budgeted /opportunities/exchange page.
import {
  AlertTriangle,
  ArrowDownRight,
  ArrowUpRight,
  CheckCircle2,
  Clock,
  Info,
  Loader2,
  MinusCircle,
  Play,
  TrendingUp,
  XCircle,
} from "lucide-react";
import { toast } from "sonner";

import { TokenChip } from "@/components/TokenChip";
import { ChainBadge } from "@/components/ChainBadge";
import { StrategyBadge } from "@/components/StrategyBadge";
import { QuarantineStrip } from "@/components/QuarantineStrip";
import { OpportunitySummaryGrid } from "@/components/opportunities/OpportunitySummaryGrid";
import { StatusPill } from "@/components/StatusPill";
import { Sparkline } from "@/components/cex/Sparkline";
import { FreshnessBadge, freshnessLevel } from "@/components/cex/FreshnessBadge";
import { flashClass, useValueFlash } from "@/components/cex/flash";
import { useRouteSeries } from "@/components/cex/route-series";
import { formatAgo } from "@/components/OpportunityTicker";
import {
  formatPctOrDash,
  formatProfitUSD,
  formatSubCentUsd,
  formatVigency,
  shortAddr,
  SUB_CENT_USD,
} from "@/lib/format";
import {
  deriveLegs,
  deriveLegLedger,
  routeTokenDecimals,
  SYNTHETIC_LEGACY_VIEW_LABEL,
  type LegLedgerEntry,
  type OmniOpportunity,
  type RouteLeg,
} from "@/lib/store/types";
// CARDS-NOTIONAL-01 — the SSOT for "which closed arithmetic may this card
// paint". Shared with the high-value notifier so the two surfaces cannot
// disagree about what a row's numbers mean.
import { buildLedger, LEDGER_COST_ROW_LABELS } from "@/lib/opportunity-ledger";
// ECON-DECLARE-01 — the notional-basis declaration the producer writes. The
// ladder band states the DECLARED notional instead of a blanket "sin notional".
import { BASIS_LABEL, formatWeiNotional } from "@/lib/opportunity-declaration";

// ─── Tone → token-based class map ────────────────────────────────────────────
const TONE_CLASS: Record<string, string> = {
  positive: "text-success",
  negative: "text-destructive",
  zero: "text-muted-foreground",
  neutral: "text-muted-foreground",
  pending: "text-muted-foreground/60 italic",
};

/**
 * Compact USD for ledger cells (`$12.5k`, `$1.8M`, `$0.0123`).
 *
 * CARDS-MAGNITUDE-01 (2026-09-26): the scale ladder used to stop at `M`, so
 * any value at or above 1e9 rendered an unbounded mantissa (`(v/1e6).toFixed(2)`
 * + "M"). PROVEN on the live feed: the route sized against a 6-decimal opening
 * token carried `simulated_amount_in_usd = 999935091316.8`, and the card painted
 * `Repay (principal + TLS fee) $1000835.03M` and `Total cost $9299.40M` — the
 * operator's "suspicious magnitude". A `$NNNNNN.M` string is never a real
 * magnitude, so the ladder is now closed at every decade (k → M → B → T) and
 * beyond T the value is rendered in engineering notation rather than as a
 * wider lie. R8: non-finite (`Infinity`/`NaN`, e.g. the inverse-sizing kernel's
 * `required = Infinity` that JSON turns into `null`) stays an honest "—".
 *
 * The sign is emitted BEFORE the `$` (matching §36's `usd()` and the exchange
 * card's `usdCost`) so one value never renders two ways on the same card
 * (was: grid `-$0.00` vs ledger `$-0.0000`).
 */
function usd(value: number | null | undefined, digits = 2): string {
  if (value == null || !Number.isFinite(value)) return "—";
  const abs = Math.abs(value);
  const sign = value < 0 ? "-" : "";
  const body = (n: number, d: number) => `$${n.toFixed(d)}`;
  // Beyond T the suffix ladder would need new letters; engineering notation is
  // the honest rendering instead of inventing a unit.
  if (abs >= 1e15) return `${sign}$${abs.toExponential(2).replace("e+", "e")}`;
  if (abs >= 1e12) return `${sign}${body(abs / 1e12, 2)}T`;
  if (abs >= 1e9) return `${sign}${body(abs / 1e9, 2)}B`;
  if (abs >= 1e6) return `${sign}${body(abs / 1e6, 2)}M`;
  if (abs >= 1e3) return `${sign}${body(abs / 1e3, 1)}k`;
  if (abs >= 1) return `${sign}${body(abs, digits)}`;
  // CARDS-FALSEZERO-01: a REAL zero keeps the ladder's 4-decimal convention
  // (`$0.0000` = computed and exactly zero, R8), while a NONZERO figure below the
  // cell's resolution must not collapse onto it. Measured on the live feed: 26 of
  // 323 unique rows carried `net_expected_profit_usd` ≈ -1.2e-5 and every cell
  // that showed it painted `-$0.0000` — a computed loss displayed as zero.
  if (abs === 0) return `${sign}${body(abs, 4)}`;
  if (abs < SUB_CENT_USD) return `${sign}$${formatSubCentUsd(abs)}`;
  return `${sign}${body(abs, 4)}`;
}

/** Freshness window (matches the table's 12s staleness heuristic). */
const STALE_SECS = 12;

/**
 * HOPS-LEDGER-04 (PER-HOP, card half) — exact wei → token units, BigInt-only.
 *
 * The sizing kernel emits per-leg amounts as EXACT wei strings; displaying them
 * requires the token's decimals, which is a deployment fact the wire may omit
 * for unregistered tokens. Discipline:
 *   · decimals unknown/invalid ⇒ `null`. The caller then prints the raw wei
 *     string with a `·wei` mark — never a guessed 18-decimals unit (R8).
 *   · fraction digits are TRUNCATED (never rounded up): a rendering must not
 *     invent a wei that the kernel did not produce.
 *   · integers longer than 12 digits collapse to `d.dde±k` so a 1e18-scale
 *     phantom cannot blow the card layout.
 * `numeric` is the float twin used ONLY for the live-price Δ marking; the text
 * stays the exact-BigInt rendering.
 */
function weiUnits(
  wei: string,
  decimals: number | undefined,
): { text: string; numeric: number | null } | null {
  if (typeof decimals !== "number" || !Number.isInteger(decimals) || decimals < 0 || decimals > 255) {
    return null;
  }
  if (typeof wei !== "string" || wei.trim() === "") return null;
  let value: bigint;
  try {
    value = BigInt(wei.trim());
  } catch {
    return null;
  }
  const negative = value < 0n;
  const abs = negative ? -value : value;
  const base = 10n ** BigInt(decimals);
  const whole = abs / base;
  const frac = abs % base;
  const wholeRaw = whole.toString();
  const compact = wholeRaw.length > 12;
  const wholeText = compact
    ? `${wholeRaw.slice(0, 1)}.${wholeRaw.slice(1, 3)}e${wholeRaw.length - 1}`
    : whole.toLocaleString("en-US");
  const fracText =
    !compact && frac > 0n
      ? frac.toString().padStart(decimals, "0").slice(0, 6).replace(/0+$/, "")
      : "";
  const sign = negative ? "-" : "";
  const numeric = Number(
    `${sign}${wholeRaw}${frac > 0n ? `.${frac.toString().padStart(decimals, "0")}` : ""}`,
  );
  return {
    text: `${sign}${wholeText}${fracText ? `.${fracText}` : ""}`,
    numeric: Number.isFinite(numeric) ? numeric : null,
  };
}

interface SimEvidence {
  passed: boolean | null;
  gasEstimateWei: string | null;
  traceId: string | null;
  failReason: string | null;
}

export interface OpportunityTradeCardProps {
  opp: OmniOpportunity;
  /** Live clock (ms epoch) from the parent ticker — drives age + vigency. */
  now: number;
  /** SSR/CSR gate so time-dependent text only renders client-side (R1). */
  isMounted: boolean;
  /** Whether a shadow-sim is currently running for this card. */
  simLoading: boolean;
  /** Effective execution terminus (paper/live) — display-only read from /killswitch state. */
  modeLabel?: "paper" | "live";
  /** Trigger the shadow simulation (wired to POST .../simulate). */
  onExecute: (opportunityId: string) => Promise<void> | void;
  /** Open the full detail dialog (the old "Inspect details"). */
  onInspect: (opp: OmniOpportunity) => void;
}

function OpportunityTradeCardImpl({
  opp,
  now,
  isMounted,
  simLoading,
  modeLabel = "paper",
  onExecute,
  onInspect,
}: OpportunityTradeCardProps) {
  const [evidence, setEvidence] = useState<SimEvidence | null>(null);

  // ── Detection time / age / vigency ─────────────────────────────────────────
  // FE-0029 (§28): detected_at is null on malformed payloads — the mapper no
  // longer fabricates now(). An undated row claims neither freshness nor
  // staleness (isStale null = "sin fecha").
  // CARDS-DEDUP-HOPS: dual vigency — age since the FIRST detection (1ª) and
  // since the LAST ratification (✓), in discrete anti-saturation units.
  // Staleness now keys on the LAST ratification: a route re-detected seconds
  // ago stays VIGENTE even if first detected hours ago.
  const detectedTime =
    opp.detected_at == null ? NaN : new Date(opp.detected_at).getTime();
  const vigency = formatVigency(
    opp.first_seen_at ?? null,
    opp.last_seen_at ?? null,
    opp.detected_at ?? null,
    now,
  );
  const lastSeenTime =
    opp.last_seen_at == null
      ? detectedTime
      : new Date(opp.last_seen_at).getTime();
  const lastAgeSecs =
    opp.detected_at == null && opp.last_seen_at == null
      ? null
      : isMounted
        ? Math.max(0, Math.floor((now - lastSeenTime) / 1000))
        : 0;
  const isStale: boolean | null =
    lastAgeSecs == null ? null : lastAgeSecs > STALE_SECS;

  // ── Net priority: CARDS-NOTIONAL-01 — ONE ladder, ONE notional ─────────────
  // The card used to pick `net` by FIELD NAME (canonical spine net preferred)
  // while painting `Total cost` from the SIM's breakdown and `Flash loan in`
  // from the SIM's notional — three producers, up to three sizes, one ladder.
  // `buildLedger` (frontend/lib/opportunity-ledger.ts) is the SSOT that decides
  // which CLOSED arithmetic this row may paint, and it is the SAME function the
  // high-value notifier consults, so the card and the toast cannot disagree.
  const ledger = buildLedger(opp);
  const quietReason = ledger.reason;
  const grossUsd = ledger.gross_usd;
  const netSource: "canonical" | "simulated" | "none" = ledger.basis;
  /**
   * CARDS-PRECEDENCE-03 (2026-09-27) — the EXECUTIVE net is the wire's net.
   *
   * `ledger.net_usd` is the net of a CLOSED ladder, which is a strictly stronger
   * requirement than "a net was computed": `buildLedger` refuses every basis
   * when the row carries no canonical gross, or when its SIM triple exists but
   * is not payable out of its own principal. Measured on the live feed
   * (`GET /api/opportunities/live`, 323 unique rows over 45 polls,
   * 2026-09-27T02:2xZ): 26 rows carried `net_expected_profit_usd` and the
   * headline painted `—`, while the SAME figure was painted two rows below by
   * the summary grid's `Net` cell — which is exactly the rule this card's own
   * header comment documents for the headline ("the CARD HEADLINE already
   * renders `net_expected_profit_usd ?? simulated_net_profit_usd`",
   * OpportunitySummaryGrid.tsx).
   *
   * R8 honesty is preserved, not weakened: the fallback never invents a figure —
   * it only ever shows a field THIS row carries, in the wire's own precedence
   * (canonical spine net, else the TS forward-sim net), with the `~`/SIM mark
   * naming the producer and `quietReason` in the `title` stating why the ladder
   * itself did not close. Nothing is fabricated and nothing is hidden.
   */
  const netFromWire: { usd: number; source: "canonical" | "simulated" } | null =
    opp.net_expected_profit_usd != null
      ? { usd: opp.net_expected_profit_usd, source: "canonical" }
      : opp.simulated_net_profit_usd != null
        ? { usd: opp.simulated_net_profit_usd, source: "simulated" }
        : null;
  /** The net the operator is shown — the closed ladder's, else the wire's own. */
  const headlineNetUsd = ledger.net_usd ?? netFromWire?.usd ?? null;
  const headlineNetSource: "canonical" | "simulated" | "none" =
    ledger.net_usd != null ? ledger.basis : (netFromWire?.source ?? "none");
  const headlineNetTitle =
    netSource !== "none"
      ? netSource === "canonical"
        ? "Canonical spine net = gross − all costs"
        : "TS forward-sim net (canonical pending)"
      : headlineNetSource === "canonical"
        ? `net_expected_profit_usd — net del wire (spine canónico); el ladder no se cierra: ${quietReason ?? "sin aritmética cerrada"}`
        : headlineNetSource === "simulated"
          ? `simulated_net_profit_usd — net computado por el forward-sim; el ladder no se cierra: ${quietReason ?? "sin aritmética cerrada"}`
          : "Not yet computed (R8: '—')";
  const net = formatProfitUSD(headlineNetUsd);
  const gross = formatProfitUSD(grossUsd);

  const roi =
    ledger.basis === "canonical"
      ? opp.economics?.roi_pct ?? opp.roi_pct
      : ledger.basis === "simulated"
        ? opp.simulated_roi_pct
        : null;
  const roiTone: "pos" | "neg" | "muted" =
    roi == null ? "muted" : roi > 0 ? "pos" : roi < 0 ? "neg" : "muted";

  // ── Step-ladder ledger: borrow → buy A → buy B → buy C → repay → net ───────
  // Every row below belongs to `ledger`'s basis or it is NOT rendered. The
  // principal rows exist only on the `"simulated"` basis — the only basis whose
  // notional is published on the wire; on the `"canonical"` basis they go quiet
  // with `quietReason` in their `title` rather than borrowing the SIM's number.
  const capitalInUsd = ledger.principal_usd;
  // OPERATOR ORDER 2026-09-27: "QUITA EL MALDITO RENDER QUE ESCONDE LOS NUMEROS."
  // This gate handed the 9 cost components to the render ONLY on the "simulated"
  // basis, so every component row painted "—" even with
  // `simulated_cost_breakdown` present on the wire. The breakdown is used
  // whenever the wire carries it; each row declares its own basis in its title.
  const cb = opp.simulated_cost_breakdown ?? null;

  // ── WO-PRICE-EXCHANGE-V1 (FE) — CEX-premium treatment on the EXISTING card ──
  // Flash memory in refs (useValueFlash): a WS/polling batch that changes
  // Gross/Net animates ONLY this card's value cells; identical values across
  // the 1s age-ticker re-renders animate nothing.
  const grossFlash = useValueFlash(grossUsd);
  // CARDS-PRECEDENCE-03: the flash tracks the figure actually PAINTED (the
  // headline net), so a wire net arriving without a closed ladder still
  // animates the cell that shows it.
  const netFlash = useValueFlash(headlineNetUsd);
  // Sparkline feed: route_key = dex_a + pair, values from the stream already
  // flowing into this card. When the backend price_history mirror lands, the
  // same Sparkline consumes that source via its `points` prop.
  const routeTrendKey = `${opp.dex_a ?? "?"}·${opp.pair_symbol ?? `${opp.token_in}→${opp.token_out}`}`;
  // Source + freshness corner badge: token_in symbol + age with a
  // StatusPill-style dot. R1: age text/level only after mount.
  const sourceSymbol =
    opp.token_in_info?.symbol ?? opp.token_in_info?.registry_symbol ?? shortAddr(opp.token_in);
  const agoText = isMounted && opp.detected_at != null ? formatAgo(opp.detected_at) : null;
  const sourceFreshness = freshnessLevel(isMounted ? lastAgeSecs : null);

  // Repay = capital in + flash-convergence fee (TLS principal + fee).
  const flashFee =
    ledger.basis === "canonical"
      ? opp.economics?.flash_fee_usd ?? null
      : cb?.flashloan_fee_usd ?? null;
  const repayUsd: number | null =
    capitalInUsd != null && flashFee != null ? capitalInUsd + flashFee : null;

  // REAL-LIVE-CARDS-SSOT-01: a configured ROI floor is never promoted to
  // observed market economics. Keep only simulations based on an observed gross.
  const tgt =
    opp.simulated_target?.estimation_basis === "observed-gross"
      ? opp.simulated_target
      : null;
  const canonicalTarget =
    opp.economics?.computation_status === "computed" &&
    opp.economics.target_net_usd != null
      ? opp.economics
      : null;
  const targetVerdict: { label: string; tone: "pass" | "fail" | "na" } = (() => {
    if (canonicalTarget?.meets_target != null)
      return canonicalTarget.meets_target
        ? { label: "PASS", tone: "pass" }
        : { label: "FAIL", tone: "fail" };
    if (tgt == null) return { label: "no target", tone: "na" };
    const infeasible =
      tgt.binding_floor === "roi-unreachable" ||
      tgt.binding_floor === "net-per-usd-nonpositive";
    return tgt.meets_target_at_cap && !infeasible
      ? { label: "PASS", tone: "pass" }
      : { label: "FAIL", tone: "fail" };
  })();

  // TOTAL COST — the ledger's own Σ, never a hand-summed subset.
  //   · `"simulated"`: Σ of the SIM breakdown INCLUDING `copied_buffer_usd`.
  //     The old code summed 8 of 9 components and dropped the copied buffer —
  //     at the live `p_copied_max = 0.5` that is half the gross, so the printed
  //     "Total cost" was ≈45% of the truth and `net == gross − total_cost`
  //     could not hold even inside one producer's numbers.
  //   · `"canonical"`: `gross − net`, the wire's OWN documented relation
  //     (`net_expected_profit_usd` is "gross - costs" on that same row).
  const costRows = ledger.cost_rows;
  const knownCostSum =
    ledger.total_cost_usd ??
    (ledger.basis === "canonical" ? opp.economics?.total_cost_usd ?? null : null);
  /** Label → value for the rendered cost rows (values exist only on the SIM basis). */
  const costValueByLabel = new Map(costRows.map((r) => [r.label, r.value]));
  // ALWAYS-COMPUTE (2026-09-27): on the CANONICAL basis the searcher's own
  // decomposition (the `economics` object) fills the component cells. This is
  // the SAME arithmetic the ladder is painting on that basis — one producer,
  // one size, and the object closes (total == Σ components, net == gross −
  // total), so it cannot mix notionals (CARDS-NOTIONAL-01 doctrine intact).
  // Components the kernel genuinely does not price (dex fees / slippage are
  // inside amount_out; capital/failure/copied are SIM-model components) stay
  // quiet dashes with their reason. Pure display — no React-side math.
  if (ledger.basis === "canonical" && opp.economics && opp.economics.computation_status !== "error") {
    const e = opp.economics;
    const fill: Array<[string, number | null]> = [
      ["Gas", e.gas_usd],
      ["LP fees", e.dex_fees_usd],
      ["Decoherence (slippage)", e.slippage_usd],
      ["TLS fee (flash)", e.flash_fee_usd],
      ["Relay fee", e.bribe_usd],
      ["Ops overhead", e.other_costs_usd],
    ];
    for (const [label, v] of fill) {
      if (v != null) costValueByLabel.set(label, v);
    }
  }
  /** True when a wire figure was suppressed because it belongs to another notional. */
  const ledgerIsQuiet = ledger.quiet;

  // ── HOPS-CARD-03: ladder legs from the PERSISTED topology (deriveLegs) ─────
  // The old hardcoded "Buy A / Buy B" 2-row ladder hid every N-leg route — a
  // 3-hop triangular rendered as 2 generic rows. Legs come from
  // route_metadata when present, else the §29 synthetic fallback (marked).
  // Symbols resolve with the exchange card's priority (F2/§11 RC1):
  // pair-info (symbol → registry_symbol, matching the card's tokenSymbol
  // chain) → server-hydrated leg_symbols (lowercased key) → shortAddr.
  const legs = deriveLegs(opp);
  const legSym = (addr: string): string => {
    const lc = addr.toLowerCase();
    if (lc === opp.token_in.toLowerCase()) {
      const s = opp.token_in_info?.symbol ?? opp.token_in_info?.registry_symbol;
      if (s) return s;
    }
    if (lc === opp.token_out.toLowerCase()) {
      const s = opp.token_out_info?.symbol ?? opp.token_out_info?.registry_symbol;
      if (s) return s;
    }
    return opp.leg_symbols?.[lc] ?? shortAddr(addr);
  };
  const hasSyntheticLegs = legs.some((l) => l.synthetic === true);

  // ── HOPS-LEDGER-04 (PER-HOP): exact per-leg ledger, rendered INSIDE the ────
  // ladder row that already owns that hop (one place per hop — no duplicate
  // block). `deriveLegLedger` is all-or-nothing and never touches §29 synthetic
  // legs, so its length always aligns with `legs` when non-null.
  const legLedger = deriveLegLedger(opp);
  /** Live PriceBus USD for a token address; null when the symbol has no price. */
  const symUsd = (addr: string): number | null => {
    const px = opp.token_prices_usd?.[legSym(addr).toUpperCase()];
    return px != null && Number.isFinite(px) && px > 0 ? px : null;
  };
  /**
   * Value cell for one hop: exact in→out wei (unit-scaled when the token's
   * decimals are known, verbatim `·wei` when they are not), plus the price-
   * marked leg Δ and — closing leg only — the exact whole-cycle delta.
   * Every figure is conditional on real inputs: a missing price or decimals
   * removes that figure instead of rendering a fabricated 0 (R8).
   */
  const hopAmountNode = (leg: RouteLeg, entry: LegLedgerEntry): React.ReactNode => {
    const rm = opp.route_metadata;
    const inView = rm ? weiUnits(entry.amount_in_wei, routeTokenDecimals(rm, leg.token_in)) : null;
    const outView = rm ? weiUnits(entry.amount_out_wei, routeTokenDecimals(rm, leg.token_out)) : null;
    const symIn = legSym(leg.token_in);
    const symOut = legSym(leg.token_out);
    const pxIn = symUsd(leg.token_in);
    const pxOut = symUsd(leg.token_out);
    const inUsd = inView?.numeric != null && pxIn != null ? inView.numeric * pxIn : null;
    const outUsd = outView?.numeric != null && pxOut != null ? outView.numeric * pxOut : null;
    const legDeltaUsd = inUsd != null && outUsd != null ? outUsd - inUsd : null;
    // Closing leg: delta of the whole cycle in the opening token's wei. The
    // closing leg's OUT token IS the opening token, so its decimals denominate.
    const cycleView =
      rm && entry.cycle_delta_wei != null
        ? weiUnits(entry.cycle_delta_wei, routeTokenDecimals(rm, leg.token_out))
        : null;
    const cycleText =
      cycleView != null && entry.cycle_delta_wei != null
        ? cycleView.text
        : entry.cycle_delta_wei != null
          ? entry.cycle_delta_wei
          : null;
    const cycleNegative = (entry.cycle_delta_wei ?? "").trim().startsWith("-");
    return (
      <span
        // CARDS-LAYOUT-01: this cell renders on its OWN full-width line
        // (LedgerRow), so it can never collide with the hop label. `w-full` +
        // `min-w-0` keep it inside the block; each of the three figures
        // truncates rather than painting outside its own box.
        className="flex w-full min-w-0 flex-col items-end leading-tight text-right text-[10px] font-mono whitespace-nowrap"
        title={
          "Montos exactos en wei del kernel de sizing (ledger on-chain, no estimación). " +
          "Δ USD valorado con los precios PriceBus en vivo — no es el neto SIM."
        }
      >
        <span className="max-w-full truncate">
          {inView != null ? `${inView.text} ${symIn}` : `${entry.amount_in_wei}·wei`}
          {" → "}
          {outView != null ? `${outView.text} ${symOut}` : `${entry.amount_out_wei}·wei`}
        </span>
        {legDeltaUsd != null && (
          <span className={`max-w-full truncate ${legDeltaUsd >= 0 ? "text-success" : "text-destructive"}`}>
            Δ {usd(legDeltaUsd)}
          </span>
        )}
        {cycleText != null && (
          <span className={`max-w-full truncate ${cycleNegative ? "text-destructive" : "text-success"}`}>
            ciclo {cycleText} {cycleView != null ? symOut : "wei"}
          </span>
        )}
      </span>
    );
  };

  // ── CARDS-TOKENPATH-01: full participating-token sequence (2..7 tokens) ────
  // deriveLegs already carries the persisted topology; the ordered token path
  // is first-in + every leg out. A closed cycle repeats the opener as the final
  // out — drop it so N hops render exactly N tokens (the operator's chip-count
  // contract). Fail-honest fallback when no topology persisted: in→out pair.
  // AUDIT-CARDS-MINOR (§3): that fallback pair used to render EXACTLY like a
  // topology-backed path — the ladder marks its §29 synthetic legs but the
  // chip row did not. tokenPathFromTopology is null when (and only when) the
  // chips did NOT come from persisted topology (synthetic legs included), so
  // the row now carries the same compact §29 mark as the ladder band.
  const tokenPathFromTopology: string[] | null = (() => {
    if (legs.length > 0) {
      const seq = [legs[0]!.token_in, ...legs.map((l) => l.token_out)];
      if (
        seq.length > 2 &&
        seq[seq.length - 1]!.toLowerCase() === seq[0]!.toLowerCase()
      ) {
        seq.pop();
      }
      if (seq.every((a) => a.length > 0)) return seq;
    }
    return null;
  })();
  const tokenPathAddrs: string[] =
    tokenPathFromTopology ?? [opp.token_in, opp.token_out];
  const tokenPathIsFallback: boolean =
    tokenPathFromTopology == null || hasSyntheticLegs;

  const handleExecute = async (e: React.MouseEvent) => {
    e.stopPropagation();
    setEvidence(null);
    try {
      await onExecute(opp.id);
      // The parent surfaces the toast; we optimistically mark a ran state.
      setEvidence({ passed: null, gasEstimateWei: null, traceId: opp.trace_id, failReason: null });
    } catch (err) {
      setEvidence({
        passed: false,
        gasEstimateWei: null,
        traceId: opp.trace_id,
        failReason: (err as Error).message,
      });
    }
  };

  return (
    <div
      // FE-0051 (§76): the logical opportunity id rides the DOM root so the
      // same entity is traceable across views (dialog/exchange/paper rows
      // carry the same attribute; repo precedent: opp-row-${id} testid).
      data-opp-id={opp.id}
      className="arbx-card-enter relative bg-card text-card-foreground border border-border rounded-2xl p-4 shadow-lg hover:shadow-xl hover:border-primary/40 transition-all overflow-hidden"
    >
      {/* ⓘ discreet Inspect affordance — top-right corner */}
      <button
        type="button"
        aria-label="Inspect details"
        title="Inspect details"
        onClick={(e) => {
          e.stopPropagation();
          onInspect(opp);
        }}
        className="absolute right-3 top-3 z-10 rounded-full p-1 text-muted-foreground/70 hover:text-foreground hover:bg-muted/60 border border-transparent hover:border-border transition-colors"
      >
        <Info size={15} />
      </button>

      {/* FE-0031 (§30): semantic violations render a QUARANTINED strip. */}
      <QuarantineStrip violations={opp.semantic_violations} />

      {/* ── HEADER: chain · strategy · status · base token  |  ROI% ── */}
      <div className="flex items-start justify-between gap-2 mb-2.5 pr-7">
        <div className="flex items-center gap-1.5 flex-wrap min-w-0">
          <ChainBadge chain_id={opp.chain_id} />
          <StrategyBadge strategy_kind={opp.strategy_kind} />
          <StatusPill status={opp.status} rejection_reason={opp.rejection_reason} />
          {/* ALWAYS-COMPUTE (2026-09-27): the honest computation-status chip.
              Pure display of the wire's `economics.computation_status` —
              "computed" = the full arithmetic exists (PASS or FAIL),
              "partial" = some real figures + reasons,
              "error" = no quote existed (title carries the reason).
              COMPUTED ≠ PROFITABLE: a FAIL row with a quote shows ECON
              computed, its numbers below. */}
          {opp.economics && (
            <span
              title={
                opp.economics.computation_status === "error"
                  ? `Sin quote computable — motivo: ${opp.economics.error_reason ?? "desconocido"} (R8: números ausentes, no inventados)`
                  : opp.economics.computation_status === "partial"
                    ? "Cálculo parcial: algunas cifras reales existen; los huecos viajan con su motivo (not_computed_reasons)"
                    : "Cálculo completo del searcher: gross/costs/net/roi/target — COMPUTED ≠ PROFITABLE"
              }
              className={`text-[10px] px-1.5 py-0.5 rounded border font-mono uppercase tracking-wide ${
                opp.economics.computation_status === "computed"
                  ? "bg-muted/50 text-muted-foreground border-border/60"
                  : opp.economics.computation_status === "partial"
                    ? "bg-info/10 text-info border-info/30"
                    : "bg-muted/60 text-muted-foreground/70 border-border/60"
              }`}
            >
              ECON {opp.economics.computation_status === "computed" ? "✓" : opp.economics.computation_status === "partial" ? "◐" : "✗"}
            </span>
          )}
          {opp.confirmations != null && opp.confirmations > 1 && (
            <span
              title={`Ruta re-detectada ${opp.confirmations} veces en la ventana (confirmaciones)`}
              className="text-[10px] px-1.5 py-0.5 rounded bg-muted/50 text-muted-foreground/90 border border-border/60 font-mono"
            >
              ×{opp.confirmations}
            </span>
          )}
          {opp.chain_base_token_symbol && (
            <span className="text-[10px] px-1.5 py-0.5 rounded bg-muted/50 text-muted-foreground/90 border border-border/60 font-mono uppercase tracking-wide">
              {opp.chain_base_token_symbol}
            </span>
          )}
        </div>
        <div
          className={`flex items-center gap-1 font-bold text-base whitespace-nowrap ${
            roiTone === "pos" ? "text-success" : roiTone === "neg" ? "text-destructive" : "text-muted-foreground"
          }`}
          title="Net Convergence Ratio (ROI %) — fail-honest '—' when not computed"
        >
          {roiTone === "pos" && <TrendingUp size={14} />}
          {formatPctOrDash(roi)}
        </div>
      </div>

      {/* ── DETECTION TIME · AGE · VIGENCY ── */}
      <div className="flex items-center justify-between gap-2 mb-3 text-[10px] font-mono">
        <div className="flex items-center gap-1.5 text-muted-foreground" suppressHydrationWarning>
          <Clock size={11} />
          <span suppressHydrationWarning>
            {isMounted
              ? opp.detected_at == null
                ? "—"
                : new Date(opp.detected_at).toLocaleTimeString([], {
                    hour12: false,
                    hour: "2-digit",
                    minute: "2-digit",
                    second: "2-digit",
                  })
              : "--:--:--"}
          </span>
          <span className="text-muted-foreground/60">·</span>
          <span
            suppressHydrationWarning
            title="Tiempo desde la primera detección"
          >
            {isMounted
              ? vigency.firstAge == null
                ? "—"
                : `1ª ${vigency.firstAge}`
              : "--"}
          </span>
          <span className="text-muted-foreground/60">·</span>
          <span
            suppressHydrationWarning
            title="Última ratificación de vigencia por el sistema"
          >
            {isMounted
              ? vigency.lastAge == null
                ? "—"
                : `✓ ${vigency.lastAge}`
              : "--"}
          </span>
        </div>
        <div className="flex items-center gap-1.5">
          {/* WO-PRICE-EXCHANGE-V1 (FE) — source + freshness badge (pure, R1). */}
          <FreshnessBadge symbol={sourceSymbol} agoText={agoText} level={sourceFreshness} />
        </div>
        <span
          title={
            isStale === null
              ? "sin detected_at en el payload (§28) — no se afirma vigencia"
              : undefined
          }
          className={`inline-flex items-center gap-1 px-1.5 py-0.5 rounded border font-bold uppercase tracking-wide ${
            isStale === null
              ? "bg-muted/60 text-muted-foreground border-border"
              : isStale
                ? "bg-destructive/10 text-destructive border-destructive/30"
                : "bg-success/10 text-success border-success/30"
          }`}
        >
          {isStale === null ? (
            <MinusCircle size={10} />
          ) : isStale ? (
            <AlertTriangle size={10} className="animate-pulse" />
          ) : (
            <CheckCircle2 size={10} />
          )}
          {isStale === null ? "sin fecha" : isStale ? "stale" : "vigente"}
        </span>
      </div>

      {/* ── TOKEN PATH — every participating token (2..7), same chip element the
            operator validated: logo + contract shortAddr + symbol. CARDS-TOKENPATH-01:
            chips keep natural size and wrap to a new line only under overflow threat
            (flex-wrap); each chip truncates internally via min-w-0 max-w-full. ── */}
      <div className="flex flex-wrap items-center gap-x-1.5 gap-y-1 mb-3 min-w-0">
        {tokenPathAddrs.map((addr, i) => {
          const lc = addr.toLowerCase();
          const isEndpointIn = lc === opp.token_in.toLowerCase();
          const isEndpointOut = !isEndpointIn && lc === opp.token_out.toLowerCase();
          return (
            <React.Fragment key={`${i}-${lc}`}>
              {i > 0 && (
                <span className="text-muted-foreground/60 shrink-0" aria-hidden="true">
                  →
                </span>
              )}
              <div className="min-w-0 max-w-full">
                <TokenChip
                  token_address={addr}
                  chain_id={isEndpointOut ? (opp.chain_id_out ?? opp.chain_id) : opp.chain_id}
                  info={isEndpointIn ? opp.token_in_info : isEndpointOut ? opp.token_out_info : null}
                  fallbackSymbol={
                    isEndpointIn || isEndpointOut ? null : (opp.leg_symbols?.[lc] ?? null)
                  }
                />
                {/* CARDS-PRICES-01: live PriceBus USD price under the chip —
                    real-time Binance WS + Chainlink snapshot. Absent = no live
                    price for that symbol (R8: nothing rendered, never a guess). */}
                {(() => {
                  const info = isEndpointIn
                    ? opp.token_in_info
                    : isEndpointOut
                      ? opp.token_out_info
                      : null;
                  const sym = (
                    info?.symbol ??
                    info?.registry_symbol ??
                    opp.leg_symbols?.[lc]
                  )?.toUpperCase();
                  const px = sym ? opp.token_prices_usd?.[sym] : undefined;
                  if (px == null || !Number.isFinite(px) || px <= 0) return null;
                  const shown =
                    px >= 1
                      ? `$${px.toLocaleString("en-US", { maximumFractionDigits: 2 })}`
                      : `$${px.toPrecision(3)}`;
                  return (
                    <div
                      className="text-[9px] font-mono leading-tight text-emerald-300/90"
                      title={`Precio live PriceBus (${sym})`}
                    >
                      {shown}
                    </div>
                  );
                })()}
              </div>
            </React.Fragment>
          );
        })}
        {/* AUDIT-CARDS-MINOR (§3): §29 fallback chip row — same compact mark
            discipline as the ladder's SYNTHETIC band (reused classes, no new
            design): the in→out pair is NOT a persisted topology. */}
        {tokenPathIsFallback && (
          <span
            title="fallback §29 — sin topología persistida"
            className="text-[9px] uppercase tracking-wide text-muted-foreground/70 shrink-0"
          >
            ·syn
          </span>
        )}
      </div>

      {/* ── FE-0033 (§36): canonical summary grid — ruta/strategy/detector/
            hops/in/Gross/Net/bps/Risk/Sim/latencia, wire-grade only. The
            FE-0034 detail dialog reuses this as the Overview spine. ── */}
      <div className="mb-3">
        <OpportunitySummaryGrid opp={opp} />
      </div>

      {/* ── WO-PRICE-EXCHANGE-V1 (FE): route trend sparkline (local stream) ──
           R1: mounted-only — SSR renders nothing here, keeping server markup
           byte-stable. Client-only component so the series recording never
           touches the memoized card's render purity. */}
      {isMounted && (
        <div className="mb-3 flex items-end justify-between gap-2">
          <span className="text-[9px] uppercase tracking-wide text-muted-foreground/70 pb-0.5">
            Route trend · local stream
          </span>
          <RouteTrend routeKey={routeTrendKey} value={grossUsd} />
        </div>
      )}

      {/* ── EXECUTIVE RESULT: net yield + target verdict ── */}
      <div className="grid grid-cols-2 gap-2 mb-3">
        <div className="rounded-lg bg-muted/40 p-2">
          <div className="text-[9px] uppercase tracking-wide text-muted-foreground">Net yield</div>
          <div className="flex items-center gap-1">
            <span
              // WO-PRICE-EXCHANGE-V1: key=seq remounts on each CHANGE so the
              // one-shot flash replays on consecutive same-direction moves.
              key={netFlash.seq}
              className={`font-mono text-lg font-bold ${TONE_CLASS[net.tone] ?? "text-muted-foreground"} ${flashClass(netFlash) ?? ""}`}
              title={headlineNetTitle}
            >
              {headlineNetSource === "simulated" ? `~${net.display}` : net.display}
            </span>
            {headlineNetSource === "simulated" && (
              <span className="text-[9px] font-bold px-1 rounded bg-info/15 text-info border border-info/40">SIM</span>
            )}
          </div>
        </div>
        <div className="rounded-lg bg-muted/40 p-2">
          <div className="text-[9px] uppercase tracking-wide text-muted-foreground">
            Target · {canonicalTarget ? "searcher" : tgt?.target_source === "strategy_config" ? "/strategies" : tgt?.target_source === "simulation_tab" ? "Sim tab" : "none"}
          </div>
          <div
            className={`font-mono text-lg font-bold ${
              targetVerdict.tone === "pass"
                ? "text-success"
                : targetVerdict.tone === "fail"
                  ? "text-destructive"
                  : "text-muted-foreground"
            }`}
          >
            {targetVerdict.label}
          </div>
          {/* ALWAYS-COMPUTE (2026-09-27): the FAIL card's OWN arithmetic —
              displayed verbatim from the searcher's economics object
              (target vs achieved vs delta). Pure display: this block never
              computes, it renders what the producer persisted on the rejected
              row ("si el resultado da -$50, la card debe decir -$50"). */}
          {opp.economics &&
            opp.economics.computation_status !== "error" &&
            (opp.economics.target_net_usd != null || opp.economics.net_profit_usd != null) && (
              <div
                className="mt-1 font-mono text-[10px] leading-tight text-muted-foreground"
                title="Aritmética del searcher (objeto economics): target / logrado / delta — el FAIL muestra sus números, no guiones."
              >
                {opp.economics.target_net_usd != null && (
                  <div>
                    target {usd(opp.economics.target_net_usd)}
                  </div>
                )}
                {opp.economics.net_profit_usd != null && (
                  <div className={opp.economics.net_profit_usd >= 0 ? "text-success" : "text-destructive"}>
                    logrado {usd(opp.economics.net_profit_usd)}
                  </div>
                )}
                {opp.economics.target_delta_usd != null && (
                  <div className={opp.economics.target_delta_usd >= 0 ? "text-success" : "text-destructive"}>
                    delta {usd(opp.economics.target_delta_usd)}
                  </div>
                )}
              </div>
            )}
          {opp.economics?.computation_status === "error" && (
            <div
              className="mt-1 font-mono text-[10px] leading-tight text-muted-foreground/80"
              title={`Sin quote computable — motivo del productor: ${opp.economics.error_reason ?? "desconocido"}`}
            >
              sin quote: {opp.economics.error_reason ?? "—"}
            </div>
          )}
        </div>
      </div>

      {/* ── STEP LADDER: capital path with running USD totals ──
           Each row shows its USD contribution so the operator reads where value
           is gained/lost at every hop. Down = cost, up = inflow.

           CARDS-NOTIONAL-01: every row here belongs to ONE basis, chosen by
           `buildLedger`. A figure whose producer computed it at a DIFFERENT
           size than the row's own notional is not painted as if it belonged to
           that ladder — its cell carries the honest dash and `quietReason`
           travels in the `title`.

           ECON-DECLARE-01 (2026-09-27): the band below used to read
           "sin notional" unconditionally, because the wire carried the figures
           with no statement of the size they were computed at. The producer now
           DECLARES that size (`route_metadata.economics_amount_in_wei`) and the
           basis of each figure, so the band states the real notional — and only
           falls back to naming the absence when no producer declared one. */}
      <div className="rounded-lg border border-border bg-muted/20 mb-3 overflow-hidden">
        <div className="px-2.5 py-1.5 border-b border-border/60 text-[9px] uppercase tracking-wide text-muted-foreground font-semibold">
          Capital path (USD)
          <span className="ml-1 normal-case tracking-normal text-muted-foreground/60">
            · {ledger.basis === "simulated"
              ? "forward-sim @ amount_in_wei"
              : ledger.basis === "canonical"
                ? ledger.notional != null
                  ? `searcher gross/net @ ${ledger.notional.usd != null ? usd(ledger.notional.usd) : ""}${formatWeiNotional(ledger.notional.wei)}${ledger.notional.basis != null ? ` (${BASIS_LABEL[ledger.notional.basis]})` : " (@undeclared)"} declarado`
                  : "searcher gross/net — sin notional declarado"
                : "sin aritmética cerrada"}
          </span>
        </div>
        <div className="p-2 space-y-0.5 font-mono text-[11px]">
          <LedgerRow
            up
            label={`Flash loan in (TLS)${opp.chain_base_token_symbol ? ` · ${opp.chain_base_token_symbol}` : ""}`}
            value={capitalInUsd}
            title={
              capitalInUsd != null
                ? ledger.basis === "canonical"
                  ? `Notional real del sizing kernel = economics.amount_in_usd (amount_in_wei=${opp.economics?.amount_in_wei ?? opp.amount_in_wei ?? "no emitido"}).`
                  : `Notional del ladder = simulated_amount_in_usd (amount_in_wei=${opp.amount_in_wei ?? "no emitido"} valorado al precio vivo).`
                : quietReason ??
                  "Principal no computado — no se pinta un notional de otro productor."
            }
          />
          {legs.length > 0 ? (
            legs.map((l) => {
              const entry = legLedger?.[l.index] ?? null;
              return (
                <LedgerRow
                  key={l.index}
                  testId={`ledger-hop-${l.index + 1}`}
                  label={`Hop ${l.index + 1}/${legs.length} · ${legSym(l.token_in)}→${legSym(l.token_out)}`}
                  value={null}
                  muted
                  hint={`${l.dex || "—"}${l.synthetic ? " · syn" : ""}`}
                  valueNode={entry != null ? hopAmountNode(l, entry) : undefined}
                />
              );
            })
          ) : (
            <LedgerRow
              label="Hops"
              value={null}
              muted
              hint="sin topología persistida (§38)"
            />
          )}
          {hasSyntheticLegs && (
            <div className="pt-0.5 text-[9px] uppercase tracking-wide text-muted-foreground/70">
              {SYNTHETIC_LEGACY_VIEW_LABEL} — fallback §29, no ROUTE VERIFIED
            </div>
          )}
          <LedgerRow
            up
            label="Gross out (AMM spread)"
            value={grossUsd}
            tone="text-foreground"
            flashCls={flashClass(grossFlash)}
            flashSeq={grossFlash.seq}
            title={
              ledger.basis === "simulated"
                ? "simulated_gross_usd — el bruto de ESTE ladder (misma llamada que el net y los costos)."
                : ledger.notional != null
                  ? `expected_profit_usd — bruto del searcher, en el notional DECLARADO por su productor (${formatWeiNotional(ledger.notional.wei)} wei)${ledger.notional.basis != null ? ` con basis ${BASIS_LABEL[ledger.notional.basis]}` : " (basis @undeclared)"}.`
                  : "expected_profit_usd — bruto del searcher en SU propio tamaño; ningún productor declaró el notional de este par, así que el ladder no pinta principal."
            }
          />
          <div className="my-1 border-t border-border/50" />
          <LedgerRow
            down
            label={`Repay (principal + TLS fee)`}
            value={repayUsd}
            title={
              repayUsd != null
                ? "principal + flashloan_fee del MISMO ladder (simulación al notional mostrado)."
                : quietReason ??
                  "Principal no computado — no se pinta un notional de otro productor."
            }
          />
          {/* CARDS-NOTIONAL-01: the 9 labelled rows always render (stable
              layout + the operator can see WHICH component is missing); their
              VALUES exist only on the basis that owns a breakdown. A row with
              no value for this basis is a quiet dash carrying `quietReason`. */}
          {(ledger.basis === "canonical"
            ? LEDGER_COST_ROW_LABELS.filter(
                (label) => !["Capital cost", "Failure buffer", "Copied buffer"].includes(label),
              )
            : LEDGER_COST_ROW_LABELS
          ).map((label) => {
            const embedded =
              ledger.basis === "canonical" &&
              !costValueByLabel.has(label) &&
              (label === "LP fees" || label === "Decoherence (slippage)");
            const reasonKey = label === "LP fees" ? "dex_fees_usd" : "slippage_usd";
            const embeddedReason = embedded
              ? opp.economics?.not_computed_reasons?.[reasonKey] ?? null
              : null;
            return (
              <LedgerRow
                key={label}
                down
                label={label}
                value={costValueByLabel.get(label) ?? null}
                valueNode={
                  embedded ? (
                    <span className="text-[10px] text-muted-foreground/80">
                      {label === "LP fees" ? "incl. en quote" : "incl. en curva"}
                    </span>
                  ) : undefined
                }
                small
                title={
                  costValueByLabel.has(label)
                    ? undefined
                    : embeddedReason ?? quietReason ??
                      "Componente no computado para el notional mostrado (R8)."
                }
              />
            );
          })}
          <div className="my-1 border-t border-border/50" />
          <LedgerRow
            label="Total cost"
            value={knownCostSum}
            tone="text-destructive"
            strong
            title={
              ledger.basis === "simulated"
                ? "Σ de los 9 componentes del breakdown (incluye copied buffer = p_copied × gross)."
                : ledger.basis === "canonical"
                  ? "Derivado: expected_profit_usd − net_expected_profit_usd (la relación que el propio wire documenta para este par)."
                  : quietReason ?? "Sin aritmética cerrada para esta fila."
            }
          />
          <LedgerRow
            up={headlineNetUsd != null && headlineNetUsd > 0}
            down={headlineNetUsd != null && headlineNetUsd < 0}
            label={`Net yield${headlineNetSource === "simulated" ? " (SIM)" : headlineNetSource === "canonical" ? " (spine)" : ""}`}
            value={headlineNetUsd}
            tone={headlineNetUsd == null ? undefined : headlineNetUsd > 0 ? "text-success" : headlineNetUsd < 0 ? "text-destructive" : "text-muted-foreground"}
            strong
            flashCls={flashClass(netFlash)}
            flashSeq={netFlash.seq}
            title={
              // CARDS-PRECEDENCE-03: when the ladder did NOT close, this row is
              // the ONLY figure the block paints (principal/gross/cost rows stay
              // quiet), so it cannot be read as one arithmetic with a figure from
              // another notional — and the reason it is alone travels here.
              headlineNetSource !== "none" && ledger.net_usd == null
                ? `${headlineNetSource === "simulated" ? "simulated_net_profit_usd" : "net_expected_profit_usd"} — net del wire, mostrado solo (el ladder no se cierra: ${quietReason ?? "sin aritmética cerrada"}); ninguna otra fila del ladder pinta cifra.`
                : netSource === "simulated"
                  ? "simulated_net_profit_usd — net del MISMO ladder: net = gross − Σcostos, exacto."
                  : netSource === "canonical"
                    ? "net_expected_profit_usd — net del searcher; por construcción net = gross − Total cost en este bloque."
                    : quietReason ?? "Net no computado (R8)."
            }
          />
        </div>
        {ledgerIsQuiet && ledger.basis !== "simulated" && (
          <div
            className="px-2.5 pb-1.5 text-[9px] leading-tight text-muted-foreground/70"
            data-testid="ledger-basis-note"
            title={quietReason ?? undefined}
          >
            {ledger.basis === "canonical"
              ? "Ladder sobre el par (gross, net) del searcher: sin principal ni desglose por componente — esos viven en otro notional y no se mezclan (CARDS-NOTIONAL-01)."
              : "Sin aritmética cerrada para esta fila: las celdas de la ruta de capital van en guion a propósito (R8)."}
            {/* CARDS-PRECEDENCE-03: a dash in a non-closed ladder is only honest
                if the operator is told WHERE the computed figure is shown — the
                executive net above and the summary grid's `Net` cell. */}
            {ledger.basis !== "canonical" && headlineNetUsd != null && (
              <> El net computado sí se muestra: titular «Net yield» y celda «Net» de la grilla.</>
            )}
          </div>
        )}
      </div>

      {/* ── APPLIED STRATEGY CONFIG (from /strategies) ── */}
      <div className="rounded-lg border border-border bg-muted/20 mb-3 p-2">
        <div className="text-[9px] uppercase tracking-wide text-muted-foreground font-semibold mb-1.5">
          Applied strategy config
        </div>
        {canonicalTarget ? (
          <div className="grid grid-cols-1 gap-y-1 font-mono text-[11px]">
            <ConfigRow label="min net USD" value={canonicalTarget.target_net_usd != null ? usd(canonicalTarget.target_net_usd) : "—"} />
            <ConfigRow label="achieved net" value={canonicalTarget.net_profit_usd != null ? usd(canonicalTarget.net_profit_usd) : "—"} tone={canonicalTarget.net_profit_usd != null && canonicalTarget.net_profit_usd < 0 ? "text-destructive" : "text-success"} />
            <ConfigRow label="delta" value={canonicalTarget.target_delta_usd != null ? usd(canonicalTarget.target_delta_usd) : "—"} tone={canonicalTarget.target_delta_usd != null && canonicalTarget.target_delta_usd < 0 ? "text-destructive" : "text-success"} />
          </div>
        ) : tgt ? (
          // CARDS-LAYOUT-02 (2026-09-26, operator report — PROVEN on the live
          // card): this block was `grid grid-cols-2`, which Tailwind compiles to
          // `repeat(2, minmax(0,1fr))`. A `minmax(0,1fr)` track may be NARROWER
          // than its content minimum, so a pair overflowed its own column
          // instead of reflowing. Measured on the live card (viewport 1280, card
          // 289px, ledger 255px): the grid resolved to 115.33px tracks; the
          // `binding floor` cell painted `scrollWidth 124 > clientWidth 115`,
          // its label wrapped to 2 lines (`binding` / `floor`) and the value
          // wrapped at its hyphens into 3 lines (`net-per-` / `usd-` /
          // `nonpositive`), spilling toward the neighbouring column — exactly
          // the operator's "binding net_per_usd / floor nonpositive stacked on
          // top of each other". `binding_floor` is `net-per-usd-nonpositive` on
          // 38/38 live rows, so the defect was systematic, not an edge case.
          //
          // One pair per LINE (the block is 250-360px wide inside a 1/2/3-col
          // page grid — half of that is never a label/value pair): every row now
          // owns the full block width, the label truncates and the value keeps
          // its own width, so the pairs stay clean and cannot collide.
          <div className="grid grid-cols-1 gap-y-1 font-mono text-[11px]">
            <ConfigRow label="min net USD" value={tgt.target_net_usd != null ? usd(tgt.target_net_usd) : "—"} />
            <ConfigRow label="min ROI %" value={tgt.target_roi_pct != null ? `${tgt.target_roi_pct.toFixed(2)}%` : "—"} />
            <ConfigRow
              label="binding floor"
              value={tgt.binding_floor}
              title={`binding_floor del wire (inverse-sizing kernel): ${tgt.binding_floor}`}
              tone={
                tgt.binding_floor === "roi-unreachable" || tgt.binding_floor === "net-per-usd-nonpositive"
                  ? "text-destructive"
                  : "text-foreground"
              }
            />
            <ConfigRow
              label="suggested borrow"
              value={Number.isFinite(tgt.suggested_amount_in_usd) ? usd(tgt.suggested_amount_in_usd) : "—"}
            />
          </div>
        ) : (
          <div className="text-[11px] font-mono text-muted-foreground/60 italic">
            No target applied (inverse-sizing not run for this route).
          </div>
        )}
      </div>

      {/* ── EXECUTE (shadow) + evidence ── */}
      <div className="space-y-2">
        <button
          type="button"
          onClick={handleExecute}
          disabled={simLoading}
          className="w-full py-2.5 rounded-lg bg-primary text-primary-foreground text-xs font-bold uppercase tracking-wide hover:bg-primary/90 active:scale-[0.99] disabled:opacity-60 disabled:cursor-not-allowed transition-all inline-flex items-center justify-center gap-2"
          title={`Execute in shadow mode — POST /api/v1/opportunities/:id/simulate against the sim-ctl + Anvil fork. Read-only, capital $0; returns pass/fail + gas + trace evidence. Effective terminus: ${modeLabel}.`}
        >
          {simLoading ? (
            <>
              <Loader2 size={14} className="animate-spin" /> Simulating…
            </>
          ) : (
            <>
              <Play size={14} /> Execute (shadow · {modeLabel === "paper" ? "PAPER" : "LIVE"})
            </>
          )}
        </button>

        {/* Shadow-sim evidence strip — verifiable, never fabricated. */}
        {evidence && (
          <div className="rounded-md border border-border bg-muted/30 px-2 py-1.5 font-mono text-[10px] text-muted-foreground flex items-center gap-2">
            {evidence.passed === true && <CheckCircle2 size={11} className="text-success" />}
            {evidence.passed === false && <XCircle size={11} className="text-destructive" />}
            <span className="truncate">
              shadow-sim dispatched · trace{" "}
              <span className="text-foreground">{evidence.traceId ?? "—"}</span>
              {evidence.gasEstimateWei ? ` · gas ${evidence.gasEstimateWei}` : ""}
              {evidence.failReason ? ` · ${evidence.failReason}` : ""}
            </span>
          </div>
        )}
      </div>
    </div>
  );
}

// PERF (2026-08-09): memoize the card so the parent re-renders do NOT re-render
// every card. The parent used to push a fresh `now` every second → all ~200
// cards re-rendered each second. The comparator re-renders a card
// only when its own data changed OR its displayed age (seconds) ticked over.
// Business-equality fields are checked because the store emits a fresh array
// after each batch replacement, so reference equality on `opp` would fail.
//
// CARDS-MEMO-COVER-01 (2026-09-27) — a memo comparator that omits a field the
// card PAINTS is a cell that keeps hiding a value that has already arrived: the
// store replaces the array every batch, the comparator says "equal", React skips
// the render, and the screen keeps the previous frame's `—` for a figure the new
// row carries. The list below used to cover only IDENTITY + the canonical
// economics, while the summary grid and the ladder paint the whole `simulated_*`
// block, `risk_score`, `detector_id`, `pipeline_latency_ms`, `amount_in_wei`,
// `rejection_reason`, `chain_base_token_symbol` and `semantic_violations` — all
// of them starved. It is now COMPLETE and machine-checked:
// `components/__tests__/CardCellPaint.gate.test.tsx` mutates each field and
// asserts the comparator (exported below for exactly that purpose) returns
// false, and classifies every key of `OmniOpportunity` so a NEW wire field
// cannot be added without an explicit compared/ignored decision.
export function opportunityTradeCardPropsEqual(
  prev: OpportunityTradeCardProps,
  next: OpportunityTradeCardProps,
): boolean {
  const p = prev.opp;
  const n = next.opp;
  // FE-0029 (§28): null detected_at → NaN age; Object.is(NaN, NaN) = true so
  // two undated rows stay memo-equal instead of re-rendering forever.
  const ageOf = (o: typeof p, now: number) =>
    Math.floor(
      (now - (o.detected_at == null ? NaN : new Date(o.detected_at).getTime())) / 1000,
    );
  const agePrev = ageOf(p, prev.now);
  const ageNext = ageOf(n, next.now);
  // CARDS-DEDUP-HOPS: the dual vigency line keys on last_seen (✓ age renders
  // seconds while < 60s), and the ×N badge keys on confirmations — a merged
  // re-detection must re-render its card even when detected_at is unchanged.
  const lastAgeOf = (o: typeof p, now: number) =>
    Math.floor(
      (now -
        (o.last_seen_at == null
          ? o.detected_at == null
            ? NaN
            : new Date(o.detected_at).getTime()
          : new Date(o.last_seen_at).getTime())) /
        1000,
    );
  const lastAgePrev = lastAgeOf(p, prev.now);
  const lastAgeNext = lastAgeOf(n, next.now);
  // HOPS-CARD-03: the ladder now renders route_metadata/leg_symbols (and the
  // §29 fallback keys off dex_a/dex_b), so a batch that only changes the
  // topology MUST re-render — same sameJson discipline the exchange card's
  // comparator already applies (serialized content, small objects).
  const sameJson = (a: unknown, b: unknown): boolean =>
    a === b || JSON.stringify(a) === JSON.stringify(b);
  return (
    p.id === n.id &&
    p.status === n.status &&
    // The sim-evidence strip renders the trace id of the row the click captured
    // (the handler closes over `opp`), so a changed trace must repaint.
    p.trace_id === n.trace_id &&
    p.expected_profit_usd === n.expected_profit_usd &&
    p.net_expected_profit_usd === n.net_expected_profit_usd &&
    p.roi_pct === n.roi_pct &&
    p.detected_at === n.detected_at &&
    p.first_seen_at === n.first_seen_at &&
    p.last_seen_at === n.last_seen_at &&
    p.confirmations === n.confirmations &&
    sameJson(p.route_metadata, n.route_metadata) &&
    sameJson(p.leg_symbols, n.leg_symbols) &&
    sameJson(p.token_prices_usd, n.token_prices_usd) &&
    p.dex_a === n.dex_a &&
    p.dex_b === n.dex_b &&
    p.token_in === n.token_in &&
    p.token_out === n.token_out &&
    p.token_in_info?.logo_url === n.token_in_info?.logo_url &&
    p.token_out_info?.logo_url === n.token_out_info?.logo_url &&
    p.token_in_info?.symbol === n.token_in_info?.symbol &&
    p.token_out_info?.symbol === n.token_out_info?.symbol &&
    p.token_in_info?.registry_symbol === n.token_in_info?.registry_symbol &&
    p.token_out_info?.registry_symbol === n.token_out_info?.registry_symbol &&
    // ── CARDS-MEMO-COVER-01: every other field the card/grid DRAWS ───────────
    // Header / chips / quarantine strip
    p.chain_id === n.chain_id &&
    p.chain_id_out === n.chain_id_out &&
    p.strategy_kind === n.strategy_kind &&
    p.chain_base_token_symbol === n.chain_base_token_symbol &&
    sameJson(p.semantic_violations, n.semantic_violations) &&
    // Summary grid (OpportunitySummaryGrid — rendered INSIDE this card)
    p.detector_id === n.detector_id &&
    p.pipeline_latency_ms === n.pipeline_latency_ms &&
    // main's comparator also covered `block_number` and the `economics` blob;
    // both are KEPT here so this merge cannot regress that coverage. `economics`
    // is not cosmetic: the summary grid's `block` cell renders
    // `economics.quote_block` and the card paints `opp.economics` in dozens of
    // places, so a changed blob MUST repaint — the failure this gate exists for is
    // a value painted one frame late, or never. A field the comparator compares
    // can no longer be declared "not painted by the card" in
    // `CardCellPaint.gate.test.tsx`, so `block_number` leaves that list.
    p.block_number === n.block_number &&
    sameJson(p.economics, n.economics) &&
    p.hop_count === n.hop_count &&
    p.risk_score === n.risk_score &&
    p.pair_symbol === n.pair_symbol &&
    p.amount_in_wei === n.amount_in_wei &&
    p.rejection_reason === n.rejection_reason &&
    // The economics the grid, the ladder AND `buildLedger` read
    p.simulated_net_profit_usd === n.simulated_net_profit_usd &&
    p.simulated_amount_in_usd === n.simulated_amount_in_usd &&
    p.simulated_roi_pct === n.simulated_roi_pct &&
    p.simulated_gross_usd === n.simulated_gross_usd &&
    p.simulated_costs_total_usd === n.simulated_costs_total_usd &&
    sameJson(p.simulated_cost_breakdown, n.simulated_cost_breakdown) &&
    sameJson(p.simulated_target, n.simulated_target) &&
    prev.isMounted === next.isMounted &&
    prev.simLoading === next.simLoading &&
    prev.modeLabel === next.modeLabel &&
    prev.onExecute === next.onExecute &&
    prev.onInspect === next.onInspect &&
    Object.is(agePrev, ageNext) &&
    Object.is(lastAgePrev, lastAgeNext)
  );
}

export const OpportunityTradeCard = React.memo(
  OpportunityTradeCardImpl,
  opportunityTradeCardPropsEqual,
);

// ─── Ledger row (capital path) ───────────────────────────────────────────────
function LedgerRow({
  label,
  value,
  valueNode,
  up = false,
  down = false,
  muted = false,
  strong = false,
  small = false,
  tone,
  hint,
  flashCls,
  flashSeq,
  testId,
  title,
}: {
  label: string;
  value?: number | null;
  /** HOPS-LEDGER-04: rich cell (exact wei + Δ). Overrides the USD formatting. */
  valueNode?: React.ReactNode;
  up?: boolean;
  down?: boolean;
  muted?: boolean;
  strong?: boolean;
  small?: boolean;
  tone?: string;
  hint?: string;
  /** WO-PRICE-EXCHANGE-V1: one-shot flash class for this row's value cell. */
  flashCls?: string;
  /** Remount key — replays the CSS animation on consecutive changes. */
  flashSeq?: number;
  /** Stable DOM id for the SSR layout gates (cards-overlap regression). */
  testId?: string;
  /**
   * CARDS-NOTIONAL-01: why this cell is a dash, or which producer/notional the
   * painted figure belongs to. R8: a suppressed figure must still be
   * ACCOUNTABLE — the machine reason is one hover away, never hidden.
   */
  title?: string;
}) {
  // CARDS-LAYOUT-01 (2026-09-26, operator report — PROVEN with geometry on the
  // live card): the row used to be `flex items-center justify-between`. In a
  // flex row the label's `truncate` (`white-space: nowrap`, `overflow: hidden`)
  // gives that span a min-content floor, and the `(dex)` hint was a SECOND flex
  // item with no `min-w-0`/`truncate`/`shrink-0` at all, so neither side could
  // reflow: measured at viewports 1280/1440 the left span resolved to w=180
  // while its own content was 184px + gap + hint — the hint painted OUTSIDE its
  // box (the row's `scrollWidth` still equalled its `clientWidth`, so nothing
  // clipped it) and landed on top of the value cell. That is the "overlapping"
  // the operator reads in the hop rows.
  //
  // The header is now a two-track GRID (`minmax(0,1fr)` + `auto`): label and
  // hint both carry `min-w-0 truncate` (each may ellipsize, neither can push the
  // other), the hint is capped at 45% of the row, and the value cell is
  // `shrink-0 whitespace-nowrap tabular-nums`. Overflow is impossible by
  // construction at any card width and for any label/hint length.
  //
  // A row carrying a rich value cell (HOPS-LEDGER-04 per-hop amounts) renders
  // that cell on its OWN full-width line: `Hop i/N · A→B (dex)` on the header
  // line and the exact wei / leg Δ / cycle Δ right-aligned underneath. The hop
  // identity and its numbers no longer compete for one line — and each hop still
  // renders EXACTLY once (one LedgerRow per leg: no duplicated row, no phantom
  // second line).
  return (
    <div
      data-testid={testId}
      title={title}
      className={`${muted ? "text-muted-foreground/70" : "text-foreground"} ${
        small ? "text-[10px]" : ""
      } ${strong ? "font-bold" : ""}`}
    >
      <div className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-x-2">
        <span className="flex min-w-0 items-center gap-1">
          {up && <ArrowUpRight size={11} className="text-success shrink-0" />}
          {down && <ArrowDownRight size={11} className="text-destructive shrink-0" />}
          <span className="min-w-0 flex-1 truncate">{label}</span>
          {hint && (
            <span className="min-w-0 max-w-[45%] shrink-0 truncate text-[9px] text-muted-foreground/50 italic">
              ({hint})
            </span>
          )}
        </span>
        {valueNode == null && (
          <span
            key={flashSeq}
            className={`shrink-0 whitespace-nowrap tabular-nums ${tone ?? (muted ? "text-muted-foreground/60" : "text-foreground")} ${flashCls ?? ""}`}
          >
            {usd(value)}
          </span>
        )}
      </div>
      {valueNode != null && (
        <div className="flex min-w-0 justify-end pl-4" data-testid="ledger-row-amounts">
          {valueNode}
        </div>
      )}
    </div>
  );
}

// ─── WO-PRICE-EXCHANGE-V1 — route trend feed + sparkline (client-only) ───────
// Isolated component so the series recording (useEffect) and its local state
// re-render stay OUTSIDE the memoized card body: a new point refreshes this
// tiny subtree only. The Sparkline's `points` prop is the future seam for the
// backend price_history / price_delta mirror.
function RouteTrend({
  routeKey,
  value,
}: {
  routeKey: string;
  value: number | null;
}) {
  const points = useRouteSeries(routeKey, value);
  return <Sparkline points={points ?? undefined} />;
}

// ─── Config row (applied strategy config) ────────────────────────────────────
function ConfigRow({
  label,
  value,
  tone = "text-foreground",
  title,
}: {
  label: string;
  value: string;
  tone?: string;
  title?: string;
}) {
  // CARDS-LAYOUT-02: label/value pair as a two-track grid — the label truncates
  // (never wraps into a second line that reads as another label) and the value
  // keeps its own width on one line (`tabular-nums` so digits line up across
  // rows). No pair can overflow into its neighbour.
  return (
    <div
      data-testid="config-pair"
      title={title}
      className="grid grid-cols-[minmax(0,1fr)_auto] items-baseline gap-x-2"
    >
      <span className="min-w-0 truncate text-muted-foreground">{label}</span>
      <span className={`shrink-0 whitespace-nowrap tabular-nums ${tone}`}>{value}</span>
    </div>
  );
}
