/**
 * Operator Risk Presets — posture templates that resolve to the 7 mandatory
 * risk-limit dimensions (arbx-risk-limits-enforcement) from operator-supplied
 * capital. This is the logic core behind the /operator presets UI.
 *
 * DOCTRINE THIS FILE OBEYS
 * ────────────────────────
 * • No-hardcode (RULE 00 + arbx-no-hardcode-doctrine): a preset NEVER carries an
 *   absolute USD cap. It carries *fractions* and *bounded knobs*. The absolute
 *   numbers are computed from `base.capitalUsd`, which is the operator's own
 *   `trading_config.capital_usd`. The operator still owns every dollar figure.
 * • Risk-limits (arbx-risk-limits-enforcement): every resolved proposal carries
 *   ALL 7 dimensions — daily-loss, per-token, per-pool, per-dex, per-chain,
 *   per-strategy, max-consecutive-losses. Omitting any is a defect, not a knob.
 * • Fail-honest (R8): capitalUsd <= 0 → every cap resolves to 0 with an explicit
 *   warning. We never invent capital to make a preset "look" usable.
 * • Ethics (arbx-mev-ethics-gate): presets only enable non-predatory strategy
 *   kinds. There is no sandwich / user-frontrun posture, at any aggression level.
 * • Purity: resolve* is a pure function (no Date.now/Math.random/IO) so it is
 *   deterministic and unit-testable, and safe to call during SSR (R1).
 *
 * A resolved posture is a *proposal* the operator reviews and confirms before it
 * is PUT to trading-config / circuit-breakers. This module writes nothing.
 */

export type RiskPostureId = "conservative" | "balanced" | "aggressive";

/**
 * Non-predatory strategy kinds a preset may enable. Mirrors the PERMITIDO list
 * in arbx-mev-ethics-gate. Deliberately excludes sandwich / generalized-frontrun
 * / oracle-nudge — those are PROHIBIDO and have no preset representation.
 */
export type AllowedStrategyKind =
  | "cross_pool_arb" // restore price parity between pools (organic divergence)
  | "triangular_arb" // holonomic loop within a single venue
  | "backrun_residual" // residual arb AFTER a user swap already settled
  | "permissioned_liquidation"; // Aave/Compound/Maker — protocol-invited

/**
 * A posture template. All capital-relative quantities are FRACTIONS in [0,1] of
 * the operator's deployed capital — never absolute USD. Knobs are bounded.
 */
export interface RiskPosture {
  id: RiskPostureId;
  label: string;
  /** One-line, risk-framed. No win-rate, no return promise. */
  description: string;

  /** Fraction of `base.capitalUsd` this posture is willing to deploy at all. */
  capitalDeploymentFraction: number;

  /** Caps as a fraction of *deployed* capital (capitalUsd * deploymentFraction). */
  perTokenCapFraction: number;
  perPoolCapFraction: number;
  perDexCapFraction: number;
  perChainCapFraction: number;
  perStrategyCapFraction: number;

  /** Daily realized+unrealized+gas loss cap as a fraction of deployed capital. */
  dailyLossCapFraction: number;

  /** Consecutive failed txs before automatic halt. Integer >= 1. */
  maxConsecutiveLosses: number;

  /** Bounded execution knobs (mapped to existing trading_config fields). */
  maxSlippagePct: number; // e.g. 0.3 == 0.3%
  minRoiPct: number; // reject opportunities below this net ROI

  /** Non-predatory strategy kinds this posture turns on. */
  enabledStrategyKinds: AllowedStrategyKind[];

  /**
   * Recommended minimum operator capital for this posture to be meaningful.
   * This is a UX guard-rail (the resolver emits a warning below it), NOT an
   * invented operational figure — the operator may proceed anyway. Overridable.
   */
  recommendedMinCapitalUsd: number;
}

/**
 * The three canonical postures. Ordering invariants (asserted by tests):
 *   exposure  (deployment & cap fractions):  conservative < balanced < aggressive
 *   safety    (dailyLossCap, slippage, ROI floor): conservative is strictest
 *   strategy breadth: conservative ⊆ balanced ⊆ aggressive
 * No posture enables a predatory strategy kind at any level.
 */
export const RISK_POSTURES: Readonly<Record<RiskPostureId, RiskPosture>> = Object.freeze({
  conservative: {
    id: "conservative",
    label: "Conservative",
    description:
      "Smallest capital at risk, tightest loss cap, only the lowest-variance " +
      "strategies. Designed to bound the worst day, not to chase the best one.",
    capitalDeploymentFraction: 0.1,
    perTokenCapFraction: 0.2,
    perPoolCapFraction: 0.15,
    perDexCapFraction: 0.35,
    perChainCapFraction: 0.5,
    perStrategyCapFraction: 0.4,
    dailyLossCapFraction: 0.02,
    maxConsecutiveLosses: 3,
    maxSlippagePct: 0.3,
    minRoiPct: 0.5,
    enabledStrategyKinds: ["cross_pool_arb", "permissioned_liquidation"],
    recommendedMinCapitalUsd: 100,
  },
  balanced: {
    id: "balanced",
    label: "Balanced",
    description:
      "Moderate capital at risk across the non-predatory strategy set, with a " +
      "loss cap that halts the day well before it becomes existential.",
    capitalDeploymentFraction: 0.25,
    perTokenCapFraction: 0.3,
    perPoolCapFraction: 0.25,
    perDexCapFraction: 0.5,
    perChainCapFraction: 0.7,
    perStrategyCapFraction: 0.6,
    dailyLossCapFraction: 0.05,
    maxConsecutiveLosses: 5,
    maxSlippagePct: 0.5,
    minRoiPct: 0.3,
    enabledStrategyKinds: ["cross_pool_arb", "triangular_arb", "backrun_residual", "permissioned_liquidation"],
    recommendedMinCapitalUsd: 500,
  },
  aggressive: {
    id: "aggressive",
    label: "Aggressive",
    description:
      "Largest capital deployment and widest strategy set, still strictly " +
      "non-predatory. Higher variance — the loss cap and consecutive-loss halt " +
      "are the only thing between a bad regime and a bad day.",
    capitalDeploymentFraction: 0.5,
    perTokenCapFraction: 0.4,
    perPoolCapFraction: 0.35,
    perDexCapFraction: 0.65,
    perChainCapFraction: 0.85,
    perStrategyCapFraction: 0.8,
    dailyLossCapFraction: 0.1,
    maxConsecutiveLosses: 8,
    maxSlippagePct: 1.0,
    minRoiPct: 0.15,
    enabledStrategyKinds: ["cross_pool_arb", "triangular_arb", "backrun_residual", "permissioned_liquidation"],
    recommendedMinCapitalUsd: 2000,
  },
});

/** Mandatory disclaimer the UI MUST render next to any preset. No performance claim. */
export const RISK_DISCLAIMER =
  "Presets configure RISK LIMITS, not returns. They bound how much you can lose, " +
  "not what you can earn. No outcome is implied or guaranteed. Past paper-trade " +
  "results do not predict future results. Capital exposed in paper mode is 0.";

export interface PostureResolutionInput {
  /** Operator's configured capital for the chain (trading_config.capital_usd). */
  capitalUsd: number;
}

/**
 * The 7 mandatory risk dimensions, resolved to absolute figures, plus the
 * bounded execution knobs and the strategy allowlist. This is the proposal
 * shape an operator confirms before it is persisted.
 */
export interface ResolvedRiskLimits {
  postureId: RiskPostureId;
  deployedCapitalUsd: number;

  // ── 7 mandatory dimensions ────────────────────────────────────────────────
  dailyLossCapUsd: number;
  perTokenCapUsd: number;
  perPoolCapUsd: number;
  perDexCapUsd: number;
  perChainCapUsd: number;
  perStrategyCapUsd: number;
  maxConsecutiveLosses: number;

  // ── Bounded execution knobs (map to existing trading_config fields) ────────
  maxSlippagePct: number;
  minRoiPct: number;
  enabledStrategyKinds: AllowedStrategyKind[];

  /** Non-fatal advisories (e.g. capital below the posture's recommended floor). */
  warnings: string[];
}

function round2(n: number): number {
  return Math.round(n * 100) / 100;
}

/**
 * Resolve a posture against operator-supplied capital. Pure & deterministic.
 *
 * Fail-honest: capitalUsd <= 0 (or non-finite) → all caps 0 + a warning. We do
 * not fabricate capital. The operator sees "configure capital first", not a
 * usable-looking-but-fake limit set.
 */
export function resolvePosture(
  postureId: RiskPostureId,
  input: PostureResolutionInput,
): ResolvedRiskLimits {
  const p = RISK_POSTURES[postureId];
  const warnings: string[] = [];

  const capital = Number.isFinite(input.capitalUsd) ? input.capitalUsd : 0;
  if (!Number.isFinite(input.capitalUsd)) {
    warnings.push("capital_usd is not a finite number; treated as 0 (configure trading_config first).");
  }
  if (capital <= 0) {
    warnings.push("No operator capital configured (capital_usd <= 0); every cap resolves to 0.");
    return {
      postureId,
      deployedCapitalUsd: 0,
      dailyLossCapUsd: 0,
      perTokenCapUsd: 0,
      perPoolCapUsd: 0,
      perDexCapUsd: 0,
      perChainCapUsd: 0,
      perStrategyCapUsd: 0,
      maxConsecutiveLosses: p.maxConsecutiveLosses,
      maxSlippagePct: p.maxSlippagePct,
      minRoiPct: p.minRoiPct,
      enabledStrategyKinds: [...p.enabledStrategyKinds],
      warnings,
    };
  }

  if (capital < p.recommendedMinCapitalUsd) {
    warnings.push(
      `Capital $${round2(capital)} is below the ${p.label} posture's recommended ` +
        `minimum of $${p.recommendedMinCapitalUsd}. Caps will be small; proceed only if intended.`,
    );
  }

  const deployed = capital * p.capitalDeploymentFraction;

  return {
    postureId,
    deployedCapitalUsd: round2(deployed),
    dailyLossCapUsd: round2(deployed * p.dailyLossCapFraction),
    perTokenCapUsd: round2(deployed * p.perTokenCapFraction),
    perPoolCapUsd: round2(deployed * p.perPoolCapFraction),
    perDexCapUsd: round2(deployed * p.perDexCapFraction),
    perChainCapUsd: round2(deployed * p.perChainCapFraction),
    perStrategyCapUsd: round2(deployed * p.perStrategyCapFraction),
    maxConsecutiveLosses: p.maxConsecutiveLosses,
    maxSlippagePct: p.maxSlippagePct,
    minRoiPct: p.minRoiPct,
    enabledStrategyKinds: [...p.enabledStrategyKinds],
    warnings,
  };
}

/** Ordered list for rendering (conservative → aggressive). */
export const RISK_POSTURE_ORDER: readonly RiskPostureId[] = [
  "conservative",
  "balanced",
  "aggressive",
] as const;
