// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// SC-10: AdminTimelock — UUPS-compatible timelock wrapper
//
// Wraps OpenZeppelin TimelockControllerUpgradeable to enforce a configurable
// delay on admin operations.  Deployed as an ERC1967Proxy (same pattern as all
// other ArbitrageX v2 contracts).
//
// Purpose
// -------
// After deploy, the operator transfers ADMIN_ROLE (DEFAULT_ADMIN_ROLE) on
// ArbitrageExecutor, AllowanceManager and FlashLoanExecutor from the EOA deployer
// to this AdminTimelock proxy.  Subsequent admin operations (setRouterApproval,
// setTokenApproval, setAllowanceManager, grantRole, pause, …) must be scheduled
// through the timelock and cannot execute until `minDelay` seconds have elapsed.
//
// This means a compromised admin EOA or signing key can only schedule a
// malicious change — it cannot execute it immediately.  The operator has
// `minDelay` to detect, cancel, and rotate keys before any damage occurs.
//
// Roles inside TimelockController
// --------------------------------
//   PROPOSER_ROLE  — can schedule operations (and cancel them as CANCELLER_ROLE)
//   EXECUTOR_ROLE  — can execute ready operations
//   DEFAULT_ADMIN_ROLE — can grant/revoke the above roles
//
// Recommended initial setup (see DEPLOY.md §9):
//   proposers  = [multisig]
//   executors  = [multisig]       (or address(0) for open execution)
//   admin      = deployer EOA     (renounce after initial role configuration)
//   minDelay   = 86400 (24h)      (or 172800 for 48h on mainnet)
//
// IMPORTANT: Do NOT set `admin = address(0)` at deploy time or you will be
// unable to add proposers/executors post-deploy.  Renounce only after the
// multisig/DAO has been granted the three operational roles.
//
// Storage layout note
// -------------------
// TimelockControllerUpgradeable uses ERC-7201 namespaced storage — it does NOT
// occupy the linear slot space starting at slot 0.  AdminTimelock adds NO
// additional state variables, so there is nothing to track here.
// =============================================================================

import "@openzeppelin/contracts-upgradeable/governance/TimelockControllerUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";

/// @title AdminTimelock — operator timelock for ArbitrageX v2 admin operations
/// @notice A thin proxy wrapper around OZ TimelockControllerUpgradeable.
///         Deploy as an ERC1967Proxy; then transfer ADMIN_ROLE on all three
///         ArbitrageX contracts from the EOA deployer to this proxy address.
///
/// @dev    This contract is intentionally NOT UUPSUpgradeable. Timelocks are
///         security infrastructure — their upgrade path must also go through
///         the timelock itself (self-administration), which TimelockController
///         already supports via `schedule + execute` targeting itself.
///         Making it UUPS would create a bypass: an attacker with the upgrade
///         key could swap the implementation and skip the delay entirely.
contract AdminTimelock is Initializable, TimelockControllerUpgradeable {
    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        _disableInitializers();
    }

    /// @notice Initializer — called exactly once via ERC1967Proxy.
    /// @param minDelay   Minimum seconds between schedule and execute.
    ///                   Recommended: 86400 (24h) on mainnet, 60 (1min) on testnet.
    /// @param proposers  Accounts granted PROPOSER_ROLE (and CANCELLER_ROLE).
    ///                   Typically: [multisig].
    /// @param executors  Accounts granted EXECUTOR_ROLE.
    ///                   Pass [address(0)] to allow anyone to execute ready operations.
    /// @param admin      Optional bootstrap admin granted DEFAULT_ADMIN_ROLE.
    ///                   Use the deployer EOA; renounce after configuration is complete.
    ///                   Pass address(0) to skip (dangerous: you won't be able to add
    ///                   proposers/executors post-deploy unless proposers are set now).
    function initialize(
        uint256 minDelay,
        address[] memory proposers,
        address[] memory executors,
        address admin
    ) public override initializer {
        __TimelockController_init(minDelay, proposers, executors, admin);
    }
}
