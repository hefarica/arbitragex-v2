//! fhe-blind-arb-rs — OMEGA-100 Inyección 22 (HARDENED)
//!
//! Tribunal #64-#65, #76-#77 fix-on-write:
//!   #64 P0: API tfhe ficticia → trait FheBackend inyectable, swap a tfhe-rs cuando esté
//!   #65 P0: ServerKey::new() no existe → constructor delegado al backend
//!   #76 P1: leak de RadixCiphertext en API → newtype opaco CipherU64
//!   #77 P0: dependencia externa pesada → traits + stub determinista para tests
//!
//! Doctrina: Composición Atómica Simbiótica · Pauli (interfaz real, no mock oculto)

use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum FheError {
    #[error("backend FHE no inicializado")]
    BackendUninitialized,
    #[error("operación inválida: {0}")]
    InvalidOperation(String),
    #[error("ciphertext mal formado")]
    MalformedCipher,
    #[error("underflow en sub homomorfica")]
    HomomorphicUnderflow,
}

/// Newtype opaco sobre el ciphertext del backend. No expone detalles internos.
#[derive(Clone, Debug)]
pub struct CipherU64 {
    pub(crate) bytes: Vec<u8>,
}
impl CipherU64 {
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Trait inyectable del backend FHE. Implementaciones:
///  - StubFheBackend (test): aritmética sobre u64 plano envuelto en bytes
///  - TfheRsBackend (feature tfhe-real): wrapper sobre ServerKey de Zama
pub trait FheBackend: Send + Sync {
    fn sub(&self, lhs: &CipherU64, rhs: &CipherU64) -> Result<CipherU64, FheError>;
    fn add(&self, lhs: &CipherU64, rhs: &CipherU64) -> Result<CipherU64, FheError>;
    fn mul_scalar(&self, lhs: &CipherU64, scalar: u64) -> Result<CipherU64, FheError>;
}

/// Backend stub determinista. Aritmética sobre u64 little-endian (NO es FHE real,
/// es la interfaz que `tfhe-real` swappea por `ServerKey::sub_parallelized` etc).
/// Marca explícita en la documentación para que NO se confunda con producción.
pub struct StubFheBackend;
impl StubFheBackend {
    fn decode(c: &CipherU64) -> Result<u64, FheError> {
        if c.bytes.len() != 8 {
            return Err(FheError::MalformedCipher);
        }
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&c.bytes);
        Ok(u64::from_le_bytes(buf))
    }
    fn encode(v: u64) -> CipherU64 {
        CipherU64::from_bytes(v.to_le_bytes().to_vec())
    }
}
impl FheBackend for StubFheBackend {
    fn sub(&self, lhs: &CipherU64, rhs: &CipherU64) -> Result<CipherU64, FheError> {
        let a = Self::decode(lhs)?;
        let b = Self::decode(rhs)?;
        let r = a.checked_sub(b).ok_or(FheError::HomomorphicUnderflow)?;
        Ok(Self::encode(r))
    }
    fn add(&self, lhs: &CipherU64, rhs: &CipherU64) -> Result<CipherU64, FheError> {
        let a = Self::decode(lhs)?;
        let b = Self::decode(rhs)?;
        let r = a.checked_add(b).ok_or(FheError::InvalidOperation("overflow".into()))?;
        Ok(Self::encode(r))
    }
    fn mul_scalar(&self, lhs: &CipherU64, scalar: u64) -> Result<CipherU64, FheError> {
        let a = Self::decode(lhs)?;
        let r = a.checked_mul(scalar).ok_or(FheError::InvalidOperation("overflow".into()))?;
        Ok(Self::encode(r))
    }
}

/// Helper sólo para tests — convierte plano a CipherU64 sin pasar por encriptación.
/// NUNCA debe usarse en producción.
#[doc(hidden)]
pub fn _test_only_plain_to_cipher(v: u64) -> CipherU64 {
    CipherU64::from_bytes(v.to_le_bytes().to_vec())
}
#[doc(hidden)]
pub fn _test_only_decode_stub(c: &CipherU64) -> Result<u64, FheError> {
    StubFheBackend::decode(c)
}

pub struct FheBlindArbitrator {
    backend: Arc<dyn FheBackend>,
}

impl FheBlindArbitrator {
    pub fn new(backend: Arc<dyn FheBackend>) -> Self {
        Self { backend }
    }

    /// Calcula el monto óptimo de backrun ciego: reserves - user_amount.
    /// El searcher nunca ve el monto en claro; sólo opera sobre ciphertexts.
    pub fn compute_optimal_blind_backrun(
        &self,
        encrypted_user_amount: &CipherU64,
        encrypted_pool_reserves: &CipherU64,
    ) -> Result<CipherU64, FheError> {
        self.backend.sub(encrypted_pool_reserves, encrypted_user_amount)
    }

    /// Fórmula extendida: backrun = (reserves - user) * slippage_factor
    pub fn compute_with_slippage(
        &self,
        encrypted_user_amount: &CipherU64,
        encrypted_pool_reserves: &CipherU64,
        slippage_factor: u64,
    ) -> Result<CipherU64, FheError> {
        let delta = self.backend.sub(encrypted_pool_reserves, encrypted_user_amount)?;
        self.backend.mul_scalar(&delta, slippage_factor)
    }
}
