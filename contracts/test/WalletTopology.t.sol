// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// WalletTopology.t.sol - coverage for src/core/WalletTopology (previously ZERO).
//
// Covers constructor validation + role grants, the three view helpers,
// admin-gated setColdTreasury (including role migration old->new), and the
// fail-honest reverts (zero / duplicate / non-admin). These are the segregation-
// of-funds guarantees the system relies on, so they must be tested, not assumed.
// =============================================================================

import "forge-std/Test.sol";
import {WalletTopology, WT_ZeroAddress, WT_DuplicateAddresses, WT_ZeroTreasury} from "../src/core/WalletTopology.sol";

contract WalletTopologyTest is Test {
    WalletTopology internal topo;

    address internal gasSponsor;
    address internal executionSigner;
    address internal coldTreasury;

    // Mirror role ids.
    bytes32 internal constant GAS_SPONSOR_ROLE = keccak256("GAS_SPONSOR_ROLE");
    bytes32 internal constant EXECUTION_SIGNER_ROLE = keccak256("EXECUTION_SIGNER_ROLE");
    bytes32 internal constant COLD_TREASURY_ROLE = keccak256("COLD_TREASURY_ROLE");

    function setUp() public {
        gasSponsor = makeAddr("gasSponsor");
        executionSigner = makeAddr("executionSigner");
        coldTreasury = makeAddr("coldTreasury");
        // Deployer (this test contract) receives DEFAULT_ADMIN_ROLE.
        topo = new WalletTopology(gasSponsor, executionSigner, coldTreasury);
    }

    // -------------------------------------------------------------------------
    // Constructor: state + roles
    // -------------------------------------------------------------------------
    function test_ConstructorSetsStateAndRoles() public view {
        assertEq(topo.gasSponsor(), gasSponsor, "gasSponsor");
        assertEq(topo.executionSigner(), executionSigner, "executionSigner");
        assertEq(topo.coldTreasury(), coldTreasury, "coldTreasury");

        assertTrue(topo.hasRole(GAS_SPONSOR_ROLE, gasSponsor), "gasSponsor role");
        assertTrue(topo.hasRole(EXECUTION_SIGNER_ROLE, executionSigner), "signer role");
        assertTrue(topo.hasRole(COLD_TREASURY_ROLE, coldTreasury), "treasury role");
        assertTrue(topo.hasRole(0x00, address(this)), "deployer is admin");
    }

    function test_GetTopologyReturnsAll() public view {
        (address gs, address es, address ct) = topo.getTopology();
        assertEq(gs, gasSponsor);
        assertEq(es, executionSigner);
        assertEq(ct, coldTreasury);
    }

    function test_ViewHelpers() public view {
        assertTrue(topo.isGasSponsor(gasSponsor));
        assertFalse(topo.isGasSponsor(executionSigner));
        assertTrue(topo.isExecutionSigner(executionSigner));
        assertFalse(topo.isExecutionSigner(gasSponsor));
        assertTrue(topo.isColdTreasury(coldTreasury));
        assertFalse(topo.isColdTreasury(gasSponsor));
    }

    // I1 (documented): at construction the three operational roles are held by
    // three distinct addresses - none overlaps another's role.
    function test_RolesAreSegregatedAtConstruction() public view {
        assertFalse(topo.hasRole(EXECUTION_SIGNER_ROLE, gasSponsor));
        assertFalse(topo.hasRole(COLD_TREASURY_ROLE, gasSponsor));
        assertFalse(topo.hasRole(GAS_SPONSOR_ROLE, executionSigner));
        assertFalse(topo.hasRole(COLD_TREASURY_ROLE, executionSigner));
        assertFalse(topo.hasRole(GAS_SPONSOR_ROLE, coldTreasury));
        assertFalse(topo.hasRole(EXECUTION_SIGNER_ROLE, coldTreasury));
    }

    function test_ConstructorEmitsWalletConfigured() public {
        address gs = makeAddr("gs2");
        address es = makeAddr("es2");
        address ct = makeAddr("ct2");
        vm.expectEmit(true, true, true, true);
        emit WalletTopology.WalletConfigured(gs, es, ct);
        new WalletTopology(gs, es, ct);
    }

    // -------------------------------------------------------------------------
    // Constructor: validation reverts
    // -------------------------------------------------------------------------
    function test_RevertZeroGasSponsor() public {
        vm.expectRevert(WT_ZeroAddress.selector);
        new WalletTopology(address(0), executionSigner, coldTreasury);
    }

    function test_RevertZeroExecutionSigner() public {
        vm.expectRevert(WT_ZeroAddress.selector);
        new WalletTopology(gasSponsor, address(0), coldTreasury);
    }

    function test_RevertZeroColdTreasury() public {
        vm.expectRevert(WT_ZeroAddress.selector);
        new WalletTopology(gasSponsor, executionSigner, address(0));
    }

    function test_RevertDuplicateSponsorSigner() public {
        vm.expectRevert(WT_DuplicateAddresses.selector);
        new WalletTopology(gasSponsor, gasSponsor, coldTreasury);
    }

    function test_RevertDuplicateSponsorTreasury() public {
        vm.expectRevert(WT_DuplicateAddresses.selector);
        new WalletTopology(gasSponsor, executionSigner, gasSponsor);
    }

    function test_RevertDuplicateSignerTreasury() public {
        vm.expectRevert(WT_DuplicateAddresses.selector);
        new WalletTopology(gasSponsor, executionSigner, executionSigner);
    }

    // -------------------------------------------------------------------------
    // setColdTreasury
    // -------------------------------------------------------------------------
    function test_SetColdTreasuryMovesRole() public {
        address newTreasury = makeAddr("newTreasury");
        topo.setColdTreasury(newTreasury);

        assertEq(topo.coldTreasury(), newTreasury, "treasury not updated");
        assertTrue(topo.hasRole(COLD_TREASURY_ROLE, newTreasury), "new lacks role");
        assertFalse(topo.hasRole(COLD_TREASURY_ROLE, coldTreasury), "old kept role");
        assertTrue(topo.isColdTreasury(newTreasury));
        assertFalse(topo.isColdTreasury(coldTreasury));
    }

    function test_SetColdTreasuryEmits() public {
        address newTreasury = makeAddr("newTreasury");
        vm.expectEmit(true, true, false, false, address(topo));
        emit WalletTopology.ColdTreasuryUpdated(coldTreasury, newTreasury);
        topo.setColdTreasury(newTreasury);
    }

    function test_RevertSetColdTreasuryZero() public {
        vm.expectRevert(WT_ZeroTreasury.selector);
        topo.setColdTreasury(address(0));
    }

    function test_RevertSetColdTreasurySameAddress() public {
        address current = topo.coldTreasury();
        vm.expectRevert(WT_DuplicateAddresses.selector);
        topo.setColdTreasury(current);
    }

    function test_RevertSetColdTreasuryNonAdmin() public {
        address stranger = makeAddr("stranger");
        address newTreasury = makeAddr("newTreasury");
        vm.prank(stranger);
        // OZ AccessControl reverts AccessControlUnauthorizedAccount for non-admin.
        vm.expectRevert();
        topo.setColdTreasury(newTreasury);
    }

    // -------------------------------------------------------------------------
    // Fuzz: any distinct, non-zero triple constructs cleanly with correct roles.
    // -------------------------------------------------------------------------
    function testFuzz_ConstructorDistinctTriple(address a, address b, address c) public {
        vm.assume(a != address(0) && b != address(0) && c != address(0));
        vm.assume(a != b && a != c && b != c);

        WalletTopology t = new WalletTopology(a, b, c);
        assertTrue(t.hasRole(GAS_SPONSOR_ROLE, a));
        assertTrue(t.hasRole(EXECUTION_SIGNER_ROLE, b));
        assertTrue(t.hasRole(COLD_TREASURY_ROLE, c));
        assertEq(t.coldTreasury(), c);
    }
}
