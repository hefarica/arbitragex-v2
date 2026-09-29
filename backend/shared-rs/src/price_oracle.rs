//! PriceOracle — per-token USD valuation for the spine evaluator.
//!
//! Replaces the buggy `TradingConfigState::profit_token_to_usd(amount)` which
//! treated every token as base-token-priced (BUG-2, see anti_reincidencia.md
//! Incidente #7). The oracle resolves prices by token symbol with two
//! priority tiers, then fails honestly:
//!
//!   1. Match `config.base_token_symbol` (case-insensitive) → `base_token_price_usd`
//!   2. Lookup in `config.token_prices_usd` map (operator-managed override)
//!   3. Otherwise → `None` (caller MUST reject — no fabricated default, and
//!      NO stablecoin $1.00 shortcut: stables resolve through the same live
//!      cascade as everything else, WO-PC4)
//!
//! The trait abstraction lets future implementations swap to Chainlink,
//! TWAP, or external feeds without touching callers in the spine.
//!
//! Doctrine: R8 Fail-Honest. An unknown token returns `None` so the spine
//! rejects the opportunity with `RejectReason::UnknownTokenPrice` instead
//! of inventing a price. Operator sees the gap explicitly via dashboard /
//! heartbeat counters and populates `token_prices_usd` to close it.

use crate::trading_config::TradingConfigState;
use std::collections::HashMap;

/// Widely-trusted stablecoins. NO default $1.00 is attached to membership
/// (WO-PC4); classification only drives staleness windows and risk filters.
/// Matching is case-insensitive. The set itself lives in ONE place —
/// `chains::STABLECOINS_MAINNET` — and this fn delegates to
/// `chains::is_stablecoin_symbol` (STABLEARR-11: no scattered per-site
/// stablecoin enumerations).
///
/// Selection criteria (locked-in 2026-05-05 — operator review required to
/// add/remove):
///   - Backed by reserves OR overcollateralized (no algorithmic stables).
///   - >$50M circulating supply at time of inclusion.
///   - Documented redemption mechanism.
///
/// Excluded from defaults — operator must set explicit price if they want to
/// trade these:
///   - USDe (Ethena, algorithmic delta-neutral, depeg episodes)
///   - MIM (Magic Internet Money, exploit history)
///   - crvUSD, GHO (newer; operator should review trust manually)
pub fn is_known_stablecoin(symbol_upper: &str) -> bool {
    crate::chains::is_stablecoin_symbol(symbol_upper)
}

/// Per-token USD price resolution. `None` means the token is outside the
/// operator's universe — callers MUST treat this as a hard reject reason
/// (see `RejectReason::UnknownTokenPrice`) rather than substituting a default.
pub trait PriceOracle {
    /// Returns USD price PER UNIT of the token (multiply by token amount to
    /// get total USD value). `token_id` may be a symbol or hex address;
    /// implementations should normalize internally.
    fn price_usd(&self, token_id: &str) -> Option<f64>;
}

/// Default oracle backed by `TradingConfigState`. Resolves base token, then
/// operator-supplied token prices. Returns `None` for any unknown symbol or
/// hex address — stablecoins included (no $1.00 default, WO-PC4).
pub struct ConfigPriceOracle<'a> {
    config: &'a TradingConfigState,
}

impl<'a> ConfigPriceOracle<'a> {
    pub fn new(config: &'a TradingConfigState) -> Self {
        Self { config }
    }
}

impl<'a> PriceOracle for ConfigPriceOracle<'a> {
    fn price_usd(&self, token_id: &str) -> Option<f64> {
        let upper = token_id.trim().to_ascii_uppercase();
        if upper.is_empty() {
            return None;
        }

        // Tier 1 — base token symbol (operator's reference asset).
        if upper == self.config.base_token_symbol.to_ascii_uppercase() {
            return Some(self.config.base_token_price_usd);
        }

        // Tier 2 — operator-managed map. Case-insensitive lookup; the operator
        // may store keys in any case ("WBTC", "wbtc", "WbTc") — all match.
        // Stables resolve here too when the operator supplies a price — they
        // are NEVER hardcoded (WO-PC4): a stable pegged at $0.97 must be
        // rejected/priced as such, not silently served as $1.00.
        for (sym, price) in self.config.token_prices_usd.iter() {
            if sym.to_ascii_uppercase() == upper {
                return Some(*price);
            }
        }

        // Tier 3 — fail-honest. No fabricated default (not even for stables).
        None
    }
}

/// Composes multiple `PriceOracle` implementations into a single cascade.
/// Tries each child oracle in declaration order; returns the first `Some(_)`
/// or `None` if every child misses. Empty / whitespace `token_id` short-
/// circuits to `None` so child oracles never see degenerate input.
///
/// Production layout (after Sprint X):
///   1. `RedisCachedPriceOracle` — sub-ms snapshot of `arbx:token_prices:1`,
///      populated every 30s by `searcher-rs::workers::price_worker` from
///      Alchemy Token Prices + Coingecko fallback.
///   2. `ConfigPriceOracle` — operator's manual overrides + base token.
///      Last-resort fallback when no live source has a price (e.g. brand-new
///      token, both feeds down, boot before first worker tick). Stablecoins
///      are NOT special-cased here (WO-PC4).
///   3. (no third tier) — falls through to `None` → `RejectReason::UnknownTokenPrice`.
///
/// **Doctrine**: cascading does NOT hide failure. If every tier returns
/// `None` the cascade returns `None` and the spine rejects fail-honestly.
/// We never substitute a "best guess" or stale value (R8).
///
/// The `'a` lifetime allows borrowed inner oracles (e.g. `ConfigPriceOracle<'a>`
/// borrowing `&'a TradingConfigState`). For owned-only cascades use `'static`.
pub struct CascadePriceOracle<'a> {
    oracles: Vec<Box<dyn PriceOracle + Send + Sync + 'a>>,
}

impl<'a> CascadePriceOracle<'a> {
    /// Build a cascade from oracles in priority order (highest first).
    /// Empty vec is permitted but produces an oracle that always returns
    /// `None` — useful as a degenerate placeholder during boot.
    pub fn new(oracles: Vec<Box<dyn PriceOracle + Send + Sync + 'a>>) -> Self {
        Self { oracles }
    }
}

impl<'a> PriceOracle for CascadePriceOracle<'a> {
    fn price_usd(&self, token_id: &str) -> Option<f64> {
        // Short-circuit on empty input. Matches `ConfigPriceOracle` semantics
        // and avoids inner oracles having to re-validate.
        if token_id.trim().is_empty() {
            return None;
        }
        for oracle in &self.oracles {
            if let Some(p) = oracle.price_usd(token_id) {
                return Some(p);
            }
        }
        None
    }
}

/// Sync `PriceOracle` over a pre-fetched `HashMap<String, f64>` snapshot.
///
/// **Why "snapshot" not "client"**: the spine evaluator is sync (per-opportunity
/// hot path). Making it `async` to round-trip Redis on every lookup would
/// cascade refactors through `evaluate()` → `score()` → caller chains. Instead,
/// the spine fetches the latest snapshot ONCE per evaluation tick (or on a TTL
/// timer) via `snapshot_from_redis()` and queries it sync at micro-second
/// latency. Staleness ≤ refresh period (30s by default) — acceptable for
/// USD valuation since intra-block price movement is bounded.
///
/// The cache is populated asynchronously by
/// `searcher-rs::workers::price_worker` from Alchemy Token Prices API +
/// Coingecko fallback. Redis hash key: `arbx:token_prices:1`. Field key
/// = uppercase token symbol. Value = USD price as `f64`.
///
/// **R8 enforcement**: snapshot construction filters out non-finite (NaN /
/// Inf) and non-positive prices so the spine never multiplies by garbage.
/// External feeds occasionally emit those (provider bugs, 0-divisions on
/// illiquid pools); silently dropping them is honest because the cache miss
/// then falls through to the next cascade tier or to `UnknownTokenPrice`.
pub struct RedisCachedPriceOracle {
    snapshot: HashMap<String, f64>,
}

impl RedisCachedPriceOracle {
    /// Build from an in-memory snapshot. Symbol keys are normalized to
    /// uppercase; non-finite / non-positive prices are dropped silently
    /// (caller's snapshot may include junk from upstream feeds; oracle
    /// will return `None` for those tokens, letting the cascade fall
    /// through honestly).
    pub fn from_snapshot(snapshot: HashMap<String, f64>) -> Self {
        let mut clean = HashMap::with_capacity(snapshot.len());
        for (k, v) in snapshot {
            if v.is_finite() && v > 0.0 {
                clean.insert(k.to_ascii_uppercase(), v);
            }
        }
        Self { snapshot: clean }
    }

    /// Fetch the entire `arbx:token_prices:<chain_id>` hash from Redis and
    /// return a fresh oracle wrapping the snapshot.
    ///
    /// Returns an oracle with an EMPTY snapshot (never an error) when:
    ///   - The Redis hash is absent (worker hasn't ticked yet at boot).
    ///   - Redis is unreachable.
    ///   - Any field fails to parse as `f64`.
    ///
    /// Rationale: the spine cascade always falls through to ConfigPriceOracle
    /// on cache miss, so an empty cache snapshot degrades gracefully (operator
    /// overrides + stablecoins still work). Surfacing the Redis error here
    /// would force every caller to handle it; instead, log + return empty,
    /// and let the caller's downstream `UnknownTokenPrice` rejections signal
    /// the problem to the operator dashboard.
    pub async fn snapshot_from_redis(
        redis: &mut redis::aio::ConnectionManager,
        chain_id: u64,
    ) -> Self {
        use redis::AsyncCommands;
        let key = redis_token_prices_key(chain_id);
        let raw: Result<HashMap<String, String>, redis::RedisError> = redis.hgetall(&key).await;
        let map = match raw {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(
                    event = "price_oracle.redis_hgetall_failed",
                    chain_id,
                    key = %key,
                    error = %e,
                    "returning empty snapshot — cascade falls through to ConfigPriceOracle"
                );
                HashMap::new()
            }
        };
        let mut parsed: HashMap<String, f64> = HashMap::with_capacity(map.len());
        for (sym, val_str) in map {
            match val_str.parse::<f64>() {
                Ok(v) => {
                    parsed.insert(sym, v);
                }
                Err(e) => {
                    tracing::debug!(
                        event = "price_oracle.parse_failed",
                        chain_id,
                        symbol = %sym,
                        raw = %val_str,
                        error = %e,
                        "dropping malformed cached price"
                    );
                }
            }
        }
        Self::from_snapshot(parsed)
    }

    /// Number of usable entries in the snapshot (post-validation). Useful
    /// for heartbeat / observability ("how many tokens are priced live?").
    pub fn len(&self) -> usize {
        self.snapshot.len()
    }

    /// True when the snapshot is empty (worker hasn't ticked OR Redis miss).
    pub fn is_empty(&self) -> bool {
        self.snapshot.is_empty()
    }

    /// Borrow the underlying validated snapshot. Used by callers that need
    /// to plug the same data into a different consumer (e.g. the spine
    /// evaluator's `with_cache` constructor) without re-fetching from Redis.
    pub fn snapshot(&self) -> &HashMap<String, f64> {
        &self.snapshot
    }

    /// Consume the oracle and return its validated snapshot. Same use case
    /// as `snapshot()` but transfers ownership when the caller doesn't need
    /// the oracle again.
    pub fn into_snapshot(self) -> HashMap<String, f64> {
        self.snapshot
    }
}

impl PriceOracle for RedisCachedPriceOracle {
    fn price_usd(&self, token_id: &str) -> Option<f64> {
        let upper = token_id.trim().to_ascii_uppercase();
        if upper.is_empty() {
            return None;
        }
        self.snapshot.get(&upper).copied()
    }
}

/// Canonical Redis key holding the live token-prices snapshot for a chain.
/// Hash schema: field = uppercase symbol, value = USD price as decimal string.
/// Populated by `searcher-rs::workers::price_worker`; consumed by
/// `RedisCachedPriceOracle::snapshot_from_redis`.
pub fn redis_token_prices_key(chain_id: u64) -> String {
    format!("arbx:token_prices:{chain_id}")
}

/// B5 (math-audit AUDIT-MATH-OPPS-2026-09-26): maximum accepted single-tick
/// change ratio for a price written by ONE source over the value already in the
/// shared hash. Three writers (Chainlink price_worker, DexScreener,
/// GeckoTerminal) target the same key with last-writer-wins and no arbitration;
/// a scaling bug in a free source poisoned tokens that have NO configured
/// oracle — live evidence: AAVE $161,339,420.31 (real ≈ $161) and SNX
/// $272,885.72 (real ≈ $0.27), both exactly ×1e6. A real asset never moves 100×
/// between consecutive writes (seconds apart); such a jump is a data error.
pub const PRICE_MAX_TICK_RATIO: f64 = 100.0;

/// B5 (math-audit AUDIT-MATH-OPPS-2026-09-26) — AUTHORITY ORDER between the three
/// writers of `arbx:token_prices:<chain>`.
///
/// The range guard above rejects an implausible JUMP, but it cannot fix a source
/// that is *consistently* wrong: the shared hash has a ~60 s TTL, so every cycle
/// is written from scratch, and a free-tier writer that derives a symbol from an
/// arbitrary pool re-proposes the same bad value every tick (live evidence:
/// AAVE = 161,339,420.31 and SNX = 272,885.72 came back after every expiry).
///
/// The sovereign stack already defines which symbols have an authoritative
/// producer: `searcher-rs::price_worker` publishes the Binance charter pairs
/// (`price_bus::canonical_pair_for_token`) and the Chainlink anchors. A free-tier
/// enricher must therefore never clobber those symbols with a pool quote.
///
/// The set is derived from the ONE charter table — no second list to drift.
pub fn is_authoritative_symbol(sym_upper: &str) -> bool {
    crate::price_bus::canonical_pair_for_token(sym_upper).is_some()
}

/// B5c (math-audit AUDIT-MATH-OPPS-2026-09-26) — relative tolerance for a
/// CORROBORATED correction of a stored price.
pub const PRICE_CORRECTION_TOLERANCE: f64 = 0.20;

/// B5c — is `candidate` a corroboration of a previously recorded pending
/// correction? Two independent observations agreeing (within
/// `PRICE_CORRECTION_TOLERANCE`) against the STORED value are evidence that the
/// stored value is the outlier, not the candidate.
///
/// WHY THIS EXISTS (live evidence, 2026-09-26 20:17Z):
/// `{"event":"geckoterminal.price_implausible_skip","symbol":"AAVE",
///   "prev":"Some(161339420.31)","new":154.1760931358}`
/// Real AAVE ≈ $154: the free tier proposed the CORRECT value and the range
/// guard refused it, because the poisoned value landed first and every write
/// refreshes the hash TTL (`prev` is therefore never absent). The guard was a
/// ONE-WAY RATCHET: it blocked the injection of a ×1e6 error but could never
/// repair one that was already stored. With this rule the second agreeing
/// observation is accepted and the stored value is overwritten; a lone outlier
/// is still refused forever (it never gets corroborated).
pub fn is_corroborated_correction(pending: Option<f64>, candidate: f64) -> bool {
    if !candidate.is_finite() || candidate <= 0.0 {
        return false;
    }
    let Some(p) = pending else {
        return false;
    };
    if !p.is_finite() || p <= 0.0 {
        return false;
    }
    let ratio = candidate / p;
    ((1.0 - PRICE_CORRECTION_TOLERANCE)..=(1.0 + PRICE_CORRECTION_TOLERANCE)).contains(&ratio)
}

/// Redis key holding the PENDING (not yet corroborated) corrections, as a hash
/// of `SYMBOL -> candidate`. Deliberately separate from the published hash so a
/// pending candidate can never be read as a price, and short-lived.
pub fn pending_corrections_key(chain_id: u64) -> String {
    format!("arbx:token_prices_pending:{chain_id}")
}

/// TTL of a pending correction (seconds). Long enough to be seen by the next
/// cycle of any writer, short enough that a one-off outlier disappears.
pub const PENDING_CORRECTION_TTL_SECS: i64 = 300;

/// B5 gate: may `new` replace `prev` for this symbol?
///
/// - non-finite or ≤ 0 `new` → never (basic honesty, mirrors the writers' guard);
/// - no previous value (first write for the symbol) → yes;
/// - a poisoned/absent previous value → yes (accept the correction);
/// - ratio outside `[1/N, N]` with `N = PRICE_MAX_TICK_RATIO` → NO: the caller
///   logs it and keeps the previous value (fail-honest: a stale plausible price
///   beats a poisoned one that would silently corrupt every gross computation).
pub fn is_plausible_price(prev: Option<f64>, new: f64) -> bool {
    if !new.is_finite() || new <= 0.0 {
        return false;
    }
    let Some(p) = prev else {
        return true;
    };
    if !p.is_finite() || p <= 0.0 {
        return true;
    }
    let ratio = new / p;
    (1.0 / PRICE_MAX_TICK_RATIO..=PRICE_MAX_TICK_RATIO).contains(&ratio)
}

// ═══════════════════════════════════════════════════════════════════════════
// B5d (2026-09-26) — the reference must OUTLIVE the published hash's TTL
// ═══════════════════════════════════════════════════════════════════════════
//
// MEASURED ON PRODUCTION (five verification runs of `arbx:token_prices:1`):
//   SNX   R1 = 272885.72   R2/R3 = 0.2610872236   R4 = 272885.72  ← RE-POISONED
//   AAVE  R1/R2/R3 = 161339420.31   R4 = 155.038   (repaired by PR #695)
//   `TTL arbx:token_prices:1` = 51 s, `HLEN` = 386, every other comparator in
//   band (WBTC 84385, WETH 2695, AAVE 155.038, …): SNX is the sole >100× outlier.
//
// ROOT CAUSE OF THE RE-POISONING: both free-tier writers validated a candidate
// against the value CURRENTLY IN THE SHARED HASH. That hash has a ~50 s TTL, so
// after every expiry the writer reads `prev = None` and `is_plausible_price(None,
// new)` returns `true` for ANY value — the ×1e6 candidate included. PR #695's
// authority order (B5b) only covers the Binance/Chainlink charter symbols
// (WETH/ETH/WBTC/BTC/USDC) and SNX has no authoritative producer, so it sits
// exactly in that hole. B5c's corroborated correction cannot help either: it
// needs a correct value proposed twice while a poisoned one is stored — the
// reverse of this case.
//
// FIX: a LONG-LIVED per-symbol baseline that survives the published TTL, and a
// candidate is validated against the baseline FIRST (when the knob below is on),
// falling back to the published value, and only then to "first-ever write".

/// Redis key holding the LONG-LIVED baseline: hash `SYMBOL -> last accepted
/// price`. Deliberately a THIRD key, distinct from the published hash
/// (`arbx:token_prices:<chain>`) and from the pending store
/// (`arbx:token_prices_pending:<chain>`), so a baseline can never be read as a
/// price and a pending candidate can never be read as a baseline.
pub fn redis_price_baseline_key(chain_id: u64) -> String {
    format!("arbx:price_baseline:{chain_id}")
}

/// TTL of the baseline hash (seconds) — 24 h. The published hash lives ~50 s
/// (measured: `TTL arbx:token_prices:1` = 51 s), which is precisely why the range
/// guard had no reference to compare against. 24 h outlives a restart, a writer
/// outage and the operator's sleep, and still lets an abandoned symbol's baseline
/// evaporate instead of pinning a price forever. Re-armed on every cycle that
/// accepted at least one write.
pub const PRICE_BASELINE_TTL_SECS: i64 = 86_400;

/// B5d knob — `ARBX_PRICE_BASELINE_GUARD`. The baseline check is ON by default
/// (fail-safe); only an EXPLICIT off-value disables it, which restores exactly the
/// pre-B5d behaviour (reference = published value only).
pub const ENV_PRICE_BASELINE_GUARD: &str = "ARBX_PRICE_BASELINE_GUARD";

/// Pure knob parse (no env access — unit-testable without touching the process
/// environment). `None`, an empty string, or any unrecognised token keeps the
/// guard ON: an operator typo must never silently disable a poison gate.
pub fn baseline_guard_enabled(raw: Option<&str>) -> bool {
    match raw {
        None => true,
        Some(v) => !matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "off" | "0" | "false" | "no" | "disabled"
        ),
    }
}

/// Knob read from the process environment. Callers evaluate it ONCE per cycle
/// (loop-invariant), never per symbol.
pub fn baseline_guard_enabled_from_env() -> bool {
    baseline_guard_enabled(std::env::var(ENV_PRICE_BASELINE_GUARD).ok().as_deref())
}

/// B5d — the ONE verdict a free-tier writer can reach for a candidate price.
/// Each variant states the caller's persistence obligations so the writers stay
/// mechanical (no policy in the I/O layer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriceWriteDecision {
    /// First-ever observation for the symbol: neither a baseline nor a published
    /// value exists, so there is nothing to compare against. Accepted HONESTLY
    /// (a fabricated reference is forbidden — RULE 00/R8), and it becomes the
    /// baseline. Caller: `HSET` published + `HSET` baseline.
    AcceptFirst,
    /// Plausible against the reference (baseline, else published). Caller:
    /// `HSET` published + `HSET` baseline (the baseline advances).
    Accept,
    /// Implausible against the reference but CORROBORATED by a previous PENDING
    /// observation (B5c) — the stored/reference value is the outlier. Caller:
    /// `HSET` published + `HSET` baseline (RESET to the correction — keeping the
    /// poisoned baseline would refuse the corrected value again next cycle) +
    /// `HDEL` pending.
    AcceptCorrection,
    /// Implausible and uncorroborated. Caller: do NOT publish; record/replace the
    /// PENDING candidate (short TTL) so a genuine second observation can heal it.
    RefusePending,
    /// Non-finite or ≤ 0 — never written and never recorded: garbage must not
    /// enter the pending store (it could never corroborate anything and would
    /// displace a legitimate pending record).
    RefuseInvalid,
}

/// B5d — decide ONE free-tier write. Pure: no I/O, no clock, fully unit-testable.
///
/// Reference ladder:
///   1. `baseline` (long-lived, survives the published hash's ~50 s TTL) — used
///      when `guard` is on and the stored value is usable;
///   2. else `published` (the current shared-hash value);
///   3. else `None` → first-ever write → `AcceptFirst`.
///
/// A candidate implausible against the reference is still refused unless a
/// previous cycle recorded a corroborating PENDING candidate (`AcceptCorrection`),
/// which keeps B5c's healing path intact. With `guard = false` the baseline is
/// ignored entirely and this function reproduces the pre-B5d decision (reference =
/// published value only), which is the documented revert.
pub fn decide_price_write(
    guard: bool,
    baseline: Option<f64>,
    published: Option<f64>,
    pending: Option<f64>,
    candidate: f64,
) -> PriceWriteDecision {
    if !candidate.is_finite() || candidate <= 0.0 {
        return PriceWriteDecision::RefuseInvalid;
    }
    // A stored value that is not finite/positive is treated as ABSENT (never as a
    // reference): mirrors `is_plausible_price`'s handling of poisoned storages.
    let usable = |v: Option<f64>| v.filter(|p| p.is_finite() && *p > 0.0);
    let reference = if guard {
        usable(baseline).or_else(|| usable(published))
    } else {
        usable(published)
    };
    let Some(reference) = reference else {
        return PriceWriteDecision::AcceptFirst;
    };
    if is_plausible_price(Some(reference), candidate) {
        return PriceWriteDecision::Accept;
    }
    if is_corroborated_correction(pending, candidate) {
        return PriceWriteDecision::AcceptCorrection;
    }
    PriceWriteDecision::RefusePending
}

/// B5d — exactly the value the caller must persist as the symbol's new baseline,
/// or `None` when the write was refused (the baseline is left untouched).
///
/// Every ACCEPTED decision advances the baseline to the candidate — including
/// `AcceptCorrection`, where the baseline must be RESET rather than kept: if the
/// poisoned reference survived a successful correction, the corrected value would
/// be implausible against it again on the very next cycle and the symbol would
/// oscillate refuse/accept forever.
pub fn baseline_value(decision: PriceWriteDecision, candidate: f64) -> Option<f64> {
    match decision {
        PriceWriteDecision::AcceptFirst
        | PriceWriteDecision::Accept
        | PriceWriteDecision::AcceptCorrection => Some(candidate),
        PriceWriteDecision::RefusePending | PriceWriteDecision::RefuseInvalid => None,
    }
}

/// Pub/sub channel notified whenever a writer persists prices into
/// `arbx:token_prices:<chain_id>` (G-PRICE-1: snapshot+push price streaming).
/// Payload is a small JSON notice (`{"source":"price_worker","written":33}`);
/// receivers re-read the hash for the actual data, keeping the Redis hash as
/// the single source of truth (no payload drift between writers).
/// Writers: searcher-rs `price_worker`, token-enricher DexScreener and
/// GeckoTerminal tiers. Consumer: api-server `prices-stream` WS bridge.
pub fn prices_updated_channel(chain_id: u64) -> String {
    format!("arbx:prices:updated:{chain_id}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trading_config::{GasPriceStrategy, TradingConfigState};
    use chrono::Utc;

    // ── B5 gate (math-audit AUDIT-MATH-OPPS-2026-09-26) ─────────────────────

    /// The exact production poisoning: AAVE ≈ $161 and SNX ≈ $0.27 both landed
    /// in the shared hash ×1e6 from a free writer. The guard must refuse the
    /// spike (keeping the stored value) while accepting normal movement and a
    /// first write.
    #[test]
    fn b5_guard_refuses_the_x1e6_poison_and_accepts_normal_moves() {
        // The live poison: 161 -> 161_339_420.31 (x1e6).
        assert!(!is_plausible_price(Some(161.0), 161_339_420.31));
        // SNX: 0.27 -> 272_885.72 (x1e6).
        assert!(!is_plausible_price(Some(0.27), 272_885.72));
        // Normal tick movement in both directions is accepted.
        assert!(is_plausible_price(Some(161.0), 165.0));
        assert!(is_plausible_price(Some(161.0), 158.0));
        assert!(is_plausible_price(Some(0.27), 0.29));
        // First write for a symbol (no stored value) is accepted.
        assert!(is_plausible_price(None, 161.0));
        // A non-finite/non-positive STORED value is treated as absent → accepted.
        assert!(is_plausible_price(Some(0.0), 161.0));
        assert!(is_plausible_price(Some(f64::NAN), 161.0));
        // The guard is SYMMETRIC: a large move in EITHER direction is refused.
        // That includes the downward correction of an already-poisoned value —
        // the hash TTL (~60s, re-armed by every write cycle) clears the stale
        // poison and the next first-write repopulates it sanely. Documented
        // trade-off: a short stale window beats silently trusting a jump that
        // looks exactly like the scaling bug this guard exists for.
        assert!(!is_plausible_price(Some(161_339_420.31), 161.0));
        // Non-finite / non-positive are never written.
        assert!(!is_plausible_price(Some(1.0), f64::NAN));
        assert!(!is_plausible_price(Some(1.0), f64::INFINITY));
        assert!(!is_plausible_price(Some(1.0), 0.0));
        assert!(!is_plausible_price(Some(1.0), -5.0));
        // Boundary: exactly the max ratio is still plausible (inclusive).
        assert!(is_plausible_price(Some(1.0), PRICE_MAX_TICK_RATIO));
        assert!(!is_plausible_price(Some(1.0), PRICE_MAX_TICK_RATIO * 1.01));
    }

    /// B5 (authority order): the Binance/Chainlink charter symbols belong to
    /// `price_worker`; a free-tier enricher must never clobber them. The set is
    /// derived from the ONE charter table, so this gate fails the moment that
    /// table changes without the writers being revisited.
    #[test]
    fn b5_authoritative_symbols_follow_the_binance_charter() {
        for sym in ["WETH", "ETH", "WBTC", "BTC", "USDC"] {
            assert!(
                is_authoritative_symbol(sym),
                "{sym} has a canonical Binance pair → price_worker owns it"
            );
            assert!(
                crate::price_bus::canonical_pair_for_token(sym).is_some(),
                "{sym} must come from the charter table, not a second list"
            );
        }
        for sym in ["AAVE", "SNX", "PEPE", "UNI", "LINK"] {
            assert!(
                !is_authoritative_symbol(sym),
                "{sym} has no canonical pair → free tiers may fill it"
            );
        }
        // Case handling mirrors the other public helpers: the caller passes the
        // uppercase hash field. A lowercase input is NOT authoritative, which is
        // why every writer uppercases before consulting this.
        assert!(!is_authoritative_symbol("weth"));
    }

    /// B5c gate — the corroborated-correction rule that repairs an ALREADY
    /// stored poison (the AAVE case: correct 154.17 refused against 161M).
    #[test]
    fn b5c_corroborated_correction_accepts_the_pair_and_refuses_a_lone_outlier() {
        // AAVE: pending 154.1760931358, next cycle proposes the same ballpark.
        assert!(is_corroborated_correction(Some(154.1760931358), 154.5));
        assert!(is_corroborated_correction(Some(154.1760931358), 130.0)); // -15.6%
        assert!(is_corroborated_correction(Some(154.1760931358), 180.0)); // +16.8%
                                                                          // Outside the tolerance → not a corroboration (the pending record is
                                                                          // simply replaced by the newer candidate).
        assert!(!is_corroborated_correction(Some(154.1760931358), 100.0));
        assert!(!is_corroborated_correction(Some(154.1760931358), 300.0));
        // No pending record → nothing to corroborate (a single observation of a
        // huge jump must never be accepted).
        assert!(!is_corroborated_correction(None, 154.1760931358));
        assert!(!is_corroborated_correction(None, 161_339_420.31));
        // Garbage never corroborates.
        assert!(!is_corroborated_correction(Some(f64::NAN), 154.0));
        assert!(!is_corroborated_correction(Some(154.0), f64::NAN));
        assert!(!is_corroborated_correction(Some(154.0), 0.0));
        assert!(!is_corroborated_correction(Some(0.0), 154.0));
        // Boundary is inclusive.
        assert!(is_corroborated_correction(
            Some(100.0),
            100.0 * (1.0 + PRICE_CORRECTION_TOLERANCE)
        ));
        assert!(!is_corroborated_correction(
            Some(100.0),
            100.0 * (1.0 + PRICE_CORRECTION_TOLERANCE) * 1.01
        ));
        // The pending store is a SEPARATE key — a pending candidate is never a price.
        assert_eq!(pending_corrections_key(1), "arbx:token_prices_pending:1");
        assert_ne!(pending_corrections_key(1), redis_token_prices_key(1));
    }

    // ── B5d gates (2026-09-26): the reference must outlive the published TTL ─

    /// GATE 1 — THE SNX CASE. The published hash has TTL-expired (51 s, measured),
    /// so the writer sees `prev = None`; the long-lived baseline still holds the
    /// real 0.2610872236 and the free tier proposes 272885.72 (×1e6). Today the
    /// guard accepts (proved pre-patch: `is_plausible_price(None, _) == true`);
    /// with the baseline it must be REFUSED and recorded as PENDING.
    #[test]
    fn b5d_gate1_prev_absent_is_refused_by_the_baseline() {
        let candidate = 272_885.72_f64;
        let decision = decide_price_write(
            true,               // knob ON (default)
            Some(0.2610872236), // baseline = the real SNX price
            None,               // published hash expired -> prev = None
            None,               // no pending record yet
            candidate,
        );
        assert_eq!(decision, PriceWriteDecision::RefusePending);
        // A refusal never advances the baseline (the real price stays the reference).
        assert_eq!(baseline_value(decision, candidate), None);
        // The same shape for a first-ever AAVE-style injection after an expiry.
        assert_eq!(
            decide_price_write(true, Some(154.0), None, None, 161_339_420.31),
            PriceWriteDecision::RefusePending
        );
        // REVERT PROOF: with the knob OFF the baseline is ignored, so the old hole
        // is back (prev absent ⇒ accepted) — that is exactly the pre-B5d behaviour.
        assert_eq!(
            decide_price_write(false, Some(0.2610872236), None, None, candidate),
            PriceWriteDecision::AcceptFirst
        );
    }

    /// GATE 2 — a first-ever write (no baseline, no published value) is accepted
    /// honestly and BECOMES the baseline. No fabricated reference (RULE 00/R8).
    #[test]
    fn b5d_gate2_first_ever_write_is_accepted_and_seeds_the_baseline() {
        let candidate = 0.2610872236_f64;
        let decision = decide_price_write(true, None, None, None, candidate);
        assert_eq!(decision, PriceWriteDecision::AcceptFirst);
        assert_eq!(
            baseline_value(decision, candidate),
            Some(candidate),
            "the first accepted price seeds the baseline"
        );
    }

    /// GATE 3 — normal movement is accepted and the baseline ADVANCES, in both
    /// directions, whether the reference came from the baseline or (bootstrap, no
    /// baseline yet) from the published hash. The baseline outranks a poisoned
    /// published value.
    #[test]
    fn b5d_gate3_normal_movement_advances_the_baseline() {
        for (reference, candidate) in [
            (0.26_f64, 0.281_f64), // up
            (0.26, 0.241),         // down
            (161.0, 165.0),        // AAVE-shaped
            (161.0, 158.0),
            (1.0, PRICE_MAX_TICK_RATIO), // boundary is inclusive
        ] {
            let decision =
                decide_price_write(true, Some(reference), Some(reference), None, candidate);
            assert_eq!(
                decision,
                PriceWriteDecision::Accept,
                "{reference} -> {candidate} is a normal tick and must be accepted"
            );
            assert_eq!(baseline_value(decision, candidate), Some(candidate));
        }
        // Bootstrap: no baseline recorded yet -> the published value is the
        // reference (so the very first baseline is validated, not blind).
        assert_eq!(
            decide_price_write(true, None, Some(161.0), None, 165.0),
            PriceWriteDecision::Accept
        );
        // The baseline WINS over a poisoned published value: a healthy baseline
        // plus a poisoned hash value still accepts the correct candidate.
        assert_eq!(
            decide_price_write(true, Some(0.2610872236), Some(272_885.72), None, 0.27),
            PriceWriteDecision::Accept
        );
    }

    /// GATE 4 — B5c's corroborated correction still heals a POISONED BASELINE
    /// (the AAVE shape: reference 161,339,420.31 vs ~154 real), and the accepted
    /// correction RESETS the baseline so the healed symbol is not refused again on
    /// the next cycle. A lone outlier is still refused; an out-of-tolerance
    /// "corroboration" is not a corroboration.
    #[test]
    fn b5d_gate4_corroborated_correction_heals_a_poisoned_baseline() {
        let poison = 161_339_420.31_f64;
        let candidate = 154.5_f64;
        let decision = decide_price_write(
            true,
            Some(poison),         // baseline itself is the outlier
            Some(poison),         // and so is the published value
            Some(154.1760931358), // previous observation agreed (~154.18)
            candidate,
        );
        assert_eq!(decision, PriceWriteDecision::AcceptCorrection);
        assert_eq!(
            baseline_value(decision, candidate),
            Some(candidate),
            "a correction must RESET the baseline, else the healed value is refused next cycle"
        );
        // …and the cycle AFTER the correction is a plain accept (no oscillation).
        assert_eq!(
            decide_price_write(true, Some(candidate), Some(candidate), None, 155.0),
            PriceWriteDecision::Accept
        );
        // A lone implausible sample never heals anything (no corroboration).
        assert_eq!(
            decide_price_write(true, Some(poison), Some(poison), None, candidate),
            PriceWriteDecision::RefusePending
        );
        // A "pending" that does not agree (outside ±20 %) is not corroboration.
        assert_eq!(
            decide_price_write(true, Some(poison), Some(poison), Some(100.0), candidate),
            PriceWriteDecision::RefusePending
        );
    }

    /// GATE 5 — the baseline key is DISTINCT from the published key and from the
    /// pending key, and its TTL genuinely outlives the pending window (the whole
    /// point: the reference must survive the published hash's ~50 s TTL).
    #[test]
    fn b5d_gate5_baseline_key_and_ttl_are_distinct() {
        assert_eq!(redis_price_baseline_key(1), "arbx:price_baseline:1");
        assert_ne!(redis_price_baseline_key(1), redis_token_prices_key(1));
        assert_ne!(redis_price_baseline_key(1), pending_corrections_key(1));
        assert_eq!(redis_price_baseline_key(8453), "arbx:price_baseline:8453");
        // Read through locals so the relations are asserted as VALUES, not folded
        // away as constant expressions (clippy::assertions_on_constants).
        let baseline_ttl = PRICE_BASELINE_TTL_SECS;
        let pending_ttl = PENDING_CORRECTION_TTL_SECS;
        // The writers derive the published hash TTL as `interval*3` floored at 60 s
        // (DEXSCREENER_PRICE_INTERVAL_MS default 15 s → 45 s, floored to 60 s); the
        // measured production TTL is ~51 s. The baseline must outlive it by orders
        // of magnitude AND survive a writer restart / outage.
        let published_ttl_default = 15_i64 * 3;
        assert_eq!(baseline_ttl, 86_400, "baseline TTL is 24 h");
        assert!(baseline_ttl > pending_ttl);
        assert!(baseline_ttl > published_ttl_default * 100);
    }

    /// GATE 6 — the knob is ON by default and only an EXPLICIT off-value disables
    /// it; garbage values keep the guard ON (a typo must not disarm a poison gate).
    #[test]
    fn b5d_gate6_knob_defaults_on_and_requires_an_explicit_off() {
        for on in [
            None,
            Some(""),
            Some("  "),
            Some("on"),
            Some("ON"),
            Some("1"),
            Some("true"),
            Some("active"),
            Some("yes"),
            Some("guard"),
        ] {
            assert!(baseline_guard_enabled(on), "{on:?} must keep the guard ON");
        }
        for off in [
            "off", "OFF", " off ", "0", "false", "FALSE", "no", "disabled",
        ] {
            assert!(
                !baseline_guard_enabled(Some(off)),
                "{off:?} is an explicit off-value"
            );
        }
    }

    /// GATE 7 — non-finite / ≤ 0 candidates are refused and NOTHING is recorded:
    /// garbage must never enter the pending store, where it would displace a
    /// legitimate pending record and could never corroborate anything.
    #[test]
    fn b5d_gate7_invalid_candidates_are_refused_without_recording() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0, -1.0] {
            let decision = decide_price_write(true, Some(1.0), Some(1.0), Some(2.0), bad);
            assert_eq!(
                decision,
                PriceWriteDecision::RefuseInvalid,
                "{bad} must be invalid"
            );
            assert_eq!(baseline_value(decision, bad), None);
        }
    }

    /// GATE 8 — timeline replay of the SNX loop with the published hash expiring
    /// every cycle (the named root cause). Proven property: a SINGLE wrong sample
    /// never reaches the published hash while the baseline holds the truth, and the
    /// refusal neither publishes anything nor advances the baseline.
    #[test]
    fn b5d_gate8_snx_timeline_single_sample_never_lands() {
        let mut published: Option<f64> = None; // the hash just expired (~51 s TTL)
        let baseline: Option<f64> = Some(0.2610872236);
        let pending: Option<f64> = None;
        let poison = 272_885.72_f64;
        let truth = 0.27_f64;

        let decision = decide_price_write(true, baseline, published, pending, poison);
        assert_eq!(decision, PriceWriteDecision::RefusePending);
        assert_eq!(
            baseline_value(decision, poison),
            None,
            "a refusal must not advance the baseline"
        );
        // Executing the RefusePending obligations: record the candidate as PENDING,
        // publish nothing, leave the baseline exactly as it was.
        let recorded_pending = Some(poison);
        assert_ne!(
            published,
            Some(poison),
            "the poison never reached the reader"
        );
        assert_eq!(
            baseline,
            Some(0.2610872236),
            "the reference survives the refusal"
        );

        // The NEXT cycle's correct observation is still accepted against the same
        // baseline: the symbol keeps a fresh honest price while poisoned samples
        // continue to be refused.
        let decision = decide_price_write(true, baseline, published, recorded_pending, truth);
        assert_eq!(decision, PriceWriteDecision::Accept);
        published = baseline_value(decision, truth);
        assert_eq!(published, Some(truth));
    }

    /// KNOWN RESIDUAL — declared, NOT closed by B5d (see the B5d report §RISKS).
    /// The corroboration rule is symmetric by design (B5c), so a source that
    /// re-proposes the SAME wrong value within the pending window (300 s)
    /// corroborates ITSELF and its second occurrence is accepted as a correction.
    /// B5d removes the single-sample / TTL-expired path (GATE 1, GATE 8); closing
    /// this one needs identity corroboration at the write site (B5b's declared
    /// remainder: publish only under the symbol `TokenIdentityIndex::symbol_for_addr`
    /// assigns to the address). Pinned here so nobody believes B5d closed it.
    #[test]
    fn b5d_known_residual_persistent_source_self_corroborates_on_second_cycle() {
        let baseline = Some(0.2610872236_f64);
        let poison = 272_885.72_f64;
        // Cycle 1: refused, and the candidate is recorded as PENDING.
        assert_eq!(
            decide_price_write(true, baseline, None, None, poison),
            PriceWriteDecision::RefusePending
        );
        // Cycle 2: the SAME value proposed again inside the pending TTL.
        assert_eq!(
            decide_price_write(true, baseline, None, Some(poison), poison),
            PriceWriteDecision::AcceptCorrection,
            "two agreeing samples — B5c semantics, symmetric by design"
        );
    }

    fn cfg_with_prices(prices: HashMap<String, f64>) -> TradingConfigState {
        TradingConfigState {
            chain_id: 1,
            capital_usd: 1000.0,
            base_token_symbol: "WETH".into(),
            base_token_price_usd: 2500.0,
            allowed_token_symbols: vec!["WETH".into(), "USDC".into(), "WBTC".into(), "UNI".into()],
            token_prices_usd: prices,
            simulation_capital_usd: None,
            simulation_per_token_amounts_usd: HashMap::new(),
            simulation_per_strategy_caps_usd: HashMap::new(),
            simulation_target_profit_usd: None,
            simulation_target_roi_pct: None,
            min_profit_usd: 1.0,
            min_roi_pct: 0.1,
            min_landing_probability: 0.5,
            min_liquidity_confidence: 0.7,
            max_token_risk_score: 1.0,
            gas_price_strategy: GasPriceStrategy::Fixed,
            fixed_gas_price_gwei: Some(20.0),
            gas_estimate_units: 200_000,
            max_slippage_pct: 0.5,
            failure_risk_buffer_pct: 0.001,
            flashloan_fee_pct: 0.0009,
            enabled_strategies: vec!["dex_arb_v2v2".into()],
            enabled_dex_ids: None,
            strategy_configs: HashMap::new(),
            capital_cost_rate_annual_pct: 0.0,
            ops_overhead_usd_per_attempt: 0.01,
            spread_sanity_mult: 3.0,
            p_copied_volume_threshold_usd: 1_000_000.0,
            p_copied_max: 0.5,
            lp_fee_default_pct: 0.003, // WO-04 (2026-09-06)
            kelly_multiplier: 0.5,
            kelly_max_per_trade_fraction: 1.0,
            kelly_gas_safety_multiplier: 1.0,
            enabled: true,
            updated_at: Utc::now(),
            updated_by: None,
        }
    }

    #[test]
    fn resolves_base_token_symbol_case_insensitive() {
        let c = cfg_with_prices(HashMap::new());
        let oracle = ConfigPriceOracle::new(&c);
        assert_eq!(oracle.price_usd("WETH"), Some(2500.0));
        assert_eq!(oracle.price_usd("weth"), Some(2500.0));
        assert_eq!(oracle.price_usd("Weth"), Some(2500.0));
        assert_eq!(oracle.price_usd("  WETH  "), Some(2500.0)); // trim whitespace
    }

    #[test]
    fn resolves_operator_token_price_map() {
        let mut prices = HashMap::new();
        prices.insert("WBTC".into(), 95_000.0);
        prices.insert("UNI".into(), 8.5);
        let c = cfg_with_prices(prices);
        let oracle = ConfigPriceOracle::new(&c);
        assert_eq!(oracle.price_usd("WBTC"), Some(95_000.0));
        assert_eq!(oracle.price_usd("uni"), Some(8.5));
    }

    #[test]
    fn stablecoins_have_no_hardcoded_default() {
        // WO-PC4: stables resolve through the SAME multi-source cascade as
        // everything else — never a fabricated $1.00 (a depegging stable must
        // surface as unpriced/rejected, not as parity).
        let c = cfg_with_prices(HashMap::new());
        let oracle = ConfigPriceOracle::new(&c);
        assert_eq!(oracle.price_usd("USDC"), None);
        assert_eq!(oracle.price_usd("usdt"), None);
        assert_eq!(oracle.price_usd("DAI"), None);
        assert_eq!(oracle.price_usd("FRAX"), None);
        assert_eq!(oracle.price_usd("PYUSD"), None);
        assert_eq!(oracle.price_usd("LUSD"), None);
    }

    #[test]
    fn operator_override_prices_stables_explicitly() {
        // Defensive use case: operator wants to model a temporary depeg.
        // E.g. FRAX trading at $0.985 during stress — operator sets it
        // explicitly so opportunity sizing reflects reality.
        let mut prices = HashMap::new();
        prices.insert("FRAX".into(), 0.985);
        let c = cfg_with_prices(prices);
        let oracle = ConfigPriceOracle::new(&c);
        assert_eq!(oracle.price_usd("FRAX"), Some(0.985));
    }

    #[test]
    fn returns_none_for_unknown_token() {
        let c = cfg_with_prices(HashMap::new());
        let oracle = ConfigPriceOracle::new(&c);
        assert_eq!(oracle.price_usd("PEPE"), None);
        assert_eq!(oracle.price_usd("UNKNOWN"), None);
        // Hex addresses (caller passed when meta cache was empty) — also None
        // because oracle indexes by symbol. Caller treats this as REJECT.
        assert_eq!(
            oracle.price_usd("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
            None
        );
    }

    #[test]
    fn returns_none_for_empty_or_whitespace_input() {
        let c = cfg_with_prices(HashMap::new());
        let oracle = ConfigPriceOracle::new(&c);
        assert_eq!(oracle.price_usd(""), None);
        assert_eq!(oracle.price_usd("   "), None);
        assert_eq!(oracle.price_usd("\t\n"), None);
    }

    #[test]
    fn is_known_stablecoin_covers_locked_trust_list() {
        // The trust list is locked-in policy (2026-05-05). Adding/removing
        // requires explicit governance update with reasoning. This test
        // documents the canonical list AND the explicitly-excluded set.
        for s in [
            "USDC", "USDT", "DAI", "BUSD", "FRAX", "LUSD", "USDP", "TUSD", "GUSD", "USDD", "PYUSD",
        ] {
            assert!(is_known_stablecoin(s), "expected {s} in trust list");
        }
        // Excluded by policy — operator must set explicit price.
        for s in ["USDE", "MIM", "CRVUSD", "GHO", "USDX"] {
            assert!(
                !is_known_stablecoin(s),
                "{s} should NOT be in trust list (operator must set explicitly)",
            );
        }
    }

    // ----------------------------------------------------------------
    // CascadePriceOracle — wraps multiple oracles in priority order
    // ----------------------------------------------------------------

    /// Trivial test oracle that returns a fixed map. Lets the cascade tests
    /// assemble arbitrary tier orderings without depending on real cache /
    /// config state.
    struct StubOracle(HashMap<String, f64>);

    impl PriceOracle for StubOracle {
        fn price_usd(&self, token_id: &str) -> Option<f64> {
            let upper = token_id.trim().to_ascii_uppercase();
            if upper.is_empty() {
                return None;
            }
            self.0.get(&upper).copied()
        }
    }

    fn stub(prices: &[(&str, f64)]) -> StubOracle {
        let mut m = HashMap::new();
        for (k, v) in prices {
            m.insert((*k).to_string(), *v);
        }
        StubOracle(m)
    }

    #[test]
    fn cascade_returns_first_match() {
        // First oracle has WETH; cascade returns it without consulting later tiers.
        let cascade = CascadePriceOracle::new(vec![
            Box::new(stub(&[("WETH", 2500.0)])),
            Box::new(stub(&[("WETH", 9999.0)])), // would override if reached — must NOT
        ]);
        assert_eq!(cascade.price_usd("WETH"), Some(2500.0));
    }

    #[test]
    fn cascade_falls_through_to_later_oracle_on_miss() {
        // First oracle misses; cascade tries the second.
        let cascade = CascadePriceOracle::new(vec![
            Box::new(stub(&[("WETH", 2500.0)])),
            Box::new(stub(&[("PEPE", 0.0001)])),
        ]);
        assert_eq!(cascade.price_usd("PEPE"), Some(0.0001));
    }

    #[test]
    fn cascade_returns_none_if_all_miss() {
        let cascade = CascadePriceOracle::new(vec![
            Box::new(stub(&[("WETH", 2500.0)])),
            Box::new(stub(&[("USDC", 1.0)])),
        ]);
        assert_eq!(cascade.price_usd("UNKNOWN"), None);
    }

    #[test]
    fn cascade_with_no_oracles_returns_none() {
        // Empty cascade is a degenerate but legal config — must return None,
        // never panic. R8 fail-honest at the most extreme edge.
        let cascade = CascadePriceOracle::new(vec![]);
        assert_eq!(cascade.price_usd("WETH"), None);
    }

    #[test]
    fn cascade_propagates_empty_input_correctly() {
        // Empty / whitespace inputs must short-circuit to None without
        // consulting any oracle (matches ConfigPriceOracle semantics).
        let cascade = CascadePriceOracle::new(vec![Box::new(stub(&[("WETH", 2500.0)]))]);
        assert_eq!(cascade.price_usd(""), None);
        assert_eq!(cascade.price_usd("   "), None);
        assert_eq!(cascade.price_usd("\t\n"), None);
    }

    #[test]
    fn cascade_preserves_config_oracle_internal_priority() {
        // The cascade does NOT flatten internal priorities of its inner oracles.
        // ConfigPriceOracle's tier 2 (operator override) is the last tier INSIDE
        // that oracle — cascade just composes oracles.
        let mut prices = HashMap::new();
        prices.insert("FRAX".into(), 0.985); // operator depeg override
        let cfg = cfg_with_prices(prices);
        let cascade = CascadePriceOracle::new(vec![Box::new(ConfigPriceOracle::new(&cfg))]);
        // FRAX hits operator override INSIDE ConfigPriceOracle, not stablecoin default.
        assert_eq!(cascade.price_usd("FRAX"), Some(0.985));
    }

    #[test]
    fn cascade_with_realistic_two_tier_layout() {
        // Production layout: tier 1 = cache snapshot (background-fetched live
        // prices), tier 2 = ConfigPriceOracle (operator overrides + base
        // token). The cache wins when populated; falls through to operator
        // config when the cache misses.
        let mut cache = HashMap::new();
        cache.insert("WETH".to_string(), 2517.42); // live price beats config base price
        let cache_oracle = RedisCachedPriceOracle::from_snapshot(cache);

        let cfg = cfg_with_prices(HashMap::new());
        let cascade: CascadePriceOracle = CascadePriceOracle::new(vec![
            Box::new(cache_oracle),
            Box::new(ConfigPriceOracle::new(&cfg)),
        ]);

        // WETH: cache hits with the LIVE price, beating ConfigPriceOracle's $2500 base.
        assert_eq!(cascade.price_usd("WETH"), Some(2517.42));
        // USDC: cache miss → no hardcoded default anymore (WO-PC4) → None,
        // caller rejects with UnknownTokenPrice until a live source prices it.
        assert_eq!(cascade.price_usd("USDC"), None);
        // PEPE: cache miss + config miss → None (R8 fail-honest).
        assert_eq!(cascade.price_usd("PEPE"), None);
    }

    // ----------------------------------------------------------------
    // RedisCachedPriceOracle — sync snapshot of an async-fetched cache
    // ----------------------------------------------------------------

    #[test]
    fn redis_cached_oracle_returns_cached_price_case_insensitive() {
        let mut snap = HashMap::new();
        snap.insert("WETH".to_string(), 2500.0);
        snap.insert("USDC".to_string(), 1.0001); // live USDC slightly off-peg
        let oracle = RedisCachedPriceOracle::from_snapshot(snap);
        assert_eq!(oracle.price_usd("WETH"), Some(2500.0));
        assert_eq!(oracle.price_usd("weth"), Some(2500.0));
        assert_eq!(oracle.price_usd("Weth"), Some(2500.0));
        assert_eq!(oracle.price_usd("  USDC  "), Some(1.0001));
    }

    #[test]
    fn redis_cached_oracle_returns_none_on_cache_miss() {
        let mut snap = HashMap::new();
        snap.insert("WETH".to_string(), 2500.0);
        let oracle = RedisCachedPriceOracle::from_snapshot(snap);
        // PEPE is not in cache → None (R8: prefer None over fabricated default).
        assert_eq!(oracle.price_usd("PEPE"), None);
    }

    #[test]
    fn redis_cached_oracle_returns_none_for_empty_input() {
        let mut snap = HashMap::new();
        snap.insert("WETH".to_string(), 2500.0);
        let oracle = RedisCachedPriceOracle::from_snapshot(snap);
        assert_eq!(oracle.price_usd(""), None);
        assert_eq!(oracle.price_usd("   "), None);
    }

    #[test]
    fn redis_cached_oracle_empty_snapshot_always_returns_none() {
        // Boot scenario: cache not yet populated (worker hasn't ticked).
        // Oracle MUST return None for everything — let the cascade fall
        // through to ConfigPriceOracle (operator's seed values).
        let oracle = RedisCachedPriceOracle::from_snapshot(HashMap::new());
        assert_eq!(oracle.price_usd("WETH"), None);
        assert_eq!(oracle.price_usd("USDC"), None);
        assert_eq!(oracle.price_usd("ANYTHING"), None);
    }

    #[test]
    fn redis_cached_oracle_rejects_non_finite_prices_at_construction() {
        // Live external feeds occasionally return NaN / Infinity (provider bug,
        // 0-division on illiquid pool, etc.). The snapshot constructor MUST
        // filter those out so the spine never multiplies by NaN. The brief
        // says R8: prefer None over a fake value — non-finite IS a fake value.
        let mut snap = HashMap::new();
        snap.insert("WETH".to_string(), 2500.0);
        snap.insert("BAD1".to_string(), f64::NAN);
        snap.insert("BAD2".to_string(), f64::INFINITY);
        snap.insert("BAD3".to_string(), f64::NEG_INFINITY);
        snap.insert("BAD4".to_string(), -1.0); // negative price = nonsense
        let oracle = RedisCachedPriceOracle::from_snapshot(snap);
        assert_eq!(oracle.price_usd("WETH"), Some(2500.0));
        assert_eq!(oracle.price_usd("BAD1"), None);
        assert_eq!(oracle.price_usd("BAD2"), None);
        assert_eq!(oracle.price_usd("BAD3"), None);
        assert_eq!(oracle.price_usd("BAD4"), None);
    }
}
