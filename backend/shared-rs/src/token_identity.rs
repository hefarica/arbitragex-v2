//! ARBX-0018 — runtime token identity: `(chain_id, address)`.
//!
//! Doctrine (REQ-QB-002): the runtime identity of a token is its address on
//! its chain. Symbols are METADATA ONLY — display, operator config input,
//! price-oracle keys — and NEVER bind a gate decision. A symbol string that
//! happens to match the operator's allowlist is NOT a token; only an address
//! resolved through the chain's own token universe is.
//!
//! `TokenIdentityIndex` is the one-shot resolution of the operator's symbol
//! allowlist against the authoritative per-chain universe
//! (`arbx:tokens:<chain_id>:<addr>` → `{symbol, decimals, is_stablecoin}`,
//! populated by the token-enricher). The result is address-keyed:
//!
//! - `is_allowed_addr(addr)` — the ONLY allowlist predicate used at runtime
//!   when an index is attached to the evaluator.
//! - `symbol_for_addr(addr)` — metadata feed for the symbol-keyed price
//!   stack (`arbx:token_prices:<chain>` hash fields are uppercase symbols);
//!   the oracle contract does not change, only WHO resolves the symbol.
//!
//! Fail-honest semantics (R8):
//! - An operator symbol matching NO universe entry is recorded in
//!   `unresolved_symbols` — reported, never fabricated into an address.
//! - A NON-EMPTY operator list that resolves to zero addresses is
//!   FAIL-CLOSED (`is_allowed_addr` → false for everything), NOT permissive.
//! - An EMPTY operator list stays permissive (legacy semantics — never
//!   silently paralyse a freshly-seeded config).
//! - Two addresses sharing one symbol are BOTH allowed when the operator
//!   allows that symbol: symbol is the operator's granularity, address is
//!   the runtime's identity. Disambiguation is the operator's call — and
//!   TW-002 gives it to them directly: allowlist entries in address form
//!   (`0x` + 40 hex) are their own identity (no universe resolution, never
//!   "unresolved"); symbol entries keep the legacy resolution path.
//!
//! The index is a snapshot (cache TTL 30s at the composition site). A token
//! enriched less than one TTL ago may be rejected until the next refresh —
//! bounded staleness, honest reason (`TokenNotAllowed:<addr>`), identical
//! shape to the pre-index cold-cache behaviour.
//!
//! Pure std — no Redis, no serde — so probes compile it standalone
//! (AppControl blocks proc-macro DLLs in the full crate).

use std::collections::{HashMap, HashSet};

/// Address-keyed resolution of the operator's token allowlist for ONE chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenIdentityIndex {
    chain_id: u64,
    /// Normalized (lowercase, trimmed) address → symbol, as declared by the
    /// chain universe. Metadata only — never a gate key.
    symbols: HashMap<String, String>,
    /// Resolved allowlist of normalized addresses. `None` = permissive
    /// (operator allowlist empty). `Some(empty)` = fail-closed (every listed
    /// symbol is unresolved).
    allowed: Option<HashSet<String>>,
    /// Operator symbols that matched no universe entry, operator order,
    /// deduplicated. R8: reported, never invented.
    unresolved_symbols: Vec<String>,
}

/// Normalize an address for identity comparison: trim + lowercase. Same
/// convention as the reserves/token cache keys.
fn norm_addr(addr: &str) -> String {
    addr.trim().to_ascii_lowercase()
}

/// Normalize a symbol for matching: trim + uppercase (operators type
/// "WETH", "weth", " Weth " — one token).
fn norm_symbol(symbol: &str) -> String {
    symbol.trim().to_ascii_uppercase()
}

/// True when `entry` is in EVM address form: `0x`/`0X` prefix + exactly 40
/// ASCII hex characters. Checksum case is accepted (identity compare runs
/// on the normalized lowercase form — EIP-55 is formatting, not identity).
/// TW-002: such entries ARE their own TokenKey; symbols are metadata.
pub fn is_address_form(entry: &str) -> bool {
    let e = entry.trim();
    let Some(hex) = e.strip_prefix("0x").or_else(|| e.strip_prefix("0X")) else {
        return false;
    };
    hex.len() == 40 && hex.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Canonical normalization for ONE allowlist entry (TW-002): address-form
/// entries lowercase (identity compare), everything else uppercases as a
/// symbol. Shared by `resolve` and the composition-site cache check so the
/// two never disagree on form.
pub fn normalize_allowlist_entry(entry: &str) -> String {
    let t = entry.trim();
    if is_address_form(t) {
        t.to_ascii_lowercase()
    } else {
        t.to_ascii_uppercase()
    }
}

impl TokenIdentityIndex {
    /// Resolve the operator allowlist against `universe` — a slice of
    /// `(address, symbol)` pairs as declared by the chain's token universe
    /// (order irrelevant; duplicate addresses resolve first-wins,
    /// deterministic regardless of source order).
    ///
    /// TW-002: an entry in address form (`0x` + 40 hex) is its own
    /// identity — inserted into the allowed set directly, with no universe
    /// lookup and never reported unresolved. A token allowed by address but
    /// absent from the universe still gates IN; its pricing then misses
    /// honestly (`symbol_for_addr` → `None`), never fabricated.
    pub fn resolve(
        chain_id: u64,
        allowed_symbols: &[String],
        universe: &[(String, String)],
    ) -> Self {
        let mut symbols = HashMap::with_capacity(universe.len());
        for (addr, sym) in universe {
            let addr = norm_addr(addr);
            if addr.is_empty() {
                continue; // degenerate universe row — skip, never fabricate
            }
            symbols.entry(addr).or_insert_with(|| sym.clone());
        }

        // Operator entries after form-aware normalization: address-form
        // entries carry their identity, everything else resolves as a
        // symbol (retro-compat). Empty after trim → permissive.
        let mut addr_entries: Vec<String> = Vec::new();
        let mut wanted: Vec<String> = Vec::new();
        for e in allowed_symbols {
            if is_address_form(e) {
                addr_entries.push(norm_addr(e));
            } else {
                let n = norm_symbol(e);
                if !n.is_empty() {
                    wanted.push(n);
                }
            }
        }

        if addr_entries.is_empty() && wanted.is_empty() {
            return Self {
                chain_id,
                symbols,
                allowed: None,
                unresolved_symbols: Vec::new(),
            };
        }

        let mut set: HashSet<String> = addr_entries.into_iter().collect();
        let mut seen_unresolved: HashSet<String> = HashSet::new();
        let mut unresolved = Vec::new();
        for want in &wanted {
            let mut matched = false;
            for (addr, sym) in &symbols {
                if &norm_symbol(sym) == want {
                    set.insert(addr.clone());
                    matched = true;
                }
            }
            if !matched && seen_unresolved.insert(want.clone()) {
                unresolved.push(want.clone());
            }
        }
        Self {
            chain_id,
            symbols,
            allowed: Some(set),
            unresolved_symbols: unresolved,
        }
    }

    /// The allowlist predicate. `(chain_id, address)` is the identity: the
    /// chain is fixed per index, the address is normalized here.
    pub fn is_allowed_addr(&self, addr: &str) -> bool {
        match &self.allowed {
            None => true,
            Some(set) => set.contains(&norm_addr(addr)),
        }
    }

    /// Symbol metadata for an address (`None` = not in the universe —
    /// callers feed the raw address onward so downstream misses stay
    /// honest instead of silently renaming).
    pub fn symbol_for_addr(&self, addr: &str) -> Option<&str> {
        self.symbols.get(&norm_addr(addr)).map(String::as_str)
    }

    /// Chain this index was resolved for.
    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }

    /// Operator symbols that matched nothing — telemetry for the operator
    /// (their allowlist has dead entries; the dashboard surfaces the gap).
    pub fn unresolved_symbols(&self) -> &[String] {
        &self.unresolved_symbols
    }

    /// Number of distinct addresses the allowlist resolved to.
    pub fn allowed_addr_count(&self) -> usize {
        self.allowed.as_ref().map(|s| s.len()).unwrap_or(0)
    }

    /// Size of the token universe snapshot.
    pub fn universe_len(&self) -> usize {
        self.symbols.len()
    }

    /// True when the operator allowlist is empty (permissive mode).
    pub fn is_permissive(&self) -> bool {
        self.allowed.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> String {
        v.to_string()
    }

    fn universe() -> Vec<(String, String)> {
        vec![
            (s("0xWETH"), s("WETH")),
            (s("0xUSDC"), s("USDC")),
            (s("0xFAKE_USDC"), s("USDC")), // symbol collision: same symbol, different token
            (s("0xPEPE"), s("PEPE")),
        ]
    }

    #[test]
    fn addr_is_identity_symbol_is_not() {
        // ARBX-0018 regression: the PROHIBITED binding. "WETH" is in the
        // operator allowlist and resolves to an address — but the STRING
        // "WETH" is not a token and must NOT pass the gate.
        let idx = TokenIdentityIndex::resolve(1, &[s("WETH"), s("USDC")], &universe());
        assert!(
            !idx.is_allowed_addr("WETH"),
            "symbol string must never pass"
        );
        assert!(!idx.is_allowed_addr("weth"));
        assert!(idx.is_allowed_addr("0xWETH"), "resolved address passes");
    }

    #[test]
    fn address_normalization_is_idempotent() {
        let idx = TokenIdentityIndex::resolve(1, &[s("WETH")], &universe());
        assert!(idx.is_allowed_addr("0xweth"));
        assert!(idx.is_allowed_addr("  0xWETH  "));
    }

    #[test]
    fn symbol_matching_is_case_insensitive_and_trimmed() {
        let idx = TokenIdentityIndex::resolve(1, &[s(" weth "), s("usdc")], &universe());
        assert!(idx.is_allowed_addr("0xWETH"));
        assert!(idx.is_allowed_addr("0xUSDC"));
        assert!(idx.unresolved_symbols().is_empty());
    }

    #[test]
    fn symbol_collision_allows_both_addresses() {
        // Operator granularity = symbol; runtime identity = address. Both
        // USDC-labelled tokens pass — disambiguation is the operator's
        // (TW-002 explicit TokenKeys), never silent.
        let idx = TokenIdentityIndex::resolve(1, &[s("USDC")], &universe());
        assert!(idx.is_allowed_addr("0xUSDC"));
        assert!(idx.is_allowed_addr("0xFAKE_USDC"));
        assert_eq!(idx.allowed_addr_count(), 2);
    }

    #[test]
    fn unresolved_symbols_reported_never_fabricated() {
        let idx = TokenIdentityIndex::resolve(1, &[s("WETH"), s("DOGE"), s("DOGE")], &universe());
        assert_eq!(idx.unresolved_symbols(), &["DOGE".to_string()]);
        assert_eq!(idx.allowed_addr_count(), 1);
    }

    #[test]
    fn non_empty_allowlist_unresolved_everywhere_fails_closed() {
        // NOT permissive: the operator listed tokens, none exist in the
        // universe — allowing everything would fabricate consent.
        let idx = TokenIdentityIndex::resolve(1, &[s("DOGE")], &universe());
        assert!(!idx.is_allowed_addr("0xWETH"));
        assert!(!idx.is_allowed_addr("anything"));
        assert!(!idx.is_permissive());
        assert_eq!(idx.allowed_addr_count(), 0);
    }

    #[test]
    fn empty_operator_list_is_permissive() {
        let idx = TokenIdentityIndex::resolve(1, &[], &universe());
        assert!(idx.is_permissive());
        assert!(idx.is_allowed_addr("0xUNKNOWN_NOT_IN_UNIVERSE"));
        assert_eq!(idx.allowed_addr_count(), 0);
    }

    #[test]
    fn blank_operator_entries_do_not_make_list_non_empty() {
        let idx = TokenIdentityIndex::resolve(1, &[s("  "), s("")], &universe());
        assert!(idx.is_permissive());
        assert!(idx.unresolved_symbols().is_empty());
    }

    #[test]
    fn symbol_metadata_feeds_price_stack_keys() {
        let idx = TokenIdentityIndex::resolve(1, &[s("WETH")], &universe());
        assert_eq!(idx.symbol_for_addr("0xweth"), Some("WETH"));
        assert_eq!(idx.symbol_for_addr("0xUNKNOWN"), None);
    }

    #[test]
    fn universe_keeps_first_seen_on_duplicate_addr() {
        let dup = vec![(s("0xWETH"), s("WETH")), (s("0xWETH"), s("SYBL"))];
        let idx = TokenIdentityIndex::resolve(1, &[], &dup);
        assert_eq!(idx.symbol_for_addr("0xWETH"), Some("WETH"));
        assert_eq!(idx.universe_len(), 1);
    }

    #[test]
    fn chains_do_not_leak_into_each_other() {
        // The index is chain-scoped by construction (one universe per chain);
        // the chain_id is carried so callers can assert they got the right one.
        let a = TokenIdentityIndex::resolve(1, &[s("WETH")], &universe());
        let b = TokenIdentityIndex::resolve(137, &[s("WETH")], &[]);
        assert_eq!(a.chain_id(), 1);
        assert_eq!(b.chain_id(), 137);
        assert!(a.is_allowed_addr("0xWETH"));
        assert!(
            !b.is_allowed_addr("0xWETH"),
            "empty universe on 137 → fail-closed there"
        );
    }

    // ── TW-002: allowlist entries in address form ────────────────────────

    /// A 40-hex entry is its own identity: allowed with NO universe, never
    /// reported unresolved, and the list is not permissive.
    #[test]
    fn address_form_entry_is_identity_without_universe() {
        let a40 = format!("0x{}", "a".repeat(40));
        let idx = TokenIdentityIndex::resolve(1, &[s(&a40)], &[]);
        assert!(idx.is_allowed_addr(&a40));
        assert!(idx.is_allowed_addr(&format!("0x{}", "A".repeat(40))));
        assert!(!idx.is_permissive());
        assert!(idx.unresolved_symbols().is_empty());
        assert_eq!(idx.allowed_addr_count(), 1);
        // Any other address (including the universe-populated ones) stays
        // out — the operator allowed exactly one identity.
        assert!(!idx.is_allowed_addr(&format!("0x{}", "b".repeat(40))));
    }

    /// A mixed list lets both forms coexist: address entries gate directly,
    /// symbol entries keep the legacy resolution.
    #[test]
    fn mixed_allowlist_addresses_and_symbols() {
        let a40 = format!("0x{}", "1".repeat(40));
        let idx = TokenIdentityIndex::resolve(1, &[s(&a40), s("WETH")], &universe());
        assert!(idx.is_allowed_addr(&a40), "address entry gates directly");
        assert!(idx.is_allowed_addr("0xWETH"), "symbol entry resolves");
        assert!(!idx.is_allowed_addr("0xUSDC"), "unlisted symbol stays out");
        assert!(idx.unresolved_symbols().is_empty());
        assert_eq!(idx.allowed_addr_count(), 2);
    }

    /// Address-form detection boundaries: exactly 40 hex after 0x/0X,
    /// checksum case accepted, anything else is a symbol.
    #[test]
    fn address_form_detection_boundaries() {
        let h40 = |c: char| format!("0x{}", c.to_string().repeat(40));
        assert!(is_address_form(&h40('a')));
        assert!(is_address_form(&h40('F')));
        assert!(is_address_form(&format!("0X{}", "a".repeat(40))));
        assert!(is_address_form(&format!("  {}  ", h40('a'))), "trimmed");
        // 48 hex chars after the prefix is NOT an address form.
        assert!(!is_address_form(&format!(
            "0x{}",
            "aBcDeF0123456789".repeat(3)
        )));
        assert!(!is_address_form(&format!("0x{}", "a".repeat(39))));
        assert!(!is_address_form(&format!("0x{}", "a".repeat(41))));
        assert!(
            !is_address_form(&format!("0x{}", "g".repeat(40))),
            "non-hex"
        );
        assert!(
            !is_address_form("0xWETH"),
            "test fixture is NOT address form"
        );
        assert!(!is_address_form("WETH"));
        assert!(!is_address_form("0x"));
        assert!(!is_address_form(""));
    }

    /// The form-aware normalization helper: addresses lowercase, symbols
    /// uppercase, idempotent on both.
    #[test]
    fn normalization_helper_is_form_aware() {
        let mixed = format!("0x{}", "Ab1".repeat(13) + "a"); // 40 hex, mixed case
        assert_eq!(mixed.len(), 42);
        let n = normalize_allowlist_entry(&mixed);
        assert_eq!(n, n.to_ascii_lowercase(), "address form lowercases");
        assert_eq!(normalize_allowlist_entry(&n), n, "idempotent");
        assert_eq!(normalize_allowlist_entry(" wEth "), "WETH");
        assert_eq!(
            normalize_allowlist_entry(&normalize_allowlist_entry(" wEth ")),
            "WETH"
        );
    }

    /// A checksummed address entry matches the universe's lowercase row:
    /// identity compares normalized, and the metadata feed still works.
    #[test]
    fn checksummed_entry_matches_lowercase_universe() {
        let low = format!("0x{}", "c".repeat(39)) + "1"; // 40 chars, lowercase
        let checksummed = format!("0x{}{}", "C".repeat(39), "1");
        let uni = vec![(low.clone(), s("SOMETOKEN"))];
        let idx = TokenIdentityIndex::resolve(1, &[s(&checksummed)], &uni);
        assert!(idx.is_allowed_addr(&low), "normalized identity match");
        assert_eq!(idx.symbol_for_addr(&low), Some("SOMETOKEN"));
    }
}
