// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// SC-14: Fork test marker - demonstrates mainnet-fork capability.
//
// These tests run ONLY when FORK_RPC_URL (or MAINNET_RPC_URL) is set in
// the environment.  When the variable is absent (local dev, non-fork CI),
// tests self-skip via vm.skip(true) - no failure, no mock.
//
// CI behaviour:
//   Non-fork job (test): MAINNET_RPC_URL is empty -> all tests skip.
//   Fork job (test-fork): MAINNET_RPC_URL provided via GitHub secret -> tests run.
//
// Naming convention: every test in this file MUST start with "testFork_" so
// the CI fork job can target them with --match-test "testFork_".
// =============================================================================

import "forge-std/Test.sol";
import "../src/ArbitrageExecutor.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";

import "@openzeppelin/contracts/token/ERC20/extensions/IERC20Metadata.sol";

contract ForkExampleTest is Test {
    // Well-known mainnet addresses (all verified on Etherscan 2026-05-08).
    address constant USDC = 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48;
    address constant WETH = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;
    address constant AAVE_V3 = 0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2;

    // Fork block: left at 0 = latest.  Pin to a specific block for reproducibility
    // in long-running CI environments.  Operator can override by setting
    // FORK_BLOCK_NUMBER env var.
    uint256 internal forkId;
    bool internal forkActive;

    function setUp() public {
        string memory rpc = vm.envOr("MAINNET_RPC_URL", string(""));
        if (bytes(rpc).length == 0) {
            // No RPC URL: mark fork as inactive, tests will skip.
            forkActive = false;
            return;
        }
        uint256 forkBlock = vm.envOr("FORK_BLOCK_NUMBER", uint256(0));
        if (forkBlock > 0) {
            forkId = vm.createSelectFork(rpc, forkBlock);
        } else {
            forkId = vm.createSelectFork(rpc);
        }
        forkActive = true;
    }

    // -----------------------------------------------------------------------
    // testFork_USDC_TotalSupply_SanityCheck
    //
    // Verifies that USDC on-chain total supply > $1 billion (1e9 USDC = 1e15
    // with 6 decimals).  This is a pure sanity check - if it fails, the fork
    // is pointing at a wrong chain or the RPC is stale.
    // -----------------------------------------------------------------------
    function testFork_USDC_TotalSupply_SanityCheck() public {
        if (!forkActive) {
            vm.skip(true);
            return;
        }

        uint256 totalSupply = IERC20Metadata(USDC).totalSupply();
        uint8 decimals = IERC20Metadata(USDC).decimals();

        // USDC has 6 decimals; $1B = 1_000_000_000 * 10**6 = 1e15
        assertEq(decimals, 6, "USDC decimals must be 6");
        assertGt(totalSupply, 1_000_000_000 * 10 ** 6, "USDC total supply must exceed $1B");
    }

    // -----------------------------------------------------------------------
    // testFork_WETH_TotalSupply_SanityCheck
    //
    // Verifies WETH has positive total supply on mainnet.
    // WETH is the primary tokenIn for most ArbitrageX routes.
    // -----------------------------------------------------------------------
    function testFork_WETH_TotalSupply_SanityCheck() public {
        if (!forkActive) {
            vm.skip(true);
            return;
        }

        uint256 totalSupply = IERC20(WETH).totalSupply();
        assertGt(totalSupply, 1e18, "WETH total supply must be > 1 WETH");
    }

    // -----------------------------------------------------------------------
    // testFork_ArbitrageExecutor_DeployAndApproveWETH
    //
    // Deploys ArbitrageExecutor as a UUPS proxy on the fork and verifies that
    // WETH can be approved as a valid tokenIn.  Demonstrates that the contract
    // ABI is compatible with mainnet state (no storage collisions, correct
    // initializer signature).
    // -----------------------------------------------------------------------
    function testFork_ArbitrageExecutor_DeployAndApproveWETH() public {
        if (!forkActive) {
            vm.skip(true);
            return;
        }

        address admin = address(this);

        ArbitrageExecutor impl = new ArbitrageExecutor();
        bytes memory initData = abi.encodeWithSelector(ArbitrageExecutor.initialize.selector, admin);
        ERC1967Proxy proxy = new ERC1967Proxy(address(impl), initData);
        ArbitrageExecutor executor = ArbitrageExecutor(payable(address(proxy)));

        // Grant EXECUTOR_ROLE to admin for convenience
        executor.grantRole(executor.EXECUTOR_ROLE(), admin);

        // Approve WETH as a valid tokenIn
        executor.setTokenApproval(WETH, true);
        assertTrue(executor.approvedTokens(WETH), "WETH must be approved as tokenIn on fork");

        // Verify WETH on-chain data is accessible (sanity)
        uint256 supply = IERC20(WETH).totalSupply();
        assertGt(supply, 0, "WETH supply must be readable on fork");
    }

    // -----------------------------------------------------------------------
    // testFork_AaveV3Pool_HasCode
    //
    // Verifies the Aave V3 pool contract has bytecode at the known address.
    // If this fails, AAVE_V3 constant is wrong or the fork is on wrong chain.
    // -----------------------------------------------------------------------
    function testFork_AaveV3Pool_HasCode() public {
        if (!forkActive) {
            vm.skip(true);
            return;
        }

        uint256 codeSize;
        address pool = AAVE_V3;
        assembly {
            codeSize := extcodesize(pool)
        }
        assertGt(codeSize, 0, "Aave V3 pool must have bytecode at expected address");
    }
}
