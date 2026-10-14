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
use tracing::warn;

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
    // ── SIM-FUND-02 (2026-10-08) ────────────────────────────────────────────
    // The three below split `slot_unresolved`, which is ONE string produced by
    // TWO different physical causes. Measured on the live fork: the counter had
    // ONLY `slot_unresolved` (1633, later 1642) — no `rpc_err`, no
    // `seeded_fresh`, no `cache_hit` — so the aggregate could not say whether
    // the write failed or the slot was wrong. That is how the same reason
    // string survived its own fix while describing ANOTHER cause.
    //
    /// `write_balance` returned `Ok(false)`: the node refused the write, so it
    /// never landed. This is cause (a).
    pub const WRITE_REJECTED: &str = "write_rejected";
    /// The write was accepted but `balanceOf` did NOT return the sentinel: the
    /// slot is not the one `balanceOf` reads. This is cause (b).
    pub const VERIFY_MISMATCH: &str = "verify_mismatch";
    /// `balanceOf` could not be decoded (revert / non-contract / RPC error).
    /// Previously swallowed as `Ok(None)` with NO counter, which made a failed
    /// READ look exactly like a wrong SLOT.
    pub const BALANCE_UNREADABLE: &str = "balance_unreadable";
}

// ── SIM-FUND-04: INSTRUMENTO NEUTRO ────────────────────────────────────────
//
// POR QUE EXISTE. t111 concluyo que `anvil_setStorageAt` no llegaba a la
// ejecucion. t112 lo retracto: el error fue computar los slots con una funcion
// **keyeada por la direccion del TOKEN** en vez de por el SIGNER, y escribir el
// centinela en un slot AJENO. El sintoma de eso —`eth_getStorageAt` muestra la
// escritura, `balanceOf` no cambia— es INDISTINGUIBLE del de una escritura que
// no llega a la ejecucion, y por eso produjo un diagnostico equivocado.
//
// QUE HACE ESTE INSTRUMENTO, Y QUE NO HACE. REGISTRA hechos crudos: el token,
// el signer, los slots probados, y para CADA slot la clave usada, el valor
// leido de vuelta del storage y el `balanceOf` de ese instante. **NO clasifica
// la causa.** No emite etiquetas interpretativas (`write_invisible`,
// `write_not_persisted` ni equivalentes): si las emitiera, volveria a dictaminar
// antes de tener la medicion, que es exactamente el error que se esta pagando.
//
// Registra AMBAS derivaciones (keyeada por signer y keyeada por token) porque el
// error de t111 fue justamente elegir una sin que nada lo mostrara: con las dos
// en el registro, una clave equivocada deja de ser invisible.
#[derive(Debug, Clone, Default)]
pub struct SlotProbe {
    pub slot: u64,
    /// Clave EFECTIVAMENTE usada — `balance_slot(slot, signer)`, keyeada por el
    /// SIGNER, que es lo que `balanceOf(signer)` lee.
    pub signer_keyed_key: String,
    /// La misma derivacion pero keyeada por el TOKEN. Es un DATO de contraste:
    /// si alguien re-keyeara por token, las dos columnas lo delatarian.
    pub token_keyed_key: String,
    /// Resultado literal de `anvil_setStorageAt`: "true" | "false" | "err: ...".
    pub set_storage_at: String,
    /// `eth_getStorageAt(signer_keyed_key)` DESPUES de escribir.
    pub storage_readback: String,
    /// `balanceOf(signer)` por `eth_call` en ese instante.
    pub balance_of: String,
    /// Igualdad cruda entre lo escrito (el centinela) y lo leido de vuelta.
    pub readback_matches_sentinel: bool,
}

/// SIM-FUND-04: el registro completo de UN intento de fondeo fallido.
#[derive(Debug, Clone, Default)]
pub struct FundingProbe {
    pub token: String,
    pub signer: String,
    pub amount_in: String,
    pub candidate_slots: Vec<u64>,
    pub probes: Vec<SlotProbe>,
    /// X2: el slot CORRECTO POR CONSTRUCCION es el que deriva `balance_slot`
    /// keyeando por el SIGNER. Este campo registra que TODAS las sondas usaron
    /// esa clave — el hecho que t111 no tenia forma de exhibir.
    pub all_probes_used_signer_key: bool,
    pub outcome: String,
}

impl FundingProbe {
    fn to_json(&self) -> String {
        let probes: Vec<serde_json::Value> = self
            .probes
            .iter()
            .map(|p| {
                serde_json::json!({
                    "slot": p.slot,
                    "signer_keyed_key": p.signer_keyed_key,
                    "token_keyed_key": p.token_keyed_key,
                    "set_storage_at": p.set_storage_at,
                    "storage_readback": p.storage_readback,
                    "balance_of": p.balance_of,
                    "readback_matches_sentinel": p.readback_matches_sentinel,
                })
            })
            .collect();
        serde_json::json!({
            "token": self.token,
            "signer": self.signer,
            "amount_in": self.amount_in,
            "candidate_slots": self.candidate_slots,
            "all_probes_used_signer_key": self.all_probes_used_signer_key,
            "outcome": self.outcome,
            "probes": probes,
        })
        .to_string()
    }
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
        // SIM-FUND-04: acumulador de HECHOS CRUDOS. No altera ninguna decision:
        // solo recoge lo que el loop ya escribe y ya lee.
        let mut probe = FundingProbe {
            token: format!("{token:?}"),
            signer: format!("{signer:?}"),
            amount_in: amount_in.to_string(),
            candidate_slots: CANDIDATE_SLOTS.to_vec(),
            probes: Vec::new(),
            all_probes_used_signer_key: true,
            outcome: String::new(),
        };
        for &slot in CANDIDATE_SLOTS.iter() {
            // Las DOS derivaciones, para que la eleccion de clave sea un DATO
            // visible y no una suposicion.
            let signer_keyed_key = format!("{:?}", balance_slot(slot, signer));
            let token_keyed_key = format!("{:?}", balance_slot(slot, token));
            if !self
                .write_balance(token, slot, signer, SENTINEL_BALANCE)
                .await?
            {
                // SIM-FUND-02: cause (a) — the node refused the write, so it
                // never landed. Counted PER SLOT so the two physical causes stop
                // sharing the single `slot_unresolved` string.
                count(outcome::WRITE_REJECTED);
                probe.probes.push(SlotProbe {
                    slot,
                    signer_keyed_key,
                    token_keyed_key,
                    set_storage_at: "false".to_string(),
                    storage_readback: "not_read".to_string(),
                    balance_of: "not_read".to_string(),
                    readback_matches_sentinel: false,
                });
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
            // SIM-FUND-04: los dos hechos crudos de ESTE slot, registrados tal
            // como salieron. Sin compararlos entre si y sin nombrar una causa.
            let readback =
                read_storage_slot(&self.provider, token, slot, signer, self.timeout).await;
            probe.probes.push(SlotProbe {
                slot,
                signer_keyed_key,
                token_keyed_key,
                set_storage_at: "true".to_string(),
                storage_readback: readback
                    .map(|h| format!("{h:?}"))
                    .unwrap_or_else(|| "read_failed".to_string()),
                balance_of: verified
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "unreadable".to_string()),
                readback_matches_sentinel: readback == Some(u256_to_h256(SENTINEL_BALANCE)),
            });
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
            } else {
                // SIM-FUND-02: cause (b) — the write was accepted (`Ok(true)`,
                // no `Err`, since an `Err` would have propagated through the `?`
                // above with its OWN message) but the sentinel did NOT reproduce
                // through `balanceOf`. The slot is not the one `balanceOf` reads.
                count(outcome::VERIFY_MISMATCH);
            }
            // Sentinel not observed — restore the ORIGINAL balance (captured
            // before any probe) so we never leak a corrupted slot onward.
            let _ = self
                .write_balance(token, slot, signer, original_balance)
                .await;
        }
        count(outcome::SLOT_UNRESOLVED);
        // SIM-FUND-04: UN registro por intento fallido con TODOS los hechos.
        // Volumen: el consumidor admite ~11 simulaciones/min, asi que esto es del
        // orden de ~10 lineas/min — muy por debajo del flooding que motivo R9
        // (183 lineas/s). No se muestrea: muestrear perderia justo la combinacion
        // token/slot que se busca.
        probe.outcome = "sim_signer_funding_slot_unresolved".to_string();
        probe.all_probes_used_signer_key = probe
            .probes
            .iter()
            .all(|p| p.signer_keyed_key == format!("{:?}", balance_slot(p.slot, signer)));
        warn!(
            event = "sim.funding_probe",
            token = %probe.token,
            signer = %probe.signer,
            amount_in = %probe.amount_in,
            candidate_slots = ?probe.candidate_slots,
            all_probes_used_signer_key = probe.all_probes_used_signer_key,
            outcome = %probe.outcome,
            probes = %probe.to_json(),
            "RAW FACTS of a failed funding attempt — the instrument records, it does NOT diagnose"
        );
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

/// Canonical ERC-20 `balanceOf(address)` selector. `cast sig
/// "balanceOf(address)"` == `0x70a08231`.
const BALANCE_OF_SELECTOR: [u8; 4] = [0x70, 0xa0, 0x82, 0x31];

/// ABI-required calldata length: 4-byte selector + ONE 32-byte argument word.
const BALANCE_OF_CALLDATA_LEN: usize = 36;

/// Build `balanceOf(signer)` calldata: the selector, then the signer
/// right-aligned inside a full 32-byte word (12 zero bytes of padding followed
/// by the 20 address bytes).
///
/// FUND-READ-CALLDATA-01 (2026-10-09): this used to append `signer.as_bytes()`
/// directly, emitting **24** bytes. The EVM zero-fills past `calldatasize`, so
/// `CALLDATALOAD(4)` returned `signer[0..20] || 0^12` and `uint160` of that
/// word kept its LOW 20 bytes — `signer[12..20] || 0^12`, a DIFFERENT
/// address. The sentinel write was always correct (32-byte `abi::encode`), so
/// the write landed in the right slot and was then read back from the wrong
/// account: a correct write could never reproduce its own sentinel.
///
/// Canonical shape mirrored from `simulator-v2/src/sequence_runner.rs:717`.
fn build_balance_of_calldata(signer: Address) -> Vec<u8> {
    // Idiomatic: let ethers' ABI encoder do the padding — the same mechanism
    // `balance_slot` below relies on. `abi::encode` right-aligns an address
    // inside its word, which is exactly what the EVM reads back.
    let mut data: Vec<u8> = Vec::with_capacity(BALANCE_OF_CALLDATA_LEN);
    data.extend_from_slice(&BALANCE_OF_SELECTOR);
    data.extend_from_slice(&ethers::abi::encode(&[ethers::abi::Token::Address(signer)]));
    debug_assert_eq!(data.len(), BALANCE_OF_CALLDATA_LEN);
    data
}

/// erc20 balanceOf(signer) → U256, typed through ethers' call decoding.
async fn balance_of(
    provider: &Provider<Http>,
    token: Address,
    signer: Address,
) -> Result<Option<U256>, ProviderError> {
    let data = build_balance_of_calldata(signer);
    let tx = TransactionRequest::new()
        .to(token)
        .data(ethers::types::Bytes::from(data));
    let typed: TypedTransaction = tx.into();
    match provider.call(&typed, None).await {
        Ok(out) => Ok(decode_u256(&out)),
        // Non-contract / revert: no decodable balance — treat as zero
        // rather than aborting the whole funding path.
        //
        // SIM-FUND-02: NO LONGER SILENT. Swallowing this as `Ok(None)` with no
        // counter made a FAILED READ indistinguishable from a WRONG SLOT — both
        // ended in `slot_unresolved`. It now has its own outcome so the next
        // measurement can tell them apart. Behaviour is unchanged (still
        // `Ok(None)`): this adds visibility, not a verdict.
        Err(_) => {
            count(outcome::BALANCE_UNREADABLE);
            Ok(None)
        }
    }
}

/// SIM-FUND-04: lee de vuelta el slot con `eth_getStorageAt`. Es un DATO mas —
/// no se compara con nada ni se etiqueta. `None` = la lectura misma fallo, y se
/// registra como `read_failed`, nunca como cero.
async fn read_storage_slot(
    provider: &Provider<Http>,
    token: Address,
    slot: u64,
    signer: Address,
    timeout: Duration,
) -> Option<H256> {
    let slot32 = balance_slot(slot, signer);
    let res: Result<String, _> = tokio::time::timeout(
        timeout,
        provider.request("eth_getStorageAt", (token, slot32, "latest")),
    )
    .await
    .ok()?;
    let hex = res.ok()?;
    let bytes = ethers::utils::hex::decode(hex.trim_start_matches("0x")).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    Some(H256::from_slice(&bytes))
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

    /// SIM-FUND-04 / X2: EL CONTROL NEGATIVO QUE FALTABA.
    ///
    /// t111 computo los slots keyeando por la direccion del TOKEN y escribio el
    /// centinela en un slot AJENO. El sintoma —`eth_getStorageAt` muestra la
    /// escritura, `balanceOf` no cambia— es INDISTINGUIBLE del de una escritura
    /// que no llega a la ejecucion, y por eso produjo un diagnostico equivocado.
    /// Lo que faltaba NO era otro valor de hash: era un control sobre QUIEN es la
    /// clave.
    ///
    /// Conserva SOLO el control de X2. El resto del cambio de #859 se DESCARTA:
    /// se construyo sobre la premisa retirada.
    #[test]
    fn x2_balance_slot_is_keyed_by_the_signer_and_not_by_the_token() {
        let signer: Address = "0x1234567890123456789012345678901234567890"
            .parse()
            .unwrap();
        let token: Address = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
            .parse()
            .unwrap();

        // Valor MEDIDO en el fork vivo: escribir el centinela aqui hace que
        // `balanceOf(signer)` lo devuelva (verificado por eth_call en t112).
        let signer_keyed_slot3 = H256::from_slice(
            &ethers::utils::hex::decode(
                "961558ef95740fe5d8173078fa8d9fd6150201cd29befffef12f314fd45a2bfc",
            )
            .unwrap(),
        );
        assert_eq!(balance_slot(3, signer), signer_keyed_slot3);

        // CONTROL NEGATIVO: keyear por el token da un hash de 32 bytes
        // PERFECTAMENTE PLAUSIBLE — y completamente ajeno a `balanceOf`.
        let token_keyed = balance_slot(3, token);
        assert_ne!(balance_slot(3, signer), token_keyed);
        assert_eq!(
            token_keyed.as_bytes().len(),
            32,
            "un slot equivocado sigue pareciendo valido: por eso hace falta el control de clave"
        );
    }

    /// SIM-FUND-04: el registro lleva TODOS los hechos crudos por intento,
    /// incluidas las DOS derivaciones. Sin este test, el instrumento podria
    /// emitir un JSON vacio sin fallar.
    #[test]
    fn simfund04_probe_records_raw_facts_and_both_key_derivations() {
        let signer: Address = "0x1234567890123456789012345678901234567890"
            .parse()
            .unwrap();
        let token: Address = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
            .parse()
            .unwrap();
        let mut p = FundingProbe {
            token: format!("{token:?}"),
            signer: format!("{signer:?}"),
            amount_in: "1000".to_string(),
            candidate_slots: CANDIDATE_SLOTS.to_vec(),
            probes: Vec::new(),
            all_probes_used_signer_key: true,
            outcome: "sim_signer_funding_slot_unresolved".to_string(),
        };
        for &slot in CANDIDATE_SLOTS.iter() {
            p.probes.push(SlotProbe {
                slot,
                signer_keyed_key: format!("{:?}", balance_slot(slot, signer)),
                token_keyed_key: format!("{:?}", balance_slot(slot, token)),
                set_storage_at: "true".to_string(),
                storage_readback: "0x00".to_string(),
                balance_of: "100".to_string(),
                readback_matches_sentinel: false,
            });
        }
        let s = p.to_json();
        let j: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(j["token"], format!("{token:?}"));
        assert_eq!(j["signer"], format!("{signer:?}"));
        assert_eq!(j["amount_in"], "1000");
        assert_eq!(j["outcome"], "sim_signer_funding_slot_unresolved");
        assert_eq!(j["all_probes_used_signer_key"], true);
        assert_eq!(
            j["candidate_slots"].as_array().unwrap().len(),
            CANDIDATE_SLOTS.len()
        );
        let probes = j["probes"].as_array().unwrap();
        // CONTROL de inventario: una entrada por slot probado, ninguna vacia.
        assert_eq!(probes.len(), CANDIDATE_SLOTS.len());
        for pr in probes {
            assert!(pr["signer_keyed_key"].as_str().unwrap().starts_with("0x"));
            assert!(pr["token_keyed_key"].as_str().unwrap().starts_with("0x"));
            // Las dos derivaciones se registran y DIFIEREN: si alguien
            // re-keyeara por token, las dos columnas lo delatarian.
            assert_ne!(pr["signer_keyed_key"], pr["token_keyed_key"]);
            assert!(pr.get("storage_readback").is_some());
            assert!(pr.get("balance_of").is_some());
        }
        // X1: el instrumento NO emite etiquetas interpretativas. Se asserta
        // sobre la salida real, no sobre la intencion.
        for forbidden in ["write_invisible", "write_not_persisted"] {
            assert!(
                !s.contains(forbidden),
                "el instrumento registra, no dictamina: {forbidden} no debe aparecer"
            );
        }
    }

    // ── FUND-READ-CALLDATA-01: la FORMA del calldata de `balanceOf` ─────────
    //
    // Una lectura MAL FORMADA no es una lectura fallida: la EVM rellena con
    // ceros mas alla de `calldatasize`, asi que un calldata de 24 bytes se
    // ejecuta igual y devuelve 32 bytes — de OTRA cuenta. Estos helpers
    // modelan los dos lados para que un solo test pueda sostener ambos.

    /// La direccion que la EVM realmente lee. Modela `CALLDATALOAD(4)` seguido
    /// de `uint160`: la palabra es calldata[4..] rellenada con ceros a la
    /// DERECHA, y `uint160` conserva sus 20 bytes BAJOS.
    fn effective_holder(calldata: &[u8]) -> Address {
        let mut word = [0u8; 32];
        let tail = &calldata[4.min(calldata.len())..];
        let n = tail.len().min(32);
        word[..n].copy_from_slice(&tail[..n]);
        Address::from_slice(&word[12..32])
    }

    /// La construccion RETIRADA, byte por byte (lo que `balance_of` emitia
    /// antes de FUND-READ-CALLDATA-01): selector + 20 bytes crudos = 24.
    fn legacy_24_byte_balance_of_calldata(signer: Address) -> Vec<u8> {
        let sel: [u8; 4] = [0x70, 0xa0, 0x82, 0x31];
        let mut data = sel.to_vec();
        data.extend_from_slice(signer.as_bytes());
        data
    }

    /// Validez ABI tal como la ve la EVM: exactamente 36 bytes, selector
    /// canonico, y los 12 bytes de padding en cero.
    fn is_canonical_balance_of(calldata: &[u8]) -> bool {
        calldata.len() == BALANCE_OF_CALLDATA_LEN
            && calldata[0..4] == BALANCE_OF_SELECTOR
            && calldata[4..16].iter().all(|b| *b == 0)
    }

    #[test]
    fn build_balance_of_calldata_is_36_bytes_with_a_right_aligned_signer() {
        let signer: Address = "0x1234567890123456789012345678901234567890"
            .parse()
            .unwrap();
        let calldata = build_balance_of_calldata(signer);

        // LONGITUD: el ABI exige 4 + 32, no 4 + 20.
        assert_eq!(calldata.len(), 36);
        assert_eq!(calldata.len(), BALANCE_OF_CALLDATA_LEN);
        // BYTES: selector, 12 ceros de padding, despues la direccion.
        assert_eq!(&calldata[0..4], &[0x70, 0xa0, 0x82, 0x31]);
        assert_eq!(&calldata[4..16], &[0u8; 12]);
        assert_eq!(&calldata[16..36], signer.as_bytes());
        assert!(is_canonical_balance_of(&calldata));

        // Bytes EXACTOS, pineados en hex (36 bytes == 72 caracteres hex).
        assert_eq!(
            ethers::utils::hex::encode(&calldata),
            "70a082310000000000000000000000001234567890123456789012345678901234567890"
        );
    }

    #[test]
    fn build_balance_of_calldata_differs_per_signer() {
        let a: Address = "0x1111111111111111111111111111111111111111"
            .parse()
            .unwrap();
        let b: Address = "0x2222222222222222222222222222222222222222"
            .parse()
            .unwrap();
        assert_ne!(build_balance_of_calldata(a), build_balance_of_calldata(b));
    }

    /// EL FALSIFICADOR, anclado en los vectores de mainnet reproducidos con
    /// `cast call` (WETH9 `0xC02aaA…56Cc2`, par UniswapV2 USDC/WETH
    /// `0xB4e16d…C9Dc`) y documentados en
    /// `docs/backend/FUND-READ-CALLDATA-01.md`.
    ///
    /// Bidireccional: el MISMO verificador acepta el calldata arreglado y
    /// rechaza el retirado, y ademas muestra que el retirado lee OTRA
    /// direccion. Revertir `build_balance_of_calldata` al append de 24 bytes
    /// pone en rojo el bloque (1).
    #[test]
    fn balance_of_calldata_bidirectional_control_on_mainnet_vectors() {
        // El par cuyo call de 24 bytes devolvio 0x0 en mainnet.
        let pair: Address = "0xb4e16d0168e52d35cacd2c6185b44281ec28c9dc"
            .parse()
            .unwrap();
        // La direccion que la EVM leyo de verdad con ese calldata de 24 bytes:
        // uint160(word) == pair[12..20] || 0^12.
        let misread: Address = "0x85b44281ec28c9dc000000000000000000000000"
            .parse()
            .unwrap();

        let fixed = build_balance_of_calldata(pair);
        let legacy = legacy_24_byte_balance_of_calldata(pair);

        // (1) ARREGLADO: 36 bytes, canonico, y la EVM lee al propio signer.
        assert_eq!(fixed.len(), 36);
        assert!(is_canonical_balance_of(&fixed));
        assert_eq!(effective_holder(&fixed), pair);
        assert_eq!(
            ethers::utils::hex::encode(&fixed),
            "70a08231000000000000000000000000b4e16d0168e52d35cacd2c6185b44281ec28c9dc"
        );

        // (2) RETIRADO: 24 bytes, NO canonico, y la EVM lee `misread` — otra
        //     cuenta, contra la cual se comparaba el centinela escrito en el
        //     slot correcto.
        assert_eq!(legacy.len(), 24);
        assert!(!is_canonical_balance_of(&legacy));
        assert_ne!(effective_holder(&legacy), pair);
        assert_eq!(effective_holder(&legacy), misread);
        assert_eq!(
            ethers::utils::hex::encode(&legacy),
            "70a08231b4e16d0168e52d35cacd2c6185b44281ec28c9dc"
        );

        // (3) El control debe VOLTEAR con la implementacion bajo test, no solo
        //     con la copia local: esta es la linea que regresiona.
        assert!(is_canonical_balance_of(&build_balance_of_calldata(pair)));
        assert_eq!(effective_holder(&build_balance_of_calldata(pair)), pair);
    }
}
