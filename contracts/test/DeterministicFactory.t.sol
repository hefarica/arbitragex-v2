// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// DeterministicFactory.t.sol - tests against the REAL src/core/DeterministicFactory.
//
// History: the previous version of this file deployed an embedded STUB contract
// (a throwaway `contract DeterministicFactory { ... }` defined inline) and never
// imported src/core/DeterministicFactory.sol. Its fuzz/invariant runs validated
// the stub, not the production contract - illusory coverage. Worse, the stub's
// deploy() CREATE2'd arbitrary fuzzed bytes as if they were init code, so under
// modern Foundry the suite was 15/24 RED (every "deploy failed"). CI's `|| true`
// hid both facts. This rewrite exercises the real contract with valid init
// bytecode (creationCode ++ encoded constructor args).
// =============================================================================

import "forge-std/Test.sol";
import {
    DeterministicFactory,
    DF_EmptyBytecode,
    DF_UnsupportedChainId,
    DF_EmptyContractName
} from "../src/core/DeterministicFactory.sol";

/// @dev Minimal deployable target with a constructor argument, so the tests use
///      real init bytecode (creationCode ++ abi.encode(arg)) rather than a stub.
contract FactoryTarget {
    uint256 public immutable value;

    constructor(uint256 _value) {
        value = _value;
    }
}

contract DeterministicFactoryTest is Test {
    DeterministicFactory internal factory;

    // Mirror of the factory's supported chain IDs.
    uint256 internal constant ETHEREUM = 1;
    uint256 internal constant ARBITRUM = 42161;
    uint256 internal constant OPTIMISM = 10;
    uint256 internal constant BASE = 8453;
    uint256 internal constant POLYGON = 137;
    uint256 internal constant BSC = 56;

    function setUp() public {
        factory = new DeterministicFactory();
    }

    /// @dev Valid init bytecode for FactoryTarget(value).
    function _code(uint256 v) internal pure returns (bytes memory) {
        return abi.encodePacked(type(FactoryTarget).creationCode, abi.encode(v));
    }

    function _isSupported(uint256 c) internal pure returns (bool) {
        return c == ETHEREUM || c == ARBITRUM || c == OPTIMISM || c == BASE || c == POLYGON || c == BSC;
    }

    // -------------------------------------------------------------------------
    // I1: prediction matches deployment (the core CREATE2 determinism guarantee)
    // -------------------------------------------------------------------------
    function test_I1_PredictedMatchesDeployed() public {
        bytes memory bc = _code(123);
        address predicted = factory.predictAddress(bc, ETHEREUM, "ArbitrageExecutor");
        address deployed = factory.deploy(bc, ETHEREUM, "ArbitrageExecutor");

        assertEq(deployed, predicted, "predicted != deployed");
        assertGt(deployed.code.length, 0, "no code at deployed address");
        assertEq(FactoryTarget(deployed).value(), 123, "constructor arg not applied");
    }

    // -------------------------------------------------------------------------
    // I2: cross-chain addresses differ (salt namespaced by chainId)
    // -------------------------------------------------------------------------
    function test_I2_CrossChainAddressesDiffer() public view {
        bytes memory bc = _code(1);
        address onEth = factory.predictAddress(bc, ETHEREUM, "X");
        address onArb = factory.predictAddress(bc, ARBITRUM, "X");
        assertTrue(onEth != onArb, "same address across chains (replay risk)");
    }

    // -------------------------------------------------------------------------
    // I3: same input -> same address (determinism)
    // -------------------------------------------------------------------------
    function test_I3_SameInputSameAddress() public view {
        bytes memory bc = _code(7);
        assertEq(
            factory.predictAddress(bc, BASE, "Y"), factory.predictAddress(bc, BASE, "Y"), "prediction not deterministic"
        );
    }

    // -------------------------------------------------------------------------
    // I4: different name -> different address
    // -------------------------------------------------------------------------
    function test_I4_DifferentNameDifferentAddress() public view {
        bytes memory bc = _code(1);
        assertTrue(
            factory.predictAddress(bc, POLYGON, "A") != factory.predictAddress(bc, POLYGON, "B"),
            "different names collided"
        );
    }

    // -------------------------------------------------------------------------
    // I5: different bytecode -> different address (same salt)
    // -------------------------------------------------------------------------
    function test_I5_DifferentBytecodeDifferentAddress() public view {
        assertTrue(
            factory.predictAddress(_code(1), BSC, "Z") != factory.predictAddress(_code(2), BSC, "Z"),
            "different bytecode collided"
        );
    }

    // -------------------------------------------------------------------------
    // Input validation
    // -------------------------------------------------------------------------
    function test_RevertOnEmptyBytecode() public {
        vm.expectRevert(DF_EmptyBytecode.selector);
        factory.deploy("", ETHEREUM, "X");
    }

    function test_RevertOnUnsupportedChain() public {
        bytes memory bc = _code(1);
        vm.expectRevert(abi.encodeWithSelector(DF_UnsupportedChainId.selector, uint256(999)));
        factory.deploy(bc, 999, "X");
    }

    function test_RevertOnEmptyName() public {
        bytes memory bc = _code(1);
        vm.expectRevert(DF_EmptyContractName.selector);
        factory.deploy(bc, ETHEREUM, "");
    }

    // -------------------------------------------------------------------------
    // Redeploy with the same salt+bytecode reverts (CREATE2 address occupied).
    // -------------------------------------------------------------------------
    function test_RevertOnDuplicateDeploy() public {
        bytes memory bc = _code(1);
        factory.deploy(bc, ETHEREUM, "Dup");
        // OZ Create2.deploy reverts when the computed address already holds code.
        vm.expectRevert();
        factory.deploy(bc, ETHEREUM, "Dup");
    }

    // -------------------------------------------------------------------------
    // All six supported chains deploy successfully (distinct addresses).
    // -------------------------------------------------------------------------
    function test_AllSupportedChainsDeploy() public {
        uint256[6] memory chains = [ETHEREUM, ARBITRUM, OPTIMISM, BASE, POLYGON, BSC];
        bytes memory bc = _code(99);
        address[6] memory deployed;
        for (uint256 i = 0; i < 6; i++) {
            deployed[i] = factory.deploy(bc, chains[i], "Suite");
            assertGt(deployed[i].code.length, 0, "no code");
            for (uint256 j = 0; j < i; j++) {
                assertTrue(deployed[i] != deployed[j], "chain addresses collided");
            }
        }
    }

    // -------------------------------------------------------------------------
    // Events
    // -------------------------------------------------------------------------
    function test_DeployEmitsDeterministicDeploy() public {
        bytes memory bc = _code(5);
        bytes32 salt = keccak256(abi.encodePacked(ETHEREUM, "Evt"));
        bytes32 bytecodeHash = keccak256(bc);
        address predicted = factory.predictAddress(bc, ETHEREUM, "Evt");

        vm.expectEmit(true, true, true, true, address(factory));
        emit DeterministicFactory.DeterministicDeploy(salt, predicted, bytecodeHash);
        factory.deploy(bc, ETHEREUM, "Evt");
    }

    function test_PredictAndLogEmits() public {
        bytes memory bc = _code(5);
        bytes32 salt = keccak256(abi.encodePacked(ARBITRUM, "Pred"));
        bytes32 bytecodeHash = keccak256(bc);
        address predicted = factory.predictAddress(bc, ARBITRUM, "Pred");

        vm.expectEmit(true, true, true, true, address(factory));
        emit DeterministicFactory.DeploymentPredicted(salt, predicted, bytecodeHash);
        address logged = factory.predictAndLog(bc, ARBITRUM, "Pred");
        assertEq(logged, predicted, "predictAndLog != predictAddress");
    }

    // -------------------------------------------------------------------------
    // Batch operations
    // -------------------------------------------------------------------------
    function test_BatchDeployMatchesBatchPredict() public {
        bytes[] memory bcs = new bytes[](2);
        bcs[0] = _code(11);
        bcs[1] = _code(22);
        uint256[] memory chains = new uint256[](2);
        chains[0] = ETHEREUM;
        chains[1] = OPTIMISM;
        string[] memory names = new string[](2);
        names[0] = "One";
        names[1] = "Two";

        address[] memory predicted = factory.batchPredict(bcs, chains, names);
        address[] memory deployed = factory.batchDeploy(bcs, chains, names);

        assertEq(deployed.length, 2, "wrong length");
        assertEq(deployed[0], predicted[0], "batch[0] mismatch");
        assertEq(deployed[1], predicted[1], "batch[1] mismatch");
        assertEq(FactoryTarget(deployed[0]).value(), 11);
        assertEq(FactoryTarget(deployed[1]).value(), 22);
    }

    function test_RevertBatchDeployLengthMismatch() public {
        bytes[] memory bcs = new bytes[](2);
        bcs[0] = _code(1);
        bcs[1] = _code(2);
        uint256[] memory chains = new uint256[](1);
        chains[0] = ETHEREUM;
        string[] memory names = new string[](2);
        names[0] = "A";
        names[1] = "B";

        vm.expectRevert(DF_EmptyBytecode.selector);
        factory.batchDeploy(bcs, chains, names);
    }

    function test_RevertBatchDeployEmpty() public {
        bytes[] memory bcs = new bytes[](0);
        uint256[] memory chains = new uint256[](0);
        string[] memory names = new string[](0);

        vm.expectRevert(DF_EmptyBytecode.selector);
        factory.batchDeploy(bcs, chains, names);
    }

    // -------------------------------------------------------------------------
    // The factory is permissionless by design (MEDIUM finding in the audit): any
    // address can deploy. This test documents/locks that behavior so a future
    // access-control change is a deliberate, visible diff.
    // -------------------------------------------------------------------------
    function test_DeployIsPermissionless() public {
        bytes memory bc = _code(1);
        address stranger = makeAddr("stranger");
        vm.prank(stranger);
        address deployed = factory.deploy(bc, ETHEREUM, "Open");
        assertGt(deployed.code.length, 0, "stranger could not deploy");
    }

    // -------------------------------------------------------------------------
    // Fuzz: prediction always matches deployment for supported chains.
    // -------------------------------------------------------------------------
    function testFuzz_PredictedMatchesDeployed(uint256 v, uint64 nameSeed) public {
        string memory name = string(abi.encodePacked("C", vm.toString(uint256(nameSeed))));
        bytes memory bc = _code(v);

        address predicted = factory.predictAddress(bc, ETHEREUM, name);
        address deployed = factory.deploy(bc, ETHEREUM, name);

        assertEq(deployed, predicted, "predicted != deployed");
        assertEq(FactoryTarget(deployed).value(), v, "constructor arg mismatch");
    }

    // -------------------------------------------------------------------------
    // Fuzz: any unsupported chainId reverts DF_UnsupportedChainId.
    // -------------------------------------------------------------------------
    function testFuzz_UnsupportedChainReverts(uint256 chainId) public {
        vm.assume(!_isSupported(chainId));
        bytes memory bc = _code(1);
        vm.expectRevert(abi.encodeWithSelector(DF_UnsupportedChainId.selector, chainId));
        factory.deploy(bc, chainId, "X");
    }
}
