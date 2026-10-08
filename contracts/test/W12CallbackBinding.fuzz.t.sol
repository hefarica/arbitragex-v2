// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// W12-F25-CALLBACK-BINDING-01 — fuzz del binding del callback (FIX A) y del piso de
// beneficio atomico (FIX C). Corre SOLO contra el arbol parcheado; NO toca produccion.
//
// Propiedades que se prueban:
//   (a) un callback NO solicitado REVIERTE (no existe peticion en vuelo);
//   (b) el balance PREVIO del wrapper queda intacto en ese revert;
//   (c) el piso de beneficio se satisface ATOMICAMENTE en la misma transaccion:
//       exito sii el beneficio devuelto por la ruta alcanza el piso.

import "forge-std/Test.sol";
import "../src/FlashLoanExecutor.sol";
import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";

contract FuzzToken is ERC20 {
    constructor() ERC20("FuzzToken", "FZT") {}

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}

/// @dev Proveedor que ejecuta un round trip REAL: transfiere -> callback -> cobra.
///      Su `triggerCallback` NO transfiere: sirve para el callback NO solicitado.
contract RoundTripProvider {
    function flashLoan(address receiver, address asset, uint256 amount, bytes calldata params) external {
        FuzzToken(asset).transfer(receiver, amount);
        IERC20[] memory tokens = new IERC20[](1);
        tokens[0] = IERC20(asset);
        uint256[] memory amounts = new uint256[](1);
        amounts[0] = amount;
        uint256[] memory fees = new uint256[](1);
        fees[0] = 0;
        // El reembolso lo hace el PROPIO contrato con safeTransfer a msg.sender,
        // que en el callback es este proveedor. No hace falta transferFrom.
        FlashLoanExecutor(receiver).receiveFlashLoan(tokens, amounts, fees, params);
    }

    /// @dev Callback sin prestamo previo: el atacante hace que el Vault "real" invoque.
    function triggerCallback(address receiver, address asset, uint256 amount, bytes calldata userData) external {
        IERC20[] memory tokens = new IERC20[](1);
        tokens[0] = IERC20(asset);
        uint256[] memory amounts = new uint256[](1);
        amounts[0] = amount;
        uint256[] memory fees = new uint256[](1);
        fees[0] = 0;
        FlashLoanExecutor(receiver).receiveFlashLoan(tokens, amounts, fees, userData);
    }
}

/// @dev Ruta simulada: en cada llamada devuelve `profit` tokens al llamador.
contract ProfitArb {
    FuzzToken public token;
    uint256 public profit;

    constructor(FuzzToken _token) {
        token = _token;
    }

    function setProfit(uint256 p) external {
        profit = p;
    }

    fallback() external {
        uint256 p = profit;
        profit = 0;
        if (p > 0) token.transfer(msg.sender, p);
    }
}

contract W12CallbackBindingFuzzTest is Test {
    FlashLoanExecutor internal flashExec;
    RoundTripProvider internal provider;
    ProfitArb internal arb;
    FuzzToken internal token;

    address internal admin;
    address internal executor;

    function setUp() public {
        admin = address(this);
        executor = makeAddr("executor");

        token = new FuzzToken();
        provider = new RoundTripProvider();
        arb = new ProfitArb(token);

        FlashLoanExecutor impl = new FlashLoanExecutor();
        bytes memory initData =
            abi.encodeWithSelector(FlashLoanExecutor.initialize.selector, admin, makeAddr("aavePool"), address(arb));
        ERC1967Proxy proxy = new ERC1967Proxy(address(impl), initData);
        flashExec = FlashLoanExecutor(address(proxy));

        flashExec.grantRole(flashExec.EXECUTOR_ROLE(), executor);
        flashExec.setBalancerVault(address(provider));
        flashExec.setFlashLoanProvider(address(provider));

        // El proveedor necesita fondos para poder prestar en cada corrida del fuzz.
        token.mint(address(provider), 1e30);
    }

    // -----------------------------------------------------------------------
    // (a) + (b): callback NO solicitado REVIERTE y el balance previo queda intacto
    // -----------------------------------------------------------------------
    function testFuzz_unsolicitedCallback_revertsAndKeepsBalance(uint256 amount, bytes32 salt) public {
        amount = bound(amount, 1, 1e24);

        // Reserva PREVIA del wrapper: es exactamente lo que F26 permitia consumir.
        token.mint(address(flashExec), 5e18);
        uint256 balBefore = token.balanceOf(address(flashExec));

        vm.expectRevert(FL_RequestMismatch.selector);
        provider.triggerCallback(address(flashExec), address(token), amount, abi.encode(salt));

        assertEq(token.balanceOf(address(flashExec)), balBefore, "balance previo alterado");
    }

    // -----------------------------------------------------------------------
    // (c): el piso de beneficio se repaga ATOMICAMENTE
    // -----------------------------------------------------------------------
    function testFuzz_profitFloor_isAtomic(uint256 amount, uint256 profit, uint256 floor) public {
        amount = bound(amount, 1, 1e24);
        profit = bound(profit, 0, 1e18);
        floor = bound(floor, 1, 1e18); // floor > 0: el caso interesante

        flashExec.setMinProfitFloor(floor);
        token.mint(address(arb), profit);
        arb.setProfit(profit);

        uint256 balBefore = token.balanceOf(address(flashExec));

        if (profit >= floor) {
            vm.startPrank(executor);
            flashExec.requestFlashLoan(address(token), amount, "");
            vm.stopPrank();
            assertEq(token.balanceOf(address(flashExec)), balBefore + profit, "no acredito el beneficio");
        } else {
            vm.startPrank(executor);
            vm.expectRevert(FL_BelowProfitFloor.selector);
            flashExec.requestFlashLoan(address(token), amount, "");
            vm.stopPrank();
            // Atomicidad: revirtio TODO, incluido el prestamo y el piso no satisfecho.
            assertEq(token.balanceOf(address(flashExec)), balBefore, "el revert no fue atomico");
        }
    }

    // -----------------------------------------------------------------------
    // Control: con floor = 0 el round trip sin beneficio SI pasa (el binding
    // acepta la peticion legitima). Aisla que el revert no viene del binding.
    // -----------------------------------------------------------------------
    function testFuzz_legitRequest_withZeroFloor_succeeds(uint256 amount) public {
        amount = bound(amount, 1, 1e24);
        flashExec.setMinProfitFloor(0);
        uint256 balBefore = token.balanceOf(address(flashExec));

        vm.startPrank(executor);
        flashExec.requestFlashLoan(address(token), amount, "");
        vm.stopPrank();

        assertEq(token.balanceOf(address(flashExec)), balBefore, "el balance debe volver al inicio sin beneficio");
    }
}
