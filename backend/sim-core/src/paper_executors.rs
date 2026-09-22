//! Paper-only fork-deploy of the executor stack (G-SIM-1 WO-LR22.13 PR-B).
//!
//! The production `ARBITRAGE_EXECUTOR` / `FLASHLOAN_EXECUTOR_1` env addresses
//! have NO code on mainnet (verified 2026-09-22, eth_getCode = "0x"), so every
//! `execute_multistep_revm` dispatch reverts at the `ExecuteCall caller → FLE`
//! step and labeled outcomes are structurally zero. This module deploys the
//! REAL compiled `ArbitrageExecutor` + `FlashLoanExecutor` bytecode INSIDE the
//! in-process fork (CacheDB) — the same real forked chain state, real Aave V3
//! pool, real routers — mirroring the `contracts/script/DeployMainnet.s.sol`
//! setup sequence:
//!
//!   1. `seed_balance(admin)` — the paper caller holds no forked mainnet ETH
//!      (CALLER-GAS gap flagged in `sim_multistep`); without the seed NO
//!      paper dispatch can ever pay gas.
//!   2. CREATE `ArbitrageExecutor` (impl directly — the ERC1967Proxy layer is
//!      production custody, not variance-relevant) + `initialize(admin)`.
//!   3. CREATE `FlashLoanExecutor` + `initialize(admin, aavePool, AE)`.
//!   4. `AE.grantRole(EXECUTOR_ROLE, FLE)` (DeployMainnet SC-13 parity).
//!   5. `FLE.grantRole(EXECUTOR_ROLE, caller)` per flashloan caller — a REAL
//!      grant (admin holds DEFAULT_ADMIN_ROLE after initialize), so the
//!      `requestFlashLoan` gate passes without any storage override on the
//!      deployed-contract path.
//!   6. `AE.setTokenApproval(token, true)` for token_in + token_out.
//!   7. `AE.setRouterApproval(router, true)` per router.
//!   8. `AE.setRouterSelectorApproval(router, selector, true)` per pair.
//!
//! Types and encoders are pure `ethers` (no feature gate); only
//! `deploy_paper_executor_stack` touches `simulator-v2` and is
//! `#[cfg(feature = "v2-simulator")]`.
//!
//! Anti-fraud invariants (mirroring `sim_multistep`):
//! 1. NO broadcast, NO signer — the deploys are REVM `transact_commit`s
//!    against the in-process `CacheDB<LazyDb>` only. Nothing leaves the host.
//! 2. The bytecode is the REAL forge output of the committed contracts,
//!    shipped as committed hex fixtures (`src/fixtures/`); a drift test
//!    compares fixture vs `contracts/out` whenever the latter exists so the
//!    fixtures can never silently diverge from source.
//! 3. Fail-closed: every deploy/setup step that reverts or halts returns a
//!    typed error; there is no partial stack.
//! 4. Selectors below are cast-verified (2026-09-22, foundry nightly):
//!    initialize(address)                    0xc4d66de8
//!    initialize(address,address,address)    0xc0c53b8b
//!    grantRole(bytes32,address)             0x2f2ff15d  (OZ standard)
//!    setTokenApproval(address,bool)         0x14d91c84
//!    setRouterApproval(address,bool)        0x47c1a9be
//!    setRouterSelectorApproval(address,bytes4,bool) 0x8a50ad03

use ethers::types::{Address, U256};
use thiserror::Error;
use tracing::{debug, warn};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error, PartialEq)]
pub enum PaperStackError {
    #[error("fixture bytecode hex failed to decode: {0}")]
    FixtureDecode(String),
    #[error("fixture {0} is empty")]
    FixtureEmpty(&'static str),
    #[error("ArbitrageExecutor deploy failed: {0}")]
    AeDeploy(String),
    #[error("FlashLoanExecutor deploy failed: {0}")]
    FleDeploy(String),
    #[error("ArbitrageExecutor.initialize failed: {0}")]
    AeInitialize(String),
    #[error("FlashLoanExecutor.initialize failed: {0}")]
    FleInitialize(String),
    #[error("grantRole(EXECUTOR_ROLE, FLE) failed: {0}")]
    GrantRole(String),
    #[error("FLE grantRole(EXECUTOR_ROLE, caller) failed: {0}")]
    FleCallerGrant(String),
    #[error("setTokenApproval failed: {0}")]
    TokenApproval(String),
    #[error("setRouterApproval failed: {0}")]
    RouterApproval(String),
    #[error("setRouterSelectorApproval failed: {0}")]
    RouterSelectorApproval(String),
    #[error("admin balance seed failed: {0}")]
    BalanceSeed(String),
    #[error("spec.aave_pool is the zero address (flashLoanProvider would fall back to zero)")]
    ZeroAavePool,
    #[error("spec.admin is the zero address")]
    ZeroAdmin,
}

impl PaperStackError {
    pub fn reason_tag(&self) -> &'static str {
        match self {
            Self::FixtureDecode(_) => "fixture_decode",
            Self::FixtureEmpty(_) => "fixture_empty",
            Self::AeDeploy(_) => "ae_deploy",
            Self::FleDeploy(_) => "fle_deploy",
            Self::AeInitialize(_) => "ae_initialize",
            Self::FleInitialize(_) => "fle_initialize",
            Self::GrantRole(_) => "grant_role",
            Self::FleCallerGrant(_) => "fle_caller_grant",
            Self::TokenApproval(_) => "token_approval",
            Self::RouterApproval(_) => "router_approval",
            Self::RouterSelectorApproval(_) => "router_selector_approval",
            Self::BalanceSeed(_) => "balance_seed",
            Self::ZeroAavePool => "zero_aave_pool",
            Self::ZeroAdmin => "zero_admin",
        }
    }
}

// ---------------------------------------------------------------------------
// Fixtures (committed forge bytecode — the REAL compiled contracts)
// ---------------------------------------------------------------------------

pub const ARBITRAGE_EXECUTOR_BYTECODE_HEX: &str =
    include_str!("fixtures/ArbitrageExecutor.bytecode.hex");
pub const FLASHLOAN_EXECUTOR_BYTECODE_HEX: &str =
    include_str!("fixtures/FlashLoanExecutor.bytecode.hex");

/// Decode a `0x`-prefixed (or bare) hex fixture into raw creation code.
/// Fail-closed on odd length / non-hex characters / empty payload.
pub fn decode_fixture(
    hex_str: &'static str,
    name: &'static str,
) -> Result<Vec<u8>, PaperStackError> {
    let stripped = hex_str.strip_prefix("0x").unwrap_or(hex_str).trim();
    if stripped.is_empty() {
        return Err(PaperStackError::FixtureEmpty(name));
    }
    hex::decode(stripped).map_err(|e| PaperStackError::FixtureDecode(format!("{name}: {e}")))
}

// ---------------------------------------------------------------------------
// ABI encoders (hand-built, same style as sequence_runner helpers)
// ---------------------------------------------------------------------------

fn addr_word(a: Address) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[12..32].copy_from_slice(a.as_bytes());
    w
}

fn bool_word(v: bool) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[31] = u8::from(v);
    w
}

/// `initialize(address)` → AE. Selector 0xc4d66de8 (cast-verified).
pub fn encode_initialize_admin(admin: Address) -> Vec<u8> {
    let mut cd = Vec::with_capacity(36);
    cd.extend_from_slice(&[0xc4, 0xd6, 0x6d, 0xe8]);
    cd.extend_from_slice(&addr_word(admin));
    cd
}

/// `initialize(address,address,address)` → FLE. Selector 0xc0c53b8b.
pub fn encode_initialize_fle(
    admin: Address,
    aave_pool: Address,
    arbitrage_executor: Address,
) -> Vec<u8> {
    let mut cd = Vec::with_capacity(4 + 96);
    cd.extend_from_slice(&[0xc0, 0xc5, 0x3b, 0x8b]);
    cd.extend_from_slice(&addr_word(admin));
    cd.extend_from_slice(&addr_word(aave_pool));
    cd.extend_from_slice(&addr_word(arbitrage_executor));
    cd
}

/// `grantRole(bytes32,address)`. Selector 0x2f2ff15d (OZ standard).
pub fn encode_grant_role(role: [u8; 32], account: Address) -> Vec<u8> {
    let mut cd = Vec::with_capacity(68);
    cd.extend_from_slice(&[0x2f, 0x2f, 0xff, 0x15]);
    cd.extend_from_slice(&role);
    cd.extend_from_slice(&addr_word(account));
    cd
}

/// `setTokenApproval(address,bool)`. Selector 0x14d91c84.
pub fn encode_set_token_approval(token: Address, status: bool) -> Vec<u8> {
    let mut cd = Vec::with_capacity(68);
    cd.extend_from_slice(&[0x14, 0xd9, 0x1c, 0x84]);
    cd.extend_from_slice(&addr_word(token));
    cd.extend_from_slice(&bool_word(status));
    cd
}

/// `setRouterApproval(address,bool)`. Selector 0x47c1a9be.
pub fn encode_set_router_approval(router: Address, status: bool) -> Vec<u8> {
    let mut cd = Vec::with_capacity(68);
    cd.extend_from_slice(&[0x47, 0xc1, 0xa9, 0xbe]);
    cd.extend_from_slice(&addr_word(router));
    cd.extend_from_slice(&bool_word(status));
    cd
}

/// `setRouterSelectorApproval(address,bytes4,bool)`. Selector 0x8a50ad03.
/// The `bytes4` head word carries the selector left-aligned (fixed-type ABI).
pub fn encode_set_router_selector_approval(
    router: Address,
    selector: [u8; 4],
    status: bool,
) -> Vec<u8> {
    let mut cd = Vec::with_capacity(100);
    cd.extend_from_slice(&[0x8a, 0x50, 0xad, 0x03]);
    cd.extend_from_slice(&addr_word(router));
    let mut sel = [0u8; 32];
    sel[..4].copy_from_slice(&selector);
    cd.extend_from_slice(&sel);
    cd.extend_from_slice(&bool_word(status));
    cd
}

// ---------------------------------------------------------------------------
// Stack spec + deploy
// ---------------------------------------------------------------------------

/// Operator-supplied inputs for the paper executor-stack deploy. Every field
/// mandatory; no defaults for economic parameters.
#[derive(Debug, Clone)]
pub struct PaperStackSpec {
    /// The paper caller (== `RoundTripContext::caller`). Receives the seeded
    /// ETH, holds ADMIN/UPGRADER on both deployed contracts, and is granted
    /// EXECUTOR_ROLE on the FLE by the stack itself (real grantRole tx, so
    /// the `requestFlashLoan` gate passes WITHOUT any storage override).
    pub admin: Address,
    /// REAL Aave V3 pool resolved from forked state (mainnet:
    /// 0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2). The FLE's
    /// `flashLoanProvider`/`aavePool` — the flash-loan source of funds.
    pub aave_pool: Address,
    /// Tokens to allowlist on the AE (token_in + token_out of the candidate).
    pub tokens: Vec<Address>,
    /// Routers to allowlist on the AE.
    pub routers: Vec<Address>,
    /// `(router, selector)` pairs to allowlist (e.g. V3 router +
    /// `exactInputSingle` 0x414bf389; V2 router + `swapExactTokensForTokens`
    /// 0x38ed1739).
    pub router_selectors: Vec<(Address, [u8; 4])>,
    /// Accounts granted `EXECUTOR_ROLE` on the deployed FLE via a REAL
    /// `grantRole` tx (admin holds `DEFAULT_ADMIN_ROLE` after
    /// `initialize`). Normally just `[RoundTripContext::caller]` — the
    /// `requestFlashLoan` `onlyRole(EXECUTOR_ROLE)` gate. Granting it for
    /// real (instead of the sim_prefund storage override) keeps the fork
    /// honest on the deployed-contract path.
    pub flashloan_callers: Vec<Address>,
    /// ETH seeded to the admin so every committed tx (deploys + setup + the
    /// later flash dispatch) can pay gas. Absolute set, not additive.
    pub admin_eth_seed_wei: U256,
    pub gas_price_wei: u128,
    pub gas_limit: u64,
}

/// The deployed paper stack addresses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaperExecutorStack {
    pub arbitrage_executor: Address,
    pub flashloan_executor: Address,
}

/// Deploy the paper executor stack into an EXISTING `SequenceContext`'s
/// CacheDB. The deploys are real REVM CREATEs against real forked state; the
/// created contracts are immediately visible to subsequent steps. Mirrors the
/// `DeployMainnet.s.sol` sequence (SC-13 grant included). Fail-closed on every
/// step — a partial stack is never returned.
#[cfg(feature = "v2-simulator")]
pub fn deploy_paper_executor_stack(
    sctx: &mut simulator_v2::sequence_runner::SequenceContext,
    spec: &PaperStackSpec,
) -> Result<PaperExecutorStack, PaperStackError> {
    use simulator_v2::sequence_runner::SequenceDeploy;

    fn to_alloy(a: Address) -> simulator_v2::AlloyAddress {
        simulator_v2::AlloyAddress::from_slice(a.as_bytes())
    }

    fn to_ethers(a: simulator_v2::AlloyAddress) -> Address {
        let mut bytes = [0u8; 20];
        bytes.copy_from_slice(a.as_slice());
        Address::from(bytes)
    }

    if spec.admin == Address::zero() {
        return Err(PaperStackError::ZeroAdmin);
    }
    if spec.aave_pool == Address::zero() {
        return Err(PaperStackError::ZeroAavePool);
    }

    let admin = to_alloy(spec.admin);

    // 1. Seed the admin's ETH (paper-only; absolute set).
    let seed = {
        let mut bytes = [0u8; 32];
        spec.admin_eth_seed_wei.to_big_endian(&mut bytes);
        simulator_v2::AlloyU256::from_be_bytes(bytes)
    };
    sctx.seed_balance(admin, seed, "paper_admin_eth_seed")
        .map_err(|e| PaperStackError::BalanceSeed(e.reason_tag().to_string()))?;

    // 2. Deploy ArbitrageExecutor + initialize(admin).
    let ae_code = decode_fixture(
        ARBITRAGE_EXECUTOR_BYTECODE_HEX,
        "ArbitrageExecutor.bytecode.hex",
    )?;
    let ae = match sctx.deploy(SequenceDeploy {
        from: admin,
        init_code: ae_code,
        gas_price_wei: spec.gas_price_wei,
        gas_limit: spec.gas_limit,
        label: "paper_deploy_arbitrage_executor",
    }) {
        Ok(simulator_v2::sequence_runner::DeployOutcome::Success { address, .. }) => address,
        Ok(other) => {
            let why = match &other {
                simulator_v2::sequence_runner::DeployOutcome::Reverted { reason, .. } => {
                    reason.clone()
                }
                simulator_v2::sequence_runner::DeployOutcome::Halted { reason, .. } => {
                    reason.clone()
                }
                _ => unreachable!("matched Success above"),
            };
            warn!(event = "paper_stack.ae_deploy_failed", why = %why);
            return Err(PaperStackError::AeDeploy(why));
        }
        Err(e) => return Err(PaperStackError::AeDeploy(e.to_string())),
    };
    exec_setup_call(
        sctx,
        spec,
        ae,
        encode_initialize_admin(spec.admin),
        "paper_ae_initialize",
        PaperStackError::AeInitialize,
    )?;
    let ae_e = to_ethers(ae);

    // 3. Deploy FlashLoanExecutor + initialize(admin, aavePool, AE).
    let fle_code = decode_fixture(
        FLASHLOAN_EXECUTOR_BYTECODE_HEX,
        "FlashLoanExecutor.bytecode.hex",
    )?;
    let fle = match sctx.deploy(SequenceDeploy {
        from: admin,
        init_code: fle_code,
        gas_price_wei: spec.gas_price_wei,
        gas_limit: spec.gas_limit,
        label: "paper_deploy_flashloan_executor",
    }) {
        Ok(simulator_v2::sequence_runner::DeployOutcome::Success { address, .. }) => address,
        Ok(other) => {
            let why = match &other {
                simulator_v2::sequence_runner::DeployOutcome::Reverted { reason, .. } => {
                    reason.clone()
                }
                simulator_v2::sequence_runner::DeployOutcome::Halted { reason, .. } => {
                    reason.clone()
                }
                _ => unreachable!("matched Success above"),
            };
            warn!(event = "paper_stack.fle_deploy_failed", why = %why);
            return Err(PaperStackError::FleDeploy(why));
        }
        Err(e) => return Err(PaperStackError::FleDeploy(e.to_string())),
    };
    exec_setup_call(
        sctx,
        spec,
        fle,
        encode_initialize_fle(spec.admin, spec.aave_pool, ae_e),
        "paper_fle_initialize",
        PaperStackError::FleInitialize,
    )?;
    let fle_e = to_ethers(fle);

    // 4. AE.grantRole(EXECUTOR_ROLE, FLE) — DeployMainnet SC-13 parity.
    exec_setup_call(
        sctx,
        spec,
        ae,
        encode_grant_role(crate::sim_prefund::executor_role_id(), fle_e),
        "paper_ae_grant_fle_executor",
        PaperStackError::GrantRole,
    )?;

    // 4b. FLE.grantRole(EXECUTOR_ROLE, caller) for each flashloan caller.
    for caller in &spec.flashloan_callers {
        exec_setup_call(
            sctx,
            spec,
            fle,
            encode_grant_role(crate::sim_prefund::executor_role_id(), *caller),
            "paper_fle_grant_caller_executor",
            PaperStackError::FleCallerGrant,
        )?;
    }

    // 5. Token allowlist.
    for token in &spec.tokens {
        exec_setup_call(
            sctx,
            spec,
            ae,
            encode_set_token_approval(*token, true),
            "paper_ae_set_token_approval",
            PaperStackError::TokenApproval,
        )?;
    }

    // 6. Router allowlist.
    for router in &spec.routers {
        exec_setup_call(
            sctx,
            spec,
            ae,
            encode_set_router_approval(*router, true),
            "paper_ae_set_router_approval",
            PaperStackError::RouterApproval,
        )?;
    }

    // 7. Router-selector allowlist.
    for (router, selector) in &spec.router_selectors {
        exec_setup_call(
            sctx,
            spec,
            ae,
            encode_set_router_selector_approval(*router, *selector, true),
            "paper_ae_set_router_selector_approval",
            PaperStackError::RouterSelectorApproval,
        )?;
    }

    debug!(
        event = "paper_stack.deployed",
        admin = ?spec.admin,
        arbitrage_executor = ?ae_e,
        flashloan_executor = ?fle_e,
        aave_pool = ?spec.aave_pool,
        tokens = spec.tokens.len(),
        routers = spec.routers.len(),
        router_selectors = spec.router_selectors.len(),
        flashloan_callers = spec.flashloan_callers.len(),
    );

    Ok(PaperExecutorStack {
        arbitrage_executor: ae_e,
        flashloan_executor: fle_e,
    })
}

#[cfg(feature = "v2-simulator")]
fn exec_setup_call(
    sctx: &mut simulator_v2::sequence_runner::SequenceContext,
    spec: &PaperStackSpec,
    to: simulator_v2::AlloyAddress,
    calldata: Vec<u8>,
    label: &'static str,
    wrap: fn(String) -> PaperStackError,
) -> Result<(), PaperStackError> {
    use simulator_v2::sequence_runner::{CallOutcome, SequenceCall};
    let from = simulator_v2::AlloyAddress::from_slice(spec.admin.as_bytes());
    let outcome = sctx
        .call(SequenceCall {
            from,
            to,
            calldata,
            value_wei: 0,
            gas_price_wei: spec.gas_price_wei,
            gas_limit: spec.gas_limit,
            label,
        })
        .map_err(|e| wrap(e.to_string()))?;
    match outcome {
        CallOutcome::Success { .. } => Ok(()),
        CallOutcome::Reverted { reason, .. } => Err(wrap(reason)),
        CallOutcome::Halted { reason, .. } => Err(wrap(reason)),
    }
}

// ---------------------------------------------------------------------------
// Tests (pure parts only — REVM-dependent coverage lives in the
// variance_benchmark harness, same M5 pattern as sim_multistep)
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn fixtures_decode_non_empty() {
        let ae = decode_fixture(
            ARBITRAGE_EXECUTOR_BYTECODE_HEX,
            "ArbitrageExecutor.bytecode.hex",
        )
        .expect("AE fixture must decode");
        assert!(!ae.is_empty(), "AE creation code must be non-empty");
        // A contract creation blob is at least the runtime push10 + constructor
        // boilerplate; anything under 100 bytes is not a real OZ-upgradeable.
        assert!(
            ae.len() > 100,
            "AE creation code suspiciously short: {}",
            ae.len()
        );

        let fle = decode_fixture(
            FLASHLOAN_EXECUTOR_BYTECODE_HEX,
            "FlashLoanExecutor.bytecode.hex",
        )
        .expect("FLE fixture must decode");
        assert!(
            fle.len() > 100,
            "FLE creation code suspiciously short: {}",
            fle.len()
        );
    }

    /// Drift guard: when `contracts/out` exists (local forge build), the
    /// committed fixtures MUST byte-match the freshly compiled creation code.
    /// `contracts/out` is untracked build output (VPS image rust:1.91 has no
    /// forge), so the check self-skips where the artifacts are absent — but
    /// any machine that DID build the contracts can never ship a stale
    /// fixture. Regenerate with `scripts/gen_executor_fixtures.sh`.
    #[test]
    fn fixtures_match_forge_output_when_present() {
        for (name, fixture) in [
            ("ArbitrageExecutor", ARBITRAGE_EXECUTOR_BYTECODE_HEX),
            ("FlashLoanExecutor", FLASHLOAN_EXECUTOR_BYTECODE_HEX),
        ] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../contracts/out")
                .join(format!("{name}.sol"))
                .join(format!("{name}.json"));
            if !path.exists() {
                continue;
            }
            let raw = std::fs::read_to_string(&path).expect("read forge artifact");
            let json: serde_json::Value = serde_json::from_str(&raw).expect("parse forge artifact");
            let object = json["bytecode"]["object"]
                .as_str()
                .expect("bytecode.object");
            // This forge version emits `object` WITH a `0x` prefix; older ones
            // emit bare hex. Normalize so the fixture stays single-prefixed.
            let want = if object.starts_with("0x") {
                object.to_string()
            } else {
                format!("0x{object}")
            };
            assert_eq!(
                fixture.trim(),
                want,
                "fixture {name} drifted from contracts/out — regenerate via scripts/gen_executor_fixtures.sh"
            );
        }
    }

    #[test]
    fn fixture_decode_rejects_garbage() {
        assert!(decode_fixture("zz", "bad").is_err());
        assert!(matches!(
            decode_fixture("0x", "empty"),
            Err(PaperStackError::FixtureEmpty(_))
        ));
        // Odd length fails.
        assert!(decode_fixture("abc", "odd").is_err());
    }

    #[test]
    fn encoders_emit_verified_selectors() {
        let a = Address::zero();
        assert_eq!(&encode_initialize_admin(a)[0..4], &[0xc4, 0xd6, 0x6d, 0xe8]);
        assert_eq!(encode_initialize_admin(a).len(), 36);

        assert_eq!(
            &encode_initialize_fle(a, a, a)[0..4],
            &[0xc0, 0xc5, 0x3b, 0x8b]
        );
        assert_eq!(encode_initialize_fle(a, a, a).len(), 100);

        assert_eq!(
            &encode_grant_role([0x11; 32], a)[0..4],
            &[0x2f, 0x2f, 0xff, 0x15]
        );
        assert_eq!(encode_grant_role([0x11; 32], a).len(), 68);

        assert_eq!(
            &encode_set_token_approval(a, true)[0..4],
            &[0x14, 0xd9, 0x1c, 0x84]
        );
        assert_eq!(encode_set_token_approval(a, true).len(), 68);
        // bool true encodes as 1 in the final word's last byte.
        let cd = encode_set_token_approval(a, true);
        assert_eq!(cd[67], 1);
        let cd = encode_set_token_approval(a, false);
        assert_eq!(cd[67], 0);

        assert_eq!(
            &encode_set_router_approval(a, true)[0..4],
            &[0x47, 0xc1, 0xa9, 0xbe]
        );

        assert_eq!(
            &encode_set_router_selector_approval(a, [0x41, 0x4b, 0xf3, 0x89], true)[0..4],
            &[0x8a, 0x50, 0xad, 0x03]
        );
        let cd = encode_set_router_selector_approval(a, [0x41, 0x4b, 0xf3, 0x89], true);
        assert_eq!(cd.len(), 100);
        // bytes4 left-aligned in its head word.
        assert_eq!(&cd[4 + 32..4 + 36], &[0x41, 0x4b, 0xf3, 0x89]);
    }
}
