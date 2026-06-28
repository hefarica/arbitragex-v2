// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// StorageLayout.t.sol — append-only storage-layout gate for the UUPS proxies.
//
// The UUPS contracts document a linear own-variable slot layout (their OZ parents
// use ERC-7201 namespaced slots and do NOT occupy slots 0..N). Reordering or
// inserting an own state variable would corrupt the layout and brick every live
// proxy on upgrade. These tests pin each own variable to its documented slot via
// vm.load, so ANY layout change fails the (now blocking) Foundry CI. New variables
// must be APPENDED after the last slot — that is the only change that keeps the
// pinned slots below intact.
// =============================================================================

import "forge-std/Test.sol";
import "../src/ArbitrageExecutor.sol";
import "../src/FlashLoanExecutor.sol";
import "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";

contract StorageLayoutTest is Test {
    function _load(address a, uint256 slot) internal view returns (bytes32) {
        return vm.load(a, bytes32(slot));
    }

    function _addr(bytes32 v) internal pure returns (address) {
        return address(uint160(uint256(v)));
    }

    /// @dev address-keyed mapping element slot: keccak256(abi.encode(key, baseSlot)).
    function _mapSlot(address key, uint256 baseSlot) internal pure returns (uint256) {
        return uint256(keccak256(abi.encode(key, baseSlot)));
    }

    // ArbitrageExecutor own layout:
    //   slot 0: approvedRouters   mapping(address => bool)
    //   slot 1: approvedTokens    mapping(address => bool)
    //   slot 2: allowanceManager  address
    //   slot 3: approvedSelectors mapping(address => mapping(bytes4 => bool))  [trailing/append target]
    function test_ArbitrageExecutor_StorageLayout() public {
        ArbitrageExecutor impl = new ArbitrageExecutor();
        ArbitrageExecutor ae = ArbitrageExecutor(payable(address(new ERC1967Proxy(
            address(impl),
            abi.encodeWithSelector(ArbitrageExecutor.initialize.selector, address(this))
        ))));

        // slot 2: allowanceManager (directly readable). Insertion before it shifts this.
        address am = makeAddr("am");
        ae.setAllowanceManager(am);
        assertEq(_addr(_load(address(ae), 2)), am, "allowanceManager must occupy slot 2");

        // slot 0: approvedRouters mapping base.
        address router = makeAddr("router");
        ae.setRouterApproval(router, true);
        assertEq(uint256(_load(address(ae), _mapSlot(router, 0))), 1, "approvedRouters must be at slot 0");

        // slot 1: approvedTokens mapping base.
        address tok = makeAddr("tok");
        ae.setTokenApproval(tok, true);
        assertEq(uint256(_load(address(ae), _mapSlot(tok, 1))), 1, "approvedTokens must be at slot 1");

        // slot 3: approvedSelectors nested mapping base (address => (bytes4 => bool)).
        bytes4 sel = bytes4(0x12345678);
        ae.setRouterSelectorApproval(router, sel, true);
        bytes32 innerBase = keccak256(abi.encode(router, uint256(3)));
        uint256 leafSlot = uint256(keccak256(abi.encode(sel, innerBase)));
        assertEq(uint256(_load(address(ae), leafSlot)), 1, "approvedSelectors must be at slot 3");
    }

    // FlashLoanExecutor TRUE (packed) own layout. NB: the in-contract slot comment
    // lists DECLARATION indices (referralCode "slot 2"), but Solidity PACKS the
    // 2-byte uint16 referralCode into the 12 free bytes of slot 1 alongside the
    // arbitrageExecutor address — so the real layout is only 4 slots, not 5. The
    // append-only invariant still holds (append after balancerVault). The contract
    // comment should be corrected to reflect packing (doc-only follow-up).
    //   slot 0: aavePool          address
    //   slot 1: arbitrageExecutor address (bits 0..159) + referralCode uint16 (bits 160..175)
    //   slot 2: flashLoanProvider address
    //   slot 3: balancerVault     address  [trailing/append target]
    function test_FlashLoanExecutor_StorageLayout() public {
        address aavePool = makeAddr("aavePool");
        address arbExec = makeAddr("arbExec");
        FlashLoanExecutor impl = new FlashLoanExecutor();
        FlashLoanExecutor fl = FlashLoanExecutor(payable(address(new ERC1967Proxy(
            address(impl),
            abi.encodeWithSelector(FlashLoanExecutor.initialize.selector, address(this), aavePool, arbExec)
        ))));

        assertEq(_addr(_load(address(fl), 0)), aavePool, "aavePool must occupy slot 0");
        assertEq(_addr(_load(address(fl), 1)), arbExec, "arbitrageExecutor must occupy slot 1 (low 160 bits)");

        // referralCode packs into slot 1, bits 160..175.
        fl.setReferralCode(7);
        uint256 slot1 = uint256(_load(address(fl), 1));
        assertEq((slot1 >> 160) & 0xffff, 7, "referralCode must pack into slot 1 bits 160..175");
        assertEq(_addr(bytes32(slot1)), arbExec, "arbitrageExecutor must still occupy slot 1 low 160 bits");

        address provider = makeAddr("provider");
        fl.setFlashLoanProvider(provider);
        assertEq(_addr(_load(address(fl), 2)), provider, "flashLoanProvider must occupy slot 2");

        address vault = makeAddr("vault");
        fl.setBalancerVault(vault);
        assertEq(_addr(_load(address(fl), 3)), vault, "balancerVault must occupy slot 3");
    }
}
