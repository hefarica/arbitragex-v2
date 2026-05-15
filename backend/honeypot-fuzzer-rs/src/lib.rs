//! OMEGA-8 Inyección 26 — Dynamic Honeypot Fuzzer (Zero-Loss Gate)
//!
//! Doctrina:
//! - Trait `EvmSandbox` = interfaz de producción (revm fork, anvil, hardhat-mainnet-fork).
//! - `StubEvmSandbox` simula buy/sell/balance/tax determinísticamente para tests.
//! - Tribunal hallazgos:
//!   #81: NO retornar `true`/`U256::ZERO` literales — todo deriva del estado del sandbox.
//!   #82: tax_loss calculado de balances WETH antes/después, no hardcoded.
//!   #83: aritmética entera u128, ratios en bps (basis points, /10000).
//!   #89: error tipado FuzzerError, sin booleans ciegos.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::Mutex;
use tracing::{debug, warn};

pub const ADDRESS_BYTES: usize = 20;
pub const BPS_DENOM: u128 = 10_000;
pub const DEFAULT_MAX_TAX_BPS: u128 = 1_000; // 10% max permitido

#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum FuzzerError {
    #[error("buy bloqueado (blacklist o revert)")]
    BuyBlocked,
    #[error("sell bloqueado (honeypot clásico)")]
    SellBlocked,
    #[error("impuesto destructivo: {tax_bps} bps (max {max_bps})")]
    DestructiveTax { tax_bps: u128, max_bps: u128 },
    #[error("sandbox error: {0}")]
    Sandbox(String),
    #[error("timeout {0:?}")]
    Timeout(Duration),
    #[error("config inválida: {0}")]
    InvalidConfig(String),
    #[error("división por cero — balance inicial 0")]
    ZeroInitialBalance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TokenAddress(pub [u8; ADDRESS_BYTES]);

impl TokenAddress {
    pub fn new(b: [u8; ADDRESS_BYTES]) -> Self {
        Self(b)
    }
}

/// Resultado del análisis — clean o catalogado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FuzzVerdict {
    Clean { tax_bps: u128 },
    Honeypot { reason: String },
}

#[async_trait]
pub trait EvmSandbox: Send + Sync {
    async fn simulate_buy(&self, token: TokenAddress, weth_in: u128) -> Result<u128, FuzzerError>;
    async fn simulate_sell(
        &self,
        token: TokenAddress,
        amount: u128,
    ) -> Result<u128, FuzzerError>;
}

/// Stub determinista — simula tres clases de token:
/// - blacklisted (TokenAddress[0] == 0xBB)
/// - sell-blocked honeypot (TokenAddress[0] == 0xDE)
/// - taxed (TokenAddress[0] == 0xAA, tax controlado por TokenAddress[1])
/// - clean (cualquier otro): 1:1 round-trip.
pub struct StubEvmSandbox {
    delay: Arc<Mutex<Duration>>,
}

impl Default for StubEvmSandbox {
    fn default() -> Self {
        Self {
            delay: Arc::new(Mutex::new(Duration::from_millis(0))),
        }
    }
}

impl StubEvmSandbox {
    pub async fn set_delay(&self, d: Duration) {
        *self.delay.lock().await = d;
    }
}

#[async_trait]
impl EvmSandbox for StubEvmSandbox {
    async fn simulate_buy(&self, token: TokenAddress, weth_in: u128) -> Result<u128, FuzzerError> {
        let d = *self.delay.lock().await;
        if !d.is_zero() {
            tokio::time::sleep(d).await;
        }
        if weth_in == 0 {
            return Err(FuzzerError::InvalidConfig("weth_in = 0".into()));
        }
        match token.0[0] {
            0xBB => Err(FuzzerError::BuyBlocked),
            0xAA => {
                // tax in: porcentaje en bps codificado en byte [1]
                let tax_bps = token.0[1] as u128 * 10; // byte 50 → 500 bps = 5%
                let kept_bps = BPS_DENOM.saturating_sub(tax_bps);
                Ok(weth_in.saturating_mul(kept_bps) / BPS_DENOM)
            }
            _ => Ok(weth_in), // clean 1:1
        }
    }

    async fn simulate_sell(&self, token: TokenAddress, amount: u128) -> Result<u128, FuzzerError> {
        let d = *self.delay.lock().await;
        if !d.is_zero() {
            tokio::time::sleep(d).await;
        }
        if amount == 0 {
            return Err(FuzzerError::InvalidConfig("amount = 0".into()));
        }
        match token.0[0] {
            0xDE => Err(FuzzerError::SellBlocked),
            0xAA => {
                let tax_bps = token.0[1] as u128 * 10;
                let kept_bps = BPS_DENOM.saturating_sub(tax_bps);
                Ok(amount.saturating_mul(kept_bps) / BPS_DENOM)
            }
            _ => Ok(amount),
        }
    }
}

pub struct DynamicHoneypotFuzzer<S: EvmSandbox> {
    sandbox: S,
    weth_probe: u128,
    max_tax_bps: u128,
    timeout: Duration,
}

impl<S: EvmSandbox> DynamicHoneypotFuzzer<S> {
    pub fn new(
        sandbox: S,
        weth_probe: u128,
        max_tax_bps: u128,
        timeout: Duration,
    ) -> Result<Self, FuzzerError> {
        if weth_probe == 0 {
            return Err(FuzzerError::InvalidConfig("weth_probe = 0".into()));
        }
        if max_tax_bps >= BPS_DENOM {
            return Err(FuzzerError::InvalidConfig(
                "max_tax_bps >= 10000".into(),
            ));
        }
        if timeout.is_zero() {
            return Err(FuzzerError::InvalidConfig("timeout = 0".into()));
        }
        Ok(Self {
            sandbox,
            weth_probe,
            max_tax_bps,
            timeout,
        })
    }

    pub async fn vivisect(&self, token: TokenAddress) -> Result<FuzzVerdict, FuzzerError> {
        debug!(?token, "vivisección iniciada");
        // Fase 1: buy con timeout
        let received = match tokio::time::timeout(
            self.timeout,
            self.sandbox.simulate_buy(token, self.weth_probe),
        )
        .await
        {
            Ok(Ok(v)) => v,
            Ok(Err(FuzzerError::BuyBlocked)) => {
                return Ok(FuzzVerdict::Honeypot {
                    reason: "buy bloqueado (blacklist)".into(),
                });
            }
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err(FuzzerError::Timeout(self.timeout)),
        };
        if received == 0 {
            return Ok(FuzzVerdict::Honeypot {
                reason: "buy retornó 0".into(),
            });
        }
        // Fase 2: sell con timeout
        let weth_back = match tokio::time::timeout(
            self.timeout,
            self.sandbox.simulate_sell(token, received),
        )
        .await
        {
            Ok(Ok(v)) => v,
            Ok(Err(FuzzerError::SellBlocked)) => {
                return Ok(FuzzVerdict::Honeypot {
                    reason: "sell prohibido".into(),
                });
            }
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err(FuzzerError::Timeout(self.timeout)),
        };
        // Fase 3: cálculo real de tax_bps
        if self.weth_probe == 0 {
            return Err(FuzzerError::ZeroInitialBalance);
        }
        let tax_bps = if weth_back >= self.weth_probe {
            0
        } else {
            let lost = self.weth_probe - weth_back;
            // bps = lost / weth_probe * 10000
            lost.saturating_mul(BPS_DENOM) / self.weth_probe
        };
        if tax_bps > self.max_tax_bps {
            warn!(tax_bps, max = self.max_tax_bps, "tax destructivo");
            return Err(FuzzerError::DestructiveTax {
                tax_bps,
                max_bps: self.max_tax_bps,
            });
        }
        Ok(FuzzVerdict::Clean { tax_bps })
    }
}
