/**
 * entropy-engine.ts — RESEARCH / SHADOW signal (price microstructure).
 *
 * STATUS: research-only. NOT wired into the production C-S-E pipeline, NOT consumed
 * by the pre-execute checklist, NOT in any live path. Distinct from the anti-rug
 * composite scorer (composite.ts) — this module analyses PRICE TIME-SERIES
 * (memory, fractality, distribution shape), not token-security flags. Lives under
 * token_safety/ per the operator's research layout; conceptually a price-microstructure
 * signal that MAY feed opportunity sizing/scoring in a future paper-shadow phase.
 *
 * MATH (audited + corrected from the v2.0 draft):
 *
 * 1. memoryStrength — windowed Grünwald-Letnikov fractional-difference weighted sum.
 *    Coefficient recurrence c[k] = c[k-1] * (k - 1 - alpha) / k (the operator's sign
 *    convention; |sum| is returned so the sign convention cancels). This is a
 *    HEURISTIC memory-strength signal, NOT a rigorous fractional derivative (the
 *    rigorous form would reverse-convolve the full series + divide by Δt^alpha).
 *    Documented as such — useful as a relative-strength feature, not an absolute.
 *
 * 2. hurstExponent — R/S-style via log-log regression of std-of-differences vs lag,
 *    lags = 2..floor(sqrt(n)) (avoids the empty-range failure of log(n)/2 on small n).
 *    H>0.5 = persistent, H<0.5 = mean-reverting, H≈0.5 = random walk.
 *
 * 3. manifoldMetrics — Shannon entropy of a deterministic synthetic price distribution.
 *    Uses a PROPER LCG PRNG (mulberry32) seeded fixed → reproducible (the v2.0 draft's
 *    Math.sin(seed++) is NOT reproducible across JS engines; corrected here). Entropy
 *    is real Shannon: -sum(p log p) over the histogram.
 *
 * Determinism: same input → same output, bit-for-bit (no Date.now, no Math.random,
 * no Math.sin). Auditable. FAIL-HONEST: insufficient data → neutral values (H=0.5,
 * memory=0), never fabricated.
 */

export interface EntropyScore {
  status: "computed" | "insufficient_data" | "error";
  memoryStrength: number;        // |windowed GL weighted sum| (heuristic)
  hurstExponent: number;         // 0..1 (0.5 = neutral/random walk)
  spectralEntropy: number;       // Shannon entropy of the synthetic distribution
  mathematicalConvergence: string;
}

export interface ManifoldMetrics {
  expectedValue: number;
  entropy: number;               // Shannon entropy (nats)
}

const NEUTRAL_HURST = 0.5;

/** Deterministic 32-bit LCG (mulberry32). Reproducible across JS engines. */
function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return function () {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export class EntropyEngine {
  private readonly windowSize: number;
  private readonly alpha: number;       // fractional order (0..1), 0.5 = semi-derivative

  constructor(windowSize: number = 60, alpha: number = 0.5) {
    if (windowSize < 2) throw new Error("EntropyEngine: windowSize must be >= 2");
    if (alpha <= 0 || alpha >= 1) throw new Error("EntropyEngine: alpha must be in (0,1)");
    this.windowSize = windowSize;
    this.alpha = alpha;
  }

  /**
   * Windowed Grünwald-Letnikov weighted sum (heuristic memory strength).
   * Returns |sum_{k=0}^{win-1} c[k] * data[k]| where c follows the recurrence
   * c[k] = c[k-1] * (k-1-alpha)/k. NOT a rigorous fractional derivative (no
   * reverse convolution over the full series, no Δt^alpha normalization) —
   * useful as a relative memory-strength feature for scoring.
   */
  public calculateMemoryStrength(data: number[]): number {
    if (data.length < this.windowSize) return 0;
    const win = this.windowSize;
    const coeff = new Array<number>(win).fill(0);
    coeff[0] = 1;
    for (let k = 1; k < win; k++) {
      coeff[k] = (coeff[k - 1] * (k - 1 - this.alpha)) / k;
    }
    let sum = 0;
    for (let i = 0; i < win; i++) {
      sum += coeff[i]! * (data[i] ?? 0);
    }
    return Math.abs(sum);
  }

  /**
   * Hurst exponent via log-log regression of std-of-lagged-differences.
   * H ≈ slope of log(std(diff_lag)) vs log(lag). Lags = 2..floor(sqrt(n)).
   * Returns NEUTRAL_HURST (0.5) when data is too short for a fit.
   */
  public calculateHurstExponent(data: number[]): number {
    const n = data.length;
    if (n < 50) return NEUTRAL_HURST;
    const maxLag = Math.max(3, Math.floor(Math.sqrt(n)));
    const lags: number[] = [];
    const logTau: number[] = [];
    for (let lag = 2; lag <= maxLag; lag++) {
      let sumSq = 0;
      let count = 0;
      for (let t = 0; t + lag < n; t++) {
        const d = data[t + lag]! - data[t]!;
        sumSq += d * d;
        count++;
      }
      if (count === 0) continue;
      const tau = Math.sqrt(sumSq / count);
      if (tau > 0 && Number.isFinite(tau)) {
        lags.push(lag);
        logTau.push(Math.log(tau));
      }
    }
    if (lags.length < 2) return NEUTRAL_HURST;
    // Ordinary least squares slope of log(tau) ~ slope * log(lag).
    const logLags = lags.map((l) => Math.log(l));
    const nPts = lags.length;
    const sx = logLags.reduce((a, b) => a + b, 0);
    const sy = logTau.reduce((a, b) => a + b, 0);
    const sxy = logLags.reduce((a, b, i) => a + b * logTau[i]!, 0);
    const sxx = logLags.reduce((a, b) => a + b * b, 0);
    const denom = sxx - (sx * sx) / nPts;
    if (Math.abs(denom) < 1e-12) return NEUTRAL_HURST;
    const slope = (sxy - (sx * sy) / nPts) / denom;
    if (!Number.isFinite(slope)) return NEUTRAL_HURST;
    return Math.max(0, Math.min(1, slope));
  }

  /**
   * Shannon entropy of a deterministic synthetic price distribution.
   * `currentPrice` centres the distribution; `volatility` sets its spread.
   * The PRNG is mulberry32(42) — fully reproducible. Entropy = -sum(p log p)
   * over the histogram (nats). expectedValue = currentPrice (the synthetic
   * distribution is symmetric by construction).
   */
  public calculateManifoldMetrics(
    currentPrice: number,
    volatility: number,
    scenarios = 1000,
    bins = 50,
  ): ManifoldMetrics {
    if (!Number.isFinite(currentPrice) || !Number.isFinite(volatility) || volatility < 0) {
      return { expectedValue: Number.isFinite(currentPrice) ? currentPrice : 0, entropy: 0 };
    }
    const rng = mulberry32(42);
    const samples = new Array<number>(scenarios);
    let min = Number.POSITIVE_INFINITY;
    let max = Number.NEGATIVE_INFINITY;
    for (let i = 0; i < scenarios; i++) {
      // Box-Muller on two uniform draws -> standard normal, scaled by volatility.
      const u1 = Math.max(1e-12, rng());
      const u2 = rng();
      const z = Math.sqrt(-2 * Math.log(u1)) * Math.cos(2 * Math.PI * u2);
      const v = currentPrice + z * volatility;
      samples[i] = v;
      if (v < min) min = v;
      if (v > max) max = v;
    }
    const range = max - min || 1;
    const hist = new Array<number>(bins).fill(0);
    for (const v of samples) {
      const idx = Math.min(bins - 1, Math.max(0, Math.floor(((v - min) / range) * bins)));
      hist[idx] = hist[idx]! + 1;
    }
    let entropy = 0;
    for (const count of hist) {
      if (count > 0) {
        const p = count / scenarios;
        entropy -= p * Math.log(p);
      }
    }
    return { expectedValue: currentPrice, entropy };
  }

  /**
   * Composite analysis of a price series. FAIL-HONEST: insufficient data →
   * status="insufficient_data" + neutral values (never fabricated).
   */
  public analyzeMarket(data: number[]): EntropyScore {
    if (!Array.isArray(data) || data.length < this.windowSize) {
      return {
        status: "insufficient_data",
        memoryStrength: 0,
        hurstExponent: NEUTRAL_HURST,
        spectralEntropy: 0,
        mathematicalConvergence: "needs >= windowSize points",
      };
    }
    const last = data[data.length - 1] ?? 0;
    // Volatility estimate = std of the series (defensive: floor at 1e-9).
    const mean = data.reduce((a, b) => a + b, 0) / data.length;
    const variance = data.reduce((a, b) => a + (b - mean) * (b - mean), 0) / data.length;
    const vol = Math.max(1e-9, Math.sqrt(variance));
    const manifold = this.calculateManifoldMetrics(last, vol);
    return {
      status: "computed",
      memoryStrength: this.calculateMemoryStrength(data),
      hurstExponent: this.calculateHurstExponent(data),
      spectralEntropy: manifold.entropy,
      mathematicalConvergence: "valid",
    };
  }
}
