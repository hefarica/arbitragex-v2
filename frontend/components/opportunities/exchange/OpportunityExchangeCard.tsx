/**
 * OpportunityExchangeCard — SSOT "glass neon" trading card for the
 * `/opportunities/exchange` grid.
 *
 * Visual language is a VERBATIM port of the canonical two-state showcase in
 * `docs/atlas_264.html` (lines 184-229): the evaluated row renders `.demo.eval`
 * (green neon glass kv ledger + QuantumX dapp-badge + ⚡ EXECUTE button), the
 * hollow-detection row renders `.demo.diag` (amber glass diagnostic with the
 * decoded "Por qué NO pasó" box). Styles live in
 * `app/opportunities/exchange/atlas-glass.css`, scoped under `.atlas-scope`.
 *
 * Kept from the previous discipline (RU-A + memory):
 *   - NO framer-motion (plain divs).
 *   - `content-visibility: auto` + `contain-intrinsic-size` per state.
 *   - React.memo + business-equality comparator (store batch replacements).
 *   - R8 fail-honest throughout: null → "—", never 0 dressed as a computed value.
 *   - R1: non-deterministic time text only after mount ("--:--:--" on SSR),
 *     suppressHydrationWarning ONLY on the time span.
 */
"use client";

import React, { useState } from "react";
import { Info, Loader2 } from "lucide-react";

import { formatPctOrDash, formatProfitUSD, shortAddr } from "@/lib/format";
import { deriveLegs, type OmniOpportunity, type TokenInfo } from "@/lib/store/types";
import { familyOf } from "@/lib/strategy-kinds";
import type { StrategyRuntimeConfig } from "@/lib/schemas";

// ── QuantumX orbital logo — SVG data-URI copied VERBATIM from the model
//    (docs/atlas_264.html line 185). Do not regenerate. ──────────────────────
const QUANTUMX_LOGO =
  "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 48 48'%3E%3Crect width='48' height='48' rx='11' fill='%230C1230'/%3E%3Cdefs%3E%3ClinearGradient id='o' x1='10' y1='10' x2='38' y2='38' gradientUnits='userSpaceOnUse'%3E%3Cstop stop-color='%239BC0FF'/%3E%3Cstop offset='0.52' stop-color='%234F7BF7'/%3E%3Cstop offset='1' stop-color='%232742E0'/%3E%3C/linearGradient%3E%3CradialGradient id='n' cx='0.5' cy='0.45' r='0.6'%3E%3Cstop stop-color='%23EAF2FF'/%3E%3Cstop offset='0.55' stop-color='%236E9CFF'/%3E%3Cstop offset='1' stop-color='%232C54EE'/%3E%3C/radialGradient%3E%3C/defs%3E%3Cellipse cx='24' cy='24' rx='15.5' ry='5.6' transform='rotate(45 24 24)' stroke='url(%23o)' stroke-width='2.4'/%3E%3Cellipse cx='24' cy='24' rx='15.5' ry='5.6' transform='rotate(-45 24 24)' stroke='url(%23o)' stroke-width='2.4'/%3E%3Ccircle cx='24' cy='24' r='3.8' fill='url(%23n)'/%3E%3Ccircle cx='34.9' cy='13.1' r='1.8' fill='%239BC0FF'/%3E%3Ccircle cx='13.1' cy='34.9' r='1.8' fill='%235B86F7'/%3E%3C/svg%3E";

/** Freshness window (matches the existing card's 12s staleness heuristic). */
const STALE_SECS = 12;

// ─── Family label for the dapp-badge subtitle ────────────────────────────────
// Mirrors StrategyBadge's canonical base labels; cartridge kinds surface their
// MEV-XX family prefix via familyOf (same source of truth).
const BASE_FAMILY_LABEL: Record<string, string> = {
  dex_arb: "DEX CONVERGENCE",
  triangular: "TRIANGULAR RESOLUTION",
  backrun: "TEMPORAL BACKRUN",
  liquidation: "ENTROPY LIQUIDATION",
  flashloan_arb: "FLASH CONVERGENCE",
};

function strategyFamilyLabel(kind: string): string {
  const base = BASE_FAMILY_LABEL[kind];
  if (base) return base;
  if (kind.startsWith("mev_") || kind.startsWith("cartridge_")) return familyOf(kind);
  return kind; // R8 fail-honest: surface the raw kind, never hide it
}

/** "mev_01_023_x_y" → "MEV-01-023" (compact showcase id, local transform). */
function compactStrategyId(kind: string): string {
  const m = kind.match(/^mev_(\d{2})_(\d{3})_/);
  if (m) return `MEV-${m[1]}-${m[2]}`;
  return kind.toUpperCase();
}

// ─── Number formatting (model: $12,000.00 / -$10.80 / +$18.93) ───────────────
/** Full USD amount with en-US commas and 2 decimals ("$12,000.00"); "—" if absent. */
function usdAmount(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value)) return "—";
  return `$${value.toLocaleString("en-US", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  })}`;
}

/** Cost cell with explicit minus ("-$10.80"); "—" if absent. */
function usdCost(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value)) return "—";
  return `-$${Math.abs(value).toFixed(2)}`;
}

/**
 * Token symbol with honest fallbacks when metadata is missing.
 * F1 (audit §11 RC2): registry_symbol is REAL curated-list data resolved by
 * the api-server for every token — surface it before the truncated address
 * instead of discarding it. R8 intact: still no fabrication.
 */
function tokenSymbol(addr: string, info: TokenInfo | null): string {
  return info?.symbol ?? info?.registry_symbol ?? shortAddr(addr);
}

/** Deterministic fallback hue from the symbol (tokEl port, model lines 308-321). */
function hueOf(sym: string): number {
  return [...sym].reduce((a, c) => a + c.charCodeAt(0), 0) % 360;
}

// ─── Single .tok chip: 18px logo (lazy) or deterministic hsl fallback ────────
function TokEl({ addr, info }: { addr: string; info: TokenInfo | null }) {
  const sym = tokenSymbol(addr, info);
  const logo = info?.logo_url;
  return (
    <span className="tok">
      {logo ? (
        // eslint-disable-next-line @next/next/no-img-element
        <img src={logo} alt={sym} width={18} height={18} loading="lazy" />
      ) : (
        <span className="fallback" style={{ background: `hsl(${hueOf(sym)} 45% 35%)` }}>
          {sym.slice(0, 2)}
        </span>
      )}
      <span>{sym}</span>
    </span>
  );
}

interface SimEvidence {
  passed: boolean | null;
  gasEstimateWei: string | null;
  traceId: string | null;
  failReason: string | null;
}

export interface OpportunityExchangeCardProps {
  opp: OmniOpportunity;
  /** Live clock (ms epoch) from the parent ticker — drives age + vigency. */
  now: number;
  /** SSR/CSR gate so time-dependent text only renders client-side (R1). */
  isMounted: boolean;
  /** Whether a shadow-sim is currently running for this card. */
  simLoading: boolean;
  /**
   * Declared per-strategy config from trading_config.strategy_configs.
   * R8: null when the endpoint has no entry for this strategy_kind → "—".
   */
  strategyConfig?: StrategyRuntimeConfig | null;
  /** Effective execution mode label ("paper" | "live") for the execute button. */
  modeLabel: "paper" | "live";
  /** Trigger the shadow simulation (wired to POST .../simulate). */
  onExecute: (opportunityId: string) => Promise<void> | void;
  /** Open the full detail dialog (the old "Inspect details"). */
  onInspect: (opp: OmniOpportunity) => void;
}

// ─── SSOT two-state gate (RU-A) ─────────────────────────────────────────────
// A row that never reached economic evaluation is NOT an opportunity — it is a
// DETECTION diagnostic. Rendering the full trading-card skeleton for it (all
// "—") dressed a hollow shell as an opportunity: the operator saw "fallback
// cards" with no arbitrage and no profit. Fail-honest fix: render the shell as
// what it IS, with the machine reason decoded, and never offer Execute on an
// unevaluated row (garbage-in simulation).
export function isUnevaluatedShell(opp: OmniOpportunity): boolean {
  return (
    opp.status === "detected" &&
    opp.rejection_reason != null &&
    opp.expected_profit_usd == null &&
    opp.net_expected_profit_usd == null &&
    opp.simulated_net_profit_usd == null
  );
}

/** Human decoding of the machine rejection codes the engines emit. */
function decodeRejectionReason(reason: string): string {
  if (reason.startsWith("cartridge_unmapped_strategy_label:")) {
    const label = reason.split(":")[1] ?? "?";
    return `El cartucho detectó la forma pero su categoría "${label}" no está mapeada a ningún motor de evaluación — la economía (profit/ROI/riesgo) nunca se computó. Mapping en cartridge_boot.rs:1145 (RU-4 lo completa).`;
  }
  if (reason.includes("impact_zero")) return "El evento impactó 0 pools indexados.";
  if (reason.includes("no_price") || reason.includes("unknown_token_price"))
    return "Falta precio USD de un token del route — el valor no puede computarse honestamente.";
  if (reason.includes("anomalous_math")) return "La matemática del pool resultó anómala (reserves inválidas).";
  return "Razón sin decodificar — ver código crudo.";
}

function OpportunityExchangeCardImpl({
  opp,
  now,
  isMounted,
  simLoading,
  strategyConfig = null,
  modeLabel,
  onExecute,
  onInspect,
}: OpportunityExchangeCardProps) {
  const [evidence, setEvidence] = useState<SimEvidence | null>(null);

  // ── SSOT gate: hollow detections render as diagnostics, never as trades ──
  if (isUnevaluatedShell(opp)) {
    return <DetectionDiagnosticCard opp={opp} now={now} isMounted={isMounted} onInspect={onInspect} />;
  }

  // ── Detection time / age / vigency ─────────────────────────────────────────
  const detectedTime = new Date(opp.detected_at).getTime();
  const ageSecs = isMounted ? Math.max(0, Math.floor((now - detectedTime) / 1000)) : 0;
  const isStale = ageSecs > STALE_SECS;

  // ── Net priority: canonical spine → TS simulated → "—" ─────────────────────
  const canonicalNet = opp.net_expected_profit_usd ?? null;
  const simulatedNet = opp.simulated_net_profit_usd ?? null;
  const netSource: "canonical" | "simulated" | "none" =
    canonicalNet != null ? "canonical" : simulatedNet != null ? "simulated" : "none";
  const netUsd = canonicalNet ?? simulatedNet;
  const netFmt = formatProfitUSD(netUsd);

  const roi = opp.roi_pct;

  // ── Route legs (A→B cycle) ─────────────────────────────────────────────────
  const legs = deriveLegs(opp);
  const legCount = legs.length;

  // ── Capital / costs (real values only — RULE 00 / R8) ──────────────────────
  const tgt = opp.simulated_target;
  const cb = opp.simulated_cost_breakdown;
  const capitalInUsd =
    opp.simulated_amount_in_usd ??
    (tgt != null && Number.isFinite(tgt.suggested_amount_in_usd) ? tgt.suggested_amount_in_usd : null);
  const flashFee = cb?.flashloan_fee_usd ?? null;
  const grossUsd = opp.expected_profit_usd ?? null;

  // Interest % = fee / amount * 100 — only when both real numbers exist.
  const interestPct =
    flashFee != null && capitalInUsd != null && capitalInUsd > 0
      ? (flashFee / capitalInUsd) * 100
      : null;

  // Gross out (AMM): capital + gross when both exist, else "—".
  const grossOutUsd =
    capitalInUsd != null && grossUsd != null ? capitalInUsd + grossUsd : null;

  // Target verdict (same infeasible floors as before).
  const targetText = (() => {
    if (tgt == null) return "—";
    const infeasible =
      tgt.binding_floor === "roi-unreachable" || tgt.binding_floor === "net-per-usd-nonpositive";
    const verdict = tgt.meets_target_at_cap && !infeasible ? "PASS" : "FAIL";
    const segs: string[] = [verdict];
    if (tgt.target_net_usd != null && Number.isFinite(tgt.target_net_usd))
      segs.push(`min $${tgt.target_net_usd.toFixed(2)}`);
    if (tgt.target_roi_pct != null && Number.isFinite(tgt.target_roi_pct))
      segs.push(`≥${tgt.target_roi_pct.toFixed(2)}%`);
    return segs.join(" · ");
  })();

  // Route summary: "SYM→SYM (DEX) → SYM (DEX)" with real symbols, shortAddr fallback.
  const routeText = (() => {
    if (legCount === 0) return "—";
    // F2 (audit §11 RC1): pair tokens use their enriched info; INTERMEDIATE
    // legs (previously always shortAddr) resolve via the server-hydrated
    // leg_symbols map. Absent entry → honest shortAddr fallback (R8).
    const sym = (addr: string): string => {
      const lc = addr.toLowerCase();
      if (lc === opp.token_in.toLowerCase() && opp.token_in_info)
        return tokenSymbol(addr, opp.token_in_info);
      if (lc === opp.token_out.toLowerCase() && opp.token_out_info)
        return tokenSymbol(addr, opp.token_out_info);
      return opp.leg_symbols?.[lc] ?? shortAddr(addr);
    };
    return legs
      .map((leg, i) =>
        i === 0
          ? `${sym(leg.token_in)}→${sym(leg.token_out)} (${leg.dex || "—"})`
          : `${sym(leg.token_out)} (${leg.dex || "—"})`,
      )
      .join(" → ");
  })();

  const handleExecute = async (e: React.MouseEvent) => {
    e.stopPropagation();
    setEvidence(null);
    try {
      await onExecute(opp.id);
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

  // ── content-visibility: the browser skips render for off-screen cards ──────
  // (native virtualization). `auto 640px` = remember last measured size,
  // initial estimate 640px so the scrollbar is correct before first paint.
  const cardStyle = {
    contentVisibility: "auto",
    containIntrinsicSize: "auto 640px",
  } as React.CSSProperties;

  return (
    <div style={cardStyle} className="demo eval">
      {/* ⓘ discreet Inspect affordance — top-right corner */}
      <button
        type="button"
        aria-label="Inspect details"
        title="Inspect details"
        onClick={(e) => {
          e.stopPropagation();
          onInspect(opp);
        }}
        className="inspect-btn"
      >
        <Info size={13} />
      </button>

      {/* ── dapp-badge: QuantumX · family label · LIVE/STALE LED ── */}
      <div className="dapp-badge">
        {/* eslint-disable-next-line @next/next/no-img-element */}
        <img src={QUANTUMX_LOGO} alt="QuantumX" />
        <span>Evaluada</span>
        <span className="sep">·</span>
        <span title={opp.strategy_kind}>{strategyFamilyLabel(opp.strategy_kind)}</span>
        <span className="led-group">
          {isStale ? <span className="led wait" /> : <span className="led on" />}
          <span className={isStale ? "led-text-wait" : "led-text-live"}>
            {isStale ? "STALE" : "LIVE"}
          </span>
        </span>
      </div>

      {/* ── identity + detection ── */}
      <div className="kv">
        <span>{opp.chain_base_token_symbol ?? String(opp.chain_id)}</span>
        <span className="hi">{compactStrategyId(opp.strategy_kind)}</span>
      </div>
      <div className="kv">
        <span>Detección</span>
        <span className="v" suppressHydrationWarning>
          {isMounted
            ? new Date(opp.detected_at).toLocaleTimeString([], {
                hour12: false,
                hour: "2-digit",
                minute: "2-digit",
                second: "2-digit",
              })
            : "--:--:--"}{" "}
          · {isMounted ? `${ageSecs}s` : "--"} · {isStale ? "stale" : "vigente"}
        </span>
      </div>

      <div style={{ height: 6 }} />

      {/* ── token pair + contracts ── */}
      <div className="kv">
        <span>Token par</span>
        <span className="v tokpair">
          <TokEl addr={opp.token_in} info={opp.token_in_info} />
          <span className="arrow">⇄</span>
          <TokEl addr={opp.token_out} info={opp.token_out_info} />
        </span>
      </div>
      <div className="kv">
        <span>Contratos</span>
        <span className="v">
          {shortAddr(opp.token_in)} / {shortAddr(opp.token_out)}
        </span>
      </div>

      <div style={{ height: 8 }} />

      {/* ── capital ledger (real values; "—" when not computed) ── */}
      <div className="kv">
        <span className="lbl-acc">Monto a invertir</span>
        <span className="hi">{usdAmount(capitalInUsd)}</span>
      </div>
      <div className="kv">
        <span className="lbl-acc">Monto a prestar (TLS)</span>
        <span className="hi">{usdAmount(capitalInUsd)}</span>
      </div>
      <div className="kv">
        <span>
          Interés a pagar{interestPct != null ? ` (${interestPct.toFixed(2)}%)` : ""}
        </span>
        <span className="neg">{usdCost(flashFee)}</span>
      </div>
      <div className="kv">
        <span>Gas (estimado)</span>
        <span className="neg">{usdCost(cb?.gas_usd ?? null)}</span>
      </div>
      <div className="kv">
        <span>LP fees{legCount > 0 ? ` (${legCount} legs)` : ""}</span>
        <span className="neg">{usdCost(cb?.lp_fees_usd ?? null)}</span>
      </div>
      <div className="kv">
        <span>Decoherencia (slip)</span>
        <span className="neg">{usdCost(cb?.slippage_usd ?? null)}</span>
      </div>

      <div style={{ height: 6, borderTop: "1px solid rgba(74,222,128,0.15)" }} />

      {/* ── results ── */}
      <div className="kv">
        <span>Gross out (AMM)</span>
        <span>{usdAmount(grossOutUsd)}</span>
      </div>
      <div className="kv">
        <span className="lbl-net">Net Yield</span>
        <span
          className={netUsd == null ? "v" : netUsd > 0 ? "num" : netUsd < 0 ? "neg" : "v"}
          title={
            netSource === "canonical"
              ? "Canonical spine net = gross − all costs"
              : netSource === "simulated"
                ? "TS forward-sim net (canonical pending)"
                : "Not yet computed (R8: '—')"
          }
        >
          {netFmt.display}
        </span>
      </div>
      <div className="kv">
        <span>ROI</span>
        <span className={roi == null ? "v" : roi > 0 ? "num" : roi < 0 ? "neg" : "v"}>
          {formatPctOrDash(roi)}
        </span>
      </div>
      <div className="kv">
        <span>Target</span>
        <span className="v">{targetText}</span>
      </div>

      <div style={{ height: 6 }} />

      {/* ── route + source ── */}
      <div className="kv">
        <span>Ruta</span>
        <span className="v">{routeText}</span>
      </div>
      <div className="kv">
        <span>Buy px / Sell px</span>
        <span className="v" title="Per-leg execution prices are not persisted on this row (R8 fail-honest)">
          —
        </span>
      </div>
      <div className="kv">
        <span>Fuente</span>
        <span className="v">
          {netSource === "canonical" ? "canonical spine" : netSource === "simulated" ? "simulated (SIM)" : "—"}
        </span>
      </div>

      {/* ── EXECUTE (shadow) + evidence ──
           The shadow simulate is mode-invariant and read-only (capital $0):
           safe under paper, testnet, and mainnet (§34). The label surfaces the
           effective terminus mode so the operator knows which settlement the
           real (non-shadow) path would target. */}
      <button
        type="button"
        onClick={handleExecute}
        disabled={simLoading}
        className="btn"
        title={`Shadow-simulate on the Anvil fork (read-only, capital $0). Effective terminus: ${modeLabel}.`}
      >
        {simLoading ? (
          <span className="sim-note">
            <Loader2 size={12} style={{ verticalAlign: "-2px", marginRight: "4px" }} className="animate-spin" /> SIMULATING…
          </span>
        ) : (
          `⚡ EXECUTE (${modeLabel === "paper" ? "PAPER SHADOW" : "LIVE SHADOW"})`
        )}
      </button>

      {evidence && (
        <div className="evidence">
          <span className="truncate">
            shadow-sim dispatched · trace <span className="ev-hi">{evidence.traceId ?? "—"}</span>
            {evidence.gasEstimateWei ? ` · gas ${evidence.gasEstimateWei}` : ""}
            {evidence.failReason ? ` · ${evidence.failReason}` : ""}
          </span>
        </div>
      )}
    </div>
  );
}

// PERF: memoize so parent re-renders (age ticker, store batch replacement) do
// NOT re-render every card. Business-equality fields are checked because the
// store emits a fresh array after each batch replacement (reference equality on
// `opp` would always fail). The comparator must cover every field the card
// READS: the rejection/simulation fields flip the SSOT two-state gate
// (diagnostic vs evaluated face), the nested simulated_* payloads drive the
// capital ledger, route_metadata drives the Ruta row, and token symbol
// enrichment replaces shortAddr fallbacks. Nested wire payloads are compared by
// serialized content (small objects) so a batch that only enriches them still
// re-renders the card.
function sameJson(a: unknown, b: unknown): boolean {
  return a === b || JSON.stringify(a) === JSON.stringify(b);
}

export const OpportunityExchangeCard = React.memo(
  OpportunityExchangeCardImpl,
  (prev: OpportunityExchangeCardProps, next: OpportunityExchangeCardProps): boolean => {
    const p = prev.opp;
    const n = next.opp;
    const agePrev = Math.floor((prev.now - new Date(p.detected_at).getTime()) / 1000);
    const ageNext = Math.floor((next.now - new Date(n.detected_at).getTime()) / 1000);
    return (
      p.id === n.id &&
      p.status === n.status &&
      p.strategy_kind === n.strategy_kind &&
      p.chain_base_token_symbol === n.chain_base_token_symbol &&
      p.detected_at === n.detected_at &&
      p.rejection_reason === n.rejection_reason &&
      p.expected_profit_usd === n.expected_profit_usd &&
      p.net_expected_profit_usd === n.net_expected_profit_usd &&
      p.roi_pct === n.roi_pct &&
      p.simulated_net_profit_usd === n.simulated_net_profit_usd &&
      p.simulated_amount_in_usd === n.simulated_amount_in_usd &&
      sameJson(p.simulated_cost_breakdown, n.simulated_cost_breakdown) &&
      sameJson(p.simulated_target, n.simulated_target) &&
      sameJson(p.route_metadata, n.route_metadata) &&
      p.token_in_info?.logo_url === n.token_in_info?.logo_url &&
      p.token_out_info?.logo_url === n.token_out_info?.logo_url &&
      p.token_in_info?.symbol === n.token_in_info?.symbol &&
      p.token_out_info?.symbol === n.token_out_info?.symbol &&
      prev.isMounted === next.isMounted &&
      prev.simLoading === next.simLoading &&
      prev.modeLabel === next.modeLabel &&
      prev.strategyConfig === next.strategyConfig &&
      prev.onExecute === next.onExecute &&
      prev.onInspect === next.onInspect &&
      agePrev === ageNext
    );
  },
);

// ─── DetectionDiagnosticCard (RU-A SSOT — .demo.diag of the model) ───────────
// The honest face of a hollow detection: identity + WHY it was never evaluated.
// No ledger numbers, no Execute — an unevaluated row has no arbitrage to show,
// and pretending otherwise was the "fallback card" lie.
function DetectionDiagnosticCard({
  opp,
  now,
  isMounted,
  onInspect,
}: Pick<OpportunityExchangeCardProps, "opp" | "now" | "isMounted" | "onInspect">) {
  const ageSecs = isMounted ? Math.max(0, Math.floor((now - new Date(opp.detected_at).getTime()) / 1000)) : 0;
  const isStale = ageSecs > STALE_SECS;
  const reason = opp.rejection_reason ?? "unknown";
  const degeneratePair = opp.token_in === opp.token_out;

  return (
    <div
      style={{ contentVisibility: "auto", containIntrinsicSize: "auto 460px" } as React.CSSProperties}
      className="demo diag"
    >
      <button
        type="button"
        aria-label="Inspect details"
        onClick={(e) => {
          e.stopPropagation();
          onInspect(opp);
        }}
        className="inspect-btn"
      >
        <Info size={13} />
      </button>

      {/* ── dapp-badge warn: QuantumX (dimmed) · Detección · PENDING LED ── */}
      <div className="dapp-badge warn">
        {/* eslint-disable-next-line @next/next/no-img-element */}
        <img src={QUANTUMX_LOGO} alt="QuantumX" />
        <span>Detección</span>
        <span className="sep">·</span>
        <span>Sin evaluar</span>
        <span className="led-group">
          <span className="led wait" />
          <span className="led-text-hot">PENDING</span>
        </span>
      </div>

      {/* what IS real: cartridge identity, timestamp, age */}
      <div className="kv">
        <span>{opp.chain_base_token_symbol ?? String(opp.chain_id)}</span>
        <span className="hi" title={opp.strategy_kind}>
          {compactStrategyId(opp.strategy_kind)}
        </span>
      </div>
      <div className="kv">
        <span>Detección</span>
        <span className="v" suppressHydrationWarning>
          {isMounted
            ? new Date(opp.detected_at).toLocaleTimeString([], {
                hour12: false,
                hour: "2-digit",
                minute: "2-digit",
                second: "2-digit",
              })
            : "--:--:--"}{" "}
          · {isMounted ? `${ageSecs}s` : "--"} · {isStale ? "stale" : "vigente"}
        </span>
      </div>

      <div style={{ height: 6 }} />

      <div className="kv">
        <span>Token par</span>
        <span className="v tokpair">
          <TokEl addr={opp.token_in} info={opp.token_in_info} />
          <span className="arrow">⇄</span>
          <TokEl addr={opp.token_out} info={opp.token_out_info} />
        </span>
      </div>
      <div className="kv">
        <span>Estado</span>
        <span className="v val-warn">{opp.status.toUpperCase()} → REJECTED</span>
      </div>

      <div style={{ height: 8 }} />

      {/* the WHY — decoded machine reason + raw code */}
      <div className="kv why-box">
        <span className="why-label">Por qué NO pasó</span>
      </div>
      <div className="kv">
        <span className="why-text">{decodeRejectionReason(reason)}</span>
      </div>

      <div style={{ height: 6 }} />

      <div className="kv">
        <span>Razón máquina</span>
        <span className="v raw-code">{reason}</span>
      </div>
      <div className="kv">
        <span>Dato crudo</span>
        <span className="v">
          {degeneratePair ? "token_in == token_out (degenerada)" : "—"}
        </span>
      </div>

      <div style={{ height: 6 }} />

      {/* explicit truth: economics never ran — nothing to trade here, yet */}
      <div className="kv">
        <span>Monto a invertir</span>
        <span className="v">—</span>
      </div>
      <div className="kv">
        <span>Monto a prestar</span>
        <span className="v">—</span>
      </div>
      <div className="kv">
        <span>Interés</span>
        <span className="v">—</span>
      </div>
      <div className="kv">
        <span>Net Yield</span>
        <span className="v">—</span>
      </div>
      <div className="kv diag-footer">
        <span className="diag-footer-text">
          Sin evaluación económica no hay Execute. Cuando el motor correspondiente la evalúe,
          esta misma fila aparecerá como oportunidad con números reales.
        </span>
      </div>
    </div>
  );
}
