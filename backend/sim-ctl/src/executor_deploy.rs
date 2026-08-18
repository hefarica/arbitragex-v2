//! §IV blocker A2 (vivid-grove audit 2026-08-17) — deterministic
//! `ArbitrageExecutor` deployment to the ephemeral Anvil fork at sim-ctl boot.
//!
//! ## Why boot-time deployment
//! The sim-ctl Anvil container fork-eas mainnet and RESETS on every container
//! restart, so no static `ARBITRAGE_EXECUTOR` value can point at the fork: the
//! contract and its address only exist for the fork's lifetime. Without an
//! executor the B2c real-sim path returns its typed 501
//! (`real_sim_env_missing`), which is blocker A2 of the §IV mathematical
//! engine. This module deploys the contract at boot and hands the address to
//! `sim_runner::RealSimEnvConfig::from_env_with_executor` (runtime override —
//! the env var itself is never mutated).
//!
//! ## Determinism (canonical addresses)
//! Verified empirically 2026-08-18 (anvil 1.7.x, mainnet fork at block
//! ~25.78M): the fork OVERRIDES dev-account balances (10000 ETH) but does NOT
//! reset their nonces — account #0 carries its live mainnet nonce (thousands),
//! so a naive first-deploy address would drift as mainnet advances. We pin
//! the deployer nonce to 0 with `anvil_setNonce` before deploying, making the
//! create1 addresses canonical on every boot:
//!   impl  = create1(acct#0, 0) = 0x5FbDB2315678afecb367f032d93F642f64180aa3
//!   proxy = create1(acct#0, 1) = 0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512
//! The receipts are asserted against these constants (fail-honest on drift).
//! The forge twin of this deployer is
//! `contracts/script/DeployArbitrageExecutor.s.sol`.
//!
//! ## Safety (§32/§33/§34 — audit/read-only posture)
//! - The signer is the PUBLIC well-known Anvil dev account #0 key (Hardhat/
//!   Anvil default mnemonic; not a real key).
//! - The key is used ONLY after the RPC identifies itself as an Anvil node via
//!   `web3_clientVersion` — the well-known key can never sign toward a live
//!   chain even if ANVIL_URL is misconfigured.
//! - Broadcasts target only the ephemeral local fork; no mainnet/testnet
//!   transaction is ever constructed here.
//!
//! ## Fail-honest (R8)
//! Every failure returns a typed Err; the caller logs a warn and continues
//! WITHOUT an executor — B2c keeps its honest 501. Nothing is fabricated.
//!
//! ## Idempotency
//! If sim-ctl restarts against a still-live anvil (fork NOT reset), the
//! canonical proxy address already has code and the deploy is skipped.

use ethers::prelude::*;
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::utils::keccak256;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

/// Anvil dev account #0 — well-known address of the default Anvil/Hardhat
/// mnemonic. Not a placeholder: it is the actual fork signer this module uses.
const ANVIL_DEV_ACCOUNT_0: &str = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";

/// Anvil dev account #0 private key — PUBLIC well-known test key from the
/// default Anvil/Hardhat mnemonic (holds real funds nowhere). Never replace
/// with a funded key: this module only ever talks to a verified Anvil node.
const ANVIL_DEV_KEY_0: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

/// Nonce the deployer is pinned to before deploying (canonical addresses).
const PINNED_NONCE: u64 = 0;

/// Canonical implementation address (create1(anvil#0, 0)). Kept in sync with
/// `contracts/script/DeployArbitrageExecutor.s.sol` (EXPECTED_IMPL).
pub const EXPECTED_EXECUTOR_IMPL: &str = "0x5FbDB2315678afecb367f032d93F642f64180aa3";

/// Canonical ERC1967 proxy address (create1(anvil#0, 1)) — the value wired
/// into `ARBITRAGE_EXECUTOR`'s runtime slot. Kept in sync with
/// `contracts/script/DeployArbitrageExecutor.s.sol` (EXPECTED_PROXY).
pub const EXPECTED_EXECUTOR_PROXY: &str = "0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512";

/// Ensure the `ArbitrageExecutor` (impl + ERC1967Proxy) exists on the anvil
/// fork behind `provider`; returns the proxy address.
///
/// Skips the deploy (Ok) when the canonical proxy already has code. Fails
/// with a typed reason otherwise; the caller must continue without an
/// executor (B2c stays 501) — never fabricate an address.
pub async fn ensure_executor_on_fork(provider: Arc<Provider<Http>>) -> Result<Address, String> {
    if std::env::var("ARBX_FORK_AUTO_DEPLOY")
        .map(|v| v.eq_ignore_ascii_case("false"))
        .unwrap_or(false)
    {
        return Err("a2_auto_deploy_disabled: ARBX_FORK_AUTO_DEPLOY=false".to_string());
    }

    // Safety gate: the well-known key below may only ever sign toward an
    // Anvil node. A misconfigured ANVIL_URL pointing at a real chain fails
    // here instead of broadcasting anywhere.
    ensure_anvil_node(&provider).await?;

    let chain_id = provider
        .get_chainid()
        .await
        .map_err(|e| format!("a2_chain_id_failed: {e}"))?
        .as_u64();
    let wallet = LocalWallet::from_str(ANVIL_DEV_KEY_0.trim_start_matches("0x"))
        .map_err(|e| format!("a2_dev_key_invalid: {e}"))?
        .with_chain_id(chain_id);
    let deployer = wallet.address();
    let expected_deployer: Address = ANVIL_DEV_ACCOUNT_0
        .parse()
        .map_err(|e| format!("a2_dev_account_invalid: {e}"))?;
    if deployer != expected_deployer {
        return Err(format!(
            "a2_dev_key_mismatch: key derives {deployer:?}, expected {expected_deployer:?}"
        ));
    }

    let expected_impl = create1(deployer, PINNED_NONCE);
    let expected_proxy = create1(deployer, PINNED_NONCE + 1);
    let const_impl: Address = EXPECTED_EXECUTOR_IMPL
        .parse()
        .map_err(|e| format!("a2_const_impl_invalid: {e}"))?;
    let const_proxy: Address = EXPECTED_EXECUTOR_PROXY
        .parse()
        .map_err(|e| format!("a2_const_proxy_invalid: {e}"))?;
    if expected_impl != const_impl || expected_proxy != const_proxy {
        return Err(format!(
            "a2_canonical_drift: computed {expected_impl:?}/{expected_proxy:?} != constants"
        ));
    }

    // Idempotency (per step, so a partial deploy from a crashed boot can be
    // completed): probe code at BOTH canonical addresses.
    let existing_proxy = provider
        .get_code(expected_proxy, None)
        .await
        .map_err(|e| format!("a2_proxy_code_probe_failed: {e}"))?;
    if !existing_proxy.is_empty() {
        tracing::info!(
            event = "a2.executor_already_deployed",
            proxy = ?expected_proxy,
            "canonical executor already live on fork — skipping deploy"
        );
        return Ok(expected_proxy);
    }
    let existing_impl = provider
        .get_code(expected_impl, None)
        .await
        .map_err(|e| format!("a2_impl_code_probe_failed: {e}"))?;

    // Determinism: pin the remote nonce to the FIRST nonce this boot will
    // actually use, so the canonical create1 addresses hold regardless of the
    // forked mainnet nonce (and no nonce gap is left when a partial deploy is
    // being completed — a gapped future-nonce tx would sit unmined).
    let first_nonce = if existing_impl.is_empty() {
        PINNED_NONCE
    } else {
        PINNED_NONCE + 1
    };
    let _: serde_json::Value = provider
        .request(
            "anvil_setNonce",
            serde_json::json!([format!("{deployer:#x}"), format!("{first_nonce:#x}")]),
        )
        .await
        .map_err(|e| format!("a2_set_nonce_failed: {e}"))?;

    // 1. ArbitrageExecutor implementation (UUPS; constructor takes no args —
    //    it only disables initializers). Skipped when a previous (crashed)
    //    deploy already landed it at the canonical address.
    let impl_addr = if existing_impl.is_empty() {
        let impl_bytecode = load_creation_bytecode("ArbitrageExecutor.bytecode.json")?;
        let impl_receipt = send_deploy_tx(&provider, &wallet, impl_bytecode, PINNED_NONCE)
            .await
            .map_err(|e| format!("a2_impl_deploy_failed: {e}"))?;
        let addr = impl_receipt
            .contract_address
            .ok_or_else(|| "a2_impl_deploy_failed: receipt without contract_address".to_string())?;
        if addr != expected_impl {
            return Err(format!(
                "a2_impl_address_drift: receipt {addr:?} != canonical {expected_impl:?}"
            ));
        }
        addr
    } else {
        tracing::info!(
            event = "a2.impl_already_deployed",
            impl = ?expected_impl,
            "impl already live on fork — completing proxy deploy only"
        );
        expected_impl
    };

    // 2. ERC1967Proxy(impl, initialize(admin = anvil#0)). Admin holds
    //    DEFAULT_ADMIN_ROLE + UPGRADER_ROLE on the ephemeral fork.
    let proxy_bytecode = load_creation_bytecode("ERC1967Proxy.bytecode.json")?;
    let init_code = proxy_init_code(&proxy_bytecode, impl_addr, deployer);
    let proxy_receipt = send_deploy_tx(&provider, &wallet, init_code, PINNED_NONCE + 1)
        .await
        .map_err(|e| format!("a2_proxy_deploy_failed: {e}"))?;
    let proxy_addr = proxy_receipt
        .contract_address
        .ok_or_else(|| "a2_proxy_deploy_failed: receipt without contract_address".to_string())?;
    if proxy_addr != expected_proxy {
        return Err(format!(
            "a2_proxy_address_drift: receipt {proxy_addr:?} != canonical {expected_proxy:?}"
        ));
    }

    // Post-check: proxy must actually carry code.
    let proxy_code = provider
        .get_code(proxy_addr, None)
        .await
        .map_err(|e| format!("a2_proxy_code_check_failed: {e}"))?;
    if proxy_code.is_empty() {
        return Err(format!(
            "a2_proxy_empty_code: {proxy_addr:?} has no code after deploy"
        ));
    }

    tracing::info!(
        event = "a2.executor_deployed",
        impl = ?impl_addr,
        proxy = ?proxy_addr,
        "ArbitrageExecutor live on anvil fork (canonical addresses)"
    );
    Ok(proxy_addr)
}

/// Refuse to proceed unless the RPC identifies itself as an Anvil node.
async fn ensure_anvil_node(provider: &Provider<Http>) -> Result<(), String> {
    let version: String = provider
        .request("web3_clientVersion", ())
        .await
        .map_err(|e| format!("a2_client_version_failed: {e}"))?;
    if !version.to_lowercase().contains("anvil") {
        return Err(format!(
            "a2_not_an_anvil_node: web3_clientVersion={version:?} — refusing to use the well-known dev key"
        ));
    }
    Ok(())
}

/// Send one contract-creation transaction signed by `wallet` at the EXPLICIT
/// `nonce` (the deploy sequence pins nonces — never trust provider-side
/// pending-count semantics mid-sequence), wait for its receipt (anvil
/// auto-mines), and return it (status must be 1).
async fn send_deploy_tx(
    provider: &Provider<Http>,
    wallet: &LocalWallet,
    data: Vec<u8>,
    nonce: u64,
) -> Result<TransactionReceipt, String> {
    let mut tx = TypedTransaction::Legacy(
        TransactionRequest::new()
            .from(wallet.address())
            .data(Bytes::from(data))
            .value(U256::zero())
            .nonce(nonce),
    );
    provider
        .fill_transaction(&mut tx, None)
        .await
        .map_err(|e| format!("a2_gas_fill_failed: {e}"))?;
    let signature = wallet
        .sign_transaction(&tx)
        .await
        .map_err(|e| format!("a2_sign_failed: {e}"))?;
    let pending = provider
        .send_raw_transaction(tx.rlp_signed(&signature))
        .await
        .map_err(|e| format!("a2_send_failed: {e}"))?;
    let receipt = tokio::time::timeout(
        Duration::from_secs(90),
        pending
            .confirmations(1)
            .interval(Duration::from_millis(250)),
    )
    .await
    .map_err(|_| "a2_deploy_timeout: no receipt within 90s".to_string())?
    .map_err(|e| format!("a2_receipt_failed: {e}"))?;
    match receipt {
        Some(r) if r.status == Some(1.into()) => Ok(r),
        Some(r) => Err(format!("a2_deploy_reverted: status {:?}", r.status)),
        None => Err("a2_deploy_timeout: receipt never arrived".to_string()),
    }
}

/// ERC1967Proxy creation input: creation bytecode ++ abi.encode(logic, data)
/// where data = initialize(address) calldata for `admin`.
fn proxy_init_code(proxy_bytecode: &[u8], logic: Address, admin: Address) -> Vec<u8> {
    let init_data = initialize_calldata(admin);
    let args = ethers::abi::encode(&[
        ethers::abi::Token::Address(logic),
        ethers::abi::Token::Bytes(init_data),
    ]);
    let mut code = Vec::with_capacity(proxy_bytecode.len() + args.len());
    code.extend_from_slice(proxy_bytecode);
    code.extend_from_slice(&args);
    code
}

/// Calldata for `ArbitrageExecutor.initialize(address)`:
/// selector = keccak256("initialize(address)")[0..4] ++ left-padded address.
fn initialize_calldata(admin: Address) -> Vec<u8> {
    let mut out = Vec::with_capacity(36);
    out.extend_from_slice(&keccak256(b"initialize(address)")[..4]);
    out.extend_from_slice(&[0u8; 12]);
    out.extend_from_slice(admin.as_bytes());
    out
}

/// create1 contract address: keccak256(rlp([sender, nonce]))[12..].
/// Handles the standard RLP integer encodings (0 → 0x80, <0x80 → itself,
/// otherwise big-endian with length prefix).
fn create1(sender: Address, nonce: u64) -> Address {
    let mut payload: Vec<u8> = Vec::with_capacity(32);
    // 20-byte string header: 0x80 + 20 = 0x94.
    payload.push(0x94);
    payload.extend_from_slice(sender.as_bytes());
    // RLP integer.
    if nonce == 0 {
        payload.push(0x80);
    } else if nonce < 0x80 {
        payload.push(nonce as u8);
    } else {
        let be = nonce.to_be_bytes();
        let first = be.iter().position(|b| *b != 0).unwrap_or(15);
        let bytes = &be[first..];
        payload.push(0x80 + bytes.len() as u8);
        payload.extend_from_slice(bytes);
    }
    // List header: payload is always ≤ 55 bytes here (21 + ≤9).
    let mut rlp = Vec::with_capacity(payload.len() + 1);
    rlp.push(0xc0 + payload.len() as u8);
    rlp.extend_from_slice(&payload);
    let hash = keccak256(&rlp);
    Address::from_slice(&hash[12..])
}

/// Resolve the forge bytecode-artifact directory.
///
/// Order: `EXECUTOR_ARTIFACTS_DIR` env → container default
/// (`/app/contracts-artifacts`, see sim-ctl/Dockerfile) → dev fallback
/// (`contracts-artifacts/` relative to the crate, i.e. `cargo run` from
/// backend/sim-ctl). Regenerate with:
///   cd contracts && forge build   # then extract bytecode.object from
///   out/ArbitrageExecutor.sol/ArbitrageExecutor.json and
///   out/ERC1967Proxy.sol/ERC1967Proxy.json into {"object":"0x..."} files.
fn artifacts_dir() -> Result<PathBuf, String> {
    if let Ok(dir) = std::env::var("EXECUTOR_ARTIFACTS_DIR") {
        if !dir.is_empty() {
            return Ok(PathBuf::from(dir));
        }
    }
    let container = PathBuf::from("/app/contracts-artifacts");
    if container.is_dir() {
        return Ok(container);
    }
    let dev = PathBuf::from("contracts-artifacts");
    if dev.is_dir() {
        return Ok(dev);
    }
    Err("a2_artifacts_dir_not_found: set EXECUTOR_ARTIFACTS_DIR".to_string())
}

/// Load the creation bytecode ("object") from a committed forge artifact.
fn load_creation_bytecode(name: &str) -> Result<Vec<u8>, String> {
    let path = artifacts_dir()?.join(name);
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("a2_artifact_read_failed({name}): {e}"))?;
    let value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("a2_artifact_parse_failed({name}): {e}"))?;
    let object = value
        .get("object")
        .and_then(|o| o.as_str())
        .ok_or_else(|| format!("a2_artifact_missing_object({name})"))?;
    let stripped = object.strip_prefix("0x").unwrap_or(object);
    hex::decode(stripped).map_err(|e| format!("a2_artifact_hex_invalid({name}): {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ground truth: the canonical first/second create1 addresses from the
    /// well-known Anvil account #0 — also verified empirically against a live
    /// mainnet fork on 2026-08-18 (see module docs).
    #[test]
    fn test_create1_canonical_addresses() {
        let deployer: Address = ANVIL_DEV_ACCOUNT_0.parse().unwrap();
        assert_eq!(
            create1(deployer, 0),
            EXPECTED_EXECUTOR_IMPL.parse::<Address>().unwrap(),
            "create1(anvil#0, 0) must be the canonical impl address"
        );
        assert_eq!(
            create1(deployer, 1),
            EXPECTED_EXECUTOR_PROXY.parse::<Address>().unwrap(),
            "create1(anvil#0, 1) must be the canonical proxy address"
        );
    }

    /// RLP integer encodings inside create1.
    #[test]
    fn test_create1_rlp_integers() {
        let deployer: Address = ANVIL_DEV_ACCOUNT_0.parse().unwrap();
        // nonce 0x41 (< 0x80) encodes as a single byte; nonce 0 encodes 0x80.
        let a = create1(deployer, 0);
        let b = create1(deployer, 1);
        let c = create1(deployer, 0x41);
        assert_ne!(a, b);
        assert_ne!(b, c);
        assert_ne!(a, c);
    }

    /// initialize(address) calldata: 4-byte keccak selector + padded address.
    #[test]
    fn test_initialize_calldata_encoding() {
        let admin: Address = ANVIL_DEV_ACCOUNT_0.parse().unwrap();
        let calldata = initialize_calldata(admin);
        assert_eq!(calldata.len(), 36, "selector + one 32-byte word");
        assert_eq!(
            &calldata[..4],
            &keccak256(b"initialize(address)")[..4],
            "selector must be keccak256(\"initialize(address)\")[0..4]"
        );
        // c4d66de8 is the cast-verified selector used by the OZ proxy init in
        // DeploySepolia.s.sol and the 2026-08-18 fork broadcast.
        assert_eq!(&calldata[..4], &[0xc4, 0xd6, 0x6d, 0xe8]);
        assert_eq!(&calldata[4..16], &[0u8; 12], "address left-padded");
        assert_eq!(&calldata[16..], admin.as_bytes());
    }

    /// The full proxy init-code constructor-args tail must match the actual
    /// forge broadcast captured on a live mainnet fork (2026-08-18):
    /// abi.encode(impl, initialize(admin)) = 5 words (160 bytes).
    #[test]
    fn test_proxy_ctor_args_match_forged_broadcast() {
        let impl_addr: Address = EXPECTED_EXECUTOR_IMPL.parse().unwrap();
        let admin: Address = ANVIL_DEV_ACCOUNT_0.parse().unwrap();
        let init_data = initialize_calldata(admin);
        let args = ethers::abi::encode(&[
            ethers::abi::Token::Address(impl_addr),
            ethers::abi::Token::Bytes(init_data),
        ]);
        let expected_hex = concat!(
            // abi.encode(impl, bytes)
            "0000000000000000000000005fbdb2315678afecb367f032d93f642f64180aa3", // logic
            "0000000000000000000000000000000000000000000000000000000000000040", // bytes offset
            "0000000000000000000000000000000000000000000000000000000000000024", // bytes length 36
            "c4d66de8",                                 // initialize(address)
            "000000000000000000000000",                 // 12-byte pad
            "f39fd6e51aad88f6f4ce6ab8827279cfffb92266", // admin = anvil#0
            "00000000000000000000000000000000000000000000000000000000"  // 28-byte tail pad
        );
        assert_eq!(hex::encode(&args), expected_hex);
    }

    /// Committed artifacts must exist, parse, and be non-trivial bytecode.
    /// (Runs against the crate-relative fallback dir under cargo test.)
    #[test]
    fn test_artifacts_parse() {
        // Force the crate-relative dir regardless of host layout.
        std::env::set_var("EXECUTOR_ARTIFACTS_DIR", "contracts-artifacts");
        let impl_bc = load_creation_bytecode("ArbitrageExecutor.bytecode.json").unwrap();
        let proxy_bc = load_creation_bytecode("ERC1967Proxy.bytecode.json").unwrap();
        std::env::remove_var("EXECUTOR_ARTIFACTS_DIR");
        assert!(
            impl_bc.len() > 5_000,
            "ArbitrageExecutor creation bytecode implausibly small ({} bytes)",
            impl_bc.len()
        );
        assert!(
            proxy_bc.len() > 500,
            "ERC1967Proxy creation bytecode implausibly small ({} bytes)",
            proxy_bc.len()
        );
        // EVM creation code starts with a PUSH/constructor prologue, never 0xef.
        assert_ne!(impl_bc[0], 0xef);
        assert_ne!(proxy_bc[0], 0xef);
    }

    /// E2E against a LOCAL anvil mainnet fork (needs network + anvil):
    ///   anvil --fork-url <mainnet rpc> --port 18545 --chain-id 1
    ///   cargo test -p sim-ctl --bins executor_deploy -- --ignored --nocapture
    /// Exercises the REAL boot path: clientVersion gate, anvil_setNonce pin,
    /// both deploy txs, canonical-address assertions, idempotent re-entry.
    #[tokio::test]
    #[ignore = "requires a local anvil fork (ANVIL_URL, default http://127.0.0.1:18545)"]
    async fn test_ensure_executor_on_live_local_anvil_fork() {
        std::env::set_var("EXECUTOR_ARTIFACTS_DIR", "contracts-artifacts");
        let url = std::env::var("ANVIL_URL").unwrap_or_else(|_| "http://127.0.0.1:18545".into());
        let provider =
            Arc::new(Provider::<Http>::try_from(url.as_str()).expect("anvil url must parse"));
        let proxy = ensure_executor_on_fork(provider.clone())
            .await
            .expect("fork deploy must succeed");
        let expected: Address = EXPECTED_EXECUTOR_PROXY.parse().unwrap();
        assert_eq!(proxy, expected, "deploy must land on the canonical proxy");
        let code = provider
            .get_code(proxy, None)
            .await
            .expect("proxy code read");
        assert!(!code.is_empty(), "proxy must carry code");
        // Idempotent second entry reuses the existing deployment.
        let again = ensure_executor_on_fork(provider.clone())
            .await
            .expect("re-entry must succeed");
        assert_eq!(again, expected);
        std::env::remove_var("EXECUTOR_ARTIFACTS_DIR");
    }
}
