// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Test.sol";
import "../src/FlashLoanExecutor.sol";
import "@openzeppelin/contracts/token/ERC20/ERC20.sol";

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

/// @dev Mock ERC20 for flash loan asset
contract MockERC20FL is ERC20 {
    constructor() ERC20("FLToken", "FLT") {}

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}

/// @dev Mock Aave V3 Pool: records calls, does not move funds
contract MockAavePool {
    address public lastReceiver;
    address public lastAsset;
    uint256 public lastAmount;
    bytes public lastParams;

    function flashLoanSimple(
        address receiverAddress,
        address asset,
        uint256 amount,
        bytes calldata params,
        uint16 /*referralCode*/
    ) external {
        lastReceiver = receiverAddress;
        lastAsset = asset;
        lastAmount = amount;
        lastParams = params;
        // In a real Aave, the pool would call executeOperation.
        // We do NOT simulate the callback here — tested separately.
    }
}

/// @dev Mock ArbitrageExecutor: records that it was called and succeeds
contract MockArbitrageExecutor {
    bool public wasCalled;

    fallback() external {
        wasCalled = true;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

contract FlashLoanExecutorTest is Test {
    FlashLoanExecutor internal flashExec;
    MockAavePool internal pool;
    MockERC20FL internal token;
    MockArbitrageExecutor internal arbExec;

    address internal admin;
    address internal executorRole;
    address internal attacker;

    function setUp() public {
        admin = address(this);
        executorRole = makeAddr("executorRole");
        attacker = makeAddr("attacker");

        pool = new MockAavePool();
        token = new MockERC20FL();
        arbExec = new MockArbitrageExecutor();

        flashExec = new FlashLoanExecutor(address(pool), address(arbExec));

        // Grant EXECUTOR_ROLE so requestFlashLoan can be called
        flashExec.grantRole(flashExec.EXECUTOR_ROLE(), executorRole);

        // Mint tokens to flash loan executor to cover repayment in the callback test
        token.mint(address(flashExec), 10_000e18);
    }

    // -----------------------------------------------------------------------
    // testReceiveFlashLoan_Authorized
    // Simulates Aave calling executeOperation from the authorized pool address.
    // The callback must not revert and must return true.
    // -----------------------------------------------------------------------
    function testReceiveFlashLoan_Authorized() public {
        uint256 loanAmount = 1_000e18;
        uint256 premium = 1e18; // 0.1% fee
        bytes memory params = abi.encodeWithSignature("noop()");

        // Simulate Aave pool calling the callback
        vm.prank(address(pool));
        bool result = flashExec.executeOperation(
            address(token),
            loanAmount,
            premium,
            address(flashExec), // initiator must be this contract
            params
        );

        assertTrue(result, "executeOperation must return true for authorized caller");
    }

    // -----------------------------------------------------------------------
    // testReceiveFlashLoan_RejectsUnauthorized
    // Callback from an address that is NOT the registered Aave pool must revert.
    // -----------------------------------------------------------------------
    function testReceiveFlashLoan_RejectsUnauthorized() public {
        uint256 loanAmount = 1_000e18;
        uint256 premium = 1e18;
        bytes memory params = "";

        vm.expectRevert("Caller must be AavePool");

        vm.prank(attacker);
        flashExec.executeOperation(
            address(token),
            loanAmount,
            premium,
            address(flashExec), // even with correct initiator — caller is wrong
            params
        );
    }
}
