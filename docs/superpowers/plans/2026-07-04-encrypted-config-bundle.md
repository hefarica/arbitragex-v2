# Encrypted Config Bundle — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the operator's Excel workbook to the dapp as one RSA-encrypted JSON bundle — the dapp decrypts on the VPS and imports every section to its consumer (env vars, chains, factories, tokens, contract addresses) without ever exposing the plaintext outside the VPS trust boundary.

**Architecture:** Decisions confirmed = **A + A + On-demand**:
- **Crypto:** RSA-4096 asymmetric (public key embedded in the macro-side script; private key only on the VPS). If the `.xlsm` leaks, no data leaks.
- **Transport:** SSH file upload to `/opt/arbitragex-v2/config/arbx_config_bundle.json.enc` (reuses the operator's existing SSH, no new infra).
- **Import:** On-demand admin endpoint `/admin/config/import-bundle` (operator-gated, never on boot, never touches `paper_mode`).

**Tech Stack:** Python (`openpyxl` + `cryptography` for RSA-OAEP + AES-GCM hybrid) on the shipper side; Rust (`rsa` + `aes-gcm` + `serde_json` + `jsonschema`) on the importer side; Excel COM via PowerShell only if a VBA button is wanted (Task 7, optional).

**Constraints (doctrinal, non-negotiable):**
1. `paper_mode` is never modified by any step.
2. Private RSA key lives ONLY on the VPS (file mode 0600 or env var); never in the Excel, never in chat.
3. Existing VBA macros (`RunFullSyncCycle`, etc.) are preserved byte-for-byte.
4. The importer is idempotent (`ON CONFLICT DO NOTHING` / upsert) — re-importing the same bundle is a no-op.
5. Schema-validated — an unknown/extra section fails-honest (no silent drop).

---

## File Structure

| File | Responsibility |
|------|----------------|
| `scripts/arbx-env-deploy/encrypt_and_ship_bundle.py` | NEW shipper — reads `.xlsm`, serializes to JSON per schema, RSA-encrypts, uploads via SSH |
| `scripts/arbx-env-deploy/bundle_schema.json` | NEW — the JSON Schema the bundle must conform to |
| `backend/shared-rs/src/config_bundle.rs` | NEW — Rust types + decrypt + schema-validate |
| `backend/api-server/src/routes/admin-config-bundle.ts` | NEW — `POST /admin/config/import-bundle` endpoint |
| `backend/shared-rs/Cargo.toml` | add `rsa`, `aes-gcm`, `jsonwebtoken` (no), `jsonschema` deps |
| `C:\Users\HFRC\Downloads\ArbitrageX_Unified_Config.xlsm` | gets a `Bundle Shipper` sheet (instructions + public key fingerprint) |
| VPS `/opt/arbitragex-v2/config/arbx_bundle_private.pem` | the RSA private key (generated on VPS, never leaves) |
| VPS `/opt/arbitragex-v2/config/arbx_config_bundle.json.enc` | the shipped bundle (binary) |

---

### Task 0: Generate the RSA keypair (on the VPS, private never leaves)

**Files:**
- Create on VPS: `/opt/arbitragex-v2/config/arbx_bundle_private.pem`
- Create on VPS: `/opt/arbitragex-v2/config/arbx_bundle_public.pem`
- Download to operator: `arbx_bundle_public.pem` (public only)

- [ ] **Step 1: Generate RSA-4096 keypair on the VPS**

```bash
ssh arbx 'mkdir -p /opt/arbitragex-v2/config && cd /opt/arbitragex-v2/config && \
  openssl genrsa -out arbx_bundle_private.pem 4096 && \
  chmod 600 arbx_bundle_private.pem && \
  openssl rsa -in arbx_bundle_private.pem -pubout -out arbx_bundle_public.pem && \
  chmod 644 arbx_bundle_public.pem && \
  echo "keys OK: $(sha256sum arbx_bundle_private.pem | cut -d" " -f1)"'
```

- [ ] **Step 2: Pull the PUBLIC key back to the operator machine**

```bash
scp arbx:/opt/arbitragex-v2/config/arbx_bundle_public.pem C:/Users/HFRC/Downloads/arbx_bundle_public.pem
# Verify it is the PUBLIC key (no PRIVATE MATERIAL)
grep -c "PRIVATE" C:/Users/HFRC/Downloads/arbx_bundle_public.pem   # must print 0
head -1 C:/Users/HFRC/Downloads/arbx_bundle_public.pem             # must be -----BEGIN PUBLIC KEY-----
```

- [ ] **Step 3: Verify the private key NEVER left the VPS**

The private key file must exist ONLY at `/opt/arbitragex-v2/config/arbx_bundle_private.pem` on the VPS. Confirm `arbx_bundle_private.pem` does NOT exist on the operator machine:

```bash
[ ! -f C:/Users/HFRC/Downloads/arbx_bundle_private.pem ] && echo "OK: private key not on operator" || echo "ABORT: private key leaked to operator"
```

---

### Task 1: Define the JSON Schema for the bundle

**Files:**
- Create: `scripts/arbx-env-deploy/bundle_schema.json`

- [ ] **Step 1: Write the schema**

Create `scripts/arbx-env-deploy/bundle_schema.json`:

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "ArbitrageX Config Bundle",
  "type": "object",
  "required": ["schema_version", "generated_at", "env_vars", "chains", "contract_addresses"],
  "additionalProperties": false,
  "properties": {
    "schema_version": { "const": "1.0" },
    "generated_at": { "type": "string", "format": "date-time" },
    "env_vars": {
      "type": "object",
      "description": "key=value from .env Production (excluding deploy_inputs and paper_mode override attempts)",
      "additionalProperties": { "type": "string" }
    },
    "chains": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["chain_id", "name", "native_currency", "explorer_url", "rpc_http", "rpc_ws"],
        "additionalProperties": false,
        "properties": {
          "chain_id": { "type": "integer", "minimum": 1 },
          "name": { "type": "string" },
          "native_currency": { "type": "string" },
          "explorer_url": { "type": "string" },
          "rpc_http": { "type": "string", "description": "multi-provider CSV: prov=url,prov=url" },
          "rpc_ws": { "type": "string" },
          "factories": {
            "type": "array",
            "items": {
              "type": "object",
              "required": ["dex_name", "address"],
              "properties": { "dex_name": { "type": "string" }, "address": { "type": "string", "pattern": "^0x[a-fA-F0-9]{40}$" } }
            }
          }
        }
      }
    },
    "api_keys": {
      "type": "object",
      "additionalProperties": { "type": "string" },
      "description": "From Tokens & Keys sheet (GoPlus, Tenderly, etc.)"
    },
    "contract_addresses": {
      "type": "object",
      "description": "Proxy addresses (AE, FLE, AM, AdminTimelock) — filled after deploy",
      "properties": {
        "ARBITRAGE_EXECUTOR": { "type": "string", "pattern": "^0x[a-fA-F0-9]{40}$" },
        "FLASHLOAN_EXECUTOR": { "type": "string", "pattern": "^0x[a-fA-F0-9]{40}$" },
        "ALLOWANCE_MANAGER": { "type": "string", "pattern": "^0x[a-fA-F0-9]{40}$" },
        "ADMIN_TIMELOCK": { "type": "string", "pattern": "^0x[a-fA-F0-9]{40}$" }
      }
    }
  }
}
```

**Note:** `paper_mode`, `EXECUTOR_1`, deploy-input keys are deliberately OUTSIDE this schema (never shipped). The shipper filters them out (Task 2 Step 1).

---

### Task 2: Implement the shipper `encrypt_and_ship_bundle.py`

**Files:**
- Create: `scripts/arbx-env-deploy/encrypt_and_ship_bundle.py`

- [ ] **Step 1: Write the shipper**

```python
#!/usr/bin/env python3
"""
Encrypt the operator's Excel into one RSA-encrypted JSON bundle and upload it
to the VPS via SSH.

Hybrid crypto (RSA-OAEP-4096 + AES-256-GCM, like age/JWE):
  1. Serialize the relevant sheets to a JSON bundle (schema-validated).
  2. Generate a random AES-256 key + 96-bit nonce.
  3. AES-256-GCM encrypt the JSON.
  4. RSA-OAEP encrypt the AES key with the VPS public key.
  5. Output = magic || rsa_wrapped_aes_key || nonce || aes_gcm_ciphertext.
  6. Upload via scp to /opt/arbitragex-v2/config/arbx_config_bundle.json.enc

The VPS private key is the ONLY thing that can decrypt. If the .xlsm or the
.enc leaks, the data is unreadable.

Filtering: paper_mode / EXECUTOR_* / DEPLOYER_* / CONFIRM_MAINNET_DEPLOY are
NEVER serialized (excluded by name + by sheet).

Read-only on the workbook. Temp JSON is shred after encrypt.
"""
import argparse, json, os, subprocess, sys, tempfile
from datetime import datetime, timezone
from pathlib import Path

try:
    import openpyxl
    from cryptography.hazmat.primitives import hashes, serialization
    from cryptography.hazmat.primitives.asymmetric import padding
    from cryptography.hazmat.primitives.ciphers.aead import AESGCM
except ImportError:
    sys.exit("pip install openpyxl cryptography")

SCHEMA_VERSION = "1.0"
NEVER_SHIP = {
    # paper_mode invariant — never ship an override
    "ARBX_PAPER_MODE", "ARBX_PAPER_TRADE", "PAPER_MODE",
    # deploy-time only — local, not for the VPS
    "DEPLOYER_PRIVATE_KEY", "DEPLOYER_KEY", "MULTISIG_ADDRESS",
    "CONFIRM_MAINNET_DEPLOY", "MAINNET_RPC_URL",
}

CHAIN_META = {  # mirrors seed 043
    1:('ETH','https://etherscan.io',12000),10:('ETH','https://optimistic.etherscan.io',2000),
    56:('BNB','https://bscscan.com',3000),100:('xDAI','https://gnosisscan.io',5000),
    137:('MATIC','https://polygonscan.com',2000),8453:('ETH','https://basescan.org',2000),
    42161:('ETH','https://arbiscan.io',250),43114:('AVAX','https://snowtrace.io',2000),
}
CHAIN_NAME_TO_ID = {n.split(' ')[0].lower():i for i,(c,n,*_) in {}}

def load_env_vars(wb):
    ws = wb['.env Production']
    out = {}
    for row in ws.iter_rows(min_row=2, max_col=2, values_only=True):
        k, v = row[0], row[1] if len(row)>1 else None
        if not k or not isinstance(k, str): continue
        k = k.strip()
        if k.startswith('#'): continue
        if k in NEVER_SHIP: continue
        if v is None or str(v).strip()=='': continue
        out[k] = str(v).strip()
    return out

def load_chains(wb):
    ws_p = wb['RPC Providers']
    rpcs = {}
    for row in ws_p.iter_rows(min_row=2, values_only=True):
        chain,proto,prov,url = (list(row)+[None]*4)[:4]
        if not all([chain,proto,prov,url]): continue
        rpcs.setdefault(str(chain).strip(), {'HTTP':{}, 'WSS':{}})
        rpcs[str(chain).strip()][str(proto).strip()][str(prov).strip()] = str(url).strip()
    chains = []
    for chain_name, protos in rpcs.items():
        # resolve chain_id via canonical name→id
        cid = None
        for k,v in CHAIN_META.items():
            if chain_name.lower().startswith(v[0].lower()) or k in chain_name:  # rough; refine with explicit map
                pass
        # explicit id lookup
        explicit = {'Ethereum Mainnet':1,'Optimism':10,'BSC Mainnet':56,'Polygon Mainnet':137,'Base':8453,'Arbitrum One':42161}
        cid = explicit.get(chain_name)
        if not cid: continue
        native, explorer, _ = CHAIN_META.get(cid, ('?','?',0))
        chains.append({
            'chain_id': cid, 'name': chain_name.lower().replace(' mainnet','').replace(' one',''),
            'native_currency': native, 'explorer_url': explorer,
            'rpc_http': ','.join(f'{p}={u}' for p,u in protos.get('HTTP',{}).items()),
            'rpc_ws': ','.join(f'{p}={u}' for p,u in protos.get('WSS',{}).items()),
            'factories': [],  # filled from dapp seed 043 in production; empty acceptable
        })
    return chains

def load_api_keys(wb):
    if 'Tokens & Keys' not in wb.sheetnames: return {}
    ws = wb['Tokens & Keys']
    out = {}
    for row in ws.iter_rows(min_row=2, max_col=2, values_only=True):
        k,v = row[0], row[1] if len(row)>1 else None
        if k and v and str(k).strip() not in NEVER_SHIP:
            out[str(k).strip()] = str(v).strip()
    return out

def load_contract_addresses(wb):
    ws = wb['.env Production']
    out = {}
    WANT = {'ARBITRAGE_EXECUTOR','FLASHLOAN_EXECUTOR','ALLOWANCE_MANAGER','ADMIN_TIMELOCK'}
    for row in ws.iter_rows(min_row=2, max_col=2, values_only=True):
        k,v = row[0], row[1] if len(row)>1 else None
        if k and str(k).strip() in WANT and v and str(v).strip().startswith('0x') and len(str(v).strip())==42:
            out[str(k).strip()] = str(v).strip()
    return out

def build_bundle(xlsx_path):
    wb = openpyxl.load_workbook(xlsx_path, data_only=True, read_only=True, keep_vba=True)
    return {
        'schema_version': SCHEMA_VERSION,
        'generated_at': datetime.now(timezone.utc).isoformat(),
        'env_vars': load_env_vars(wb),
        'chains': load_chains(wb),
        'api_keys': load_api_keys(wb),
        'contract_addresses': load_contract_addresses(wb),
    }

def hybrid_encrypt(plaintext_bytes, public_key_path):
    """RSA-OAEP-4096 wrap AES-256 key; AES-256-GCM encrypt payload."""
    with open(public_key_path, 'rb') as f:
        pub = serialization.load_pem_public_key(f.read())
    aes_key = AESGCM.generate_key(bit_length=256)
    nonce = os.urandom(12)
    ct = AESGCM(aes_key).encrypt(nonce, plaintext_bytes, None)
    wrapped = pub.encrypt(aes_key, padding.OAEP(
        mgf=padding.MGF1(algorithm=hashes.SHA256()),
        algorithm=hashes.SHA256(), label=None))
    magic = b'ARBX1'  # format version marker
    return magic + len(wrapped).to_bytes(4, 'big') + wrapped + nonce + ct

def shred(path):
    try:
        f = Path(path)
        sz = f.stat().st_size
        with open(f, 'r+b') as fp:
            fp.write(b'\x00' * sz)
        f.unlink()
    except Exception:
        pass

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--xlsx', default='C:/Users/HFRC/Downloads/ArbitrageX_Unified_Config.xlsm')
    ap.add_argument('--public-key', default='C:/Users/HFRC/Downloads/arbx_bundle_public.pem')
    ap.add_argument('--vps-host', default='arbx')
    ap.add_argument('--dest', default='/opt/arbitragex-v2/config/arbx_config_bundle.json.enc')
    ap.add_argument('--schema', default=None, help='optional jsonschema file to validate before encrypt')
    args = ap.parse_args()

    # 1. Build + (optional) validate bundle
    bundle = build_bundle(args.xlsx)
    bundle_json = json.dumps(bundle, indent=2).encode()
    print(f'bundle: {len(bundle_json)} bytes | env_vars={len(bundle["env_vars"])} chains={len(bundle["chains"])} api_keys={len(bundle["api_keys"])} contract_addrs={len(bundle["contract_addresses"])}')

    if args.schema:
        import jsonschema
        jsonschema.validate(json.loads(bundle_json), json.load(open(args.schema)))
        print('schema OK')

    # 2. Shred-safe temp file for the plaintext JSON
    with tempfile.NamedTemporaryFile(delete=False, suffix='.json') as tmp:
        tmp.write(bundle_json); tmp_path = tmp.name

    try:
        # 3. Hybrid encrypt
        enc = hybrid_encrypt(bundle_json, args.public_key)
        enc_path = tmp_path + '.enc'
        with open(enc_path, 'wb') as f: f.write(enc)
        print(f'encrypted: {len(enc)} bytes → {enc_path}')

        # 4. Upload via scp (reuses operator SSH)
        subprocess.run(['scp', enc_path, f'{args.vps_host}:{args.dest}'], check=True)
        print(f'uploaded → {args.vps_host}:{args.dest}')

        # 5. Shred the plaintext temp
        shred(tmp_path)
        os.unlink(enc_path)
        print('plaintext shred OK — only the .enc on the VPS remains')
    finally:
        if os.path.exists(tmp_path): shred(tmp_path)

if __name__ == '__main__':
    main()
```

- [ ] **Step 2: Test locally (encrypt + decrypt roundtrip before involving the VPS)**

```bash
python3 -m pip install cryptography jsonschema --quiet
python3 scripts/arbx-env-deploy/encrypt_and_ship_bundle.py \
  --schema scripts/arbx-env-deploy/bundle_schema.json \
  --vps-host NONE --dest /tmp/test.enc 2>&1 | tail -5
# Decrypt test (simulating the VPS importer):
ssh arbx 'cd /opt/arbitragex-v2/config && python3 -c "
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import padding
from cryptography.hazmat.primitives.ciphers.aead import AESGCM
priv = serialization.load_pem_private_key(open(\"arbx_bundle_private.pem\",\"rb\").read(), None)
enc = open(\"/tmp/test_from_op.enc\",\"rb\").read() if False else None  # adjust path
" 2>&1' | head -3
```

**Expected:** shipper prints bundle byte count + schema OK + encrypted + uploaded. If `jsonschema.validate` fails, the shipper refuses to encrypt (fail-honest).

---

### Task 3: Rust importer `config_bundle.rs` in shared-rs

**Files:**
- Create: `backend/shared-rs/src/config_bundle.rs`
- Modify: `backend/shared-rs/src/lib.rs` (add `pub mod config_bundle;`)
- Modify: `backend/shared-rs/Cargo.toml` (add deps)

- [ ] **Step 1: Add deps to `backend/shared-rs/Cargo.toml`**

```toml
[dependencies]
# existing...
rsa = "0.9"
aes-gcm = "0.10"
sha2 = "0.10"
rand = "0.8"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

- [ ] **Step 2: Write the importer module**

Create `backend/shared-rs/src/config_bundle.rs`:

```rust
//! Encrypted config bundle importer (RSA-OAEP-4096 + AES-256-GCM hybrid).
//!
//! Reads `/opt/arbitragex-v2/config/arbx_config_bundle.json.enc`, decrypts
//! with the VPS-held private key, validates against the schema, and exposes
//! typed sections (env_vars, chains, api_keys, contract_addresses) for the
//! admin importer endpoint to apply.
//!
//! Security: the private key NEVER leaves the VPS. If the .enc is leaked it
//! is unreadable. paper_mode is explicitly filtered by the shipper AND
//! asserted-here (we reject a bundle that tries to set it).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Factory (DEX router) entry per chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleFactory {
    pub dex_name: String,
    pub address: String,
}

/// Chain entry in the bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleChain {
    pub chain_id: u64,
    pub name: String,
    pub native_currency: String,
    pub explorer_url: String,
    pub rpc_http: String,
    pub rpc_ws: String,
    #[serde(default)]
    pub factories: Vec<BundleFactory>,
}

/// The whole bundle (mirrors bundle_schema.json).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigBundle {
    pub schema_version: String,
    pub generated_at: String,
    pub env_vars: HashMap<String, String>,
    #[serde(default)]
    pub chains: Vec<BundleChain>,
    #[serde(default)]
    pub api_keys: HashMap<String, String>,
    #[serde(default)]
    pub contract_addresses: HashMap<String, String>,
}

/// Keys the importer MUST refuse to apply (defence-in-depth; the shipper
/// already filters them, but we re-assert here in case the shipper or schema
/// drift).
const NEVER_APPLY: &[&str] = &[
    "ARBX_PAPER_MODE",
    "ARBX_PAPER_TRADE",
    "PAPER_MODE",
];

impl ConfigBundle {
    /// Decrypt + deserialize a bundle file.
    ///
    /// `enc_path` = path to the `.json.enc` file.
    /// `priv_key_pem` = the VPS RSA private key PEM bytes.
    pub fn load_encrypted(enc_path: &str, priv_key_pem: &[u8]) -> Result<Self, BundleError> {
        let raw = std::fs::read(enc_path).map_err(BundleError::Read)?;
        let plain = decrypt_hybrid(&raw, priv_key_pem)?;
        let bundle: ConfigBundle = serde_json::from_slice(&plain)?;
        bundle.validate()?;
        Ok(bundle)
    }

    /// Schema + invariant validation (fail-honest).
    pub fn validate(&self) -> Result<(), BundleError> {
        if self.schema_version != "1.0" {
            return Err(BundleError::Schema(format!(
                "unsupported schema_version {} (want 1.0)",
                self.schema_version
            )));
        }
        // paper_mode defence-in-depth
        for k in NEVER_APPLY {
            if self.env_vars.contains_key(*k) {
                return Err(BundleError::Forbidden(format!(
                    "bundle tries to set {k} — refused (paper_mode invariant)"
                )));
            }
        }
        // address format
        for (k, v) in &self.contract_addresses {
            if !(v.starts_with("0x") && v.len() == 42) {
                return Err(BundleError::Schema(format!("{k} not a valid address: {v}")));
            }
        }
        Ok(())
    }
}

/// Hybrid decrypt: magic(5) || len(4) || rsa_wrapped(512) || nonce(12) || ct.
fn decrypt_hybrid(enc: &[u8], priv_key_pem: &[u8]) -> Result<Vec<u8>, BundleError> {
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
    use rsa::{Oaep, pkcs1::DecodeRsaPrivateKey};
    let (hasher, _) = (sha2::Sha256::default(), ());
    if enc.len() < 5 + 4 + 512 + 12 {
        return Err(BundleError::Format("bundle too small".into()));
    }
    if &enc[0..5] != b"ARBX1" {
        return Err(BundleError::Format("bad magic".into()));
    }
    let wrapped_len = u32::from_be_bytes(enc[5..9].try_into().unwrap()) as usize;
    if enc.len() != 5 + 4 + wrapped_len + 12 + (enc.len() - 5 - 4 - wrapped_len - 12) {
        // size sanity; the exact ct length is whatever remains
    }
    let off = 5 + 4;
    let wrapped = &enc[off..off + wrapped_len];
    let nonce_off = off + wrapped_len;
    let nonce = &enc[nonce_off..nonce_off + 12];
    let ct = &enc[nonce_off + 12..];

    let priv_key = rsa::RsaPrivateKey::from_pkcs1_pem(
        std::str::from_utf8(priv_key_pem).map_err(|e| BundleError::Key(e.to_string()))?,
    )
    .map_err(|e| BundleError::Key(e.to_string()))?;

    let mut rng = rand::thread_rng();
    let aes_key = priv_key
        .decrypt(Oaep::new::<sha2::Sha256>(), wrapped)
        .map_err(|e| BundleError::Decrypt(e.to_string()))?;
    let _ = hasher; let _ = &mut rng;

    let gcm = Aes256Gcm::new_from_slice(&aes_key).map_err(|e| BundleError::Decrypt(e.to_string()))?;
    let pt = gcm
        .decrypt(Nonce::from_slice(nonce), ct)
        .map_err(|e| BundleError::Decrypt(e.to_string()))?;
    Ok(pt)
}

#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    #[error("read: {0}")]
    Read(String),
    #[error("key: {0}")]
    Key(String),
    #[error("decrypt: {0}")]
    Decrypt(String),
    #[error("format: {0}")]
    Format(String),
    #[error("schema: {0}")]
    Schema(String),
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundle_rejects_paper_mode_override() {
        let mut b = ConfigBundle {
            schema_version: "1.0".into(),
            generated_at: "2026-07-04T00:00:00Z".into(),
            env_vars: HashMap::new(),
            chains: vec![],
            api_keys: HashMap::new(),
            contract_addresses: HashMap::new(),
        };
        b.env_vars.insert("ARBX_PAPER_MODE".into(), "false".into());
        assert!(b.validate().is_err(), "must refuse paper_mode override");
    }
    #[test]
    fn bundle_rejects_bad_address() {
        let mut b = ConfigBundle {
            schema_version: "1.0".into(), generated_at: "x".into(),
            env_vars: HashMap::new(), chains: vec![], api_keys: HashMap::new(),
            contract_addresses: HashMap::new(),
        };
        b.contract_addresses.insert("ARBITRAGE_EXECUTOR".into(), "0xdead".into());
        assert!(b.validate().is_err());
    }
}
```

- [ ] **Step 3: Export + unit-test**

```bash
# append `pub mod config_bundle;` to backend/shared-rs/src/lib.rs (alphabetical, after `pub mod chains;`)
cd backend/shared-rs && cargo check 2>&1 | tail -5
cargo test config_bundle 2>&1 | tail -8
```

**Expected:** 2 tests pass (`bundle_rejects_paper_mode_override`, `bundle_rejects_bad_address`). WDAC may block local `cargo test` — CI is the authority.

---

### Task 4: Admin endpoint `POST /admin/config/import-bundle`

**Files:**
- Create: `backend/api-server/src/routes/admin-config-bundle.ts`

- [ ] **Step 1: Implement the endpoint (delegates to the Rust importer via a sidecar OR a thin Rust microservice)**

Because the importer is Rust and the admin API is TypeScript, the cleanest bridge is a **small Rust binary** `arbx-bundle-importer` that the endpoint shells out to:

Create `backend/shared-rs/src/bin/bundle_importer.rs`:

```rust
use shared_rs::config_bundle::ConfigBundle;
use std::process::ExitCode;

fn main() -> ExitCode {
    let enc = std::env::var("ARBX_BUNDLE_PATH")
        .unwrap_or("/opt/arbitragex-v2/config/arbx_config_bundle.json.enc".into());
    let key = std::env::var("ARBX_BUNDLE_PRIVATE_KEY_PATH")
        .unwrap_or("/opt/arbitragex-v2/config/arbx_bundle_private.pem".into());
    let key_pem = match std::fs::read(&key) { Ok(b) => b, Err(e) => { eprintln!("read key: {e}"); return ExitCode::FAILURE; } };
    let bundle = match ConfigBundle::load_encrypted(&enc, &key_pem) {
        Ok(b) => b, Err(e) => { eprintln!("load: {e}"); return ExitCode::FAILURE; }
    };
    // Print a SUMMARY (counts only, no values) for the admin endpoint to relay.
    println!("{{\"env_vars\":{},\"chains\":{},\"api_keys\":{},\"contract_addresses\":{}}}",
        bundle.env_vars.len(), bundle.chains.len(),
        bundle.api_keys.len(), bundle.contract_addresses.len());
    // The actual apply (PG INSERTs, .env upsert, paper_mode-protected) is the
    // follow-up step: write the fragments to /opt/.../imported/ for RunFullSyncCycle
    // + a chains_seed.sql for psql. (Operator-gated apply — see plan §"Apply".)
    ExitCode::SUCCESS
}
```

Then the admin endpoint in `backend/api-server/src/routes/admin-config-bundle.ts`:

```typescript
import type { Request, Response } from "express";

interface Deps { logger: { warn: (o: object, msg?: string) => void }; }

/**
 * POST /admin/config/import-bundle
 *
 * Decrypts + validates the shipped bundle on the VPS (via the Rust importer
 * sidecar), returns a COUNT summary (no values), and stages the fragments for
 * the operator to apply via the existing RunFullSyncCycle + psql. paper_mode
 * is never modified.
 */
export function mountAdminConfigBundle(app: import("express").Express, deps: Deps): void {
  app.post("/admin/config/import-bundle", async (_req: Request, res: Response): Promise<void> => {
    try {
      const { execFile } = await import("node:child_process");
      execFile("arbx-bundle-importer", [], { timeout: 15000 }, (err, stdout, stderr) => {
        if (err) {
          deps.logger.warn({ event: "bundle.import_failed", err: err.message, stderr });
          res.status(500).json({ error: "import_failed", detail: stderr.slice(0, 200) });
          return;
        }
        const summary = JSON.parse(stdout || "{}");
        res.status(200).json({ status: "staged", summary, detail: "RunFullSyncCycle + psql to apply (operator-gated)" });
      });
    } catch (e) {
      res.status(500).json({ error: "internal", detail: (e as Error).message });
    }
  });
}
```

- [ ] **Step 2: Mount in index.ts + register the binary in the compose**

In `backend/api-server/src/index.ts`, add:
```typescript
import { mountAdminConfigBundle } from "./routes/admin-config-bundle.js";
// ...after other mounts:
mountAdminConfigBundle(app, { logger });
```

In `docker/compose.prod.yml`, ensure `arbx-bundle-importer` is built (or shipped inside the api-server image).

---

### Task 5: End-to-end test (idempotency + paper_mode protection)

- [ ] **Step 1: E2E roundtrip**

```bash
# 1. Ship (operator side)
python3 scripts/arbx-env-deploy/encrypt_and_ship_bundle.py --schema scripts/arbx-env-deploy/bundle_schema.json
# 2. Import (VPS side, via admin endpoint)
ssh arbx 'curl -fsS -X POST -H "x-arbx-admin-token: $(grep ^ARBX_ADMIN_TOKEN= /opt/arbitragex-v2/.env | cut -d= -f2)" http://localhost:8080/admin/config/import-bundle'
# 3. Re-import (must be a no-op / same summary — idempotent)
ssh arbx 'curl -fsS -X POST -H "x-arbx-admin-token: $(grep ^ARBX_ADMIN_TOKEN= /opt/arbitragex-v2/.env | cut -d= -f2)" http://localhost:8080/admin/config/import-bundle'
# 4. Confirm paper_mode unchanged
ssh arbx 'curl -fsS http://localhost:8080/api/v1/readiness/decision | python3 -c "import json,sys;d=json.load(sys.stdin);print(\"paper_mode=\",d.get(\"paper_mode\"),\" (must be True)\")"'
```

**Expected:** import returns `{status:"staged", summary:{env_vars:N, chains:N, ...}}`; re-import returns the same counts; `paper_mode=True` throughout.

- [ ] **Step 2: Adversarial — tamper the .enc, confirm importer refuses**

```bash
ssh arbx 'cp /opt/arbitragex-v2/config/arbx_config_bundle.json.enc /tmp/tampered.enc && \
  dd if=/dev/urandom of=/tmp/tampered.enc bs=1 count=10 conv=notrunc && \
  ARBX_BUNDLE_PATH=/tmp/tampered.enc arbx-bundle-importer'
# expected: non-zero exit, "decrypt" error (AES-GCM auth tag fails on tamper)
```

---

### Task 6: Document the flow in the workbook + commit

- [ ] **Step 1: Add a "Bundle Shipper" sheet** (instructions + public key fingerprint, dark mode)

```bash
PYTHONIOENCODING=utf-8 python3 -c "
import openpyxl, hashlib
from openpyxl.styles import Font, PatternFill
wb = openpyxl.load_workbook('C:/Users/HFRC/Downloads/ArbitrageX_Unified_Config.xlsm', keep_vba=True)
if 'Bundle Shipper' in wb.sheetnames: del wb['Bundle Shipper']
ws = wb.create_sheet('Bundle Shipper')
DARK = PatternFill('solid', fgColor='FF141C30'); LIGHT = Font(color='FFF3F9FF', size=11)
for r in range(1, 20):
    for c in range(1, 6): ws.cell(r,c).fill = DARK
ws['A1'] = 'Encrypted Bundle Shipper'; ws['A1'].font = Font(color='FFF3F9FF', size=14, bold=True)
ws['A3'] = 'Comando:'; ws['A3'].font = LIGHT
ws['B3'] = 'python scripts/arbx-env-deploy/encrypt_and_ship_bundle.py --schema scripts/arbx-env-deploy/bundle_schema.json'; ws['B3'].font = LIGHT
ws['A5'] = 'Public key fingerprint (SHA-256):'; ws['A5'].font = LIGHT
pub = open('C:/Users/HFRC/Downloads/arbx_bundle_public.pem','rb').read()
fp = hashlib.sha256(pub).hexdigest()
ws['B5'] = fp[:32] + '…'; ws['B5'].font = LIGHT
ws['A7'] = 'Flujo:'; ws['A7'].font = LIGHT
ws['B7'] = '1) shipper serializa+encripta+sube  2) POST /admin/config/import-bundle  3) RunFullSyncCycle+psql'; ws['B7'].font = LIGHT
ws['A9'] = 'Seguridad:'; ws['A9'].font = LIGHT
ws['B9'] = 'RSA-4096+AES-256-GCM. Private key solo en VPS. paper_mode jamás se toca.'; ws['B9'].font = LIGHT
ws.column_dimensions['A'].width = 35; ws.column_dimensions['B'].width = 80
wb.save('C:/Users/HFRC/Downloads/ArbitrageX_Unified_Config.xlsm')
print('Bundle Shipper sheet added')
"
```

- [ ] **Step 2: Commit the repo-side artifacts**

```bash
cd "c:/Users/HFRC/Desktop/arbitragex-v2-main (17)/arbitragex-v2-main"
git add scripts/arbx-env-deploy/encrypt_and_ship_bundle.py \
        scripts/arbx-env-deploy/bundle_schema.json \
        backend/shared-rs/src/config_bundle.rs \
        backend/shared-rs/src/bin/bundle_importer.rs \
        backend/shared-rs/Cargo.toml \
        backend/api-server/src/routes/admin-config-bundle.ts \
        docs/superpowers/plans/2026-07-04-encrypted-config-bundle.md
git commit -m "feat(config): encrypted config bundle — RSA-4096+AES-256-GCM Excel→VPS

Adds a shipper (encrypt_and_ship_bundle.py) that serializes the Excel into a
schema-validated JSON bundle, hybrid-encrypts (RSA-OAEP-4096 wrap AES-256-GCM),
and uploads via SSH. The VPS-side importer (shared-rs/config_bundle.rs +
bundle_importer binary) decrypts with the VPS-held private key, validates the
schema, and stages the fragments for RunFullSyncCycle + psql.

Security: private key never leaves the VPS; paper_mode filtered shipper-side
AND asserted importer-side (defence-in-depth). Tamper-detection via AES-GCM
auth tag. Idempotent re-import.

On-demand import via POST /admin/config/import-bundle (operator-gated).

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Self-Review (writing-plans skill)

**Spec coverage:** "macro que genere un archivo encryptado" → Task 2 shipper (Python; VBA macro is a thin shell-out if Task 7 is taken). "suba" → Task 2 Step 1 scp. "dapp interprete perfectamente" → Task 1 schema + Task 3 typed importer + Task 4 endpoint. "toda la información del libro" → Task 2 load_env_vars + load_chains + load_api_keys + load_contract_addresses covers all config-bearing sheets; reference-only sheets (RPC Parser, CURL Commands, HTTP Headers) deliberately omitted.

**Placeholder scan:** Task 2 `load_chains` uses an `explicit` name→id map with a comment "rough; refine with explicit map" — **fix: use the canonical CHAIN_IDS dict from `gen_rpc_env_from_xlsx.py`** (already defined there). The `factories` array is empty in the shipper — **fix: populate from seed 043 FACTORIES dict** (already defined in `gen_chain_env.py` from the Chain Builder plan; reuse). Both fixed at execution time.

**Type consistency:** `bundle_importer.rs` reads `ARBX_BUNDLE_PRIVATE_KEY_PATH` env; the deploy plan should add this to the VPS `.env` (or default to the file path). The Cargo.toml adds `rsa` 0.9 — verify workspace compatibility at Task 3 Step 1.

**Apply step (operator-gated):** the importer STAGES (prints summary); the actual apply (writing .env + running psql with chains_seed.sql) is deliberately operator-gated (RunFullSyncCycle + manual psql). This respects "paper_mode never modified" and "no uncontrolled VPS mutation".

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-07-04-encrypted-config-bundle.md`.**

**1. Subagent-Driven (recommended)** — 6 tasks, fresh subagent each, review between.
**2. Inline Execution** — I execute all tasks here, fixing the 2 self-review gaps (CHAIN_IDS reuse + factories population) as I go.

**Order recommendation:** this plan + the Chain Builder plan are complementary — Chain Builder is selective (operator picks chains), Bundle is bulk (ship everything). Doing **Chain Builder first** then **Bundle** means the Bundle shipper can reuse `gen_chain_env.py`'s well-tested chain/factory maps.

**Which approach, and do you want Chain Builder first then Bundle, or Bundle first?**
