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
    #[error("could not resolve the chain's Aave V3 pool: {0}")]
    AavePoolUnresolved(String),
    #[error(
        "chain {chain_id} has no Aave V3 PoolAddressesProvider in the catalog \
         (no AAVE_POOL_{chain_id} override either) — fail-closed, no fallback pool"
    )]
    AavePoolProviderMissing { chain_id: u64 },
    #[error("Aave V3 getPool() read failed for chain {chain_id}: {reason}")]
    AavePoolReadFailed { chain_id: u64, reason: String },
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
            Self::AavePoolUnresolved(_) => "aave_pool_unresolved",
            Self::AavePoolProviderMissing { .. } => "aave_pool_provider_missing",
            Self::AavePoolReadFailed { .. } => "aave_pool_read_failed",
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
/// ERC1967 proxy creation code (OpenZeppelin). PR-D1 (2026-10-18): the paper
/// stack deploys each executor BEHIND this proxy; the proxy's constructor
/// delegatecalls `initialize`, because the implementation's constructor calls
/// `_disableInitializers()` and a direct `initialize` on the impl reverts with
/// `InvalidInitialization()` (0xf92ee8a9).
pub const ERC1967_PROXY_BYTECODE_HEX: &str = include_str!("fixtures/ERC1967Proxy.bytecode.hex");

/// Build the ERC1967Proxy creation code for `logic` initialized with `data`.
///
/// `ERC1967Proxy(address _logic, bytes memory _data)` — ABI head is
/// `[logic, offset]`, then the dynamic `bytes` tail (`len || padded data`).
/// `data` is the implementation's `initialize(...)` calldata. The proxy
/// constructor `delegatecall`s `data` to `logic`, so the initialization lands
/// in the PROXY's storage (the impl stays uninitialized / disabled).
pub fn erc1967_proxy_init_code(proxy_creation: &[u8], logic: Address, data: &[u8]) -> Vec<u8> {
    let mut cd = Vec::with_capacity(proxy_creation.len() + 96 + data.len() + 32);
    cd.extend_from_slice(proxy_creation);
    // head word 0: logic address
    cd.extend_from_slice(&addr_word(logic));
    // head word 1: offset of the dynamic bytes payload = 0x40 (two head words)
    let mut offset = [0u8; 32];
    offset[31] = 0x40;
    cd.extend_from_slice(&offset);
    // tail: length (u256, big-endian) then the data, zero-padded to 32 bytes
    let mut len = [0u8; 32];
    len[24..32].copy_from_slice(&(data.len() as u64).to_be_bytes());
    cd.extend_from_slice(&len);
    cd.extend_from_slice(data);
    let rem = data.len() % 32;
    if rem != 0 {
        cd.resize(cd.len() + (32 - rem), 0u8);
    }
    cd
}

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

/// Split a solc creation-code blob into `(executable code, trailing metadata)`.
///
/// solc appends a CBOR metadata section at the very END of the code, preceded
/// by a 2-byte big-endian length. That section carries the compiler version and
/// the ipfs/keccak hash of the metadata — i.e. **build artifacts, not
/// executable semantics**. Two builds of the SAME source can differ there
/// (toolchain, absolute paths, settings) while every executable byte is
/// identical.
///
/// Fail-honest (R8) — this NEVER guesses. If `code.len() < 2`, or the declared
/// length does not fit (`L + 2 > code.len()`), **or the declared length would
/// consume the whole blob** (`L + 2 == code.len()`, i.e. no executable bytes
/// would be left), the whole blob is returned as executable code with an empty
/// metadata slice and the caller compares **every byte**, i.e. strictly. There
/// is no "assume it is metadata" branch.
///
/// R-1 (2026-10-18): the `L + 2 == code.len()` case used to fall through and
/// return `executable = &[]` with the entire blob as metadata. A caller that
/// split BOTH sides could then compare two empty slices and pass trivially —
/// exactly the hollow pass the exclusion is not allowed to create. Treating a
/// whole-blob "metadata" as *not* a well-formed split (compare everything,
/// strictly) removes the degenerate branch; a real creation blob always keeps
/// non-empty executable code.
///
/// Invariants: `executable.len() + metadata.len() == code.len()`, and for any
/// non-empty input `!executable.is_empty()` (pinned by
/// `solc_metadata_split_is_explicit_and_fail_closed`).
pub fn split_solc_metadata(code: &[u8]) -> (&[u8], &[u8]) {
    if code.len() < 2 {
        return (code, &[]);
    }
    let declared = u16::from_be_bytes([code[code.len() - 2], code[code.len() - 1]]) as usize;
    // `>=` (not `>`): `declared + 2 == code.len()` would leave zero executable
    // bytes — a whole-blob "metadata" is never a valid split.
    if declared + 2 >= code.len() {
        return (code, &[]);
    }
    let start = code.len() - (declared + 2);
    (&code[..start], &code[start..])
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
    cd.extend_from_slice(&[0x2f, 0x2f, 0xf1, 0x5d]);
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
    /// The chain the spec targets. `for_round_trip` fills it from its own
    /// `chain_id` argument; `deploy_paper_executor_stack` uses it to resolve the
    /// dynamic Pool from the chain's `PoolAddressesProvider`.
    pub chain_id: u64,
    /// The paper caller (== `RoundTripContext::caller`). Receives the seeded
    /// ETH, holds ADMIN/UPGRADER on both deployed contracts, and is granted
    /// EXECUTOR_ROLE on the FLE by the stack itself (real grantRole tx, so
    /// the `requestFlashLoan` gate passes WITHOUT any storage override).
    pub admin: Address,
    /// REAL Aave V3 pool for the FLE's `flashLoanProvider`/`aavePool` — the
    /// flash-loan source of funds.
    ///
    /// `Some(pool)` is an EXPLICIT override (from `AAVE_POOL_<chain_id>`) and
    /// wins. `None` means "resolve it on-chain at deploy time":
    /// `deploy_paper_executor_stack` reads
    /// `IPoolAddressesProvider.getPool()` from the chain's canonical provider
    /// (`shared_rs::chains::aave_pool_addresses_provider`), which is the stable
    /// governance anchor — the Pool itself is not a constant (RULE 00: resolve
    /// `getPool()` instead of hardcoding). Mainnet's Pool today:
    /// 0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2.
    pub aave_pool: Option<Address>,
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

/// Paper-only ETH seeded to the admin when the spec is derived from a route.
/// Absolute set (not additive); enough for the deploys + setup + the later
/// flash dispatch to pay gas.
pub const PAPER_ADMIN_ETH_SEED_WEI: u128 = 1_000_000_000_000_000_000; // 1 ETH

/// PR-D2 (2026-10-18): maximum per-transaction gas the paper stack will ask
/// for. EIP-7825 caps a single transaction's gas limit at `2^24 = 16_777_216`
/// from the OSAKA fork on, and the REVM transactor REJECTS a transaction whose
/// gas limit exceeds that cap *before* executing it (`transaction gas limit
/// (30000000) is greater than the cap (16777216)`). The paper `SequenceContext`
/// pins `SpecId::OSAKA` regardless of the forked block, so the cap always
/// applies. `searcher-rs::scanner` passed `gas_limit_per_step = 30_000_000`,
/// which made a wired `PAPER_STACK_DEPLOY` unusable — the defect PR-D2 fixes.
///
/// Every spec built through [`PaperStackSpec::for_round_trip`] is capped here,
/// one notch below the consensus cap (the same margin the verified path keeps:
/// `verified_simulation` caps at `min(gas_limit, 16_777_216)`).
pub const PAPER_STACK_MAX_TX_GAS: u64 = 16_000_000;

/// Environment flag that turns the paper-stack deploy ON for the sim paths.
/// Absent, or any value other than `1`/`true`/`yes`/`on`, leaves the spec
/// `None`, so the default behaviour of every call site is unchanged
/// (P3 doctrine: flag OFF when it touches the wire).
pub const PAPER_STACK_DEPLOY_ENV: &str = "PAPER_STACK_DEPLOY";

/// `PAPER_STACK_DEPLOY` truthiness (`1`/`true`/`yes`/`on`, case-insensitive).
pub fn paper_stack_deploy_enabled() -> bool {
    match std::env::var(PAPER_STACK_DEPLOY_ENV) {
        Ok(v) => matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => false,
    }
}

impl PaperStackSpec {
    /// Derive a spec from the SAME route data the encoder uses, so the paper
    /// stack deploys an executor whose allowlist matches exactly the routers,
    /// tokens and selectors the round trip will call.
    ///
    /// The Aave V3 pool is an **explicit override** here: `AAVE_POOL_<chain_id>`
    /// when set (a malformed or zero value still rejects fail-closed — never
    /// silently dropped), else `None`, and `deploy_paper_executor_stack`
    /// resolves it dynamically from the chain's `PoolAddressesProvider`
    /// (`getPool()`) — no hardcoded pool. A bare `RoundTripContext` carries no
    /// provider, so this builder cannot read the chain itself.
    pub fn for_round_trip(
        ctx: &prioritization_spine::round_trip_executor::RoundTripContext,
        chain_id: u64,
        gas_price_wei: u128,
        gas_limit: u64,
    ) -> Result<Self, PaperStackError> {
        let aave_pool = shared_rs::chains::resolve_aave_pool_address_optional(chain_id)
            .map_err(|e| PaperStackError::AavePoolUnresolved(format!("chain {chain_id}: {e}")))?;

        // The selector the encoder will actually call, read from the plan's
        // calldata — never a hardcoded constant.
        let plan = prioritization_spine::round_trip_executor::build_round_trip_plan(ctx);
        let mut selector = [0u8; 4];
        if plan.forward_calldata.len() >= 4 {
            selector.copy_from_slice(&plan.forward_calldata[..4]);
        }

        let mut tokens = vec![ctx.token_in];
        if ctx.token_out != ctx.token_in {
            tokens.push(ctx.token_out);
        }
        let mut routers: Vec<Address> = Vec::new();
        let mut router_selectors: Vec<(Address, [u8; 4])> = Vec::new();
        for router in [ctx.forward_router, ctx.backward_router] {
            if router == Address::zero() {
                continue;
            }
            if !routers.contains(&router) {
                routers.push(router);
            }
            if !router_selectors.contains(&(router, selector)) {
                router_selectors.push((router, selector));
            }
        }

        Ok(Self {
            chain_id,
            admin: ctx.caller,
            aave_pool,
            tokens,
            routers,
            router_selectors,
            flashloan_callers: vec![ctx.caller],
            admin_eth_seed_wei: U256::from(PAPER_ADMIN_ETH_SEED_WEI),
            gas_price_wei,
            // PR-D2: never ask for more than the OSAKA per-tx gas cap
            // (EIP-7825). The `scanner` call site passes `30_000_000`; uncapped,
            // the transactor rejects the deploy tx with
            // `transaction gas limit (30000000) is greater than the cap
            // (16777216)` before executing — a wired `PAPER_STACK_DEPLOY` could
            // never turn on. The cap is a no-op for call sites already under it.
            gas_limit: gas_limit.min(PAPER_STACK_MAX_TX_GAS),
        })
    }
}

/// Convenience for the production sim call sites: build the spec for `ctx`
/// when `PAPER_STACK_DEPLOY` is enabled, else `None`. A resolution failure is
/// logged and degrades to `None` — the pre-PR-D1 behaviour (sim against the
/// env-deployed executor) — rather than failing the whole simulation.
pub fn paper_stack_for_round_trip(
    ctx: &prioritization_spine::round_trip_executor::RoundTripContext,
    chain_id: u64,
    gas_price_wei: u128,
    gas_limit: u64,
) -> Option<PaperStackSpec> {
    if !paper_stack_deploy_enabled() {
        return None;
    }
    match PaperStackSpec::for_round_trip(ctx, chain_id, gas_price_wei, gas_limit) {
        Ok(spec) => Some(spec),
        Err(error) => {
            warn!(
                event = "paper_stack.spec_unresolved",
                chain_id, %error,
                "PAPER_STACK_DEPLOY set but the spec could not be built — \
                 falling back to the env-deployed executor path"
            );
            None
        }
    }
}

/// Resolve the Aave V3 Pool for `chain_id` DYNAMICALLY, from the chain's
/// canonical `PoolAddressesProvider` (`IPoolAddressesProvider.getPool()`), via a
/// non-committing read against the context's forked state — the provider is the
/// stable governance anchor, so a `setPool` upgrade needs no code change
/// (RULE 00, `.claude/skills/arbx-live-engineering/references/biblioteca/14-onchain-execution-contracts.md`).
///
/// Fail-closed on EVERY adverse path: no provider in the catalog, a failed or
/// empty read (no contract at the provider address), a non-32-byte return, or
/// the zero address each return a typed error — never an invented Pool.
#[cfg(feature = "v2-simulator")]
pub fn resolve_aave_pool_from_provider(
    sctx: &mut simulator_v2::sequence_runner::SequenceContext,
    chain_id: u64,
) -> Result<Address, PaperStackError> {
    let provider = shared_rs::chains::aave_pool_addresses_provider(chain_id)
        .ok_or(PaperStackError::AavePoolProviderMissing { chain_id })?;
    let provider = simulator_v2::AlloyAddress::from_slice(provider.as_bytes());

    let output = sctx
        .read_call(
            provider,
            shared_rs::chains::AAVE_V3_GET_POOL_SELECTOR.to_vec(),
            "aave_pool_addresses_provider_get_pool",
        )
        .map_err(|e| PaperStackError::AavePoolReadFailed {
            chain_id,
            reason: e.to_string(),
        })?
        .ok_or(PaperStackError::AavePoolReadFailed {
            chain_id,
            reason: "getPool() returned no data (no contract at the provider address)".to_string(),
        })?;

    if output.len() != 32 {
        return Err(PaperStackError::AavePoolReadFailed {
            chain_id,
            reason: format!("getPool() returned {} bytes, expected 32", output.len()),
        });
    }
    let mut bytes = [0u8; 20];
    bytes.copy_from_slice(&output[12..32]);
    let pool = Address::from(bytes);
    if pool == Address::zero() {
        return Err(PaperStackError::ZeroAavePool);
    }
    Ok(pool)
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

    // Resolve the Aave V3 pool BEFORE any mutation: an explicit
    // `AAVE_POOL_<chain_id>` override (spec.aave_pool = Some) wins and keeps
    // today's behaviour exactly; otherwise resolve it DYNAMICALLY from the
    // chain's canonical `PoolAddressesProvider` (`getPool()`) — the stable
    // governance anchor — failing closed when there is no provider, the read
    // fails, or it returns the zero address. Never an invented pool (RULE 00).
    let aave_pool = match spec.aave_pool {
        Some(pool) => pool,
        None => resolve_aave_pool_from_provider(sctx, spec.chain_id)?,
    };
    if aave_pool == Address::zero() {
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

    // 2. Deploy the ArbitrageExecutor implementation, then an ERC1967Proxy that
    //    initializes it in the SAME tx (the proxy constructor delegatecalls
    //    `initialize`). PR-D1 (2026-10-18): the implementation's constructor
    //    calls `_disableInitializers()`, so calling `initialize` directly on the
    //    impl reverts with InvalidInitialization() — 0xf92ee8a9. The proxy is the
    //    live address that every later step (grantRole, allowlists) targets.
    let ae_code = decode_fixture(
        ARBITRAGE_EXECUTOR_BYTECODE_HEX,
        "ArbitrageExecutor.bytecode.hex",
    )?;
    let ae_impl = match sctx.deploy(SequenceDeploy {
        from: admin,
        init_code: ae_code,
        gas_price_wei: spec.gas_price_wei,
        gas_limit: spec.gas_limit,
        label: "paper_deploy_arbitrage_executor_impl",
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
    let ae = deploy_erc1967_proxy(
        sctx,
        spec,
        admin,
        ae_impl,
        encode_initialize_admin(spec.admin),
        "paper_deploy_arbitrage_executor_proxy",
        PaperStackError::AeInitialize,
    )?;
    let ae_e = to_ethers(ae);

    // 3. Deploy the FlashLoanExecutor implementation, then its ERC1967Proxy
    //    initialized with (admin, aavePool, AE).
    let fle_code = decode_fixture(
        FLASHLOAN_EXECUTOR_BYTECODE_HEX,
        "FlashLoanExecutor.bytecode.hex",
    )?;
    let fle_impl = match sctx.deploy(SequenceDeploy {
        from: admin,
        init_code: fle_code,
        gas_price_wei: spec.gas_price_wei,
        gas_limit: spec.gas_limit,
        label: "paper_deploy_flashloan_executor_impl",
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
    let fle = deploy_erc1967_proxy(
        sctx,
        spec,
        admin,
        fle_impl,
        encode_initialize_fle(spec.admin, aave_pool, ae_e),
        "paper_deploy_flashloan_executor_proxy",
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
        aave_pool = ?aave_pool,
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

/// PR-D1 (2026-10-18): deploy an ERC1967Proxy that initializes `logic` in its
/// own constructor and return the PROXY address. Fail-closed: a revert/halt of
/// the proxy deployment is surfaced through `wrap` (never a partial stack).
#[cfg(feature = "v2-simulator")]
fn deploy_erc1967_proxy(
    sctx: &mut simulator_v2::sequence_runner::SequenceContext,
    spec: &PaperStackSpec,
    admin: simulator_v2::AlloyAddress,
    logic: simulator_v2::AlloyAddress,
    initialize_data: Vec<u8>,
    label: &'static str,
    wrap: fn(String) -> PaperStackError,
) -> Result<simulator_v2::AlloyAddress, PaperStackError> {
    use simulator_v2::sequence_runner::{DeployOutcome, SequenceDeploy};

    let proxy_code = decode_fixture(ERC1967_PROXY_BYTECODE_HEX, "ERC1967Proxy.bytecode.hex")?;
    let mut logic_bytes = [0u8; 20];
    logic_bytes.copy_from_slice(logic.as_slice());
    let init_code =
        erc1967_proxy_init_code(&proxy_code, Address::from(logic_bytes), &initialize_data);

    match sctx.deploy(SequenceDeploy {
        from: admin,
        init_code,
        gas_price_wei: spec.gas_price_wei,
        gas_limit: spec.gas_limit,
        label,
    }) {
        Ok(DeployOutcome::Success { address, .. }) => Ok(address),
        Ok(other) => {
            let why = match &other {
                DeployOutcome::Reverted { reason, .. } => reason.clone(),
                DeployOutcome::Halted { reason, .. } => reason.clone(),
                _ => unreachable!("matched Success above"),
            };
            warn!(event = "paper_stack.proxy_deploy_failed", label = label, why = %why);
            Err(wrap(why))
        }
        Err(e) => Err(wrap(e.to_string())),
    }
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
    /// committed fixtures MUST match the freshly compiled creation code.
    ///
    /// METADATA POLICY (PR-D2 / ADR-004, 2026-10-18): the trailing solc CBOR
    /// metadata section is a **build artifact** (compiler version + ipfs/keccak
    /// of the metadata), not executable semantics, so it is EXCLUDED from the
    /// comparison — and the exclusion is **explicit, never silent**: the guard
    /// asserts that every byte BEFORE that section is identical and names, in
    /// the failure message, the exact byte ranges it ignores. If
    /// [`split_solc_metadata`] cannot find a well-formed trailing section it
    /// strips nothing and the comparison is byte-for-byte over the whole blob.
    ///
    /// `contracts/out` is untracked build output (the VPS image has no
    /// `forge`), so the check self-skips where the artifacts are absent — but
    /// any machine that DID build the contracts can never ship a stale fixture.
    /// Regenerate with `scripts/gen_executor_fixtures.sh`.
    #[test]
    fn fixtures_match_forge_output_when_present() {
        for (name, fixture) in [
            ("ArbitrageExecutor", ARBITRAGE_EXECUTOR_BYTECODE_HEX),
            ("FlashLoanExecutor", FLASHLOAN_EXECUTOR_BYTECODE_HEX),
            // PR-D1 (2026-10-18): the proxy creation code is a first-class
            // fixture (the paper stack deploys each executor BEHIND it), so the
            // drift guard covers it too — no more hardcoded two-name list.
            ("ERC1967Proxy", ERC1967_PROXY_BYTECODE_HEX),
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

            let fixture_bytes = decode_fixture(fixture, "fixture").expect("fixture must decode");
            let want_bytes = hex::decode(want.trim_start_matches("0x")).expect("forge object hex");

            let (fixture_code, fixture_meta) = split_solc_metadata(&fixture_bytes);
            let (want_code, want_meta) = split_solc_metadata(&want_bytes);

            // R-1: the executable side of a real creation blob is NEVER empty.
            // Without this, a degenerate `declared + 2 == code.len()` split on
            // BOTH sides would compare two empty slices and PASS trivially.
            assert!(
                !fixture_code.is_empty(),
                "{name}: fixture split produced an EMPTY executable slice \
                 ({} bytes total) — refusing a hollow pass",
                fixture_bytes.len()
            );
            assert!(
                !want_code.is_empty(),
                "{name}: contracts/out split produced an EMPTY executable slice \
                 ({} bytes total) — refusing a hollow pass",
                want_bytes.len()
            );

            // The split must be lossless — a guard that silently drops bytes
            // would be worse than no guard.
            assert_eq!(
                fixture_code.len() + fixture_meta.len(),
                fixture_bytes.len(),
                "{name}: split_solc_metadata lost bytes on the fixture"
            );
            assert_eq!(
                want_code.len() + want_meta.len(),
                want_bytes.len(),
                "{name}: split_solc_metadata lost bytes on contracts/out"
            );

            // The ONE declared exclusion, spelled out for every failure below.
            let ignored = format!(
                "DECLARED EXCLUSION — ignoring only the trailing solc CBOR metadata: \
                 fixture {} bytes at [{}, {}), contracts/out {} bytes at [{}, {})",
                fixture_meta.len(),
                fixture_code.len(),
                fixture_bytes.len(),
                want_meta.len(),
                want_code.len(),
                want_bytes.len(),
            );

            assert_eq!(
                fixture_code, want_code,
                "{ignored} — fixture {name} EXECUTABLE code drifted from contracts/out \
                 (the exclusion covers metadata only, so this is a real byte drift). \
                 Regenerate via scripts/gen_executor_fixtures.sh"
            );
        }
    }

    /// The exclusion above is only legitimate if the splitter is exact and
    /// fail-closed. This pins: boundaries, losslessness, and the fallbacks
    /// (R-1: including the whole-blob `declared + 2 == code.len()` case).
    #[test]
    fn solc_metadata_split_is_explicit_and_fail_closed() {
        // Well-formed: tail length 0x0033 = 51 bytes of CBOR after it.
        let mut code = vec![0x60, 0x80, 0x60, 0x40];
        code.extend_from_slice(&[0xaa; 51]);
        code.extend_from_slice(&[0x00, 0x33]);
        let (exec, meta) = split_solc_metadata(&code);
        assert_eq!(exec, &[0x60, 0x80, 0x60, 0x40]);
        assert_eq!(meta.len(), 53, "metadata section = 51 payload + 2 length");
        assert_eq!(exec.len() + meta.len(), code.len());

        // Fallback 1: nothing to read a length from ⇒ compare everything.
        let short = vec![0x60u8];
        let (e, m) = split_solc_metadata(&short);
        assert_eq!(e, short.as_slice());
        assert!(m.is_empty(), "short input must not be treated as metadata");

        // Fallback 2: declared length does not fit ⇒ compare everything.
        let bogus = vec![0x60u8, 0x80, 0xff, 0xff];
        let (e2, m2) = split_solc_metadata(&bogus);
        assert_eq!(e2, bogus.as_slice());
        assert!(m2.is_empty(), "oversized length must not strip anything");

        // R-1 pin: the 2-byte blob `0x0000` declares 0 bytes of metadata. It
        // must stay EXECUTABLE — returning an empty executable slice here is
        // the hollow-pass bug (two such splits would match).
        let two = vec![0x00u8, 0x00];
        let (e3, m3) = split_solc_metadata(&two);
        assert_eq!(e3, two.as_slice(), "0x0000 must not become empty metadata");
        assert!(m3.is_empty(), "0x0000 declares no metadata");

        // R-1 pin: `declared + 2 == code.len()` claims the WHOLE blob as
        // metadata ⇒ not a well-formed split, compare everything strictly.
        let mut whole = vec![0xaau8; 4];
        whole.extend_from_slice(&[0x00, 0x04]);
        let (e4, m4) = split_solc_metadata(&whole);
        assert_eq!(
            e4,
            whole.as_slice(),
            "whole-blob metadata is not a valid split — must compare everything"
        );
        assert!(m4.is_empty(), "whole-blob metadata must strip nothing");
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
            &[0x2f, 0x2f, 0xf1, 0x5d]
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

    /// PR-D1: the ERC1967Proxy creation code must carry a well-formed
    /// `(address logic, bytes data)` ABI tail so the proxy constructor can
    /// delegatecall `initialize` against the implementation.
    #[test]
    fn erc1967_proxy_init_code_abi_is_well_formed() {
        let proxy = vec![0x60u8, 0x80, 0x60, 0x40, 0x52]; // stand-in creation code
        let logic = Address::from([0x11u8; 20]);
        let data = encode_initialize_admin(Address::from([0x22u8; 20]));
        assert_eq!(
            data.len(),
            36,
            "initialize(address) calldata is 4 + 32 bytes"
        );

        let cd = erc1967_proxy_init_code(&proxy, logic, &data);

        // The creation code stays verbatim at the front.
        assert_eq!(&cd[..proxy.len()], proxy.as_slice());
        let tail = &cd[proxy.len()..];
        // head word 0: logic, left-padded to 32 bytes.
        assert_eq!(&tail[12..32], logic.as_bytes());
        // head word 1: offset of the dynamic bytes = 0x40.
        assert_eq!(tail[63], 0x40);
        // dynamic tail: length word then the data, zero-padded to 32 bytes.
        assert_eq!(&tail[64 + 24..64 + 32], &(data.len() as u64).to_be_bytes());
        assert_eq!(&tail[96..96 + data.len()], data.as_slice());
        assert_eq!(
            (cd.len() - proxy.len()) % 32,
            0,
            "ABI tail is 32-byte aligned"
        );
        assert!(cd.len() > proxy.len() + 96, "data must be present");

        // The committed fixture decodes and is non-empty.
        let code = decode_fixture(ERC1967_PROXY_BYTECODE_HEX, "ERC1967Proxy.bytecode.hex")
            .expect("ERC1967Proxy fixture must decode");
        assert!(!code.is_empty(), "ERC1967Proxy fixture must not be empty");
    }

    /// PR-D1 gate: construct a `PaperStackSpec`, run the REAL deploy
    /// (impl + ERC1967Proxy whose constructor initializes) and assert `Ok`.
    /// Requires an RPC that answers `eth_blockNumber`/`eth_getCode` — point
    /// `PAPER_STACK_RPC` at a bare `anvil` (default http://127.0.0.1:8545).
    /// Before PR-D1 this returned `Err(AeInitialize)` (revert 0xf92ee8a9).
    #[test]
    #[ignore = "requires an RPC at PAPER_STACK_RPC (default http://127.0.0.1:8545); run with --ignored"]
    fn paper_stack_deploy_initializes_via_erc1967_proxy_ok() {
        let rpc = std::env::var("PAPER_STACK_RPC")
            .unwrap_or_else(|_| "http://127.0.0.1:8545".to_string());
        let block = std::env::var("PAPER_STACK_BLOCK")
            .ok()
            .and_then(|s| s.parse::<u64>().ok());

        let lazy = simulator_v2::LazyDb::new(&rpc, block).expect("connect PAPER_STACK_RPC");
        let mut sctx =
            simulator_v2::sequence_runner::SequenceContext::new(lazy, 1, block.unwrap_or(1));

        let admin = Address::from([0x11u8; 20]);
        let spec = PaperStackSpec {
            chain_id: 1,
            admin,
            aave_pool: Some(Address::from([0x22u8; 20])),
            tokens: vec![],
            routers: vec![],
            router_selectors: vec![],
            flashloan_callers: vec![admin],
            admin_eth_seed_wei: U256::from(1_000_000_000_000_000_000u128),
            gas_price_wei: 1_000_000_000,
            gas_limit: 15_000_000,
        };

        let stack = deploy_paper_executor_stack(&mut sctx, &spec)
            .expect("paper stack deploy must be Ok via ERC1967Proxy (no InvalidInitialization)");
        assert_ne!(stack.arbitrage_executor, Address::zero());
        assert_ne!(stack.flashloan_executor, Address::zero());
        assert_ne!(stack.arbitrage_executor, stack.flashloan_executor);
        eprintln!(
            "PAPER_STACK_DEPLOY_OUTCOME=OK ae={:?} fle={:?}",
            stack.arbitrage_executor, stack.flashloan_executor
        );
    }

    /// PR-D2 root-cause pin: an UNCAPPED spec at the scanner's `30_000_000`
    /// must be REJECTED by the OSAKA per-tx cap (EIP-7825) before it executes.
    /// This is the defect PR-D2 fixes, pinned so a future change that drops the
    /// cap in `for_round_trip` cannot pass unnoticed. The spec is built
    /// literally (bypassing the cap) — the cap is exercised by the wired test.
    #[test]
    #[ignore = "requires an RPC at PAPER_STACK_RPC (default http://127.0.0.1:8545); run with --ignored"]
    fn paper_stack_uncapped_gas_is_rejected_by_the_osaka_tx_cap() {
        let rpc = std::env::var("PAPER_STACK_RPC")
            .unwrap_or_else(|_| "http://127.0.0.1:8545".to_string());
        let block = std::env::var("PAPER_STACK_BLOCK")
            .ok()
            .and_then(|s| s.parse::<u64>().ok());

        let lazy = simulator_v2::LazyDb::new(&rpc, block).expect("connect PAPER_STACK_RPC");
        let mut sctx =
            simulator_v2::sequence_runner::SequenceContext::new(lazy, 1, block.unwrap_or(1));

        let admin = Address::from([0x11u8; 20]);
        let spec = PaperStackSpec {
            chain_id: 1,
            admin,
            aave_pool: Some(Address::from([0x22u8; 20])),
            tokens: vec![],
            routers: vec![],
            router_selectors: vec![],
            flashloan_callers: vec![admin],
            admin_eth_seed_wei: U256::from(1_000_000_000_000_000_000u128),
            gas_price_wei: 1_000_000_000,
            // Deliberately UNCAPPED: the scanner's real value.
            gas_limit: 30_000_000,
        };
        let err = deploy_paper_executor_stack(&mut sctx, &spec)
            .expect_err("30_000_000 must be rejected by the OSAKA per-tx cap, not executed");
        let msg = err.to_string();
        assert!(
            msg.contains("16777216") || msg.to_lowercase().contains("cap"),
            "expected the EIP-7825 per-tx-cap rejection, got: {msg}"
        );
        eprintln!("PAPER_STACK_UNCAPPED_30M_REJECTED={msg}");
    }

    /// PR-D2 gate: run the WIRED path with the REAL gas each call site passes —
    /// `searcher-rs::scanner`'s `30_000_000` included — and require the deploy
    /// to be `Ok`. The spec comes from `PaperStackSpec::for_round_trip`, the
    /// exact builder behind `paper_stack_for_round_trip`, i.e. production.
    ///
    /// Before the PR-D2 cap this FAILS: the paper `SequenceContext` pins
    /// `SpecId::OSAKA`, EIP-7825 caps a transaction at `2^24 = 16_777_216`, and
    /// the transactor rejects the deploy tx BEFORE executing it
    /// (`transaction gas limit (30000000) is greater than the cap (16777216)`).
    /// `G-PAPER-STACK-01` stayed green with the wire broken only because it
    /// deployed a HAND-BUILT spec at `15_000_000` — the real gas of the call
    /// sites was never exercised. This test closes that gap.
    ///
    /// Requires an RPC at `PAPER_STACK_RPC` (bare `anvil` works). Gas values:
    /// `PAPER_STACK_GAS_VALUES` (comma-separated; default the real
    /// DEPLOY-CAPABLE call-site values `30000000,16000000`). `PAPER_STACK_CHAIN_ID`
    /// overrides the fork chain (default 1).
    ///
    /// The sim-ctl call sites pass a `500_000` default, which is below the
    /// paper-stack DEPLOY FLOOR (measured >2_000_000: the AE creation code alone
    /// deposits ~9.5 KB) — a deploy there fails with `OutOfGas` regardless of
    /// the cap. That is a separate, declared limitation of enabling
    /// `PAPER_STACK_DEPLOY` through sim-ctl with defaults, so 500_000 is covered
    /// by the offline cap test (`…caps_gas_below_osaka_tx_cap_for_every_call_site`)
    /// instead of here.
    #[test]
    #[ignore = "requires an RPC at PAPER_STACK_RPC (default http://127.0.0.1:8545); run with --ignored"]
    fn paper_stack_wired_path_deploys_ok_at_call_site_gas() {
        let rpc = std::env::var("PAPER_STACK_RPC")
            .unwrap_or_else(|_| "http://127.0.0.1:8545".to_string());
        let block = std::env::var("PAPER_STACK_BLOCK")
            .ok()
            .and_then(|s| s.parse::<u64>().ok());
        let chain_id = std::env::var("PAPER_STACK_CHAIN_ID")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(1);
        // `for_round_trip` resolves the Aave pool fail-closed from
        // `AAVE_POOL_<chain_id>`; mainnet's real pool keeps the fork honest.
        std::env::set_var(
            format!("AAVE_POOL_{chain_id}"),
            "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2",
        );

        let gas_values = std::env::var("PAPER_STACK_GAS_VALUES")
            .unwrap_or_else(|_| "30000000,16000000".to_string());
        let ctx = diag_ctx();

        let mut checked = 0usize;
        for raw in gas_values.split(',') {
            let raw = raw.trim();
            if raw.is_empty() {
                continue;
            }
            let gas: u64 = raw.parse().expect("PAPER_STACK_GAS_VALUES: not a u64");
            // The wired spec — the same builder every call site uses.
            let spec = PaperStackSpec::for_round_trip(&ctx, chain_id, 1_000_000_000, gas)
                .expect("wired spec must build");
            assert!(
                spec.gas_limit <= 16_777_216,
                "wired spec gas_limit {} exceeds the OSAKA per-tx cap for requested {gas}",
                spec.gas_limit
            );
            assert_eq!(
                spec.gas_limit,
                gas.min(PAPER_STACK_MAX_TX_GAS),
                "cap mismatch for requested gas {gas}"
            );

            let lazy = simulator_v2::LazyDb::new(&rpc, block).expect("connect PAPER_STACK_RPC");
            let mut sctx = simulator_v2::sequence_runner::SequenceContext::new(
                lazy,
                chain_id,
                block.unwrap_or(1),
            );
            let stack = deploy_paper_executor_stack(&mut sctx, &spec).unwrap_or_else(|e| {
                panic!(
                    "WIRED deploy must be Ok at call-site gas {gas} (capped {}): {e}",
                    spec.gas_limit
                )
            });
            assert_ne!(stack.arbitrage_executor, Address::zero());
            assert_ne!(stack.flashloan_executor, Address::zero());
            eprintln!(
                "PAPER_STACK_WIRED_AT_CALL_SITE_GAS requested={gas} capped={} OUTCOME=OK ae={:?} fle={:?}",
                spec.gas_limit, stack.arbitrage_executor, stack.flashloan_executor
            );
            checked += 1;
        }
        assert!(checked > 0, "no gas values exercised");
        std::env::remove_var(format!("AAVE_POOL_{chain_id}"));
    }

    /// Pool-dynamics (a): an explicit `AAVE_POOL_<chain>` override WINS over the
    /// dynamic provider resolution. Runs against a MAINNET FORK
    /// (`PAPER_STACK_RPC`): a bogus non-zero override is deployed as the FLE's
    /// `aavePool`, and reading it back from the deployed proxy proves
    /// `getPool()` was NOT consulted (the real mainnet pool differs).
    #[test]
    #[ignore = "requires a MAINNET-FORK RPC at PAPER_STACK_RPC; run with --ignored"]
    fn paper_stack_pool_env_override_beats_provider() {
        let rpc = std::env::var("PAPER_STACK_RPC")
            .unwrap_or_else(|_| "http://127.0.0.1:8545".to_string());
        let block = std::env::var("PAPER_STACK_BLOCK")
            .ok()
            .and_then(|s| s.parse::<u64>().ok());
        let chain_id = 1u64;
        let override_pool = Address::from([0x11u8; 20]);
        std::env::set_var(
            format!("AAVE_POOL_{chain_id}"),
            "0x1111111111111111111111111111111111111111",
        );

        let ctx = diag_ctx();
        let spec = PaperStackSpec::for_round_trip(&ctx, chain_id, 1_000_000_000, 16_000_000)
            .expect("spec");
        assert_eq!(spec.aave_pool, Some(override_pool));

        let lazy = simulator_v2::LazyDb::new(&rpc, block).expect("connect PAPER_STACK_RPC");
        let mut sctx =
            simulator_v2::sequence_runner::SequenceContext::new(lazy, chain_id, block.unwrap_or(1));
        let stack =
            deploy_paper_executor_stack(&mut sctx, &spec).expect("deploy with env override");
        let got = read_public_address(
            &mut sctx,
            stack.flashloan_executor,
            FLE_AAVE_POOL_SELECTOR,
            "fle_aave_pool_env_override",
        );
        assert_eq!(
            got, override_pool,
            "the env override must win over provider.getPool()"
        );
        eprintln!("PAPER_STACK_POOL_ENV_OVERRIDE_WINS={got:?}");
        std::env::remove_var(format!("AAVE_POOL_{chain_id}"));
    }

    /// Pool-dynamics (b)+(d): with NO env override the pool is read DYNAMICALLY
    /// from the chain's `PoolAddressesProvider` (`getPool()`); on mainnet the
    /// value MUST equal the canonical Pool `0x87870Bca…`, and the wired deploy
    /// must initialize the FLE with that same pool. Runs against a MAINNET FORK
    /// (`PAPER_STACK_RPC`).
    #[test]
    #[ignore = "requires a MAINNET-FORK RPC at PAPER_STACK_RPC; run with --ignored"]
    fn paper_stack_pool_resolved_from_provider_matches_mainnet_canonical() {
        let rpc = std::env::var("PAPER_STACK_RPC")
            .unwrap_or_else(|_| "http://127.0.0.1:8545".to_string());
        let block = std::env::var("PAPER_STACK_BLOCK")
            .ok()
            .and_then(|s| s.parse::<u64>().ok());
        let chain_id = 1u64;
        std::env::remove_var(format!("AAVE_POOL_{chain_id}"));
        let canonical: Address = "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2"
            .parse()
            .expect("canonical mainnet pool");

        let lazy = simulator_v2::LazyDb::new(&rpc, block).expect("connect PAPER_STACK_RPC");
        let mut sctx =
            simulator_v2::sequence_runner::SequenceContext::new(lazy, chain_id, block.unwrap_or(1));

        // (b) the dynamic read itself.
        let pool = resolve_aave_pool_from_provider(&mut sctx, chain_id).expect("getPool() read");
        // (d) mainnet parity: the read value is the canonical Pool.
        assert_eq!(pool, canonical, "mainnet getPool() must match 0x87870Bca…");

        // Wired deploy with no override resolves the pool dynamically.
        let ctx = diag_ctx();
        let spec = PaperStackSpec::for_round_trip(&ctx, chain_id, 1_000_000_000, 16_000_000)
            .expect("spec");
        assert_eq!(spec.aave_pool, None);
        let stack =
            deploy_paper_executor_stack(&mut sctx, &spec).expect("dynamic deploy must be Ok");
        let got = read_public_address(
            &mut sctx,
            stack.flashloan_executor,
            FLE_AAVE_POOL_SELECTOR,
            "fle_aave_pool_from_provider",
        );
        assert_eq!(
            got, canonical,
            "the FLE must be initialized with the provider-resolved pool"
        );
        eprintln!(
            "PAPER_STACK_POOL_FROM_PROVIDER={pool:?} ae={:?} fle={:?}",
            stack.arbitrage_executor, stack.flashloan_executor
        );
    }

    /// Selector of the `FlashLoanExecutor`'s `aavePool()` public getter
    /// (`cast sig "aavePool()"`), used to read back the pool the stack wired.
    const FLE_AAVE_POOL_SELECTOR: [u8; 4] = [0xa0, 0x3e, 0x4b, 0xc3];

    /// Read a `public address` getter (`selector()` → 32-byte word, low 20
    /// bytes) from `to` via the non-committing `read_call`.
    fn read_public_address(
        sctx: &mut simulator_v2::sequence_runner::SequenceContext,
        to: Address,
        selector: [u8; 4],
        label: &str,
    ) -> Address {
        let to = simulator_v2::AlloyAddress::from_slice(to.as_bytes());
        let out = sctx
            .read_call(to, selector.to_vec(), label)
            .expect("read_call must succeed")
            .expect("contract must return data");
        assert_eq!(out.len(), 32, "{label}: expected a 32-byte word");
        let mut bytes = [0u8; 20];
        bytes.copy_from_slice(&out[12..32]);
        Address::from(bytes)
    }

    fn diag_ctx() -> prioritization_spine::round_trip_executor::RoundTripContext {
        prioritization_spine::round_trip_executor::RoundTripContext {
            caller: Address::from([0x11u8; 20]),
            token_in: Address::from([0x01u8; 20]),
            token_out: Address::from([0x02u8; 20]),
            amount_in: U256::from(1u64),
            forward_router: Address::from([0xaau8; 20]),
            forward_path: vec![Address::from([0x01u8; 20]), Address::from([0x02u8; 20])],
            backward_router: Address::from([0xbbu8; 20]),
            backward_path: vec![Address::from([0x02u8; 20]), Address::from([0x01u8; 20])],
            deadline: U256::from(1u64),
        }
    }

    /// PR-D1 wiring: the spec is derived from the SAME route data the encoder
    /// uses, and the Aave pool comes from the env — never hardcoded.
    #[test]
    fn paper_stack_spec_for_round_trip_derives_route_data_and_env_pool() {
        let chain = 4_242_424u64; // test-only chain id, no other test touches it
        std::env::set_var(
            format!("AAVE_POOL_{chain}"),
            "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2",
        );
        let ctx = diag_ctx();
        let spec = PaperStackSpec::for_round_trip(&ctx, chain, 7, 16_000_000).expect("spec");

        assert_eq!(
            spec.aave_pool,
            Some(
                "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2"
                    .parse::<Address>()
                    .unwrap()
            )
        );
        assert_eq!(spec.chain_id, chain);
        assert_eq!(
            spec.tokens,
            vec![Address::from([0x01u8; 20]), Address::from([0x02u8; 20])]
        );
        assert_eq!(
            spec.routers,
            vec![Address::from([0xaau8; 20]), Address::from([0xbbu8; 20])]
        );
        // Selector read from the plan calldata (V2 swapExactTokensForTokens).
        assert_eq!(
            spec.router_selectors,
            vec![
                (Address::from([0xaau8; 20]), [0x38, 0xed, 0x17, 0x39]),
                (Address::from([0xbbu8; 20]), [0x38, 0xed, 0x17, 0x39]),
            ]
        );
        assert_eq!(spec.admin, Address::from([0x11u8; 20]));
        assert_eq!(spec.flashloan_callers, vec![Address::from([0x11u8; 20])]);
        assert_eq!(spec.gas_price_wei, 7);
        assert_eq!(spec.gas_limit, 16_000_000);
        std::env::remove_var(format!("AAVE_POOL_{chain}"));
    }

    /// PR-D2: `for_round_trip` caps the per-tx gas below the OSAKA cap
    /// (EIP-7825, `2^24 = 16_777_216`) for EVERY wired call site, using each
    /// call site's real `gas_limit_per_step`. The scanner passes `30_000_000`
    /// — the value that made a wired `PAPER_STACK_DEPLOY` unusable (the
    /// transactor rejected the deploy tx before executing). Below the cap the
    /// value is untouched (no behaviour change for the 500k/16M call sites).
    #[test]
    fn paper_stack_spec_caps_gas_below_osaka_tx_cap_for_every_call_site() {
        const OSAKA_TX_CAP: u64 = 16_777_216; // EIP-7825, revm 42 SpecId::OSAKA
        let chain = 4_242_428u64; // test-only chain id, no other test touches it
        std::env::set_var(
            format!("AAVE_POOL_{chain}"),
            "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2",
        );
        let ctx = diag_ctx();

        // (call site, the real gas it passes today)
        for (site, gas) in [
            ("searcher-rs/scanner.rs", 30_000_000u64),
            ("searcher-rs/candidate_simulation.rs", 16_000_000),
            ("relays-client/execution_admission.rs", 16_000_000),
            (
                "sim-ctl/sim_runner.rs (SIM_GAS_LIMIT_PER_STEP default)",
                500_000,
            ),
            (
                "sim-ctl/canonical_plan_consumer.rs (same env default)",
                500_000,
            ),
        ] {
            let spec = PaperStackSpec::for_round_trip(&ctx, chain, 7, gas).expect(site);
            assert!(
                spec.gas_limit <= OSAKA_TX_CAP,
                "{site}: gas_limit {} exceeds the OSAKA per-tx cap {OSAKA_TX_CAP}",
                spec.gas_limit
            );
            assert_eq!(
                spec.gas_limit,
                gas.min(PAPER_STACK_MAX_TX_GAS),
                "{site}: the cap must be exactly min(gas, {PAPER_STACK_MAX_TX_GAS})"
            );
        }
        std::env::remove_var(format!("AAVE_POOL_{chain}"));
    }

    /// Pool-dynamics: with `AAVE_POOL_<chain>` unset the spec carries `None`
    /// (no override) — the dynamic path takes over at deploy — while a
    /// MALFORMED override still rejects fail-closed, never silently dropped.
    #[test]
    fn paper_stack_spec_pool_env_absent_is_none_and_invalid_rejects() {
        let chain = 4_242_425u64;
        std::env::remove_var(format!("AAVE_POOL_{chain}"));
        let ctx = diag_ctx();

        let spec = PaperStackSpec::for_round_trip(&ctx, chain, 0, 16_000_000).expect("spec");
        assert_eq!(spec.aave_pool, None);
        assert_eq!(spec.chain_id, chain);

        std::env::set_var(format!("AAVE_POOL_{chain}"), "0xnothex");
        let err = PaperStackSpec::for_round_trip(&ctx, chain, 0, 16_000_000).unwrap_err();
        assert!(matches!(err, PaperStackError::AavePoolUnresolved(_)));
        // POOL-DYN-01 review (2026-10-18): the surfaced message must name the
        // CONCRETE variable. It used to say `EXECUTOR_<chain>` because the
        // shared error's Display hardcoded that prefix; assert on the TEXT so a
        // future refactor cannot reintroduce the misnaming (the variant-only
        // assert above did not catch it).
        assert!(
            err.to_string()
                .contains(&format!("AAVE_POOL_{chain} env var")),
            "the override error must name AAVE_POOL_{chain}, got: {err}"
        );
        assert!(
            !err.to_string().contains("EXECUTOR_"),
            "the override error must not name EXECUTOR_: {err}"
        );
        std::env::remove_var(format!("AAVE_POOL_{chain}"));
    }

    /// Pool-dynamics (c): a chain with NO provider in the catalog and no env
    /// override returns the typed `AavePoolProviderMissing` — and does so
    /// BEFORE any RPC read (the never-reachable URL proves no read is
    /// attempted), so a `None` spec can never become an invented pool.
    #[test]
    fn paper_stack_pool_resolution_fails_closed_without_provider() {
        let lazy = simulator_v2::LazyDb::new("http://127.0.0.1:1/never", Some(1))
            .expect("well-formed URL");
        let mut sctx = simulator_v2::sequence_runner::SequenceContext::new(lazy, 1, 1);
        let err = resolve_aave_pool_from_provider(&mut sctx, 4_242_401).unwrap_err();
        assert!(matches!(
            err,
            PaperStackError::AavePoolProviderMissing {
                chain_id: 4_242_401
            }
        ));
    }

    /// Pool-dynamics (c): a FAILING read is fail-closed with the typed
    /// `AavePoolReadFailed` — never a fallback pool. Offline: the
    /// never-reachable RPC guarantees the read errors.
    #[test]
    fn paper_stack_pool_resolution_fails_closed_when_read_errors() {
        let lazy = simulator_v2::LazyDb::new("http://127.0.0.1:1/never", Some(1))
            .expect("well-formed URL");
        let mut sctx = simulator_v2::sequence_runner::SequenceContext::new(lazy, 1, 1);
        let err = resolve_aave_pool_from_provider(&mut sctx, 1).unwrap_err();
        assert!(matches!(
            err,
            PaperStackError::AavePoolReadFailed { chain_id: 1, .. }
        ));
    }

    /// The deploy wire is OFF unless the flag is set (P3: flag OFF by default).
    #[test]
    fn paper_stack_for_round_trip_is_off_by_default() {
        let chain = 4_242_426u64;
        std::env::set_var(
            format!("AAVE_POOL_{chain}"),
            "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2",
        );
        std::env::remove_var(PAPER_STACK_DEPLOY_ENV);
        let ctx = diag_ctx();
        assert!(paper_stack_for_round_trip(&ctx, chain, 0, 16_000_000).is_none());
        std::env::set_var(PAPER_STACK_DEPLOY_ENV, "1");
        assert!(paper_stack_for_round_trip(&ctx, chain, 0, 16_000_000).is_some());
        std::env::remove_var(PAPER_STACK_DEPLOY_ENV);
        std::env::remove_var(format!("AAVE_POOL_{chain}"));
    }
}
