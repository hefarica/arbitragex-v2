//! config_bundle — Encrypted Config Bundle importer types + decrypt + validate.
//!
//! Mirrors the Python shipper (`scripts/arbx-env-deploy/encrypt_and_ship_bundle.py`)
//! byte-for-byte so a bundle encrypted on the operator PC decrypts cleanly here.
//!
//! ## Binary format (magic-prefixed, big-endian length)
//!   `ARBX1` || `u32(wrapped_key_len)` || `rsa_wrapped_aes_key` || `nonce(12)` || `aes_gcm_ct`
//!
//! - `rsa_wrapped_aes_key` = RSA-OAEP-SHA256-4096 of a random 32-byte AES key.
//! - `aes_gcm_ct`          = AES-256-GCM of the UTF-8 JSON bundle payload,
//!                           with the 16-byte auth tag appended (cryptography
//!                           lib / aes-gcm crate both use the ct||tag convention).
//!
//! ## Doctrinal invariants (RULE 00 + gates 6/10)
//! - `paper_mode` / deploy keys / multisig / mainnet-RPC NEVER appear in a bundle.
//!   Filtered at serialize-time (Python) AND re-asserted here post-decrypt (3rd layer).
//! - Capital stays 0. This module never touches the executor, signer, or paper_mode flag.
//! - Fail-honest (R8): every failure mode surfaces a typed `BundleError`; none are silent.

use std::collections::HashMap;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use rsa::pkcs8::DecodePrivateKey;
use rsa::{Oaep, RsaPrivateKey};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

/// Magic prefix the shipper writes at the head of every `.enc` bundle.
pub const MAGIC: &[u8; 5] = b"ARBX1";

/// AES-GCM nonce length (bytes). Fixed by the shipper.
const NONCE_LEN: usize = 12;

/// Keys that must NEVER ship in a bundle — the master safety surface.
/// Replicated from the Python shipper's `NEVER_SHIP` set (defense-in-depth layer 3
/// of 3: VBA skips → Python asserts → importer re-asserts).
pub const NEVER_SHIP: &[&str] = &[
    "ARBX_PAPER_MODE",
    "ARBX_PAPER_TRADE",
    "PAPER_MODE",
    "DEPLOYER_PRIVATE_KEY",
    "DEPLOYER_KEY",
    "MULTISIG_ADDRESS",
    "CONFIRM_MAINNET_DEPLOY",
    "MAINNET_RPC_URL",
];

// ---------------------------------------------------------------------------
// Typed mirror of bundle_schema.json
// ---------------------------------------------------------------------------

/// A factory (DEX router) on a chain. The `dex_name` resolves to a `dexes.id` UUID
/// at apply-time via subquery (FK-safe, UUID-independent) — see `gen_chain_env.py`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactoryEntry {
    pub dex_name: String,
    pub address: String,
}

/// One chain entry: metadata + multi-provider RPC CSVs + the DEX routers active on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainEntry {
    pub chain_id: i64,
    pub name: String,
    pub native_currency: String,
    pub explorer_url: String,
    /// Multi-provider CSV: `prov=url,prov=url` (order shuffled per-run on the shipper side).
    pub rpc_http: String,
    pub rpc_ws: String,
    #[serde(default)]
    pub factories: Vec<FactoryEntry>,
}

/// The full decrypted + validated config bundle. One Excel → one bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigBundle {
    pub schema_version: String,
    pub generated_at: String,
    pub env_vars: HashMap<String, String>,
    pub chains: Vec<ChainEntry>,
    #[serde(default)]
    pub api_keys: HashMap<String, String>,
    pub contract_addresses: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Every bundle failure mode. None are silent; each carries enough context to surface
/// to the operator via the importer's JSON report or non-zero exit.
#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    #[error("magic mismatch: expected {expected:?}, got {got:?}")]
    MagicMismatch { expected: String, got: String },
    #[error("truncated .enc: need {needed} bytes for the length header, have {have}")]
    TruncatedLength { needed: usize, have: usize },
    #[error("truncated .enc: declared wrapped_key_len={declared} but only {have} bytes follow the header")]
    TruncatedBody { declared: usize, have: usize },
    #[error("truncated .enc: missing nonce ({have} bytes, need {need}")]
    TruncatedNonce { have: usize, need: usize },
    #[error("AES key unwrap produced {got} bytes, expected 32 (AES-256)")]
    BadAesKeyLen { got: usize },
    #[error("RSA private-key parse failed: {0}")]
    KeyParse(String),
    #[error("RSA-OAEP unwrap failed (wrong key / corrupted wrapped block): {0}")]
    RsaUnwrap(String),
    #[error("AES-256-GCM decrypt failed (tampered ciphertext or nonce): {0}")]
    AesDecrypt(String),
    #[error("decrypted payload is not valid JSON: {0}")]
    BadJson(String),
    #[error("schema validation failed: {0}")]
    SchemaValidation(String),
    #[error("FATAL — forbidden key shipped in env_vars: {0:?}")]
    NeverShipLeak(Vec<String>),
    #[error("schema_version mismatch: expected \"1.0\", got {0:?}")]
    BadSchemaVersion(String),
}

// ---------------------------------------------------------------------------
// Binary-format parse + hybrid decrypt
// ---------------------------------------------------------------------------

/// Parsed binary envelope: the three regions the shipper concatenated.
struct Envelope {
    wrapped_key: Vec<u8>,
    nonce: Vec<u8>,
    ct: Vec<u8>,
}

fn parse_envelope(enc: &[u8]) -> Result<Envelope, BundleError> {
    if enc.len() < MAGIC.len() + 4 {
        return Err(BundleError::TruncatedLength {
            needed: MAGIC.len() + 4,
            have: enc.len(),
        });
    }
    if &enc[..MAGIC.len()] != MAGIC {
        return Err(BundleError::MagicMismatch {
            expected: String::from_utf8_lossy(MAGIC).to_string(),
            got: String::from_utf8_lossy(&enc[..MAGIC.len()]).to_string(),
        });
    }
    let len_bytes = &enc[MAGIC.len()..MAGIC.len() + 4];
    let wrapped_key_len = u32::from_be_bytes([
        len_bytes[0],
        len_bytes[1],
        len_bytes[2],
        len_bytes[3],
    ]) as usize;
    let body_start = MAGIC.len() + 4;
    let wrapped_end = body_start.checked_add(wrapped_key_len).ok_or_else(|| {
        BundleError::TruncatedBody { declared: wrapped_key_len, have: enc.len() - body_start }
    })?;
    if enc.len() < wrapped_end + NONCE_LEN {
        return Err(BundleError::TruncatedBody {
            declared: wrapped_key_len,
            have: enc.len().saturating_sub(body_start),
        });
    }
    let wrapped_key = enc[body_start..wrapped_end].to_vec();
    let nonce = enc[wrapped_end..wrapped_end + NONCE_LEN].to_vec();
    let ct = enc[wrapped_end + NONCE_LEN..].to_vec();
    if ct.is_empty() {
        return Err(BundleError::AesDecrypt("empty ciphertext body".into()));
    }
    Ok(Envelope { wrapped_key, nonce, ct })
}

/// RSA-OAEP-SHA256 unwrap the random AES key, then AES-256-GCM decrypt the payload.
/// Returns the raw UTF-8 JSON bytes (NOT yet schema-validated).
pub fn decrypt_payload(enc: &[u8], private_key_pem: &str) -> Result<Vec<u8>, BundleError> {
    let env = parse_envelope(enc)?;
    let private_key = RsaPrivateKey::from_pkcs8_pem(private_key_pem)
        .map_err(|e| BundleError::KeyParse(e.to_string()))?;
    let oaep = Oaep::new::<Sha256>();
    let aes_key = private_key
        .decrypt(oaep, &env.wrapped_key)
        .map_err(|e| BundleError::RsaUnwrap(e.to_string()))?;
    if aes_key.len() != 32 {
        return Err(BundleError::BadAesKeyLen { got: aes_key.len() });
    }
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&aes_key));
    let plaintext = cipher
        .decrypt(Nonce::from_slice(&env.nonce), env.ct.as_ref())
        .map_err(|e| BundleError::AesDecrypt(e.to_string()))?;
    Ok(plaintext)
}

// ---------------------------------------------------------------------------
// Schema + doctrinal validation
// ---------------------------------------------------------------------------

/// Validate a raw decrypted JSON `Value` against the bundle JSON Schema.
/// Kept as a `Value` (not the typed struct) so jsonschema can report exact paths.
pub fn validate_schema(bundle_value: &serde_json::Value, schema_json: &str) -> Result<(), BundleError> {
    let schema: serde_json::Value = serde_json::from_str(schema_json)
        .map_err(|e| BundleError::SchemaValidation(format!("schema itself is not valid JSON: {e}")))?;
    let compiled = jsonschema::JSONSchema::compile(&schema)
        .map_err(|e| BundleError::SchemaValidation(format!("schema compile failed: {e}")))?;
    if compiled.is_valid(bundle_value) {
        return Ok(());
    }
    let mut msgs = Vec::new();
    for err in compiled.iter_errors(bundle_value) {
        msgs.push(format!("{err}"));
    }
    Err(BundleError::SchemaValidation(msgs.join("; ")))
}

/// Defense-in-depth (layer 3 of 3): re-assert NONE of the NEVER_SHIP keys sneaked in.
/// Python asserts at serialize-time; we assert again post-decrypt in case the shipper
/// or schema drift let one through.
pub fn assert_never_ship(bundle: &ConfigBundle) -> Result<(), BundleError> {
    let leaked: Vec<String> = bundle
        .env_vars
        .keys()
        .filter(|k| NEVER_SHIP.iter().any(|n| n.eq_ignore_ascii_case(k)))
        .cloned()
        .collect();
    if leaked.is_empty() {
        Ok(())
    } else {
        Err(BundleError::NeverShipLeak(leaked))
    }
}

// ---------------------------------------------------------------------------
// Top-level orchestration: .enc bytes → typed + validated ConfigBundle
// ---------------------------------------------------------------------------

/// Decrypt → validate-against-schema → deserialize → assert NEVER_SHIP.
/// This is the single entry point the importer binary and any tests use.
pub fn load_bundle(
    enc: &[u8],
    private_key_pem: &str,
    schema_json: &str,
) -> Result<ConfigBundle, BundleError> {
    let plaintext = decrypt_payload(enc, private_key_pem)?;
    let bundle_value: serde_json::Value = serde_json::from_slice(&plaintext)
        .map_err(|e| BundleError::BadJson(e.to_string()))?;
    validate_schema(&bundle_value, schema_json)?;
    let bundle: ConfigBundle = serde_json::from_value(bundle_value)
        .map_err(|e| BundleError::BadJson(format!("typed deserialize failed: {e}")))?;
    if bundle.schema_version != "1.0" {
        return Err(BundleError::BadSchemaVersion(bundle.schema_version));
    }
    assert_never_ship(&bundle)?;
    Ok(bundle)
}

// ---------------------------------------------------------------------------
// Tests — self-contained roundtrip + tamper + NEVER_SHIP + schema drift.
// The encrypt side lives here under cfg(test) so tests don't depend on the Python
// shipper; production builds only decrypt (no encrypt surface).
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;
    use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey};
    use rsa::{Oaep, RsaPrivateKey, RsaPublicKey};
    use sha2::Sha256;

    /// Mirror the shipper's hybrid_encrypt: produces the exact binary format load_bundle expects.
    fn hybrid_encrypt(plaintext: &[u8], public_key: &RsaPublicKey) -> Vec<u8> {
        use aes_gcm::aead::Aead;
        use aes_gcm::{Aes256Gcm, Key, Nonce};
        let mut rng = OsRng;
        let aes_key = Aes256Gcm::generate_key(&mut rng);
        let nonce_bytes = Aes256Gcm::generate_nonce(&mut rng);
        let ct = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&aes_key))
            .encrypt(&nonce_bytes, plaintext)
            .expect("aes-gcm encrypt in test");
        let oaep = Oaep::new::<Sha256>();
        let wrapped = public_key
            .encrypt(&mut rng, oaep, &aes_key)
            .expect("rsa encrypt in test");
        let mut out = Vec::with_capacity(MAGIC.len() + 4 + wrapped.len() + NONCE_LEN + ct.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&(wrapped.len() as u32).to_be_bytes());
        out.extend_from_slice(&wrapped);
        out.extend_from_slice(nonce_bytes.as_slice());
        out.extend_from_slice(&ct);
        out
    }

    /// A canonical bundle JSON that passes the real schema (matches bundle_schema.json shape).
    fn sample_bundle_json() -> &'static str {
        r#"{
            "schema_version": "1.0",
            "generated_at": "2026-07-05T00:00:00Z",
            "env_vars": {"ARBX_ENABLED_CHAINS": "1,56", "RPC_HTTP_1": "acme=https://x.example"},
            "chains": [{
                "chain_id": 1, "name": "ethereum", "native_currency": "ETH",
                "explorer_url": "https://etherscan.io",
                "rpc_http": "acme=https://x.example", "rpc_ws": "",
                "factories": [{"dex_name": "UniswapV2", "address": "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f"}]
            }],
            "api_keys": {"GOPLUS_API_KEY": "k"},
            "contract_addresses": {"ARBITRAGE_EXECUTOR": "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f"}
        }"#
    }

    /// The production schema, inlined so the test is self-contained (no file I/O).
    /// Path: config_bundle.rs is at backend/shared-rs/src/ → ../../../ = repo root.
    fn schema_json() -> &'static str {
        include_str!("../../../scripts/arbx-env-deploy/bundle_schema.json")
    }

    fn fresh_keypair() -> (RsaPrivateKey, RsaPublicKey) {
        // 2048 bits in tests for speed; production uses 4096 (the format is key-size agnostic).
        let mut rng = rand::thread_rng();
        let priv_key = RsaPrivateKey::new(&mut rng, 2048).expect("gen rsa");
        let pub_key = RsaPublicKey::from(&priv_key);
        (priv_key, pub_key)
    }

    #[test]
    fn roundtrip_load_bundle_decrypts_and_validates() {
        let (priv_key, pub_key) = fresh_keypair();
        let pem = priv_key
            .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
            .expect("pkcs8 pem")
            .to_string();
        let enc = hybrid_encrypt(sample_bundle_json().as_bytes(), &pub_key);
        let bundle = load_bundle(&enc, &pem, schema_json()).expect("load_bundle must succeed");
        assert_eq!(bundle.schema_version, "1.0");
        assert_eq!(bundle.chains.len(), 1);
        assert_eq!(bundle.chains[0].chain_id, 1);
        assert_eq!(bundle.env_vars.get("ARBX_ENABLED_CHAINS"), Some(&"1,56".to_string()));
        assert_eq!(bundle.contract_addresses.len(), 1);
    }

    #[test]
    fn tampered_ciphertext_is_rejected_by_aes_gcm_tag() {
        let (priv_key, pub_key) = fresh_keypair();
        let pem = priv_key.to_pkcs8_pem(rsa::pkcs8::LineEnding::LF).unwrap().to_string();
        let mut enc = hybrid_encrypt(sample_bundle_json().as_bytes(), &pub_key);
        // Flip the last byte (inside the AES-GCM auth tag region).
        let last = enc.len() - 1;
        enc[last] ^= 0xFF;
        let err = load_bundle(&enc, &pem, schema_json()).unwrap_err();
        assert!(matches!(err, BundleError::AesDecrypt(_)), "got {err:?}");
    }

    #[test]
    fn never_ship_key_is_caught_post_decrypt() {
        let (priv_key, pub_key) = fresh_keypair();
        let pem = priv_key.to_pkcs8_pem(rsa::pkcs8::LineEnding::LF).unwrap().to_string();
        // Build a bundle that smuggles PAPER_MODE — would be caught at serialize-time by the
        // shipper; we verify the importer ALSO catches it (layer 3).
        let mut v: serde_json::Value = serde_json::from_str(sample_bundle_json()).unwrap();
        v["env_vars"]["PAPER_MODE"] = serde_json::json!("true");
        let enc = hybrid_encrypt(serde_json::to_vec(&v).unwrap().as_slice(), &pub_key);
        let err = load_bundle(&enc, &pem, schema_json()).unwrap_err();
        assert!(matches!(err, BundleError::NeverShipLeak(_)), "got {err:?}");
    }

    #[test]
    fn bad_magic_is_rejected() {
        let (_, pub_key) = fresh_keypair();
        let mut enc = hybrid_encrypt(sample_bundle_json().as_bytes(), &pub_key);
        enc[0] = b'X'; // corrupt magic
        let (priv_key, _) = fresh_keypair();
        let pem = priv_key.to_pkcs8_pem(rsa::pkcs8::LineEnding::LF).unwrap().to_string();
        let err = load_bundle(&enc, &pem, schema_json()).unwrap_err();
        assert!(matches!(err, BundleError::MagicMismatch { .. }), "got {err:?}");
    }

    #[test]
    fn truncated_envelope_is_rejected() {
        let (_, pub_key) = fresh_keypair();
        let enc = hybrid_encrypt(sample_bundle_json().as_bytes(), &pub_key);
        let (priv_key, _) = fresh_keypair();
        let pem = priv_key.to_pkcs8_pem(rsa::pkcs8::LineEnding::LF).unwrap().to_string();
        // Cut to just the magic + 2 bytes of length.
        let truncated = &enc[..MAGIC.len() + 2];
        let err = load_bundle(truncated, &pem, schema_json()).unwrap_err();
        assert!(matches!(err, BundleError::TruncatedLength { .. }), "got {err:?}");
    }
}
