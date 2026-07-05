// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {MakerDaoDssFlashAdapter} from "../src/flashloans/MakerDaoDssFlashAdapter.sol";

/**
 * @title MakerDaoDssFlashAdapter.t.sol
 * @notice FASE 4 — unit tests for the MakerDAO DSS Flash adapter. Mock-based
 *         (no fork, no real DAI) so it runs in the non-fork CI job. Mirrors the
 *         FlashLoanAdapters.t.sol pattern (constructor-reject-zero, delegates,
 *         fee-correct, max-correct, stores-pool).
 */
contract MockDssFlash {
    address public lastReceiver;
    uint256 public lastAmount;
    bytes public lastData;

    function flashLoan(address receiver, uint256 amount, bytes calldata data) external {
        lastReceiver = receiver;
        lastAmount = amount;
        lastData = data;
    }
}

contract MockERC20 {
    mapping(address => uint256) private _bal;
    string public symbol;

    constructor(string memory sym) {
        symbol = sym;
    }

    function balanceOf(address a) external view returns (uint256) {
        return _bal[a];
    }

    function mint(address to, uint256 amt) external {
        _bal[to] += amt;
    }
}

contract MakerDaoDssFlashAdapterTest is Test {
    MockDssFlash internal mock;
    MockERC20 internal dai;
    MakerDaoDssFlashAdapter internal adapter;

    function setUp() public {
        mock = new MockDssFlash();
        dai = new MockERC20("DAI");
        adapter = new MakerDaoDssFlashAdapter(address(mock));
    }

    function testConstructor_RejectsZeroDssFlash() public {
        vm.expectRevert("MakerDaoDssFlashAdapter: zero dssFlash");
        new MakerDaoDssFlashAdapter(address(0));
    }

    function testConstructor_StoresDssFlash() public view {
        assertEq(address(adapter.dssFlash()), address(mock));
    }

    function testFlashLoan_DelegatesToDssFlash() public {
        bytes memory params = abi.encode("arb-payload");
        adapter.flashLoan(address(0xBEEF), address(dai), 1_000 ether, params);
        assertEq(mock.lastReceiver(), address(0xBEEF));
        assertEq(mock.lastAmount(), 1_000 ether);
        assertEq(keccak256(mock.lastData()), keccak256(params));
    }

    function testFlashLoan_RevertsOnZeroAsset() public {
        vm.expectRevert("MakerDaoDssFlashAdapter: zero asset");
        adapter.flashLoan(address(this), address(0), 100, "");
    }

    function testFlashLoanFee_ReturnsZero() public view {
        // DSS Flash fee is 0 within the free window — adapter reports 0 so the
        // off-chain ranker treats it as a zero-fee provider (fee config lives
        // in contract_registry.metadata.fee_bps, no-hardcode on-chain).
        assertEq(adapter.flashLoanFee(1_000_000 ether), 0);
        assertEq(adapter.flashLoanFee(0), 0);
    }

    function testMaxFlashLoan_ReturnsDssFlashDaiBalance() public {
        // maxFlashLoan approximates with DssFlash's own DAI balance (safe lower bound).
        dai.mint(address(mock), 500 ether);
        assertEq(adapter.maxFlashLoan(address(dai)), 500 ether);
    }

    function testMaxFlashLoan_ZeroWhenDssFlashEmpty() public {
        assertEq(adapter.maxFlashLoan(address(dai)), 0);
    }
}
