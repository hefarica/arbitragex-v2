//! SIM-FUND-01 (2026-09-17) — fork-side signer funding for anvil probes.
//!
//! Incident (audits/real-cycles-audit-20260916/00-SYNTHESIS.md, funnel
//! 2026-09-17T04:00Z): the S4 probe is sent as `eth_call(from =
//! SIM_SIGNER_ADDRESS)` while the router pulls `transferFrom(signer, …)` —
//! the signer holds ZERO tokens on the fork (it is an observation address,
//! not a funded actor), so every honest probe reverted
//! `TransferHelper: TRANSFER_FROM_FAILED` (V2) / `STF` (V3): 1,128 of 1,667
//! sims in a 2h window — the probe never tested the route, only the
//! signer's empty balance.
//!
//! Fix: before the probe `eth_call`, seed the signer's token balance in the
//! FORK ONLY, via `anvil_setStorageAt` on a storage slot that is VERIFIED by
//! sentinel: write sentinel balance → `balanceOf` check → only then write the
//! real amount, and always inside the caller's snapshot window so
//! `evm_revert` erases it. Slot probing tries the canonical Solidity
//! mapping layouts (0, 2, 3, 9) but NEVER trusts an unverified slot: a slot
//! that does not reproduce the sentinel through `balanceOf` is skipped
//! (exotic proxies keep their storage intact).
//!
//! NO-ACTIVE: simulation fork only — `anvil_setStorageAt` is an Anvil
//! debug RPC that does not exist on real networks; mainnet state is
//! untouched by construction. The executor capital barrier is downstream
//! and unaffected.

use ethers::core::types::transaction::eip2718::TypedTransaction;
use ethers::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Sentinel balance used for slot verification. Distinct from any realistic
/// real amount so `balanceOf == SENTINEL` can only be explained by OUR write.
const SENTINEL_BALANCE: U256 = U256([0x5EED_F00D_0000_0001, 0, 0, 0]);

/// Storage slots tried, in order. Covers:
///   slot 0 — `mapping(address => uint256) balances;` as first state var,
///   slot 2 / 3 / 9 — one/two vars before `balances` (name/symbol/decimals
///   orderings found across mainstream ERC20s, incl. USDT at 2 on some
///   forks), and common manual `allowances-then-balances` layouts.
const CANDIDATE_SLOTS: [u64; 4] = [0, 2, 3, 9];

/// Filled balance per token: enough for any probe the funnel sizes today.
const FUND_AMOUNT: U256 = U256([0xFFFF_FFFF_FFFF_FFFF, 0xFFFF_FFFF_FFFF_FFFF, 0, 0]);

/// Verified slot cache: token -> storage slot. Shared across simulations so
/// each exotic token pays discovery at most once per process (per snapshot
/// revert the WRITES are undone, but the slot NUMBER stays valid — layout is
/// a property of the contract code, not of storage contents).
type SlotCache = Arc<Mutex<HashMap<Address, u64>>>;

/// Outcome counter label for `arbx_sim_funding_total{outcome}` (R8: real
/// outcomes only, never fabricated).
pub mod outcome {
    pub const SEEDED_FRESH: &str = "seeded_fresh";
    pub const CACHE_HIT: &str = "cache_hit";
    pub const SLOT_UNRESOLVED: &str = "slot_unresolved";
    pub const RPC_ERR: &str = "rpc_err";
}

fn count(outcome: &str) {
    shared_rs::metrics::SIM_FUNDING_TOTAL
        .with_label_values(&[outcome])
        .inc();
}

pub struct SignerFunder {
    provider: Arc<Provider<Http>>,
    slot_cache: SlotCache,
    timeout: Duration,
}

impl SignerFunder {
    pub fn new(provider: Arc<Provider<Http>>) -> Self {
        Self {
            provider,
            slot_cache: Arc::new(Mutex::new(HashMap::new())),
            timeout: Duration::from_secs(8),
        }
    }

    /// Ensure `signer` can cover `amount_in` of `token` on the fork. Returns
    /// Ok(()) when the balance is (already or newly) sufficient, Err with a
    /// short machine reason when funding could not be established — the
    /// caller converts that into a typed fail_reason (fail-closed).
    pub async fn ensure_funded(
        &self,
        token: Address,
        signer: Address,
        amount_in: U256,
    ) -> Result<(), String> {
        if amount_in.is_zero() {
            return Ok(()); // nothing to fund; builder rejects zero amounts
        }
        let bal = tokio::time::timeout(self.timeout, balance_of(&self.provider, token, signer))
            .await
            .map_err(|_| {
                count(outcome::RPC_ERR);
                "funding_balanceof_timeout".to_string()
            })?
            .map_err(|e| {
                count(outcome::RPC_ERR);
                format!("funding_balanceof_rpc: {e}")
            })?;
        if bal.unwrap_or_default() >= amount_in {
            // The write is inside the snapshot window; a surviving balance
            // means the snapshot was already funded this window.
            return Ok(());
        }

        // Fast path: slot verified earlier in this process.
        if let Some(slot) = self.cached_slot(token) {
            let set = self
                .write_balance(token, slot, signer, FUND_AMOUNT)
                .await
                .map_err(|e| {
                    count(outcome::RPC_ERR);
                    format!("funding_setstorage_rpc: {e}")
                })?;
            if set {
                count(outcome::CACHE_HIT);
                return Ok(());
            }
            // Slot stopped reproducing (contract changed / wrong cache) —
            // fall through to rediscovery.
            self.invalidate_slot(token);
        }

        // Sentinel-verified discovery.
        // SIM-03 fix (2026-09-24): capture the ORIGINAL balance BEFORE probing
        // slots so the restore after a failed sentinel writes the REAL prior
        // value (not an assumed zero — on a fork with pre-existing state the
        // zero-write corrupted the slot within the snapshot window).
        let original_balance = bal.unwrap_or_default();
        for &slot in CANDIDATE_SLOTS.iter() {
            if !self
                .write_balance(token, slot, signer, SENTINEL_BALANCE)
                .await?
            {
                continue; // RPC-level failure on this slot — try next
            }
            let verified =
                tokio::time::timeout(self.timeout, balance_of(&self.provider, token, signer))
                    .await
                    .map_err(|_| {
                        count(outcome::RPC_ERR);
                        "funding_verify_timeout".to_string()
                    })?
                    .map_err(|e| {
                        count(outcome::RPC_ERR);
                        format!("funding_verify_rpc: {e}")
                    })?;
            if verified == Some(SENTINEL_BALANCE) {
                // Verified: this slot IS the signer's balance. Write the real
                // amount and cache the slot.
                let set = self
                    .write_balance(token, slot, signer, FUND_AMOUNT)
                    .await
                    .map_err(|e| {
                        count(outcome::RPC_ERR);
                        format!("funding_setstorage_rpc: {e}")
                    })?;
                if set {
                    self.store_slot(token, slot);
                    count(outcome::SEEDED_FRESH);
                    return Ok(());
                }
            }
            // Sentinel not observed — restore the ORIGINAL balance (captured
            // before any probe) so we never leak a corrupted slot onward.
            let _ = self
                .write_balance(token, slot, signer, original_balance)
                .await;
        }
        count(outcome::SLOT_UNRESOLVED);
        Err("sim_signer_funding_slot_unresolved".to_string())
    }

    fn cached_slot(&self, token: Address) -> Option<u64> {
        self.slot_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&token)
            .copied()
    }

    fn store_slot(&self, token: Address, slot: u64) {
        self.slot_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(token, slot);
    }

    fn invalidate_slot(&self, token: Address) {
        self.slot_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&token);
    }

    /// `anvil_setStorageAt(token, slot32, value32)` for the signer's balance
    /// slot. Returns Ok(false) when the node refuses (non-anvil endpoint,
    /// unknown block) without aborting the search.
    ///
    /// SIM-10 fix (2026-09-24): wrapped in the same timeout as `balance_of`
    /// (8s default) — an HTTP hang here previously froze the entire funding
    /// path inside the snapshot window (no per-command timeout on raw
    /// `provider.request`, unlike `balance_of` which the caller wraps).
    async fn write_balance(
        &self,
        token: Address,
        slot: u64,
        signer: Address,
        value: U256,
    ) -> Result<bool, String> {
        let slot32 = balance_slot(slot, signer);
        let value32 = u256_to_h256(value);
        let res: Result<bool, _> = tokio::time::timeout(
            self.timeout,
            self.provider
                .request::<_, bool>("anvil_setStorageAt", (token, slot32, value32)),
        )
        .await
        .map_err(|_| "anvil_setStorageAt_timeout".to_string())?;
        match res {
            Ok(true) => Ok(true),
            Ok(false) => Ok(false),
            Err(e) => Err(format!("anvil_setStorageAt: {e}")),
        }
    }
}

/// erc20 balanceOf(signer) → U256, typed through ethers' call decoding.
async fn balance_of(
    provider: &Provider<Http>,
    token: Address,
    signer: Address,
) -> Result<Option<U256>, ProviderError> {
    let sel: [u8; 4] = [0x70, 0xa0, 0x82, 0x31]; // balanceOf(address)
    let mut data = sel.to_vec();
    data.extend_from_slice(signer.as_bytes());
    let tx = TransactionRequest::new()
        .to(token)
        .data(ethers::types::Bytes::from(data));
    let typed: TypedTransaction = tx.into();
    match provider.call(&typed, None).await {
        Ok(out) => Ok(decode_u256(&out)),
        // Non-contract / revert: no decodable balance — treat as zero
        // rather than aborting the whole funding path.
        Err(_) => Ok(None),
    }
}

fn decode_u256(out: &Bytes) -> Option<U256> {
    if out.len() < 32 {
        return None;
    }
    Some(U256::from_big_endian(&out[..32]))
}

/// keccak256(abi.encode(signer, slot)) — the canonical Solidity mapping
/// value location for `mapping(address => uint256)`.
/// SIM-FUND-01b (2026-09-17): `abi.encode` RIGHT-pads an address inside its
/// 32-byte word (bytes 12..31). The previous left-padded write computed a
/// slot nobody reads — every `anvil_setStorageAt` landed in dead storage, the
/// sentinel never reproduced through `balanceOf`, and 706/706 funding
/// attempts died `slot_unresolved` on the live fork. Reproduced by contrast:
/// the same write with `cast index` (correct padding) surfaces the sentinel
/// immediately on the same anvil.
fn balance_slot(slot: u64, signer: Address) -> H256 {
    // Idiomatic (Sancho-validated SIM-FUND-01b): let ethers' ABI encoder do
    // the padding — keccak256(abi.encode(address, uint256)), exactly what
    // `cast index <addr> <slot>` computes. Manual byte alignment here caused
    // the 706/706 slot_unresolved production failure (address left-padded +
    // slot top-aligned instead of right-aligned words).
    H256(ethers::utils::keccak256(ethers::abi::encode(&[
        ethers::abi::Token::Address(signer),
        ethers::abi::Token::Uint(ethers::types::U256::from(slot)),
    ])))
}

fn u256_to_h256(v: U256) -> H256 {
    let mut out = [0u8; 32];
    v.to_big_endian(&mut out);
    H256(out)
}

// ── unit tests: pure slot/encode helpers (no RPC) ──────────────────────────
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn balance_slot_is_keccak_of_abi_encoded_signer_slot() {
        let signer: Address = "0x1111111111111111111111111111111111111111"
            .parse()
            .unwrap();
        // SIM-FUND-01b: `cast index address 0x1111…1111 9` (foundry, correct
        // abi.encode padding) — the regression vector that the left-padded
        // implementation failed. Cross-checked live on the VPS anvil: writing
        // the sentinel at THIS slot reproduces through balanceOf.
        let known = H256::from_slice(
            &ethers::utils::hex::decode(
                "233b1b49de63438bb1ac1a57ef81babcc52ccd4555c968bb144593ea539bbebc",
            )
            .unwrap(),
        );
        assert_eq!(balance_slot(9, signer), known);
        // abi.encode semantics pinned by the EXTERNAL cast vector above — this
        // structural check would have caught both original padding bugs.
        assert_eq!(
            balance_slot(9, signer),
            H256(ethers::utils::keccak256(ethers::abi::encode(&[
                ethers::abi::Token::Address(signer),
                ethers::abi::Token::Uint(ethers::types::U256::from(9u64)),
            ])))
        );
        // Different slot → different location.
        assert_ne!(balance_slot(0, signer), balance_slot(2, signer));
        // Different signer → different location.
        let other: Address = "0x2222222222222222222222222222222222222222"
            .parse()
            .unwrap();
        assert_ne!(balance_slot(0, signer), balance_slot(0, other));
    }

    #[test]
    fn decode_u256_accepts_only_32_byte_words() {
        assert_eq!(decode_u256(&Bytes::from(vec![0u8; 32])), Some(U256::zero()));
        let mut out = vec![0u8; 32];
        out[31] = 42;
        assert_eq!(decode_u256(&Bytes::from(out)), Some(U256::from(42u64)));
        assert_eq!(decode_u256(&Bytes::from(vec![0u8; 31])), None);
        assert_eq!(decode_u256(&Bytes::default()), None);
    }

    #[test]
    fn sentinel_is_distinct_and_nonzero() {
        assert!(!SENTINEL_BALANCE.is_zero());
        assert_ne!(SENTINEL_BALANCE, FUND_AMOUNT);
    }
}
