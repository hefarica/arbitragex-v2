//! Bundle builder — constructs a signed EIP-1559 transaction for the
//! observed opportunity, wrapped as a Flashbots bundle.
//!
//! Reuses the `tx_builder` pattern from sim-ctl: same router-selector logic,
//! but with a signed RLP output instead of just calldata for eth_call.
//!
//! Safety: `max_value_eth` cap enforced here. BuildError::ValueExceedsCap
//! must propagate to a risk_event (severity=critical) in the caller.
//!
//! M2 carry-through (2026-06-29): when the simulation path attaches the exact
//! `executeArbitrage(...)` calldata it validated (as `Opportunity`'s carried
//! `exec_payload`), the broadcast path signs THOSE bytes verbatim — giving
//! provable sim↔broadcast byte-parity. Re-encoding here can never byte-match
//! sim because sim's deadline is `now+60` (time-dependent) and the emitted
//! `Opportunity` drops the routing detail. When no `exec_payload` is carried,
//! we fall back to the legacy direct-router swap encoding (still M1-gated).

use crate::nonce_manager::NonceManager;
use crate::signer::Signer;
use alloy::eips::BlockId;
use alloy::providers::Provider as AlloyProvider;
use alloy::rpc::types::BlockNumberOrTag;
use anyhow::Result;
use ethers::abi::{encode, Token};
use ethers::core::types::transaction::eip2718::TypedTransaction;
use ethers::prelude::*;
use ethers::signers::Signer as EthersSigner;
use shared_rs::chains::{routers_for_chain, RouterKind};
use shared_rs::contracts::{ExecPayload, Opportunity, StrategyKind};
use shared_rs::rpc_failover::AlloyHttpProvider;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("unsupported strategy: {0:?}")]
    UnsupportedStrategy(StrategyKind),
    #[error("live execution denied: {0}")]
    LiveExecDenied(String),
    #[error("dex '{0}' not in catalog")]
    UnknownRouter(String),
    #[error("amount invalid: {0}")]
    InvalidAmount(String),
    #[error("invalid address: {0}")]
    InvalidAddress(String),
    #[error("invalid exec_payload: {0}")]
    InvalidExecPayload(String),
    #[error("value {value_eth} ETH exceeds max_value_eth {cap_eth}")]
    ValueExceedsCap { value_eth: f64, cap_eth: f64 },
    #[error("provider error: {0}")]
    Provider(String),
}

pub struct SignedBundle {
    #[allow(dead_code)]
    pub opportunity_id: uuid::Uuid,
    pub target_block: u64,
    pub tx_raw_hex: String,
    pub tx_hash: H256,
    pub from: Address,
    pub nonce: u64,
    #[allow(dead_code)]
    pub value_wei: U256,
}

/// Builds + signs a single-tx bundle.
///
/// `provider` is the alloy 1.0 HTTP provider (used only for `get_block` to
/// read the current base fee). The signing path remains ethers (signer.wallet).
///
/// `exec_payload`: when `Some`, the carried `executeArbitrage` calldata from the
/// simulation path is signed verbatim (`.to` = the carried executor address).
/// When `None`, the legacy direct-router encoding is used (back-compat).
pub async fn build_and_sign(
    opp: &Opportunity,
    exec_payload: Option<&ExecPayload>,
    signer: &Signer,
    provider: &AlloyHttpProvider,
    nonce_mgr: &NonceManager,
    max_value_eth: f64,
    target_block_offset: u64,
) -> Result<SignedBundle, BuildError> {
    // M1 (2026-06-28): physical broadcast barrier — default-deny + testnet-only.
    // Was a hard `chain_id != 1` reject — i.e. MAINNET-ONLY, the inverse of safe.
    // Now live execution must be explicitly enabled AND target an allowlisted
    // testnet; mainnet (chain_id=1) is physically refused. Re-read per call
    // (un-bypassable) so a paper_mode flip at runtime still cannot reach mainnet.
    // See live_exec_policy.
    crate::live_exec_policy::LiveExecPolicy::from_env()
        .assert_broadcast_allowed(opp.chain_id)
        .map_err(|e| BuildError::LiveExecDenied(e.to_string()))?;
    if !matches!(opp.strategy_kind, StrategyKind::DexArb) {
        return Err(BuildError::UnsupportedStrategy(opp.strategy_kind.clone()));
    }
    let amount_in = U256::from_dec_str(&opp.amount_in_wei)
        .map_err(|_| BuildError::InvalidAmount(opp.amount_in_wei.clone()))?;

    // Enforce value cap. For ERC20 swaps, value=0; for ETH-in, value=amount_in.
    // S5 only builds ERC20 swaps (no ETH-in tx); ETH-in deferred. But we still
    // enforce the cap against amount_in in eth-denominated terms.
    let value_wei = U256::zero();
    if amount_in_to_eth(amount_in) > max_value_eth {
        return Err(BuildError::ValueExceedsCap {
            value_eth: amount_in_to_eth(amount_in),
            cap_eth: max_value_eth,
        });
    }

    // Deadline is only consumed by the direct-router fallback encoder; the
    // carried `executeArbitrage` calldata already has its own deadline baked in
    // (from sim time) and is signed verbatim.
    let deadline = U256::from(now_secs() + 120);

    // M2 carry-through: prefer the exact sim-validated calldata when present.
    let (to_addr, data) =
        select_to_and_data(opp, exec_payload, signer.address, amount_in, deadline)?;
    if exec_payload.is_none() {
        tracing::debug!(
            event = "bundle_builder.direct_router_fallback",
            opp_id = %opp.id,
            "no exec_payload carried — using legacy direct-router encoding"
        );
    }

    let nonce = nonce_mgr
        .next(opp.chain_id, signer.address)
        .await
        .map_err(|e| BuildError::Provider(e.to_string()))?;

    // Fee estimation. For S5 we use simple "latest base fee + 2 gwei priority".
    //
    // Alloy 1.0: `get_block(BlockId)` requires a `BlockId` wrapper around
    // `BlockNumberOrTag`. `Block.header.base_fee_per_gas` is `Option<u64>` in
    // alloy 1.0 (it was `Option<U256>` in ethers; u64 is safe — max ~18.4 gwei
    // ceiling that fits in u64 trivially). `Block.header.number` is `u64`.
    let latest_block = provider
        .get_block(BlockId::Number(BlockNumberOrTag::Latest))
        .await
        .map_err(|e| BuildError::Provider(e.to_string()))?
        .ok_or_else(|| BuildError::Provider("no_latest_block".into()))?;
    // base_fee_per_gas is u64 in alloy 1.0 → convert to ethers U256 for signing.
    let base_fee_u64: u64 = latest_block
        .header
        .base_fee_per_gas
        .unwrap_or(30_000_000_000u64); // 30 gwei fallback
    let base_fee = U256::from(base_fee_u64);
    let priority_fee = U256::from(2_000_000_000u64); // 2 gwei default
    let max_fee = base_fee * 2 + priority_fee;

    let target_block = latest_block.header.number + target_block_offset;

    let tx = Eip1559TransactionRequest::new()
        .to(to_addr)
        .from(signer.address)
        .value(value_wei)
        .data(data)
        .nonce(nonce)
        .chain_id(opp.chain_id)
        .max_priority_fee_per_gas(priority_fee)
        .max_fee_per_gas(max_fee)
        .gas(500_000u64);

    let typed: TypedTransaction = tx.into();
    let signature = signer
        .wallet
        .sign_transaction(&typed)
        .await
        .map_err(|e| BuildError::Provider(format!("sign_tx: {e}")))?;
    let raw: Bytes = typed.rlp_signed(&signature);
    let tx_hash = keccak256_bytes(&raw);

    Ok(SignedBundle {
        opportunity_id: opp.id,
        target_block,
        tx_raw_hex: format!("0x{}", hex::encode(&raw)),
        tx_hash: H256::from(tx_hash),
        from: signer.address,
        nonce,
        value_wei,
    })
}

/// Decide the transaction `to` address + calldata.
///
/// When `exec_payload` is `Some`, the simulation path already built and
/// validated the exact `executeArbitrage(...)` calldata — it is signed VERBATIM
/// so broadcast bytes equal sim bytes (provable parity). When `None`, fall back
/// to the legacy direct-router swap encoding (`recipient` + `deadline` are only
/// used by the fallback).
///
/// Pure (no I/O) so the parity guarantee is unit-testable without a network.
fn select_to_and_data(
    opp: &Opportunity,
    exec_payload: Option<&ExecPayload>,
    recipient: Address,
    amount_in: U256,
    deadline: U256,
) -> Result<(Address, Bytes), BuildError> {
    if let Some(p) = exec_payload {
        let to = parse_addr(&p.to)?;
        let data = decode_hex_0x(&p.calldata_hex)?;
        if data.len() < 4 {
            return Err(BuildError::InvalidExecPayload(
                "calldata shorter than a 4-byte selector".to_string(),
            ));
        }
        return Ok((to, Bytes::from(data)));
    }

    // Fallback: legacy direct-router encoding.
    let token_in = parse_addr(&opp.token_in)?;
    let token_out = parse_addr(&opp.token_out)?;
    let router_entry = routers_for_chain(opp.chain_id)
        .iter()
        .find(|r| r.name.starts_with(&opp.dex_a) || r.kind.as_str() == opp.dex_a)
        .ok_or_else(|| BuildError::UnknownRouter(opp.dex_a.clone()))?;
    let data = match router_entry.kind {
        RouterKind::UniswapV2 | RouterKind::Sushi => {
            encode_v2(token_in, token_out, amount_in, recipient, deadline)
        }
        RouterKind::UniswapV3 => encode_v3(token_in, token_out, amount_in, recipient, deadline),
        _ => {
            return Err(BuildError::UnknownRouter(format!(
                "{:?}",
                router_entry.kind
            )))
        }
    };
    Ok((Address::from(router_entry.address), data))
}

/// Decode a `0x`-prefixed (or bare) hex string into bytes.
fn decode_hex_0x(s: &str) -> Result<Vec<u8>, BuildError> {
    let cleaned = s.trim_start_matches("0x").trim_start_matches("0X");
    hex::decode(cleaned)
        .map_err(|e| BuildError::InvalidExecPayload(format!("bad calldata hex: {e}")))
}

fn keccak256_bytes(b: &Bytes) -> [u8; 32] {
    use ethers::utils::keccak256;
    keccak256(b.as_ref())
}

fn amount_in_to_eth(amount: U256) -> f64 {
    let wei = amount.as_u128();
    (wei as f64) / 1e18_f64
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn parse_addr(s: &str) -> Result<Address, BuildError> {
    let cleaned = s.trim_start_matches("0x").trim_start_matches("0X");
    if cleaned.len() != 40 || !cleaned.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(BuildError::InvalidAddress(s.to_string()));
    }
    let bytes = hex::decode(cleaned).map_err(|_| BuildError::InvalidAddress(s.to_string()))?;
    let mut arr = [0u8; 20];
    arr.copy_from_slice(&bytes);
    Ok(Address::from(arr))
}

fn encode_v2(
    token_in: Address,
    token_out: Address,
    amount_in: U256,
    to: Address,
    deadline: U256,
) -> Bytes {
    let selector: [u8; 4] = [0x38, 0xed, 0x17, 0x39];
    let tokens = vec![
        Token::Uint(amount_in),
        Token::Uint(U256::from(1u8)),
        Token::Array(vec![Token::Address(token_in), Token::Address(token_out)]),
        Token::Address(to),
        Token::Uint(deadline),
    ];
    let mut buf = selector.to_vec();
    buf.extend(encode(&tokens));
    Bytes::from(buf)
}

fn encode_v3(
    token_in: Address,
    token_out: Address,
    amount_in: U256,
    to: Address,
    deadline: U256,
) -> Bytes {
    let selector: [u8; 4] = [0x41, 0x4b, 0xf3, 0x89];
    let params = Token::Tuple(vec![
        Token::Address(token_in),
        Token::Address(token_out),
        Token::Uint(U256::from(3000u32)),
        Token::Address(to),
        Token::Uint(deadline),
        Token::Uint(amount_in),
        Token::Uint(U256::zero()),
        Token::Uint(U256::zero()),
    ]);
    let mut buf = selector.to_vec();
    buf.extend(encode(&[params]));
    Bytes::from(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_opp(dex_a: &str, chain_id: u64) -> Opportunity {
        Opportunity {
            id: uuid::Uuid::nil(),
            chain_id,
            strategy_kind: StrategyKind::DexArb,
            dex_a: dex_a.to_string(),
            dex_b: None,
            pair_symbol: "T/U".to_string(),
            token_in: "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2".to_string(),
            token_out: "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string(),
            amount_in_wei: "1000".to_string(),
            expected_profit_usd: None,
            net_expected_profit_usd: None,
            roi_pct: None,
            risk_score: None,
            block_number: None,
            rejection_reason: None,
            detected_at: chrono::Utc::now(),
            trace_id: uuid::Uuid::nil(),
        }
    }

    #[test]
    fn value_cap_triggers_on_huge_amount() {
        // Direct cap check — doesn't need network.
        let big = U256::from(10u128) * U256::from(10u128).pow(18.into()); // 10 ETH
        assert!(amount_in_to_eth(big) > 1.0);
        let small = U256::from(5u128) * U256::from(10u128).pow(17.into()); // 0.5 ETH
        assert!(amount_in_to_eth(small) < 1.0);
    }

    #[test]
    fn addr_parse_rejects_invalid() {
        assert!(parse_addr("nope").is_err());
        assert!(parse_addr("0x123").is_err());
        assert!(parse_addr("0xC02aaa39b223FE8D0A0e5C4F27eAD9083C756Cc2").is_ok());
    }

    /// Provable parity: when sim attaches the executeArbitrage calldata, the
    /// broadcast path signs exactly those bytes — no re-encoding.
    #[test]
    fn exec_payload_is_replayed_verbatim() {
        let opp = test_opp("uniswap_v2", 1);
        let calldata_hex =
            "0x76d81cdf00000000000000000000000000000000000000000000000000000000deadbeef";
        let to_hex = "0x2222222222222222222222222222222222222222";
        let payload = ExecPayload {
            calldata_hex: calldata_hex.to_string(),
            to: to_hex.to_string(),
            selector: "0x76d81cdf".to_string(),
        };
        let Ok((to, data)) = select_to_and_data(
            &opp,
            Some(&payload),
            Address::zero(),
            U256::from(1u8),
            U256::from(123u64),
        ) else {
            panic!("exec_payload path must succeed");
        };
        let Ok(want_to) = parse_addr(to_hex) else {
            panic!("test to_hex must be a valid address");
        };
        assert_eq!(to, want_to, "to must be the carried executor");
        let Ok(expected) = hex::decode(calldata_hex.trim_start_matches("0x")) else {
            panic!("test calldata_hex must be valid hex");
        };
        assert_eq!(
            data.as_ref(),
            expected.as_slice(),
            "calldata must be replayed verbatim (byte-parity with sim)"
        );
    }

    /// A too-short carried calldata (no selector) is rejected, not signed.
    #[test]
    fn exec_payload_too_short_is_rejected() {
        let opp = test_opp("uniswap_v2", 1);
        let payload = ExecPayload {
            calldata_hex: "0xabcd".to_string(), // 2 bytes < 4-byte selector
            to: "0x2222222222222222222222222222222222222222".to_string(),
            selector: "0xabcdef00".to_string(),
        };
        let r = select_to_and_data(
            &opp,
            Some(&payload),
            Address::zero(),
            U256::from(1u8),
            U256::from(123u64),
        );
        assert!(matches!(r, Err(BuildError::InvalidExecPayload(_))));
    }

    /// With no carried calldata and an unknown dex, the fallback path is taken
    /// (proven by the UnknownRouter error from the catalogue lookup) — i.e. it
    /// does NOT silently reuse any carried bytes.
    #[test]
    fn no_exec_payload_takes_direct_router_fallback() {
        let opp = test_opp("totally_unknown_dex_xyz", 1);
        let r = select_to_and_data(
            &opp,
            None,
            Address::zero(),
            U256::from(1000u64),
            U256::from(123u64),
        );
        assert!(
            matches!(r, Err(BuildError::UnknownRouter(_))),
            "expected fallback to the router catalogue, got {r:?}"
        );
    }
}
