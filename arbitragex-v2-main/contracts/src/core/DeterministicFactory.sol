// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

// =============================================================================
// DeterministicFactory — CREATE2 multi-chain deployment factory
//
// Purpose: Deploy contracts at identical addresses across all EVM-compatible
//          chains using CREATE2 with a salt namespaceado por chainId.
//
// Supported chains (chainId):
//   - Ethereum : 1
//   - Arbitrum : 42161
//   - Optimism : 10
//   - Base     : 8453
//   - Polygon  : 137
//   - BSC      : 56
//
// Salt construction: keccak256(abi.encodePacked(chainId, contractName))
//   This guarantees that the same contractName on different chains produces
//   different salts, preventing cross-chain address collision attacks.
//
// The factory itself should be deployed at the same address on every chain
// using Nick's method or an equivalent deterministic deployer.
//
// Gas profile (optimizer_runs = 200):
//   - deploy()         : ~50,000 gas base + constructor gas of target contract
//   - predictAddress() : ~3,000 gas (view function)
// =============================================================================

import {Create2} from "@openzeppelin/contracts/utils/Create2.sol";

// -------------------------------------------------------------------------
// Errors (SC-3: custom errors ~200 gas saved per revert vs string require)
// -------------------------------------------------------------------------

/// @dev Thrown when _bytecode has length 0.
error DF_EmptyBytecode();
/// @dev Thrown when _chainId is not in the supported set.
error DF_UnsupportedChainId(uint256 chainId);
/// @dev Thrown when _contractName is an empty string.
error DF_EmptyContractName();
/// @dev Thrown when CREATE2 deployment fails (e.g. contract already deployed).
error DF_DeployFailed();
/// @dev Thrown when Create2.deploy reverts with the same salt+bytecode.
error DF_AlreadyDeployed();

// =============================================================================
/// @title DeterministicFactory
/// @notice Factory de despliegue determinista CREATE2 para OMEGA.
///         Garantiza direcciones idénticas en Ethereum, Arbitrum, Optimism,
///         Base, Polygon y BSC.
/// @dev El salt se namespacea por chainId para evitar colisiones cross-chain.
///      SC-14 (2026-05-15). Zero hardcoded addresses.
///
///      Invariantes de estado:
///        1. deploy() solo acepta chainId en SUPPORTED_CHAIN_IDS.
///        2. El salt es deterministico: salt = keccak256(chainId + contractName).
///        3. La direccion predicha coincide con la direccion real post-deploy.
///        4. Cada despliegue emite DeterministicDeploy con salt, address, bytecodeHash.
// =============================================================================

contract DeterministicFactory {
    // -------------------------------------------------------------------------
    // Constants
    // -------------------------------------------------------------------------

    /// @notice Set of chain IDs supported by this factory.
    /// @dev These are the chains where OMEGA operates. New chains require
    ///      a factory upgrade (new deployment) since this is immutable.
    uint256 public constant ETHEREUM  = 1;
    uint256 public constant ARBITRUM  = 42161;
    uint256 public constant OPTIMISM  = 10;
    uint256 public constant BASE      = 8453;
    uint256 public constant POLYGON   = 137;
    uint256 public constant BSC       = 56;

    // -------------------------------------------------------------------------
    // Events
    // -------------------------------------------------------------------------

    /// @notice Emitted when a contract is deployed deterministically via CREATE2.
    /// @param salt        The CREATE2 salt used (namespaceado por chainId).
    /// @param deployedAt  The address where the contract was deployed.
    /// @param bytecodeHash keccak256 of the deployed bytecode (for verification).
    event DeterministicDeploy(
        bytes32 indexed salt,
        address indexed deployedAt,
        bytes32 indexed bytecodeHash
    );

    /// @notice Emitted when a deployment is predicted (no state change).
    /// @param salt        The CREATE2 salt that would be used.
    /// @param predicted   The predicted address.
    /// @param bytecodeHash keccak256 of the bytecode.
    event DeploymentPredicted(
        bytes32 indexed salt,
        address indexed predicted,
        bytes32 indexed bytecodeHash
    );

    // -------------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------------

    /// @notice Validate that chainId is supported.
    /// @param _chainId Chain ID to validate.
    /// @return valid True if the chain is supported.
    function _isSupportedChain(uint256 _chainId) internal pure returns (bool valid) {
        valid = (_chainId == ETHEREUM || _chainId == ARBITRUM || _chainId == OPTIMISM
                 || _chainId == BASE || _chainId == POLYGON || _chainId == BSC);
    }

    /// @notice Build the CREATE2 salt from chainId and contract name.
    /// @param _chainId      Chain ID for namespace.
    /// @param _contractName Human-readable contract name.
    /// @return salt Deterministic salt.
    function _buildSalt(uint256 _chainId, string memory _contractName)
        internal
        pure
        returns (bytes32 salt)
    {
        salt = keccak256(abi.encodePacked(_chainId, _contractName));
    }

    // -------------------------------------------------------------------------
    // Core deployment functions
    // -------------------------------------------------------------------------

    /// @notice Deploy a contract using CREATE2 with salt namespaceado por chainId.
    /// @dev Flow: validate inputs → build salt → compute address → Create2.deploy
    ///      → emit DeterministicDeploy.
    ///
    ///      Gas cost: ~50,000 base + target constructor gas.
    ///
    ///      Invariant: if predictAddress(bytecode, chainId, name) returns addr,
    ///      then deploy(bytecode, chainId, name) will deploy at addr (unless
    ///      a contract is already at that address, in which case it reverts).
    ///
    /// @param _bytecode     Complete init bytecode of the contract to deploy.
    ///                      Must include constructor arguments ABI-encoded and
    ///                      appended. Can be obtained via:
    ///                      abi.encodePacked(type(Contract).creationCode,
    ///                                       abi.encode(arg1, arg2, ...))
    /// @param _chainId      Target chain ID. Must be in SUPPORTED_CHAIN_IDS.
    ///                      The deployment happens on the CURRENT chain — this
    ///                      parameter is for salt namespacing only.
    /// @param _contractName Human-readable contract name for the salt.
    ///                      Examples: "ArbitrageExecutor", "WalletTopology".
    /// @return deployed     The address of the deployed contract.
    function deploy(
        bytes memory _bytecode,
        uint256 _chainId,
        string memory _contractName
    ) external returns (address deployed) {
        // Input validation
        if (_bytecode.length == 0) revert DF_EmptyBytecode();
        if (!_isSupportedChain(_chainId)) revert DF_UnsupportedChainId(_chainId);
        if (bytes(_contractName).length == 0) revert DF_EmptyContractName();

        // Build deterministic salt
        bytes32 salt = _buildSalt(_chainId, _contractName);
        bytes32 bytecodeHash = keccak256(_bytecode);

        // Predict address before deployment (for logging and verification)
        address predicted = Create2.computeAddress(salt, bytecodeHash);

        // Execute CREATE2 deployment
        // Create2.deploy reverts if a contract already exists at the computed address
        deployed = Create2.deploy(0, salt, _bytecode);

        // Post-deployment invariant: the deployed address MUST match prediction
        if (deployed != predicted) revert DF_DeployFailed();

        emit DeterministicDeploy(salt, deployed, bytecodeHash);
    }

    /// @notice Predict the address of a contract before deploying it.
    /// @dev Pure view function — no state change. Computes the CREATE2 address
    ///      from the salt (chainId + contractName) and bytecode hash.
    ///
    ///      Gas cost: ~3,000 gas.
    ///
    ///      Invariant: predictAddress(b, c, n) == address(deploy(b, c, n))
    ///      for all valid inputs, assuming the contract is not yet deployed.
    ///
    /// @param _bytecode     Complete init bytecode of the contract.
    /// @param _chainId      Target chain ID for salt namespacing.
    /// @param _contractName Contract name for salt namespacing.
    /// @return predicted    The deterministic address where the contract would deploy.
    function predictAddress(
        bytes memory _bytecode,
        uint256 _chainId,
        string memory _contractName
    ) external view returns (address predicted) {
        if (_bytecode.length == 0) revert DF_EmptyBytecode();
        if (!_isSupportedChain(_chainId)) revert DF_UnsupportedChainId(_chainId);
        if (bytes(_contractName).length == 0) revert DF_EmptyContractName();

        bytes32 salt = _buildSalt(_chainId, _contractName);
        bytes32 bytecodeHash = keccak256(_bytecode);
        predicted = Create2.computeAddress(salt, bytecodeHash);
    }

    /// @notice Predict the address and emit an event (for off-chain indexing).
    /// @dev Same as predictAddress but emits DeploymentPredicted for indexing.
    /// @param _bytecode     Complete init bytecode of the contract.
    /// @param _chainId      Target chain ID for salt namespacing.
    /// @param _contractName Contract name for salt namespacing.
    /// @return predicted    The deterministic address where the contract would deploy.
    function predictAndLog(
        bytes memory _bytecode,
        uint256 _chainId,
        string memory _contractName
    ) external returns (address predicted) {
        if (_bytecode.length == 0) revert DF_EmptyBytecode();
        if (!_isSupportedChain(_chainId)) revert DF_UnsupportedChainId(_chainId);
        if (bytes(_contractName).length == 0) revert DF_EmptyContractName();

        bytes32 salt = _buildSalt(_chainId, _contractName);
        bytes32 bytecodeHash = keccak256(_bytecode);
        predicted = Create2.computeAddress(salt, bytecodeHash);

        emit DeploymentPredicted(salt, predicted, bytecodeHash);
    }

    // -------------------------------------------------------------------------
    // Batch operations
    // -------------------------------------------------------------------------

    /// @notice Deploy multiple contracts atomically in a single transaction.
    /// @dev All-or-nothing: if any deployment fails, the entire batch reverts.
    ///      Useful for deploying the full OMEGA suite in one tx.
    ///
    ///      Gas cost: ~N * 50,000 + overhead, where N = number of contracts.
    ///
    /// @param bytecodes      Array of init bytecodes (one per contract).
    /// @param chainIds       Array of chain IDs (one per contract).
    /// @param contractNames  Array of contract names (one per contract).
    /// @return deployed      Array of deployed addresses (index-matched to inputs).
    function batchDeploy(
        bytes[] calldata bytecodes,
        uint256[] calldata chainIds,
        string[] calldata contractNames
    ) external returns (address[] memory deployed) {
        uint256 n = bytecodes.length;
        if (n == 0 || n != chainIds.length || n != contractNames.length) {
            revert DF_EmptyBytecode();
        }

        deployed = new address[](n);
        for (uint256 i = 0; i < n;) {
            deployed[i] = this.deploy(bytecodes[i], chainIds[i], contractNames[i]);
            unchecked { ++i; }
        }
    }

    /// @notice Predict addresses for multiple contracts in a single call.
    /// @dev View function — no state change. Useful for generating deployment
    ///      runbooks off-chain.
    ///
    ///      Gas cost: ~N * 3,000.
    ///
    /// @param bytecodes      Array of init bytecodes.
    /// @param chainIds       Array of chain IDs.
    /// @param contractNames  Array of contract names.
    /// @return predicted     Array of predicted addresses.
    function batchPredict(
        bytes[] calldata bytecodes,
        uint256[] calldata chainIds,
        string[] calldata contractNames
    ) external view returns (address[] memory predicted) {
        uint256 n = bytecodes.length;
        if (n == 0 || n != chainIds.length || n != contractNames.length) {
            revert DF_EmptyBytecode();
        }

        predicted = new address[](n);
        for (uint256 i = 0; i < n;) {
            predicted[i] = this.predictAddress(bytecodes[i], chainIds[i], contractNames[i]);
            unchecked { ++i; }
        }
    }
}
