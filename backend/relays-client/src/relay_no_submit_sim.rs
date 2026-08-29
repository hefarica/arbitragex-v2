//! ARBX-RDY-05 (A.7) — private-relay NO-SUBMIT simulation.
//!
//! §32/§33 doctrine (audit/scaffold/shadow/read-only): this module NEVER
//! submits. Validation is 100% LOCAL against each relay's publicly documented
//! `eth_sendBundle` acceptance shape; the bundle is consumed, validated, and
//! discarded (`drop`) — log-only. There is ZERO network egress on this path:
//! the module imports no `reqwest`, no `tokio`, no client/provider of any
//! kind (inspect the `use` statements below — that absence IS the proof).
//!
//! Documented shapes encoded per relay (the differences are deliberate):
//! - Flashbots Protect — docs.flashbots.net/flashbots-auction/advanced/rpc-endpoint:
//!   `txs` REQUIRED (raw signed txs; ≤100 txs and ≤300kB per bundle),
//!   `blockNumber` REQUIRED (hex quantity), `minTimestamp`/`maxTimestamp`
//!   OPTIONAL plain Numbers, `revertingTxHashes` OPTIONAL 32-byte hashes.
//! - MEV-Blocker — docs.mevblocker.io/how-to/searchers/bid:
//!   `txs` REQUIRED; the documented searcher flow is a 2-tx backrun bundle
//!   whose FIRST element is the HASH of the pending target tx ("instead of
//!   the fully encoded transaction"), `blockNumber` OPTIONAL hex quantity
//!   ("Default, current block number"), `replacementUuid` OPTIONAL.
//!   `revertingTxHashes`/timestamps are NOT in the documented shape →
//!   tolerated, not vouched.
//! - Titan — docs.titanbuilder.xyz/api/eth_sendbundle:
//!   `txs` required (raw signed txs; "list can be empty for bundle
//!   cancellations" — this sim validates SUBMISSION bundles, so ≥1 tx),
//!   `blockNumber` OPTIONAL hex quantity ("Default, current block number"),
//!   `minTimestamp` OPTIONAL Number, `revertingTxHashes` OPTIONAL.
//!   `maxTimestamp` is NOT documented → tolerated, not vouched.
//!
//! Design decision — UNSIGNED / shape-only: `bundle_builder::build_and_sign`
//! requires a live alloy provider (network), so it can never sit on a
//! zero-egress path. Validators therefore check the documented WIRE SHAPE of
//! the params (0x-hex encoding, lengths, documented caps, requiredness), never
//! a signature. No env/secret key is read anywhere in this module; tests use
//! arbitrary well-formed 0x-hex strings, so no key fixture is needed at all.
//!
//! Wiring note: `main.rs` has no CLI subcommand pattern — this is exposed as
//! a library function and unit-tested here; wiring a CLI entry is deferred.
#![allow(dead_code)] // exercised by unit tests; no binary caller yet (CLI wiring deferred)

use crate::bundle_builder::SignedBundle;
use tracing::{debug, info};

// ─── Payload ─────────────────────────────────────────────────────────────────

/// The `eth_sendBundle` params object this crate would wire onto a relay —
/// mirrored locally (typed, owned) for shape validation. Built from the
/// existing [`SignedBundle`] type; never serialized or sent from this module.
#[derive(Debug, Clone)]
pub struct SimBundleParams {
    /// `txs` — array of 0x-hex entries (raw signed txs; MEV-Blocker's flow
    /// also allows a target-tx HASH as the first element).
    pub txs: Vec<String>,
    /// `blockNumber` — hex quantity (`0x…`). REQUIRED by Flashbots; optional
    /// (defaults to current block) for MEV-Blocker and Titan.
    pub block_number: Option<String>,
    /// `minTimestamp` — optional plain Number (unix seconds), by typing.
    pub min_timestamp: Option<u64>,
    /// `maxTimestamp` — optional plain Number (unix seconds), by typing.
    /// Documented by Flashbots only; tolerated elsewhere.
    pub max_timestamp: Option<u64>,
    /// `revertingTxHashes` — optional array of 32-byte 0x-hex tx hashes.
    pub reverting_tx_hashes: Vec<String>,
}

impl SimBundleParams {
    /// Canonical single-tx wire shape (mirrors `relay_flashbots::BundleRequest`):
    /// `txs=[tx_raw_hex]`, `blockNumber="0x{target_block:x}"`.
    pub fn from_signed_bundle(bundle: &SignedBundle) -> Self {
        Self {
            txs: vec![bundle.tx_raw_hex.clone()],
            block_number: Some(format!("0x{:x}", bundle.target_block)),
            min_timestamp: None,
            max_timestamp: None,
            reverting_tx_hashes: Vec::new(),
        }
    }
}

// ─── Verdict ─────────────────────────────────────────────────────────────────

/// Per-relay outcome of the LOCAL schema check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelayVerdict {
    /// Bundle params satisfy the relay's documented acceptance shape.
    AcceptedShape,
    /// Bundle params violate the documented shape; carries a compact
    /// machine-parseable reason (e.g. `block_number_required`).
    RejectedShape(String),
}

impl RelayVerdict {
    /// True when the relay's documented shape accepts these params.
    pub fn is_accepted(&self) -> bool {
        matches!(self, RelayVerdict::AcceptedShape)
    }

    /// Compact log form: `accepted_shape` / `rejected_shape:<reason>`.
    pub fn as_log_str(&self) -> String {
        match self {
            RelayVerdict::AcceptedShape => "accepted_shape".to_string(),
            RelayVerdict::RejectedShape(reason) => format!("rejected_shape:{reason}"),
        }
    }
}

// ─── Aggregate report ────────────────────────────────────────────────────────

/// Aggregated verdict for one no-submit simulation run: all three doctrine
/// relays plus the summary.
#[derive(Debug, Clone)]
pub struct NoSubmitReport {
    /// Flashbots Protect verdict.
    pub flashbots: RelayVerdict,
    /// MEV-Blocker verdict.
    pub mev_blocker: RelayVerdict,
    /// Titan verdict.
    pub titan: RelayVerdict,
}

impl NoSubmitReport {
    /// How many of the three doctrine relays accept this bundle's shape.
    pub fn accepted_count(&self) -> usize {
        [&self.flashbots, &self.mev_blocker, &self.titan]
            .into_iter()
            .filter(|v| v.is_accepted())
            .count()
    }

    /// Compact one-line summary for logs.
    pub fn summary(&self) -> String {
        format!(
            "accepted={}/3 flashbots={} mev_blocker={} titan={}",
            self.accepted_count(),
            self.flashbots.as_log_str(),
            self.mev_blocker.as_log_str(),
            self.titan.as_log_str(),
        )
    }
}

// ─── Local hex helpers (no decode, no allocation) ────────────────────────────

/// Hex QUANTITY per JSON-RPC: `0x` + ≥1 hex digit (odd digit counts are legal
/// — Titan/MEV-Blocker docs themselves show `0x102286B`).
fn is_hex_quantity(s: &str) -> bool {
    match s.strip_prefix("0x") {
        Some(rest) => !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_hexdigit()),
        None => false,
    }
}

/// Raw transaction bytes: `0x` + non-empty EVEN-length hex (whole bytes).
fn is_raw_tx_hex(s: &str) -> bool {
    match s.strip_prefix("0x") {
        Some(rest) => {
            !rest.is_empty() && rest.len() % 2 == 0 && rest.bytes().all(|b| b.is_ascii_hexdigit())
        }
        None => false,
    }
}

/// 32-byte hash (`0x` + exactly 64 hex digits) — tx hash / reverting hash form.
fn is_0x_hash32(s: &str) -> bool {
    match s.strip_prefix("0x") {
        Some(rest) => rest.len() == 64 && rest.bytes().all(|b| b.is_ascii_hexdigit()),
        None => false,
    }
}

/// Decoded byte length of a `0x…` hex string. Callers must pre-validate with
/// [`is_raw_tx_hex`] — this is pure arithmetic for the documented size cap.
fn decoded_byte_len(s: &str) -> usize {
    s.len().saturating_sub(2) / 2
}

// ─── Per-relay local validators ──────────────────────────────────────────────

/// Flashbots Protect: strictest documented shape — `blockNumber` REQUIRED,
/// ≤100 txs, ≤300kB, every `txs` element a fully signed raw transaction.
fn validate_flashbots(p: &SimBundleParams) -> RelayVerdict {
    if p.txs.is_empty() {
        return RelayVerdict::RejectedShape("txs_empty".into());
    }
    for (i, tx) in p.txs.iter().enumerate() {
        if !is_raw_tx_hex(tx) {
            return RelayVerdict::RejectedShape(format!("txs[{i}]_not_0x_raw_hex"));
        }
        if decoded_byte_len(tx) == 32 {
            // A 32-byte entry is a tx HASH, not a signed tx (RLP-signed txs
            // are always >32 bytes). Flashbots documents `txs` as signed txs.
            return RelayVerdict::RejectedShape(format!("txs[{i}]_is_32b_hash_not_signed_tx"));
        }
    }
    if p.txs.len() > 100 {
        return RelayVerdict::RejectedShape("txs_count_over_100_cap".into());
    }
    let total_bytes: usize = p.txs.iter().map(|tx| decoded_byte_len(tx)).sum();
    if total_bytes > 300_000 {
        return RelayVerdict::RejectedShape("bundle_over_300000_bytes_cap".into());
    }
    // Flashbots is the only one of the three that REQUIRES blockNumber.
    let Some(block_number) = &p.block_number else {
        return RelayVerdict::RejectedShape("block_number_required".into());
    };
    if !is_hex_quantity(block_number) {
        return RelayVerdict::RejectedShape("block_number_not_0x_hex_quantity".into());
    }
    // minTimestamp/maxTimestamp: OPTIONAL plain Numbers (unix seconds). Typed
    // `Option<u64>` in SimBundleParams — "plain int, not hex" holds by
    // construction; no further documented local constraint.
    for (i, h) in p.reverting_tx_hashes.iter().enumerate() {
        if !is_0x_hash32(h) {
            return RelayVerdict::RejectedShape(format!(
                "reverting_tx_hashes[{i}]_not_0x_32b_hash"
            ));
        }
    }
    RelayVerdict::AcceptedShape
}

/// Titan: `blockNumber` OPTIONAL ("Default, current block number"), no
/// documented tx-count/size cap, `maxTimestamp` not documented (tolerated).
fn validate_titan(p: &SimBundleParams) -> RelayVerdict {
    if p.txs.is_empty() {
        // Titan's reference notes the txs "list can be empty for bundle
        // cancellations" — this sim validates SUBMISSION bundles only.
        return RelayVerdict::RejectedShape("txs_empty_for_submission".into());
    }
    for (i, tx) in p.txs.iter().enumerate() {
        if !is_raw_tx_hex(tx) {
            return RelayVerdict::RejectedShape(format!("txs[{i}]_not_0x_raw_hex"));
        }
        if decoded_byte_len(tx) == 32 {
            return RelayVerdict::RejectedShape(format!("txs[{i}]_is_32b_hash_not_signed_tx"));
        }
    }
    if let Some(b) = &p.block_number {
        if !is_hex_quantity(b) {
            return RelayVerdict::RejectedShape("block_number_not_0x_hex_quantity".into());
        }
    }
    // minTimestamp: OPTIONAL Number (typed Option<u64>). maxTimestamp: not in
    // Titan's documented parameter table — tolerated, not vouched.
    for (i, h) in p.reverting_tx_hashes.iter().enumerate() {
        if !is_0x_hash32(h) {
            return RelayVerdict::RejectedShape(format!(
                "reverting_tx_hashes[{i}]_not_0x_32b_hash"
            ));
        }
    }
    RelayVerdict::AcceptedShape
}

/// MEV-Blocker: documented searcher flow is a 2-tx backrun bundle whose FIRST
/// element is the pending target's HASH; `blockNumber` OPTIONAL hex quantity.
/// `revertingTxHashes` and timestamps are not in the documented searcher
/// shape → tolerated, not vouched.
fn validate_mev_blocker(p: &SimBundleParams) -> RelayVerdict {
    if p.txs.is_empty() {
        return RelayVerdict::RejectedShape("txs_empty".into());
    }
    for (i, tx) in p.txs.iter().enumerate() {
        // Documented convention: the first element may be the target tx HASH
        // "instead of the fully encoded transaction" (docs.mevblocker.io).
        if i == 0 && is_0x_hash32(tx) {
            continue;
        }
        if !is_raw_tx_hex(tx) {
            return RelayVerdict::RejectedShape(format!("txs[{i}]_not_0x_raw_hex"));
        }
    }
    if let Some(b) = &p.block_number {
        if !is_hex_quantity(b) {
            return RelayVerdict::RejectedShape("block_number_not_0x_hex_quantity".into());
        }
    }
    RelayVerdict::AcceptedShape
}

// ─── Entry point ─────────────────────────────────────────────────────────────

/// Run the LOCAL three-relay validation and DISCARD the bundle.
///
/// Consumes `params` by value: once verdicts are computed the payload is
/// explicitly `drop`ped — nothing is forwarded, persisted, or transmitted
/// (§32/§33). Zero network egress by construction: this module imports no
/// HTTP client and holds no I/O capability whatsoever.
///
/// Logging (R9 anti-logflood): ONE `info!` per run carrying the per-relay
/// verdicts; detail lives at `debug!`.
pub fn validate_and_discard(params: SimBundleParams) -> NoSubmitReport {
    let report = validate_all(&params);

    // Loud discard — the bundle leaves scope here, log-only.
    drop(params);

    info!(
        event = "relay_sim.no_submit.validated",
        accepted = report.accepted_count(),
        total = 3,
        flashbots = %report.flashbots.as_log_str(),
        mev_blocker = %report.mev_blocker.as_log_str(),
        titan = %report.titan.as_log_str(),
        "no-submit simulation: bundle validated locally against 3 relay schemas and discarded (zero network egress)"
    );

    debug!(
        event = "relay_sim.no_submit.detail",
        summary = %report.summary(),
        "no-submit simulation run detail"
    );

    report
}

/// All three doctrine-relay validators over one borrowed payload.
fn validate_all(p: &SimBundleParams) -> NoSubmitReport {
    NoSubmitReport {
        flashbots: validate_flashbots(p),
        mev_blocker: validate_mev_blocker(p),
        titan: validate_titan(p),
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)] // M11: test module
    use super::*;
    use ethers::types::{Address, H256, U256};

    /// Arbitrary well-formed 0x-hex raw-tx stand-in of `bytes` bytes.
    /// Shape-only sim: no signature, no key (see module docs).
    fn raw_tx_hex(bytes: usize) -> String {
        format!("0x{}", "ab".repeat(bytes))
    }

    /// A 32-byte 0x-hex hash (`0x` + 64 hex digits).
    fn hash32_hex() -> String {
        format!("0x{}", "cd".repeat(32))
    }

    /// SignedBundle fixture — same dummy-field pattern as `multi_relay` tests.
    fn signed_bundle_fixture() -> SignedBundle {
        SignedBundle {
            opportunity_id: uuid::Uuid::new_v4(),
            target_block: 20_000_000,
            tx_raw_hex: raw_tx_hex(110),
            tx_hash: H256::zero(),
            from: Address::zero(),
            nonce: 0,
            value_wei: U256::zero(),
        }
    }

    /// Bundle params valid for ALL three documented shapes. `0x102286b` is the
    /// literal blockNumber form used in the MEV-Blocker docs example (odd hex
    /// digit count — a legal JSON-RPC quantity).
    fn valid_params() -> SimBundleParams {
        SimBundleParams {
            txs: vec![raw_tx_hex(110)],
            block_number: Some("0x102286b".to_string()),
            min_timestamp: None,
            max_timestamp: None,
            reverting_tx_hashes: Vec::new(),
        }
    }

    fn assert_rejected_with(v: &RelayVerdict, needle: &str) {
        match v {
            RelayVerdict::RejectedShape(reason) => {
                assert!(reason.contains(needle), "reason={reason} lacks '{needle}'");
            }
            RelayVerdict::AcceptedShape => {
                panic!("expected rejected_shape containing '{needle}', got accepted_shape");
            }
        }
    }

    /// Required case: valid bundle → all three relays accepted_shape.
    #[test]
    fn test_valid_bundle_accepted_by_all_three_relays() {
        let report =
            validate_and_discard(SimBundleParams::from_signed_bundle(&signed_bundle_fixture()));
        assert_eq!(report.flashbots, RelayVerdict::AcceptedShape);
        assert_eq!(report.mev_blocker, RelayVerdict::AcceptedShape);
        assert_eq!(report.titan, RelayVerdict::AcceptedShape);
        assert_eq!(report.accepted_count(), 3);
    }

    /// Required case: missing txs → rejected with a txs reason by all three.
    #[test]
    fn test_empty_txs_rejected_by_all_three_relays() {
        let mut p = valid_params();
        p.txs = Vec::new();
        let r = validate_all(&p);
        assert_rejected_with(&r.flashbots, "txs");
        assert_rejected_with(&r.mev_blocker, "txs");
        assert_rejected_with(&r.titan, "txs");
        assert_eq!(r.accepted_count(), 0);
    }

    /// Required case: malformed hex tx (no 0x / odd length / non-hex / empty)
    /// → rejected by all three.
    #[test]
    fn test_malformed_hex_tx_rejected_by_all_three_relays() {
        for bad in ["deadbeef", "0xabc", "0xzzgg", "0x"] {
            let mut p = valid_params();
            p.txs = vec![bad.to_string()];
            let r = validate_all(&p);
            assert_rejected_with(&r.flashbots, "txs[0]");
            assert_rejected_with(&r.mev_blocker, "txs[0]");
            assert_rejected_with(&r.titan, "txs[0]");
        }
    }

    /// Required case: bad-format reverting hash → rejected by the relays that
    /// DOCUMENT the field (Flashbots, Titan). MEV-Blocker's documented
    /// searcher shape has no revertingTxHashes → tolerated.
    #[test]
    fn test_malformed_reverting_hash_rejected_by_documenting_relays() {
        let mut p = valid_params();
        p.reverting_tx_hashes = vec!["0x1234".to_string()]; // not 32 bytes
        let r = validate_all(&p);
        assert_rejected_with(&r.flashbots, "reverting_tx_hashes");
        assert_rejected_with(&r.titan, "reverting_tx_hashes");
        assert_eq!(r.mev_blocker, RelayVerdict::AcceptedShape);
    }

    /// Well-formed reverting hash + timestamps → accepted everywhere that
    /// documents them; timestamps are typed Option<u64> (plain Numbers).
    #[test]
    fn test_well_formed_optional_fields_accepted() {
        let mut p = valid_params();
        p.reverting_tx_hashes = vec![hash32_hex()];
        p.min_timestamp = Some(1_700_000_000);
        p.max_timestamp = Some(1_700_000_060);
        let r = validate_all(&p);
        assert_eq!(r.flashbots, RelayVerdict::AcceptedShape);
        assert_eq!(r.titan, RelayVerdict::AcceptedShape);
        assert_eq!(r.mev_blocker, RelayVerdict::AcceptedShape);
    }

    /// Documented differential: blockNumber REQUIRED only by Flashbots;
    /// Titan and MEV-Blocker document it optional ("default, current block").
    #[test]
    fn test_block_number_required_only_for_flashbots() {
        let mut p = valid_params();
        p.block_number = None;
        let r = validate_all(&p);
        assert_rejected_with(&r.flashbots, "block_number_required");
        assert_eq!(r.mev_blocker, RelayVerdict::AcceptedShape);
        assert_eq!(r.titan, RelayVerdict::AcceptedShape);
        assert_eq!(r.accepted_count(), 2);
    }

    /// Malformed blockNumber (present but not a hex quantity) → rejected by
    /// all three (each documents the hex-encoded form).
    #[test]
    fn test_malformed_block_number_rejected_when_present() {
        for bad in ["102286b", "0xzz"] {
            let mut p = valid_params();
            p.block_number = Some(bad.to_string());
            let r = validate_all(&p);
            assert_rejected_with(&r.flashbots, "block_number");
            assert_rejected_with(&r.mev_blocker, "block_number");
            assert_rejected_with(&r.titan, "block_number");
        }
    }

    /// Documented differential: MEV-Blocker's backrun flow allows the FIRST
    /// txs element to be the target tx HASH; Flashbots/Titan require raw
    /// signed txs (a 32-byte entry cannot be one).
    #[test]
    fn test_hash_first_tx_only_accepted_by_mev_blocker() {
        let mut p = valid_params();
        p.txs = vec![hash32_hex(), raw_tx_hex(110)];
        let r = validate_all(&p);
        assert_rejected_with(&r.flashbots, "hash");
        assert_rejected_with(&r.titan, "hash");
        assert_eq!(r.mev_blocker, RelayVerdict::AcceptedShape);
    }

    /// Documented differential: the 100-tx cap is documented by Flashbots
    /// only; Titan and MEV-Blocker document no count cap.
    #[test]
    fn test_tx_count_cap_enforced_only_by_flashbots() {
        let mut p = valid_params();
        p.txs = vec![raw_tx_hex(10); 101];
        let r = validate_all(&p);
        assert_rejected_with(&r.flashbots, "txs_count_over_100_cap");
        assert_eq!(r.mev_blocker, RelayVerdict::AcceptedShape);
        assert_eq!(r.titan, RelayVerdict::AcceptedShape);
    }

    /// Documented differential: the 300kB bundle cap is documented by
    /// Flashbots only (2 × 150,001 bytes = 300,002 > 300,000).
    #[test]
    fn test_bundle_size_cap_enforced_only_by_flashbots() {
        let mut p = valid_params();
        p.txs = vec![raw_tx_hex(150_001), raw_tx_hex(150_001)];
        let r = validate_all(&p);
        assert_rejected_with(&r.flashbots, "bundle_over_300000_bytes_cap");
        assert_eq!(r.mev_blocker, RelayVerdict::AcceptedShape);
        assert_eq!(r.titan, RelayVerdict::AcceptedShape);
    }

    /// `from_signed_bundle` maps the canonical wire shape this crate already
    /// sends (txs=[tx_raw_hex], blockNumber=0x{target_block:x}).
    #[test]
    fn test_from_signed_bundle_maps_canonical_wire_shape() {
        let b = signed_bundle_fixture();
        let p = SimBundleParams::from_signed_bundle(&b);
        assert_eq!(p.txs, vec![b.tx_raw_hex.clone()]);
        assert_eq!(p.block_number, Some(format!("0x{:x}", b.target_block)));
        assert!(p.min_timestamp.is_none());
        assert!(p.max_timestamp.is_none());
        assert!(p.reverting_tx_hashes.is_empty());
        // The mapped shape is valid for all three relays.
        let r = validate_all(&p);
        assert_eq!(r.accepted_count(), 3);
    }

    /// Verdict/report helpers used by the logging path.
    #[test]
    fn test_verdict_and_summary_helpers() {
        let mut r = validate_all(&valid_params());
        assert_eq!(
            r.summary(),
            "accepted=3/3 flashbots=accepted_shape mev_blocker=accepted_shape titan=accepted_shape"
        );
        r.flashbots = RelayVerdict::RejectedShape("block_number_required".into());
        assert_eq!(r.accepted_count(), 2);
        assert_eq!(
            r.flashbots.as_log_str(),
            "rejected_shape:block_number_required"
        );
        assert!(!r.flashbots.is_accepted());
    }
}
