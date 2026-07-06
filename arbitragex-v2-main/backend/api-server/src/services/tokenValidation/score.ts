/**
 * FinalTokenScore — composes the outputs of all Phase-1 validators into a
 * single (status, score, reasons) triple that the dashboard renders.
 *
 * Status taxonomy (mutually exclusive — exactly one applies):
 *   - INVALID       — `is_contract=false` OR `standard != ERC20`. The
 *                     "token" at this address doesn't behave like an ERC-20
 *                     contract. Should never appear in opportunity flow.
 *   - VERIFIED      — `registry_verified=true`. In Uniswap default list or
 *                     CoinGecko coinslist, on-chain symbol matches. Safe
 *                     baseline assumption: it's a real token.
 *   - VIABLE        — Not in registry but has `liquidity_usd ≥ $5,000`
 *                     AND `volume_24h_usd ≥ $1,000`. New/long-tail token
 *                     with real market activity; tradeable.
 *   - LOW_LIQUIDITY — `$100 ≤ liquidity_usd < $5,000`. Has SOME liquidity
 *                     but below sane trading thresholds.
 *   - ILLIQUID      — `liquidity_usd < $100`. Either rugpulled, scam, or
 *                     pre-launch with no real depth. The SCFN/IVQ/69420
 *                     pattern.
 *   - NO_DATA       — DEX Screener has no pair indexed. Token might exist
 *                     on-chain but has no public market.
 *   - PENDING       — Validators couldn't run (RPC offline, DEX Screener
 *                     timeout). Persist with `score=0` so the dashboard
 *                     shows "validating…" rather than mis-classifying.
 *
 * Score (0-100, displayed as colored badge in UI):
 *   ≥ 70  → green  (tradeable)
 *   50-69 → yellow (caution)
 *   < 50  → red    (do not trade)
 *
 * Score weights (sum to 100 when all signals firing):
 *   exists_onchain + is_contract           +15
 *   standard == 'ERC20'                    +10
 *   registry_verified                      +30
 *   liquidity_usd ≥ $5k                    +25
 *   liquidity_usd ≥ $100 (LOW threshold)   +10
 *   volume_24h_usd ≥ $1k                   +10
 *   pair_count ≥ 2                         +5
 *   volume / liquidity ratio < 100 (no wash) +5
 *
 * The weights are deliberately conservative — a registry-verified token
 * with low liquidity (a brand-new launch) still scores 55-65 (yellow).
 * An unverified token with $0.01 liquidity gets ≤ 25 (red).
 */

import type { LiquidityRealityResult } from "./liquidityReality.js";
import type { OnChainTruthResult } from "./onChainTruth.js";
import type { VerificationResult } from "../tokenRegistry.js";

export type TokenValidationStatus =
  | "INVALID"
  | "VERIFIED"
  | "VIABLE"
  | "LOW_LIQUIDITY"
  | "ILLIQUID"
  | "NO_DATA"
  | "PENDING";

export interface FinalScoreResult {
  status: TokenValidationStatus;
  /** 0-100 composite. UI colour thresholds: ≥70 green, 50-69 yellow, <50 red. */
  score: number;
  /** Per-signal breakdown for tooltip rendering. */
  reasons: Array<{ key: string; delta: number; note: string }>;
}

const LIQUIDITY_VIABLE_USD = 5_000;
const LIQUIDITY_LOW_USD    = 100;
const VOLUME_VIABLE_24H_USD = 1_000;
const WASH_RATIO_MAX        = 100;  // volume/liquidity ratio above which we flag wash

export function composeFinalScore(
  onChain: OnChainTruthResult,
  liquidity: LiquidityRealityResult,
  registry: VerificationResult,
): FinalScoreResult {
  const reasons: Array<{ key: string; delta: number; note: string }> = [];
  let score = 0;

  // ── INVALID short-circuit ────────────────────────────────────────────────
  // Contract doesn't exist or isn't an ERC-20. Don't even compute liquidity
  // scoring; the wire shape will mark INVALID and the operator should never
  // see the token in tradeable rows.
  if (onChain.ran && !onChain.error) {
    if (!onChain.is_contract) {
      return {
        status: "INVALID",
        score: 0,
        reasons: [{ key: "not-a-contract", delta: 0, note: "address has no deployed bytecode" }],
      };
    }
    if (onChain.standard !== "ERC20") {
      return {
        status: "INVALID",
        score: 0,
        reasons: [
          {
            key: "not-erc20",
            delta: 0,
            note: `contract did not respond to all ERC-20 standard calls (name/symbol/decimals/totalSupply)`,
          },
        ],
      };
    }
    score += 15;
    reasons.push({ key: "exists-onchain", delta: 15, note: "contract bytecode present" });
    score += 10;
    reasons.push({ key: "erc20-abi", delta: 10, note: "responds to ERC-20 standard calls" });
  } else if (!onChain.ran || onChain.error) {
    // On-chain validator couldn't run. We can still score from registry +
    // liquidity, but don't add the on-chain points.
    reasons.push({ key: "onchain-unavailable", delta: 0, note: onChain.error_message ?? "validator skipped" });
  }

  // ── Registry verification ────────────────────────────────────────────────
  if (registry.verified) {
    score += 30;
    reasons.push({
      key: "registry-verified",
      delta: 30,
      note: `in ${registry.source ?? "registry"} (canonical: ${registry.registry_name ?? registry.registry_symbol})`,
    });
  } else if (registry.notes.includes("symbol-mismatch")) {
    // Critical penalty — impersonation attempt.
    score = Math.max(0, score - 25);
    reasons.push({
      key: "symbol-mismatch",
      delta: -25,
      note: `IMPERSONATION RISK — claims symbol does not match registry's canonical symbol "${registry.registry_symbol}"`,
    });
  }

  // ── Liquidity reality ────────────────────────────────────────────────────
  let liquidityTier: "viable" | "low" | "illiquid" | "no-data" = "no-data";
  if (liquidity.ran && !liquidity.error && !liquidity.no_data) {
    const liq = liquidity.liquidity_usd;
    if (liq != null && liq >= LIQUIDITY_VIABLE_USD) {
      liquidityTier = "viable";
      score += 25;
      reasons.push({ key: "liquidity-viable", delta: 25, note: `liquidity_usd=$${liq.toFixed(2)} ≥ $${LIQUIDITY_VIABLE_USD}` });
    } else if (liq != null && liq >= LIQUIDITY_LOW_USD) {
      liquidityTier = "low";
      score += 10;
      reasons.push({ key: "liquidity-low", delta: 10, note: `liquidity_usd=$${liq.toFixed(2)} below trading threshold` });
    } else if (liq != null) {
      liquidityTier = "illiquid";
      reasons.push({ key: "liquidity-illiquid", delta: 0, note: `liquidity_usd=$${liq.toFixed(2)} essentially zero` });
    }

    const vol = liquidity.volume_24h_usd;
    if (vol != null && vol >= VOLUME_VIABLE_24H_USD) {
      score += 10;
      reasons.push({ key: "volume-active", delta: 10, note: `volume_24h_usd=$${vol.toFixed(2)} ≥ $${VOLUME_VIABLE_24H_USD}` });
    }
    if (liquidity.pair_count >= 2) {
      score += 5;
      reasons.push({ key: "multi-pair", delta: 5, note: `${liquidity.pair_count} pairs indexed` });
    }
    // Wash-trade detection: volume disproportionate to liquidity is a
    // classic scam signature. SCFN's $4446 volume on $0.01 liquidity
    // gives a ratio of 444,600 — far above the threshold.
    if (vol != null && liq != null && liq > 0) {
      const ratio = vol / liq;
      if (ratio > WASH_RATIO_MAX) {
        const penalty = Math.min(15, Math.floor(ratio / WASH_RATIO_MAX) * 5);
        score = Math.max(0, score - penalty);
        reasons.push({
          key: "wash-trade-signature",
          delta: -penalty,
          note: `volume/liquidity ratio = ${ratio.toFixed(0)}× (above ${WASH_RATIO_MAX}× → likely wash trading)`,
        });
      } else {
        score += 5;
        reasons.push({ key: "healthy-vol-liq-ratio", delta: 5, note: `volume/liquidity = ${ratio.toFixed(1)}×` });
      }
    }
  } else if (liquidity.ran && liquidity.no_data) {
    reasons.push({ key: "no-dex-data", delta: 0, note: "DEX Screener has no pair indexed" });
  } else {
    reasons.push({ key: "liquidity-unavailable", delta: 0, note: liquidity.error_message ?? "validator skipped" });
  }

  // Clamp score [0, 100].
  score = Math.max(0, Math.min(100, score));

  // ── Status decision ─────────────────────────────────────────────────────
  // Priority: registry-verified > viable > low > illiquid > no-data > pending.
  let status: TokenValidationStatus;
  if (registry.verified) {
    status = "VERIFIED";
  } else if (liquidityTier === "viable") {
    status = "VIABLE";
  } else if (liquidityTier === "low") {
    status = "LOW_LIQUIDITY";
  } else if (liquidityTier === "illiquid") {
    status = "ILLIQUID";
  } else if (liquidity.ran && liquidity.no_data) {
    status = "NO_DATA";
  } else if (!liquidity.ran && !onChain.ran) {
    status = "PENDING";
  } else if (onChain.ran && onChain.is_contract && onChain.standard === "ERC20") {
    // On-chain confirms it's a real ERC-20 but no market data → NO_DATA
    // (we can't conclude ILLIQUID without evidence of zero liquidity).
    status = "NO_DATA";
  } else {
    status = "PENDING";
  }

  return { status, score, reasons };
}
