// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// W12a-F25-SOLO-01 — tests del SUBCONJUNTO F25 (binding del callback).
//
// Dos propiedades, y la segunda es la que el bundle de t138 violaba:
//   (1) un callback NO solicitado REVIERTE con FL_RequestMismatch y el balance
//       previo del wrapper queda INTACTO;
//   (2) ★ el CAMINO LEGITIMO COMPLETA: un flash loan solicitado por este contrato
//       se ejecuta y se repaga. Sin este test, "arreglar F25" y "romper la
//       ejecucion" son indistinguibles.
// Se agrega (3): el compromiso se consume, asi que el callback no se puede repetir.

import "forge-std/Test.sol";
import "../src/FlashLoanExecutor.sol";
import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";

contract W12aToken is ERC20 {
    constructor() ERC20("W12aToken", "W12A") {}

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}

/// @dev Proveedor que ejecuta un round trip REAL: desembolsa -> callback -> cobra
///      el reembolso que el propio contrato hace con safeTransfer a msg.sender.
contract W12aProvider {
    function flashLoan(address receiver, address asset, uint256 amount, bytes calldata params) external {
        W12aToken(asset).transfer(receiver, amount);
        IERC20[] memory tokens = new IERC20[](1);
        tokens[0] = IERC20(asset);
        uint256[] memory amounts = new uint256[](1);
        amounts[0] = amount;
        uint256[] memory fees = new uint256[](1);
        fees[0] = 0;
        FlashLoanExecutor(receiver).receiveFlashLoan(tokens, amounts, fees, params);
    }

    /// @dev Callback SIN prestamo previo: el atacante hace que el Vault configurado
    ///      invoque. NO transfiere fondos: es el vector de F25.
    function triggerCallback(address receiver, address asset, uint256 amount, bytes calldata userData) external {
        IERC20[] memory tokens = new IERC20[](1);
        tokens[0] = IERC20(asset);
        uint256[] memory amounts = new uint256[](1);
        amounts[0] = amount;
        uint256[] memory fees = new uint256[](1);
        fees[0] = 0;
        FlashLoanExecutor(receiver).receiveFlashLoan(tokens, amounts, fees, userData);
    }

    function replaySamePayload(address receiver, address asset, uint256 amount, bytes calldata userData) external {
        IERC20[] memory tokens = new IERC20[](1);
        tokens[0] = IERC20(asset);
        uint256[] memory amounts = new uint256[](1);
        amounts[0] = amount;
        uint256[] memory fees = new uint256[](1);
        fees[0] = 0;
        FlashLoanExecutor(receiver).receiveFlashLoan(tokens, amounts, fees, userData);
    }
}

/// @dev Ruta no-op: el "swap" no devuelve nada, el prestamo se repaga con el principal.
contract W12aNoopArb {
    fallback() external {}
}

contract W12aF25BindingOnlyTest is Test {
    FlashLoanExecutor internal flashExec;
    W12aProvider internal provider;
    W12aNoopArb internal arb;
    W12aToken internal token;

    address internal admin;
    address internal executor;

    function setUp() public {
        admin = address(this);
        executor = makeAddr("executor");

        token = new W12aToken();
        provider = new W12aProvider();
        arb = new W12aNoopArb();

        FlashLoanExecutor impl = new FlashLoanExecutor();
        bytes memory initData =
            abi.encodeWithSelector(FlashLoanExecutor.initialize.selector, admin, makeAddr("aavePool"), address(arb));
        ERC1967Proxy proxy = new ERC1967Proxy(address(impl), initData);
        flashExec = FlashLoanExecutor(address(proxy));

        flashExec.grantRole(flashExec.EXECUTOR_ROLE(), executor);
        flashExec.setBalancerVault(address(provider));
        flashExec.setFlashLoanProvider(address(provider));

        // El proveedor necesita fondos para prestar en cada corrida del fuzz.
        token.mint(address(provider), 1e30);
    }

    // -----------------------------------------------------------------------
    // (1) El binding: callback NO solicitado revierte y NO toca el balance previo
    // -----------------------------------------------------------------------
    function testFuzz_unsolicitedCallback_revertsAndKeepsBalance(uint256 amount, bytes32 salt) public {
        amount = bound(amount, 1, 1e24);
        token.mint(address(flashExec), 5e18); // reserva PREVIA: lo que F26 permitia consumir
        uint256 balBefore = token.balanceOf(address(flashExec));

        vm.expectRevert(FL_RequestMismatch.selector);
        provider.triggerCallback(address(flashExec), address(token), amount, abi.encode(salt));

        assertEq(token.balanceOf(address(flashExec)), balBefore, "balance previo alterado");
    }

    // -----------------------------------------------------------------------
    // (2) ★ EL CAMINO LEGITIMO COMPLETA (el criterio que el bundle violaba)
    // -----------------------------------------------------------------------
    function testFuzz_legitPath_completes(uint256 amount) public {
        amount = bound(amount, 1, 1e24);
        uint256 balBeforeExec = token.balanceOf(address(flashExec));

        vm.startPrank(executor);
        flashExec.requestFlashLoan(address(token), amount, "");
        vm.stopPrank();

        // Llego hasta aca => el callback acepto la peticion legitima y repago.
        assertEq(token.balanceOf(address(flashExec)), balBeforeExec, "el round trip no cerro limpio");
    }

    /// @dev Mismo camino con la ruta devolviendo un excedente: debe completar y quedar el excedente.
    function testFuzz_legitPath_withSurplus_completes(uint256 amount, uint256 surplus) public {
        amount = bound(amount, 1, 1e21);
        surplus = bound(surplus, 0, 1e18);
        token.mint(address(flashExec), surplus);
        uint256 balBeforeExec = token.balanceOf(address(flashExec));

        vm.startPrank(executor);
        flashExec.requestFlashLoan(address(token), amount, "");
        vm.stopPrank();

        assertEq(token.balanceOf(address(flashExec)), balBeforeExec, "el excedente previo debe seguir ahi");
    }

    // -----------------------------------------------------------------------
    // (3) El compromiso se CONSUME: el mismo callback no se puede repetir
    // -----------------------------------------------------------------------
    function test_callbackCannotBeReplayed() public {
        vm.prank(executor);
        flashExec.requestFlashLoan(address(token), 100e18, "");

        // Ya se consumo: repetir el mismo payload revierte.
        vm.expectRevert(FL_RequestMismatch.selector);
        provider.replaySamePayload(address(flashExec), address(token), 100e18, "");
    }
}
