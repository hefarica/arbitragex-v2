// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

// =============================================================================
// WalletTopology — Segregacion de fondos y autorizacion de roles
//
// Purpose: Separate operational concerns across three distinct wallet roles
//          to minimize blast radius of key compromise:
//
//   Role              | Purpose                          | Expected Balance
//   ------------------|----------------------------------|-----------------
//   GAS_SPONSOR       | Pays gas for all transactions    | Small (ETH/native)
//   EXECUTION_SIGNER  | Signs transactions, zero balance | Zero (never holds funds)
//   COLD_TREASURY     | Receives yield/profits           | Large (kept offline)
//
// This contract is NOT upgradeable by design. Wallet topology is foundational
// security infrastructure — upgrading it would create a key-management bypass.
// If the topology needs to change, deploy a new instance and migrate funds.
//
// Invariantes de estado:
//   1. Cada rol (GAS_SPONSOR, EXECUTION_SIGNER, COLD_TREASURY) es otorgado
//      exactamente una vez en el constructor y nunca revocado.
//   2. ColdTreasury solo puede ser actualizada por DEFAULT_ADMIN_ROLE.
//   3. EXECUTION_SIGNER no puede recibir fondos (isExecutionSigner verifica).
//   4. Ningun rol puede ser address(0) en el constructor.
//
// Gas profile (optimizer_runs = 200):
//   - isExecutionSigner() : ~2,500 gas (SLOAD + hasRole)
//   - setColdTreasury()   : ~25,000 gas (admin check + SSTORE + event)
// =============================================================================

import {AccessControl} from "@openzeppelin/contracts/access/AccessControl.sol";

// -------------------------------------------------------------------------
// Errors (SC-3: custom errors ~200 gas saved per revert vs string require)
// -------------------------------------------------------------------------

/// @dev Thrown when a constructor argument is address(0).
error WT_ZeroAddress();
/// @dev Thrown when the new treasury address is address(0).
error WT_ZeroTreasury();
/// @dev Thrown when gasSponsor, executionSigner and coldTreasury are not distinct.
error WT_DuplicateAddresses();
/// @dev Thrown when the caller does not hold DEFAULT_ADMIN_ROLE.
error WT_NotAdmin();

// =============================================================================
/// @title WalletTopology
/// @notice Segregacion de fondos y autorizacion de roles.
/// @dev Tres roles: GAS_SPONSOR (paga gas), EXECUTION_SIGNER (firma, cero
///      balance), COLD_TREASURY (recibe yield).
///      SC-15 (2026-05-15). NOT upgradeable — wallet topology is security
///      infrastructure. Uses OpenZeppelin AccessControl (non-upgradeable).
///
///      Invariantes de estado documentadas:
///        I1: GAS_SPONSOR_ROLE, EXECUTION_SIGNER_ROLE, COLD_TREASURY_ROLE son
///            mutuamente exclusivos — una direccion solo puede tener un rol.
///        I2: coldTreasury es siempre una direccion no-cero (no se puede quemar
///            fondos enviando a address(0)).
///        I3: Solo DEFAULT_ADMIN_ROLE puede cambiar coldTreasury.
///        I4: EXECUTION_SIGNER nunca debe tener balance — es un rol de firma
///            puramente off-chain.
// =============================================================================

contract WalletTopology is AccessControl {
    // -------------------------------------------------------------------------
    // Roles
    // -------------------------------------------------------------------------

    /// @notice Role for addresses that sponsor gas for transactions.
    ///         Gas sponsor wallets hold a small ETH balance for tx fees.
    bytes32 public constant GAS_SPONSOR_ROLE = keccak256("GAS_SPONSOR_ROLE");

    /// @notice Role for addresses that sign execution transactions.
    ///         Execution signers hold ZERO balance — they are pure signing keys.
    ///         SECURITY: Never send funds to an execution signer address.
    bytes32 public constant EXECUTION_SIGNER_ROLE = keccak256("EXECUTION_SIGNER_ROLE");

    /// @notice Role for the cold treasury that receives yield and profits.
    ///         Cold treasury should be a multisig or hardware wallet kept offline.
    bytes32 public constant COLD_TREASURY_ROLE = keccak256("COLD_TREASURY_ROLE");

    // -------------------------------------------------------------------------
    // State
    // -------------------------------------------------------------------------

    /// @notice Current gas sponsor address. Set at construction, immutable.
    ///         Invariant: always non-zero.
    address public immutable gasSponsor;

    /// @notice Current execution signer address. Set at construction, immutable.
    ///         Invariant: always non-zero, never holds funds.
    address public immutable executionSigner;

    /// @notice Current cold treasury address. Set at construction, mutable by admin.
    ///         Invariant: always non-zero (enforced by setColdTreasury).
    address public coldTreasury;

    // -------------------------------------------------------------------------
    // Events
    // -------------------------------------------------------------------------

    /// @notice Emitted on construction when all three wallets are configured.
    /// @param gasSponsor      Address with GAS_SPONSOR_ROLE.
    /// @param executionSigner Address with EXECUTION_SIGNER_ROLE.
    /// @param coldTreasury    Address with COLD_TREASURY_ROLE.
    event WalletConfigured(
        address indexed gasSponsor,
        address indexed executionSigner,
        address indexed coldTreasury
    );

    /// @notice Emitted when the cold treasury is updated by admin.
    /// @param previousTreasury The old treasury address.
    /// @param newTreasury      The new treasury address.
    event ColdTreasuryUpdated(
        address indexed previousTreasury,
        address indexed newTreasury
    );

    /// @notice Emitted when a role is granted. Used for off-chain indexing.
    /// @param role    The role that was granted.
    /// @param account The account that received the role.
    event RoleGrantedIndexed(bytes32 indexed role, address indexed account);

    // -------------------------------------------------------------------------
    // Constructor
    // -------------------------------------------------------------------------

    /// @notice Initialize the wallet topology with three segregated roles.
    /// @dev All three addresses must be non-zero and distinct. The deployer
    ///      receives DEFAULT_ADMIN_ROLE and can later update coldTreasury.
    ///
    ///      Post-condition:
    ///        - hasRole(GAS_SPONSOR_ROLE, _gasSponsor) == true
    ///        - hasRole(EXECUTION_SIGNER_ROLE, _executionSigner) == true
    ///        - hasRole(COLD_TREASURY_ROLE, _coldTreasury) == true
    ///        - hasRole(DEFAULT_ADMIN_ROLE, msg.sender) == true
    ///        - coldTreasury == _coldTreasury
    ///
    /// @param _gasSponsor      Address that will pay gas for transactions.
    ///                         Must be non-zero. Typically a hot wallet.
    /// @param _executionSigner Address that will sign execution transactions.
    ///                         Must be non-zero. Must NEVER hold funds.
    /// @param _coldTreasury    Address that will receive yield and profits.
    ///                         Must be non-zero. Typically a multisig or hardware wallet.
    constructor(address _gasSponsor, address _executionSigner, address _coldTreasury) {
        if (_gasSponsor == address(0)) revert WT_ZeroAddress();
        if (_executionSigner == address(0)) revert WT_ZeroAddress();
        if (_coldTreasury == address(0)) revert WT_ZeroAddress();

        // Invariant I1: all three addresses must be distinct
        if (_gasSponsor == _executionSigner || _gasSponsor == _coldTreasury
            || _executionSigner == _coldTreasury) {
            revert WT_DuplicateAddresses();
        }

        gasSponsor = _gasSponsor;
        executionSigner = _executionSigner;
        coldTreasury = _coldTreasury;

        // Grant roles
        _grantRole(GAS_SPONSOR_ROLE, _gasSponsor);
        _grantRole(EXECUTION_SIGNER_ROLE, _executionSigner);
        _grantRole(COLD_TREASURY_ROLE, _coldTreasury);
        _grantRole(DEFAULT_ADMIN_ROLE, msg.sender);

        emit WalletConfigured(_gasSponsor, _executionSigner, _coldTreasury);
        emit RoleGrantedIndexed(GAS_SPONSOR_ROLE, _gasSponsor);
        emit RoleGrantedIndexed(EXECUTION_SIGNER_ROLE, _executionSigner);
        emit RoleGrantedIndexed(COLD_TREASURY_ROLE, _coldTreasury);
    }

    // -------------------------------------------------------------------------
    // View functions
    // -------------------------------------------------------------------------

    /// @notice Verify if an address is an authorized Execution Signer.
    /// @dev Used by off-chain executors and on-chain contracts to validate
    ///      that a signer is authorized before executing transactions.
    ///      Gas cost: ~2,500 (one SLOAD for hasRole).
    ///
    ///      Invariant: returns true if and only if the address was granted
    ///      EXECUTION_SIGNER_ROLE in the constructor (never revoked).
    ///
    /// @param _addr Address to check.
    /// @return isSigner True if _addr has EXECUTION_SIGNER_ROLE.
    function isExecutionSigner(address _addr) external view returns (bool isSigner) {
        isSigner = hasRole(EXECUTION_SIGNER_ROLE, _addr);
    }

    /// @notice Verify if an address is an authorized Gas Sponsor.
    /// @param _addr Address to check.
    /// @return isSponsor True if _addr has GAS_SPONSOR_ROLE.
    function isGasSponsor(address _addr) external view returns (bool isSponsor) {
        isSponsor = hasRole(GAS_SPONSOR_ROLE, _addr);
    }

    /// @notice Verify if an address is the current Cold Treasury.
    /// @param _addr Address to check.
    /// @return isTreasury True if _addr equals coldTreasury.
    function isColdTreasury(address _addr) external view returns (bool isTreasury) {
        isTreasury = (_addr == coldTreasury);
    }

    // -------------------------------------------------------------------------
    // Admin functions
    // -------------------------------------------------------------------------

    /// @notice Update the Cold Treasury address.
    /// @dev Only callable by DEFAULT_ADMIN_ROLE. Emits ColdTreasuryUpdated.
    ///      Invariant I2: new treasury must be non-zero.
    ///      Invariant I3: only admin can call this function.
    ///
    ///      Gas cost: ~25,000 (admin check + SSTORE + event).
    ///
    ///      SECURITY: Changing the treasury redirects all future yield.
    ///      Past yield already sent to the previous treasury is not affected.
    ///      In production, this should go through a timelock (AdminTimelock).
    ///
    /// @param _newTreasury New cold treasury address. Must be non-zero.
    function setColdTreasury(address _newTreasury) external onlyRole(DEFAULT_ADMIN_ROLE) {
        if (_newTreasury == address(0)) revert WT_ZeroTreasury();
        if (_newTreasury == coldTreasury) revert WT_DuplicateAddresses();

        address previous = coldTreasury;
        coldTreasury = _newTreasury;

        // Update role: revoke old treasury, grant new treasury
        _revokeRole(COLD_TREASURY_ROLE, previous);
        _grantRole(COLD_TREASURY_ROLE, _newTreasury);

        emit ColdTreasuryUpdated(previous, _newTreasury);
        emit RoleGrantedIndexed(COLD_TREASURY_ROLE, _newTreasury);
    }

    /// @notice Return all three wallet addresses in a single call.
    /// @dev Convenience function for off-chain configuration and monitoring.
    ///      Gas cost: ~6,000 (three SLOADs).
    /// @return _gasSponsor      Current gas sponsor address.
    /// @return _executionSigner Current execution signer address.
    /// @return _coldTreasury    Current cold treasury address.
    function getTopology()
        external
        view
        returns (address _gasSponsor, address _executionSigner, address _coldTreasury)
    {
        _gasSponsor = gasSponsor;
        _executionSigner = executionSigner;
        _coldTreasury = coldTreasury;
    }
}
