/**
 * composite.ts — pure 11-signal anti-rug scorer (FASE 2).
 *
 * Deterministic, side-effect-free. Takes parsed GoPlus signals (+ optional
 * enrichment) and produces a 0..100 composite + SAFE/WARN/DROP classification.
 * The TS scorer is the canonical runtime path (consumed by the Rust pre-execute
 * checklist via the PG `token_safety_cache.safety_score` column).
 *
 * HARD GATES (any → score 0, classification DROP):
 *   - honeypot
 *   - fake_token
 *   - cannot_sell_all (honeypot-adjacent: can buy but can't sell)
 *   - buy_tax OR sell_tax > 20%
 *   - TVL < $10K (only enforceable when TVL is known — enrichment)
 *   - token_age < 1 day (only enforceable when age is known — enrichment)
 *
 * POSITIVE ACCUMULATION (max 100, before pausable/slippage penalties):
 *   open_source +20, ownership_safe +10, holder_count +10, concentration +10,
 *   liquidity_locked +10, tax_le_5pct +15, age_ge_7d +10, tvl_ge_50k +10
 *   = 95 base; +5 bonus if NOT slippage_modifiable (anti-manipulation).
 *
 * PENALTIES:
 *   transfer_pausable -20, slippage_modifiable -10 (cancels the bonus).
 *
 * CLASSIFICATION:
 *   score >= 75 → SAFE (admitted to live paths, paper-shadow doctrine still gates)
 *   50 <= score < 75 → WARN (paper-shadow ONLY — never live)
 *   score < 50 → DROP (excluded from the route graph entirely)
 *
 * FAIL-HONEST: signals that cannot be computed from GoPlus alone (token_age,
 * tvl_usd) arrive as `null` and are marked `enrichment_pending`. They contribute
 * 0 (NEVER fabricated) — a separate enrichment worker (block-age + DexScreener
 * TVL) is required to populate them. Until then the composite reflects only the
 * known signals, which is honest about what was verified.
 *
 * Doctrinal gates: arbx-token-safety-screen (this IS the scorer), arbx-mev-ethics
 * (only ethical arb on SAFE tokens), paper-trade-first (WARN never live).
 */

export type Classification = "SAFE" | "WARN" | "DROP";

/** Parsed, typed view of the signals the composite consumes. */
export interface CompositeInput {
  // GoPlus-derived (always available when GoPlus responds)
  is_honeypot: boolean;
  fake_token: boolean;
  cannot_sell_all: boolean;
  is_open_source: boolean;
  holder_count: number;
  top_holder_pct: number | null;     // from lp_holders; null if uncomputable
  liquidity_locked_days: number | null;
  ownership_safe: boolean;           // multi-owner OR renounced, AND no hidden_owner
  hidden_owner: boolean;
  buy_tax_pct: number | null;        // 0..100 (parsed to percent)
  sell_tax_pct: number | null;
  transfer_pausable: boolean;
  slippage_modifiable: boolean;      // true = slippage CAN be modified (risk)
  // Enrichment-required (null until a separate worker populates them)
  token_age_days: number | null;
  tvl_usd: number | null;
}

export interface SubSignals {
  is_honeypot: boolean;
  fake_token: boolean;
  cannot_sell_all: boolean;
  is_open_source: boolean;
  holder_count: number;
  top_holder_pct: number | null;
  liquidity_locked_days: number | null;
  ownership_safe: boolean;
  hidden_owner: boolean;
  buy_tax_pct: number | null;
  sell_tax_pct: number | null;
  transfer_pausable: boolean;
  slippage_modifiable: boolean;
  token_age_days: number | null;
  tvl_usd: number | null;
}

export interface CompositeResult {
  score: number;                     // 0..100 integer
  classification: Classification;
  sub_signals: SubSignals;
  hard_gate: string | null;          // which gate fired (null if none / not DROP-via-gate)
  enrichment_pending: string[];      // e.g. ["age","tvl"]
  paper_shadow_only: boolean;        // true for WARN (never live)
}

const SAFE_FLOOR = 75;
const WARN_FLOOR = 50;
const TAX_HARD_GATE_PCT = 20;
const TVL_HARD_GATE_USD = 10_000;
const TVL_FULL_USD = 50_000;
const AGE_HARD_GATE_DAYS = 1;
const AGE_FULL_DAYS = 7;

function drop(gate: string, enrichment: string[]): CompositeResult {
  return { score: 0, classification: "DROP", sub_signals: emptySub(), hard_gate: gate, enrichment_pending: enrichment, paper_shadow_only: false };
}
function emptySub(): SubSignals {
  return {
    is_honeypot: false, fake_token: false, cannot_sell_all: false, is_open_source: false,
    holder_count: 0, top_holder_pct: null, liquidity_locked_days: null,
    ownership_safe: false, hidden_owner: false, buy_tax_pct: null, sell_tax_pct: null,
    transfer_pausable: false, slippage_modifiable: false, token_age_days: null, tvl_usd: null,
  };
}

/**
 * The pure composite. Given a fully-parsed CompositeInput, returns the score +
 * classification + sub-signals (for cache). Deterministic: same input → same output.
 */
export function computeComposite(input: CompositeInput): CompositeResult {
  const enrichment: string[] = [];
  if (input.token_age_days === null) enrichment.push("age");
  if (input.tvl_usd === null) enrichment.push("tvl");

  // --- HARD GATES (any → DROP, score 0) ---
  if (input.is_honeypot) return drop("honeypot", enrichment);
  if (input.fake_token) return drop("fake_token", enrichment);
  if (input.cannot_sell_all) return drop("cannot_sell_all", enrichment);

  const buyTax = input.buy_tax_pct ?? 0;
  const sellTax = input.sell_tax_pct ?? 0;
  const maxTax = Math.max(buyTax, sellTax);
  if (maxTax > TAX_HARD_GATE_PCT) return drop("tax_gt_20_pct", enrichment);

  if (input.tvl_usd !== null && input.tvl_usd < TVL_HARD_GATE_USD) {
    return drop("tvl_lt_10k", enrichment);
  }
  if (input.token_age_days !== null && input.token_age_days < AGE_HARD_GATE_DAYS) {
    return drop("age_lt_1d", enrichment);
  }

  // --- POSITIVE ACCUMULATION ---
  let score = 0;
  if (input.is_open_source) score += 20;
  if (input.ownership_safe) score += 10;
  if (input.holder_count >= 100) score += 10;
  else if (input.holder_count >= 50) score += 5;
  if (input.top_holder_pct !== null) {
    if (input.top_holder_pct <= 20) score += 10;
    else if (input.top_holder_pct <= 40) score += 5;
  }
  if (input.liquidity_locked_days !== null && input.liquidity_locked_days >= 30) score += 10;
  // tax: scaled (≤5% → +15, ≤10% → +10, ≤20% → +5)
  if (maxTax <= 5) score += 15;
  else if (maxTax <= 10) score += 10;
  else if (maxTax <= 20) score += 5;
  if (input.token_age_days !== null) {
    if (input.token_age_days >= AGE_FULL_DAYS) score += 10;
    else if (input.token_age_days >= AGE_HARD_GATE_DAYS) score += 5;
  }
  if (input.tvl_usd !== null) {
    if (input.tvl_usd >= TVL_FULL_USD) score += 10;
    else if (input.tvl_usd >= TVL_HARD_GATE_USD) score += 5;
  }
  if (!input.slippage_modifiable) score += 5; // anti-manipulation bonus

  // --- PENALTIES ---
  if (input.transfer_pausable) score -= 20;
  if (input.slippage_modifiable) score -= 10; // cancels the bonus + extra risk

  score = Math.max(0, Math.min(100, Math.round(score)));
  const classification: Classification =
    score >= SAFE_FLOOR ? "SAFE" : score >= WARN_FLOOR ? "WARN" : "DROP";
  const paper_shadow_only = classification === "WARN";

  return {
    score,
    classification,
    sub_signals: {
      is_honeypot: input.is_honeypot,
      fake_token: input.fake_token,
      cannot_sell_all: input.cannot_sell_all,
      is_open_source: input.is_open_source,
      holder_count: input.holder_count,
      top_holder_pct: input.top_holder_pct,
      liquidity_locked_days: input.liquidity_locked_days,
      ownership_safe: input.ownership_safe,
      hidden_owner: input.hidden_owner,
      buy_tax_pct: input.buy_tax_pct,
      sell_tax_pct: input.sell_tax_pct,
      transfer_pausable: input.transfer_pausable,
      slippage_modifiable: input.slippage_modifiable,
      token_age_days: input.token_age_days,
      tvl_usd: input.tvl_usd,
    },
    hard_gate: classification === "DROP" && score === 0 ? "sub_50_no_gate" : null,
    enrichment_pending: enrichment,
    paper_shadow_only,
  };
}

// ---------------------------------------------------------------------------
// GoPlus raw-flags parser → CompositeInput.
// Defensive: any malformed field → null/false (fail-honest, never fabricate).
// ---------------------------------------------------------------------------

export type GoPlusFlags = Record<string, string | number | undefined>;

function flagIs1(flags: GoPlusFlags, key: string): boolean {
  return flags[key] === "1" || flags[key] === 1;
}

/** Parse a GoPlus tax value to a 0..100 percentage. Handles both "0.05" (decimal)
 * and "5" (already-pct) conventions. Returns null if unparseable. */
function parseTaxPct(raw: string | number | undefined): number | null {
  if (raw === undefined || raw === null || raw === "") return null;
  const n = typeof raw === "number" ? raw : Number(raw);
  if (!Number.isFinite(n) || n < 0) return null;
  return n <= 1 ? n * 100 : n;
}

/** Parse GoPlus lp_holders (a JSON string) → top_holder_pct + liquidity_locked_days.
 * Defensive: returns nulls if the field is absent/malformed. */
function parseLpHolders(
  raw: string | number | undefined,
  now: Date,
): { top_holder_pct: number | null; liquidity_locked_days: number | null } {
  if (typeof raw !== "string" || raw.length === 0) {
    return { top_holder_pct: null, liquidity_locked_days: null };
  }
  let holders: Array<{ address?: string; amount?: string; locked_amount?: string; locked_time?: number; ratio?: number }>;
  try {
    holders = JSON.parse(raw);
  } catch {
    return { top_holder_pct: null, liquidity_locked_days: null };
  }
  if (!Array.isArray(holders) || holders.length === 0) {
    return { top_holder_pct: null, liquidity_locked_days: null };
  }
  // top_holder_pct: prefer an explicit ratio field; else compute amount/sum.
  let top_holder_pct: number | null = null;
  if (typeof holders[0]!.ratio === "number") {
    top_holder_pct = holders[0]!.ratio * 100;
  } else {
    const amounts = holders
      .map((h) => Number(h.amount ?? "0"))
      .filter((a) => Number.isFinite(a) && a > 0);
    const total = amounts.reduce((s, a) => s + a, 0);
    if (total > 0 && amounts.length > 0) {
      top_holder_pct = (amounts[0]! / total) * 100;
    }
  }
  // liquidity_locked_days: max(locked_time - now) across holders, in days.
  let liquidity_locked_days: number | null = null;
  const nowSec = Math.floor(now.getTime() / 1000);
  for (const h of holders) {
    if (typeof h.locked_time === "number" && h.locked_time > nowSec) {
      const days = (h.locked_time - nowSec) / 86400;
      if (liquidity_locked_days === null || days > liquidity_locked_days) {
        liquidity_locked_days = days;
      }
    }
  }
  return { top_holder_pct, liquidity_locked_days };
}

/** Parse GoPlus raw flags into a typed CompositeInput. token_age_days + tvl_usd
 * are LEFT null (enrichment-pending) — GoPlus alone cannot compute them
 * (age needs current-block + chain block-time; TVL needs DexScreener/market data). */
export function parseGoPlusFlags(flags: GoPlusFlags, now: Date = new Date()): CompositeInput {
  const lp = parseLpHolders(flags["lp_holders"], now);
  const isMultiOwner = flagIs1(flags, "is_owner_address_multi_token_mode");
  const hiddenOwner = flagIs1(flags, "hidden_owner");
  // ownership_safe: multi-owner mode is the strongest positive signal we can read.
  // (Renunciation isn't a direct GoPlus flag; if it becomes available, OR it in.)
  const ownership_safe = isMultiOwner && !hiddenOwner;
  return {
    is_honeypot: flagIs1(flags, "is_honeypot"),
    fake_token: flagIs1(flags, "fake_token") || flagIs1(flags, "is_fake_token"),
    cannot_sell_all: flagIs1(flags, "cannot_sell_all"),
    is_open_source: flagIs1(flags, "is_open_source"),
    holder_count: Number(flags["holder_count"] ?? 0),
    top_holder_pct: lp.top_holder_pct,
    liquidity_locked_days: lp.liquidity_locked_days,
    ownership_safe,
    hidden_owner: hiddenOwner,
    buy_tax_pct: parseTaxPct(flags["buy_tax"]),
    sell_tax_pct: parseTaxPct(flags["sell_tax"]),
    transfer_pausable: flagIs1(flags, "transfer_pausable"),
    slippage_modifiable: flagIs1(flags, "slippage_modifiable"),
    token_age_days: null, // enrichment-pending (block-age worker)
    tvl_usd: null,        // enrichment-pending (DexScreener/market worker)
  };
}
