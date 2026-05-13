/**
 * BayesianToxicityEngine — order-flow toxicity scoring via Beta-likelihood
 * Bayesian update.
 *
 * Replaces the orphan `backend/prioritization-spine/src/engine.ts` (placed
 * inside a Rust crate by mistake, with a multiplicative "likelihood" that
 * did not normalize to a valid posterior). This implementation is a
 * proper Bayesian inference engine:
 *
 *   posterior(toxic | r) =
 *      L_toxic(r) × prior(toxic)
 *      ─────────────────────────────────────────────
 *      L_toxic(r) × prior(toxic) + L_safe(r) × (1 − prior(toxic))
 *
 * with L_hypothesis modelled as Beta(α, β) density at the observed revertRate
 * r ∈ [0, 1].  The normalization constant cancels in the posterior ratio so
 * we compute in unnormalized log-space to avoid underflow.
 *
 * The Beta shape parameters per hypothesis are caller-supplied (no
 * hardcoded "magic priors" — operators tune from telemetry). Defaults are
 * documented constants intended for cold-start only.
 */

/**
 * Beta distribution shape parameters. Constrain α, β > 0.
 * Mean of Beta(α, β) is α / (α + β). Peak (mode) for α, β > 1 is
 * (α − 1) / (α + β − 2).
 */
export interface BetaParams {
  alpha: number;
  beta: number;
}

export interface BayesianToxicityConfig {
  /** Initial prior probability of toxic flow ∈ [0, 1]. */
  prior: number;
  /** Beta-likelihood shape for the "toxic" hypothesis.
   *  Default mean ≈ 0.5 — toxic flow exhibits ~50% reverts on average. */
  likelihoodToxic?: BetaParams;
  /** Beta-likelihood shape for the "safe / uninformed" hypothesis.
   *  Default mean ≈ 0.2 — uninformed flow exhibits ~20% reverts on average. */
  likelihoodSafe?: BetaParams;
}

const DEFAULT_LIKELIHOOD_TOXIC: BetaParams = { alpha: 5, beta: 5 }; // mean 0.5
const DEFAULT_LIKELIHOOD_SAFE: BetaParams = { alpha: 2, beta: 8 }; // mean 0.2

/** Compute log(exp(a) + exp(b)) without intermediate overflow. */
function logSumExp(a: number, b: number): number {
  if (a > b) return a + Math.log1p(Math.exp(b - a));
  return b + Math.log1p(Math.exp(a - b));
}

function validateBeta(p: BetaParams, label: string): void {
  if (!Number.isFinite(p.alpha) || !Number.isFinite(p.beta) || p.alpha <= 0 || p.beta <= 0) {
    throw new Error(`BayesianToxicityEngine: ${label} requires α, β > 0 (got ${p.alpha}, ${p.beta})`);
  }
}

export class BayesianToxicityEngine {
  private posterior: number;
  private readonly toxic: BetaParams;
  private readonly safe: BetaParams;

  constructor(cfg: BayesianToxicityConfig) {
    if (!Number.isFinite(cfg.prior) || cfg.prior < 0 || cfg.prior > 1) {
      throw new Error(`BayesianToxicityEngine: prior must be in [0, 1] (got ${cfg.prior})`);
    }
    this.toxic = cfg.likelihoodToxic ?? DEFAULT_LIKELIHOOD_TOXIC;
    this.safe = cfg.likelihoodSafe ?? DEFAULT_LIKELIHOOD_SAFE;
    validateBeta(this.toxic, "likelihoodToxic");
    validateBeta(this.safe, "likelihoodSafe");
    this.posterior = cfg.prior;
  }

  /** Current posterior P(toxic). */
  public getToxicity(): number {
    return this.posterior;
  }

  /** Reset to a fresh prior (operator override or new window). */
  public reset(prior: number): void {
    if (!Number.isFinite(prior) || prior < 0 || prior > 1) {
      throw new Error(`BayesianToxicityEngine.reset: prior must be in [0, 1] (got ${prior})`);
    }
    this.posterior = prior;
  }

  /**
   * Update posterior given an observed revertRate ∈ [0, 1].
   *
   * Invalid inputs (NaN, Inf, out-of-range) return the current posterior
   * unchanged — never throw, never fabricate a value (R8 fail-honest).
   *
   * @returns the updated posterior, also stored internally.
   */
  public updateOnRevertRate(revertRate: number): number {
    if (!Number.isFinite(revertRate) || revertRate < 0 || revertRate > 1) {
      return this.posterior;
    }
    // Clamp away from {0, 1} to keep log(0) finite. Epsilon is small
    // enough that the clamp does not bias the result for non-degenerate
    // observations.
    const epsilon = 1e-9;
    const r = Math.max(epsilon, Math.min(1 - epsilon, revertRate));

    const logL = (params: BetaParams) =>
      (params.alpha - 1) * Math.log(r) + (params.beta - 1) * Math.log(1 - r);

    const logPriorToxic = Math.log(Math.max(epsilon, this.posterior));
    const logPriorSafe = Math.log(Math.max(epsilon, 1 - this.posterior));

    const logNumer = logL(this.toxic) + logPriorToxic;
    const logDenom = logSumExp(logNumer, logL(this.safe) + logPriorSafe);

    const next = Math.exp(logNumer - logDenom);
    // Guard against numerical edge cases — keep posterior in [0, 1].
    this.posterior = Math.min(1, Math.max(0, next));
    return this.posterior;
  }

  /**
   * Convenience: classify flow as toxic at a given decision threshold.
   * Returns the posterior alongside the boolean so callers see the
   * underlying score, never a stand-alone yes/no without context.
   */
  public classify(threshold: number): { toxic: boolean; posterior: number } {
    if (!Number.isFinite(threshold) || threshold < 0 || threshold > 1) {
      throw new Error(`classify: threshold must be in [0, 1] (got ${threshold})`);
    }
    return { toxic: this.posterior >= threshold, posterior: this.posterior };
  }
}
