//! DL-02 / SIMCTL-VERDICT-01 — the STRICT FRONTIER on `arbx:opps:validated`.
//!
//! # The defect this closes, and how it was measured
//!
//! `arbx:opps:validated` is a **decision log**, not an accept queue (DL-01 /
//! FIX-SELGATE-01, `selector-api/src/consumer.ts:376-392`): the producer
//! publishes EVERY decided opportunity together with `verdict` (`"accept"` /
//! `"reject"`) and `verdict_reason` — see `publishValidated`, which stamps both
//! keys unconditionally (`consumer.ts:447-458`), with the reason vocabulary
//! owned by `policy/engine.ts`. The measurement this change is specified
//! against: **10.000 of 10.000** payloads on the channel carry
//! `"verdict":"reject"` and `"verdict_reason":"producer_rejected"`.
//!
//! `sim-ctl` deserialised those payloads into
//! `shared_rs::contracts::Opportunity`, which does **not declare** `verdict` or
//! `verdict_reason`, so **serde dropped both silently**. `Ok(..)` from
//! `from_str::<Opportunity>` was therefore the MEASURE of that drop, never a
//! signal that the message was admissible. The consequence was a blind skip:
//! every message walked the whole simulation path (and, for the entries that
//! died structurally, still wrote a `simulations` row), while `passed = true`
//! stayed at zero and could not distinguish "the market offers nothing" from
//! "I am not looking at the market".
//!
//! # Why the verdict is NOT a field on `Opportunity`
//!
//! Adding `verdict` / `verdict_reason` to the shared struct was attempted and
//! MEASURED (PR #869): `cargo check --workspace` produced **15
//! `error[E0063]: missing fields ... in initializer of Opportunity`**, all in
//! `searcher-rs`, whose real scope is **52 `Opportunity {` literals across 29
//! files** — and `cargo check` without `--tests` never compiles the
//! `#[cfg(test)]` ones, so 15 is a LOWER bound. `#[serde(default)]` does not
//! help: it covers DESERIALISATION, not CONSTRUCTION, and `Opportunity`
//! deliberately does not derive `Default` (a default `Opportunity` would be a
//! fabricated row — RULE 00). The verdict is therefore read as an **envelope
//! over the raw JSON**, and `Opportunity` stays untouched.
//!
//! # Two sets, one frontier
//!
//! * [`Admission::EligibleSimulations`] — `verdict == "accept"`. The ONLY set
//!   that may spend fork RPC.
//! * [`Admission::DecisionLog`] — every other observed state, each with a typed
//!   reason. Recorded and ACKed; it acquires no rate/in-flight permit and
//!   reaches no simulator.
//!
//! [`Admission::record`] is called at exactly ONE point, before the branch, so
//! both sets are counted on every message and the split is observable as
//! `arbx_sim_validated_frontier_total{outcome,reason}` — one series per reason
//! family, which is what makes `passed = true = 0` interpretable again.
//!
//! # The non-negotiable safety property of the reader
//!
//! An `Err` from `from_str::<Opportunity>` is terminal in the consumer: the
//! entry is XACKed and the WHOLE message is discarded
//! (`sim_consumer.invalid_msg_parse`). This reader must therefore never be the
//! thing that turns a good message into a discarded one. That is why the
//! envelope types both keys as `Option<serde_json::Value>`: a present-but-
//! wrong-typed `verdict` (`123`, `true`, `{..}`) is CLASSIFIED as
//! [`Verdict::Malformed`], never an `Err` that could propagate. A bare
//! `Option<String>` with `#[serde(default)]` would instead have been an `Err`
//! on `"verdict": 123` — the type-vs-drop behaviour measured in PR #869.
//!
//! A payload that is not a JSON object at all is reported as
//! [`Verdict::Absent`]; the typed parse in the consumer keeps ownership of that
//! case and still answers it with `invalid_msg_parse`.
//!
//! # The declared behaviour of a missing `verdict`: NOT admitted
//!
//! Declared here explicitly rather than left implicit: **absent, `null`,
//! unknown-string or wrong-typed `verdict` all land in
//! [`Admission::DecisionLog`]** — i.e. FAIL-CLOSED, each with its own bounded
//! family. Only an explicit `"accept"` admits. The reason is structural:
//! `publishValidated` stamps `verdict` on EVERY payload it emits, so an absent
//! verdict means the message did not come from this producer — it is not a
//! validated decision, and defaulting it to admissible would let ANY payload
//! that reaches the stream spend the fork (the scarce resource). Fail-closed is
//! acceptable ONLY because the frontier is also LOUD: `envelope_missing` is a
//! first-class counter, so a producer regression raises an alarm instead of
//! silently reproducing the blind skip this change exists to end.

use shared_rs::metrics::SIM_VALIDATED_FRONTIER_TOTAL;

/// The prefix of every `fail_reason` this frontier persists. Namespaced on
/// purpose: `persistence::is_sim_capability_gap` recognises it, so the
/// opportunity is NOT flipped to `rejected` — see the arm's comment there for
/// why the producer, not this service, owns that column.
pub const SKIP_REASON_PREFIX: &str = "producer_verdict:";

/// `outcome` label of the admissible set.
pub const OUTCOME_ELIGIBLE: &str = "eligible_simulations";

/// `outcome` label of the diagnosed set.
pub const OUTCOME_DECISION_LOG: &str = "decision_log";

/// Cap on a raw verdict string copied into a persisted reason. The raw value is
/// attacker-controlled as far as this reader knows, and a bounded blast radius
/// is cheaper than trusting it.
const RAW_CAP: usize = 64;

/// What the RAW payload says about the producer's decision.
///
/// `Absent` is deliberately NOT `Accept`: see the module docs for the declared
/// fail-closed behaviour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// `"verdict":"accept"` — the producer admitted it.
    Accept,
    /// `"verdict":"reject"` with the producer's reason (absent if it published
    /// a null/missing `verdict_reason`, which is reported as a distinct family
    /// rather than as an empty reason).
    Reject { reason: Option<String> },
    /// No `verdict` key, an explicit `null`, or a payload that is not a JSON
    /// object. Fail-closed.
    Absent,
    /// `verdict` present and a string, but not a value this frontier knows.
    Unknown { raw: String },
    /// `verdict` present with a non-string JSON type.
    Malformed { kind: &'static str },
}

/// The envelope: the ONLY keys this reader looks at.
///
/// Unknown keys are ignored by serde's default behaviour, which is what keeps
/// this read strictly ADDITIVE — it can never reject a payload the typed parse
/// would have accepted.
#[derive(serde::Deserialize)]
struct VerdictEnvelope {
    #[serde(default)]
    verdict: Option<serde_json::Value>,
    #[serde(default)]
    verdict_reason: Option<serde_json::Value>,
}

/// Read the producer's verdict from the raw JSON.
///
/// Never fails: the worst case is [`Verdict::Absent`], which is fail-closed and
/// observable. Total by construction — every input maps to exactly one variant.
pub fn read_verdict(json: &str) -> Verdict {
    // A whole-message parse error here means the payload is not a JSON object,
    // so it cannot be an `Opportunity` either. This reader must not be the one
    // that decides that: the typed parse still owns the message and will answer
    // `invalid_msg_parse`. Report `Absent` (fail-closed) and let it.
    let Ok(env) = serde_json::from_str::<VerdictEnvelope>(json) else {
        return Verdict::Absent;
    };
    match env.verdict {
        // `Option<Value>` folds a missing key AND an explicit `null` into
        // `None`; both are the same fail-closed state, so they share a variant
        // instead of pretending to be distinguishable.
        None | Some(serde_json::Value::Null) => Verdict::Absent,
        Some(serde_json::Value::String(s)) if s == "accept" => Verdict::Accept,
        Some(serde_json::Value::String(s)) if s == "reject" => {
            let reason = match env.verdict_reason {
                Some(serde_json::Value::String(r)) if !r.trim().is_empty() => Some(r),
                _ => None,
            };
            Verdict::Reject { reason }
        }
        Some(serde_json::Value::String(s)) => Verdict::Unknown {
            raw: bounded(&s, RAW_CAP),
        },
        Some(other) => Verdict::Malformed {
            kind: json_type_name(&other),
        },
    }
}

/// Which of the two sets this message belongs to, and (for the diagnosed set)
/// why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Admission {
    /// The ONLY set that may consume fork RPC — and the only set whose entries
    /// can ever produce `passed = true`.
    EligibleSimulations,
    /// Explained, recorded and ACKed. Acquires no rate/in-flight permit and
    /// reaches no simulator.
    DecisionLog {
        /// The `fail_reason` persisted verbatim on the `simulations` row.
        reason: String,
        /// The BOUNDED family used as the metric label.
        family: ReasonFamily,
    },
}

impl Admission {
    /// True for the admissible set. Used as the single predicate callers and
    /// tests branch on.
    pub fn is_eligible(&self) -> bool {
        matches!(self, Admission::EligibleSimulations)
    }

    /// Count this message in `arbx_sim_validated_frontier_total{outcome,reason}`.
    ///
    /// Called for BOTH sets at ONE call site in the consumer, so "how many
    /// admissible" and "how many rejected, and why" are two series of the same
    /// instrument and cannot drift apart.
    pub fn record(&self) {
        let (outcome, family) = match self {
            Admission::EligibleSimulations => (OUTCOME_ELIGIBLE, ReasonFamily::Accept),
            Admission::DecisionLog { family, .. } => (OUTCOME_DECISION_LOG, *family),
        };
        SIM_VALIDATED_FRONTIER_TOTAL
            .with_label_values(&[outcome, family.label()])
            .inc();
    }
}

/// Map a raw payload verdict to a set. Pure, total, no I/O.
#[must_use]
pub fn admission(verdict: &Verdict) -> Admission {
    match verdict {
        Verdict::Accept => Admission::EligibleSimulations,
        Verdict::Reject { reason } => {
            let family = match reason.as_deref() {
                Some(r) => ReasonFamily::from_producer_reason(r),
                None => ReasonFamily::ReasonAbsent,
            };
            Admission::DecisionLog {
                reason: format!(
                    "{SKIP_REASON_PREFIX}reject:{}",
                    reason.as_deref().unwrap_or("reason_absent")
                ),
                family,
            }
        }
        Verdict::Absent => Admission::DecisionLog {
            reason: format!("{SKIP_REASON_PREFIX}envelope_missing"),
            family: ReasonFamily::EnvelopeMissing,
        },
        Verdict::Unknown { raw } => Admission::DecisionLog {
            reason: format!("{SKIP_REASON_PREFIX}verdict_unknown:{raw}"),
            family: ReasonFamily::VerdictUnknown,
        },
        Verdict::Malformed { kind } => Admission::DecisionLog {
            reason: format!("{SKIP_REASON_PREFIX}verdict_malformed:{kind}"),
            family: ReasonFamily::VerdictMalformed,
        },
    }
}

/// The BOUNDED reason vocabulary. Every name here is a metric label value, so
/// the set is an ALLOWLIST and never the raw producer string: `invalid_token_
/// address:<label>` and any future reason would otherwise be an unbounded
/// cardinality leak. The producer's reason is still persisted VERBATIM on the
/// `simulations` row (`DecisionLog::reason`), so nothing is lost — the label is
/// bounded, the evidence is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasonFamily {
    /// The admissible set.
    Accept,
    ReasonAbsent,
    ProducerRejected,
    KillSwitchOn,
    BlacklistHit,
    InvalidTokenAddress,
    TokenSafetyCircuitOpen,
    SafetyBelowThreshold,
    SimulationFailed,
    RevertRiskTooHigh,
    ScoreBelowMin,
    Other,
    EnvelopeMissing,
    VerdictUnknown,
    VerdictMalformed,
}

/// Exact reasons owned by `selector-api/src/policy/engine.ts`. The list is
/// closed on purpose; `Other` is the honest bucket for anything new, and is a
/// signal to extend this list rather than a place to hide.
const PRODUCER_REASONS: &[(&str, ReasonFamily)] = &[
    ("producer_rejected", ReasonFamily::ProducerRejected),
    ("kill_switch_on", ReasonFamily::KillSwitchOn),
    ("blacklist_hit", ReasonFamily::BlacklistHit),
    (
        "token_safety_circuit_open",
        ReasonFamily::TokenSafetyCircuitOpen,
    ),
    ("safety_below_threshold", ReasonFamily::SafetyBelowThreshold),
    ("simulation_failed", ReasonFamily::SimulationFailed),
    ("revert_risk_too_high", ReasonFamily::RevertRiskTooHigh),
    ("score_below_min", ReasonFamily::ScoreBelowMin),
];

impl ReasonFamily {
    /// The metric label value.
    pub fn label(self) -> &'static str {
        match self {
            ReasonFamily::Accept => "accept",
            ReasonFamily::ReasonAbsent => "reject:reason_absent",
            ReasonFamily::ProducerRejected => "reject:producer_rejected",
            ReasonFamily::KillSwitchOn => "reject:kill_switch_on",
            ReasonFamily::BlacklistHit => "reject:blacklist_hit",
            ReasonFamily::InvalidTokenAddress => "reject:invalid_token_address",
            ReasonFamily::TokenSafetyCircuitOpen => "reject:token_safety_circuit_open",
            ReasonFamily::SafetyBelowThreshold => "reject:safety_below_threshold",
            ReasonFamily::SimulationFailed => "reject:simulation_failed",
            ReasonFamily::RevertRiskTooHigh => "reject:revert_risk_too_high",
            ReasonFamily::ScoreBelowMin => "reject:score_below_min",
            ReasonFamily::Other => "reject:other",
            ReasonFamily::EnvelopeMissing => "envelope_missing",
            ReasonFamily::VerdictUnknown => "verdict_unknown",
            ReasonFamily::VerdictMalformed => "verdict_malformed",
        }
    }

    /// Classify a producer reason into a bounded family.
    pub fn from_producer_reason(reason: &str) -> Self {
        // `blacklist.ts` emits `invalid_token_address:<label>`; the family is
        // the prefix, so the suffix cannot grow the label set.
        if reason.starts_with("invalid_token_address") {
            return ReasonFamily::InvalidTokenAddress;
        }
        for (name, family) in PRODUCER_REASONS {
            if *name == reason {
                return *family;
            }
        }
        ReasonFamily::Other
    }
}

/// The JSON type of a value, for a classified (never fatal) report.
fn json_type_name(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

/// Truncate on a char boundary at or below `cap` bytes.
fn bounded(s: &str, cap: usize) -> String {
    if s.len() <= cap {
        return s.to_string();
    }
    let mut end = cap;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

#[cfg(test)]
mod dl02_frontier_tests {
    use super::{
        admission, bounded, read_verdict, Admission, ReasonFamily, Verdict, OUTCOME_DECISION_LOG,
        OUTCOME_ELIGIBLE, RAW_CAP, SKIP_REASON_PREFIX,
    };
    use serde_json::{json, Value};
    use shared_rs::contracts::Opportunity;
    use shared_rs::metrics::SIM_VALIDATED_FRONTIER_TOTAL;

    /// A payload shaped EXACTLY like the producer's. `publishValidated`
    /// (`selector-api/src/consumer.ts:447-458`) spreads the opportunity row and
    /// stamps `verdict` / `verdict_reason` next to it, so every `Opportunity`
    /// field is present here on purpose: the fixture MUST be a message the
    /// typed parse accepts, otherwise the bidirectional test would be proving
    /// nothing about the real stream.
    fn base_payload() -> Value {
        json!({
            "id": "11111111-1111-4111-8111-111111111111",
            "chain_id": 1,
            "strategy_kind": "dex_arb",
            "dex_a": "uniswap_v2",
            "dex_b": "sushiswap",
            "pair_symbol": "WETH/USDC",
            "token_in": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
            "token_out": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
            "amount_in_wei": "1000000000000000000",
            "expected_profit_usd": 12.5,
            "net_expected_profit_usd": 4.25,
            "roi_pct": 0.42,
            "risk_score": 0.1,
            "block_number": 21000000,
            "rejection_reason": null,
            "cartridge_id": null,
            "detector_id": null,
            "pipeline_latency_ms": 7,
            "economics": null,
            "detected_at": "2026-10-08T09:29:46Z",
            "trace_id": "22222222-2222-4222-8222-222222222222"
        })
    }

    /// The producer's message, with the two envelope keys injected.
    fn payload(verdict: Option<Value>, reason: Option<Value>) -> String {
        let mut v = base_payload();
        if let Some(x) = verdict {
            v["verdict"] = x;
        }
        if let Some(x) = reason {
            v["verdict_reason"] = x;
        }
        v.to_string()
    }

    /// The reader's safety property, asserted on EVERY fixture: reading the
    /// envelope must never make a message the typed parse could accept become a
    /// discarded one (an `Err` here is terminal in the consumer —
    /// `invalid_msg_parse` ACKs and drops the WHOLE entry).
    fn assert_typed_parse_accepts(label: &str, raw: &str) {
        match serde_json::from_str::<Opportunity>(raw) {
            Ok(opp) => assert_eq!(
                opp.chain_id, 1,
                "{label}: fixture must be the producer's row"
            ),
            Err(e) => panic!(
                "{label}: the envelope reader must never make a good message unparseable, \
                 but from_str::<Opportunity> failed: {e}"
            ),
        }
    }

    /// THE BIDIRECTIONAL TEST. One payload per side, on the SAME base row, so
    /// the only difference is the producer's verdict: `accept` admits and
    /// `reject` does not, and the two sets are mutually exclusive.
    #[test]
    fn accept_is_admitted_and_reject_is_not() {
        let accepted = payload(Some(json!("accept")), Some(json!(null)));
        let rejected = payload(Some(json!("reject")), Some(json!("producer_rejected")));
        assert_typed_parse_accepts("accept", &accepted);
        assert_typed_parse_accepts("reject", &rejected);

        let a = admission(&read_verdict(&accepted));
        let r = admission(&read_verdict(&rejected));

        assert!(
            a.is_eligible(),
            "verdict:accept must reach the simulation path"
        );
        assert!(
            !r.is_eligible(),
            "verdict:reject must NOT reach the simulation path"
        );
        // Exclusivity, stated as a property rather than as two assertions about
        // two different enum variants.
        assert_ne!(a, r, "the two verdicts must land in different sets");

        match r {
            Admission::DecisionLog { reason, family } => {
                assert_eq!(family, ReasonFamily::ProducerRejected);
                assert_eq!(family.label(), "reject:producer_rejected");
                assert_eq!(reason, "producer_verdict:reject:producer_rejected");
                assert!(reason.starts_with(SKIP_REASON_PREFIX));
            }
            Admission::EligibleSimulations => panic!("reject was promoted to accept"),
        }
    }

    /// THE DECLARED BOUNDARY CASE: no `verdict` key at all.
    ///
    /// Declared behaviour: FAIL-CLOSED — not admitted, and diagnosed under its
    /// own family so a producer regression is visible instead of silent. The
    /// alternative (absent ⇒ admitted) would let any payload that reaches the
    /// stream spend the fork; see the module docs.
    #[test]
    fn absent_verdict_is_fail_closed_and_named() {
        let raw = payload(None, None);
        assert_typed_parse_accepts("absent", &raw);
        assert_eq!(read_verdict(&raw), Verdict::Absent);

        match admission(&read_verdict(&raw)) {
            Admission::DecisionLog { reason, family } => {
                assert_eq!(family, ReasonFamily::EnvelopeMissing);
                assert_eq!(family.label(), "envelope_missing");
                assert_eq!(reason, "producer_verdict:envelope_missing");
            }
            Admission::EligibleSimulations => {
                panic!("a missing verdict was admitted — that is default-allow, not a frontier")
            }
        }
    }

    /// An explicit `null` is the same state as a missing key: serde folds both
    /// into `None` for `Option<..>`, and this frontier deliberately does not
    /// pretend they are distinguishable. Same fail-closed outcome.
    #[test]
    fn null_verdict_is_the_same_state_as_absent() {
        let raw = payload(Some(json!(null)), None);
        assert_typed_parse_accepts("null", &raw);
        assert_eq!(read_verdict(&raw), Verdict::Absent);
        assert!(!admission(&read_verdict(&raw)).is_eligible());
    }

    /// A reason the producer could not supply is reported AS ITS OWN family —
    /// never as an empty reason and never as an invented one.
    #[test]
    fn reject_without_a_reason_gets_its_own_family() {
        for missing in [None, Some(json!(null)), Some(json!(""))] {
            let raw = payload(Some(json!("reject")), missing.clone());
            match admission(&read_verdict(&raw)) {
                Admission::DecisionLog { reason, family } => {
                    assert_eq!(family, ReasonFamily::ReasonAbsent, "input {missing:?}");
                    assert_eq!(reason, "producer_verdict:reject:reason_absent");
                }
                Admission::EligibleSimulations => panic!("reject admitted"),
            }
        }
    }

    /// Unknown verdict STRINGS and wrong-typed values are both fail-closed, and
    /// both keep the message parseable — the second half is the property that
    /// protects the stream from a new producer key shape.
    #[test]
    fn unknown_and_malformed_verdicts_are_classified_never_fatal() {
        let unknown = payload(Some(json!("deferred")), None);
        assert_typed_parse_accepts("unknown", &unknown);
        assert_eq!(
            read_verdict(&unknown),
            Verdict::Unknown {
                raw: "deferred".to_string()
            }
        );
        match admission(&read_verdict(&unknown)) {
            Admission::DecisionLog { family, reason } => {
                assert_eq!(family, ReasonFamily::VerdictUnknown);
                assert_eq!(reason, "producer_verdict:verdict_unknown:deferred");
            }
            Admission::EligibleSimulations => panic!("an unknown verdict was admitted"),
        }

        // `#[serde(default)] verdict: Option<String>` would have returned `Err`
        // on every one of these (measured in PR #869) and an `Err` of the
        // ENVELOPE would have been a lost message. It is not an error here.
        for (label, bad) in [
            ("number", json!(123)),
            ("bool", json!(true)),
            ("object", json!({"kind": "accept"})),
            ("array", json!(["accept"])),
        ] {
            let raw = payload(Some(bad), None);
            assert_typed_parse_accepts(label, &raw);
            let v = read_verdict(&raw);
            match &v {
                Verdict::Malformed { kind } => assert_eq!(*kind, label, "type name for {label}"),
                other => panic!("{label}: expected Malformed, got {other:?}"),
            }
            match admission(&v) {
                Admission::DecisionLog { family, reason } => {
                    assert_eq!(family, ReasonFamily::VerdictMalformed);
                    assert_eq!(
                        reason,
                        format!("producer_verdict:verdict_malformed:{label}")
                    );
                }
                Admission::EligibleSimulations => panic!("{label}: malformed verdict admitted"),
            }
        }
    }

    /// TOTALLITY: for every observed state there is exactly ONE set, and it is
    /// one of the two. A third state, or a state that is both, would be the
    /// silent drop this change removes.
    #[test]
    fn every_observed_state_lands_in_exactly_one_set() {
        let cases: Vec<(&str, String)> = vec![
            ("accept", payload(Some(json!("accept")), None)),
            (
                "reject+reason",
                payload(Some(json!("reject")), Some(json!("score_below_min"))),
            ),
            ("absent", payload(None, None)),
            ("null", payload(Some(json!(null)), None)),
            ("unknown", payload(Some(json!("whatever")), None)),
            ("malformed", payload(Some(json!(0.5)), None)),
        ];
        let mut eligible = 0usize;
        for (label, raw) in &cases {
            assert_typed_parse_accepts(label, raw);
            let a = admission(&read_verdict(raw));
            match a {
                Admission::EligibleSimulations => eligible += 1,
                Admission::DecisionLog { reason, .. } => {
                    assert!(
                        reason.starts_with(SKIP_REASON_PREFIX),
                        "{label}: every diagnosed reason must be namespaced, got {reason}"
                    );
                }
            }
        }
        assert_eq!(
            eligible, 1,
            "exactly one of the observed states is admissible (accept)"
        );
        assert_eq!(cases.len() - eligible, 5, "the other five are diagnosed");
    }

    /// The label set is bounded: a raw producer string must never become a
    /// label, and a new reason must fall into `Other` rather than leak.
    #[test]
    fn reason_families_stay_bounded() {
        assert_eq!(
            ReasonFamily::from_producer_reason("producer_rejected"),
            ReasonFamily::ProducerRejected
        );
        assert_eq!(
            ReasonFamily::from_producer_reason("invalid_token_address:0xdeadbeef"),
            ReasonFamily::InvalidTokenAddress
        );
        assert_eq!(
            ReasonFamily::from_producer_reason("a_reason_invented_tomorrow"),
            ReasonFamily::Other
        );
        // The READER is the boundary that ingests untrusted input, so the cap
        // belongs there — a 500-char verdict string must not travel into a
        // persisted reason at full length.
        let long = "x".repeat(500);
        let raw = payload(Some(json!(long.clone())), None);
        match read_verdict(&raw) {
            Verdict::Unknown { raw: extracted } => assert!(
                extracted.len() <= RAW_CAP,
                "the reader must cap an untrusted verdict string ({} bytes kept)",
                extracted.len()
            ),
            other => panic!("a 500-char verdict must classify as Unknown, got {other:?}"),
        }
        // And the reason built from the capped value stays far below the raw
        // payload's length.
        match admission(&Verdict::Unknown {
            raw: bounded(&long, RAW_CAP),
        }) {
            Admission::DecisionLog { reason, .. } => assert!(
                reason.len() < long.len(),
                "the persisted reason must not carry the raw string at full length"
            ),
            Admission::EligibleSimulations => panic!("unknown verdict admitted"),
        }
    }

    /// TELEMETRY OF THE TWO SETS: both move, and the reason family is carried.
    ///
    /// Asserted as a STRICT INCREASE per series rather than an exact delta:
    /// counters are process-global and the test harness runs tests in parallel,
    /// so exact deltas would be a flakiness generator. The labels used here are
    /// not recorded by any sibling test in this module.
    #[test]
    fn both_sets_are_counted_and_reject_carries_its_family() {
        let eligible = || {
            SIM_VALIDATED_FRONTIER_TOTAL
                .with_label_values(&[OUTCOME_ELIGIBLE, ReasonFamily::Accept.label()])
                .get()
        };
        let malformed = || {
            SIM_VALIDATED_FRONTIER_TOTAL
                .with_label_values(&[OUTCOME_DECISION_LOG, ReasonFamily::VerdictMalformed.label()])
                .get()
        };

        let e0 = eligible();
        admission(&Verdict::Accept).record();
        assert!(
            eligible() > e0,
            "the admissible set must be counted — otherwise 'passed=0' stays uninterpretable"
        );

        let m0 = malformed();
        admission(&Verdict::Malformed { kind: "number" }).record();
        assert!(
            malformed() > m0,
            "the diagnosed set must be counted per reason family"
        );
    }

    /// WIRING INVARIANT (structural, not executable — see the report): the
    /// frontier must sit BEFORE the fork budget and BEFORE every simulator, and
    /// the diagnosed set must be RECORDED, not dropped.
    ///
    /// The full consumer cannot be executed in this gate (it needs Redis, PG
    /// and a fork), so the placement is pinned the way this repository already
    /// pins cross-file contracts (`pipeline-invariants.test.ts`,
    /// `sel-gate01.test.ts` assert on source text for the same reason).
    #[test]
    fn frontier_runs_before_the_fork_budget_and_the_simulators() {
        let src = include_str!("consumer.rs");
        let body = {
            let Some(start) = src.find("async fn process_message(") else {
                panic!("consumer.rs must define process_message");
            };
            let Some(end) = src[start..].find("async fn finish(") else {
                panic!("process_message must be followed by finish()");
            };
            &src[start..start + end]
        };
        let at = |needle: &str| match body.find(needle) {
            Some(i) => i,
            None => panic!("process_message must contain {needle:?}"),
        };

        let frontier = at("decision_frontier::admission(");
        // No fork budget: rejects must not consume a rate or in-flight permit.
        assert!(
            frontier < at("in_flight.clone().try_acquire_owned()"),
            "the frontier must run before the in-flight permit is acquired"
        );
        assert!(
            frontier < at("bound.try_admit(now)"),
            "the frontier must run before the rate budget is consumed"
        );
        // No simulator, on either path.
        assert!(
            frontier < at("self.simulate_b2c("),
            "the frontier must run before the B2c REVM path"
        );
        assert!(
            frontier < at(".simulate_with_route("),
            "the frontier must run before the legacy backend path"
        );
        // Exactly one dispatch each: a second one could bypass the frontier.
        assert_eq!(body.matches("self.simulate_b2c(").count(), 1);
        assert_eq!(body.matches(".simulate_with_route(").count(), 1);
        // Recorded, not dropped: the diagnosed arm reaches the shared tail.
        // Both offsets are converted back to absolute body indices — comparing
        // a relative offset against an absolute one is how this assertion would
        // silently pass.
        let dl = match body[frontier..].find("Admission::DecisionLog") {
            Some(i) => frontier + i,
            None => panic!("the frontier must have a DecisionLog arm"),
        };
        let ret = match body[dl..].find("return self.finish(") {
            Some(i) => dl + i,
            None => panic!("the diagnosed set must be persisted through finish(), never discarded"),
        };
        assert!(
            ret < at("self.simulate_b2c("),
            "the diagnosed arm must return before any simulation"
        );
    }

    /// The prefix is a cross-crate contract: `persistence::is_sim_capability_gap`
    /// recognises it so a skipped simulation never overwrites the producer's
    /// `rejection_reason`. Pinned as a literal so a rename cannot pass silently.
    #[test]
    fn skip_reason_prefix_is_the_one_the_classifier_knows() {
        assert_eq!(SKIP_REASON_PREFIX, "producer_verdict:");
        assert!(!admission(&Verdict::Absent).is_eligible());
    }
}
