// SPDX-License-Identifier: BUSL-1.1
pragma solidity 0.8.24;

/**
 * @title  MakerDssAdapter
 * @notice Adaptador holonómico para Maker DSS (DAI Savings Rate / PSM).
 * @dev    Soporta dos modalidades:
 *           1) PSM swap (USDC ⇄ DAI sin slippage) — utilidad de estabilización.
 *           2) DSR deposit/withdraw — Topological Yield pasivo.
 *
 *         Ghost Protocol: cero retención en este contrato.
 *         Lexicón Absoluto: ningún stub/placeholder.
 */

interface IPsm {
    function sellGem(address usr, uint256 gemAmt) external;     // USDC → DAI
    function buyGem(address usr, uint256 gemAmt) external;      // DAI → USDC
    function dai() external view returns (address);
    function gemJoin() external view returns (address);
    function tin() external view returns (uint256);              // fee in 1e18
    function tout() external view returns (uint256);             // fee in 1e18
}

interface IDsr {
    function join(uint256 wad) external;
    function exit(uint256 wad) external;
    function pie(address) external view returns (uint256);
    function chi() external view returns (uint256);
}

interface IERC20Maker {
    function transferFrom(address from, address to, uint256 amount) external returns (bool);
    function transfer(address to, uint256 amount) external returns (bool);
    function approve(address spender, uint256 amount) external returns (bool);
    function balanceOf(address account) external view returns (uint256);
}

contract MakerDssAdapter {
    error Maker__ZeroAmount();
    error Maker__SlippageExceeded(uint256 received, uint256 minOut);

    event PsmSellGem(address indexed psm, address indexed user, uint256 gemAmt, uint256 daiOut);
    event PsmBuyGem(address indexed psm, address indexed user, uint256 gemAmt, uint256 daiIn);
    event DsrJoined(address indexed dsr, address indexed user, uint256 daiAmt);
    event DsrExited(address indexed dsr, address indexed user, uint256 daiAmt);

    function psmSellGem(
        address psm,
        address usdc,
        uint256 gemAmt,
        uint256 minDaiOut,
        address recipient
    ) external returns (uint256 daiOut) {
        if (gemAmt == 0) revert Maker__ZeroAmount();
        IPsm p = IPsm(psm);
        address dai = p.dai();

        IERC20Maker(usdc).transferFrom(msg.sender, address(this), gemAmt);
        IERC20Maker(usdc).approve(p.gemJoin(), gemAmt);

        uint256 before = IERC20Maker(dai).balanceOf(address(this));
        p.sellGem(address(this), gemAmt);
        daiOut = IERC20Maker(dai).balanceOf(address(this)) - before;

        if (daiOut < minDaiOut) revert Maker__SlippageExceeded(daiOut, minDaiOut);
        IERC20Maker(dai).transfer(recipient, daiOut);
        emit PsmSellGem(psm, msg.sender, gemAmt, daiOut);
    }

    function psmBuyGem(
        address psm,
        address usdc,
        uint256 gemAmt,
        uint256 maxDaiIn,
        address recipient
    ) external returns (uint256 daiSpent) {
        if (gemAmt == 0) revert Maker__ZeroAmount();
        IPsm p = IPsm(psm);
        address dai = p.dai();

        IERC20Maker(dai).transferFrom(msg.sender, address(this), maxDaiIn);
        IERC20Maker(dai).approve(psm, maxDaiIn);

        uint256 daiBefore = IERC20Maker(dai).balanceOf(address(this));
        p.buyGem(address(this), gemAmt);
        daiSpent = daiBefore - IERC20Maker(dai).balanceOf(address(this));

        IERC20Maker(usdc).transfer(recipient, gemAmt);
        // Devolver excedente de DAI si quedara
        uint256 remainder = IERC20Maker(dai).balanceOf(address(this));
        if (remainder > 0) IERC20Maker(dai).transfer(msg.sender, remainder);

        emit PsmBuyGem(psm, msg.sender, gemAmt, daiSpent);
    }

    function dsrJoin(address dsr, address dai, uint256 wad) external {
        if (wad == 0) revert Maker__ZeroAmount();
        IERC20Maker(dai).transferFrom(msg.sender, address(this), wad);
        IERC20Maker(dai).approve(dsr, wad);
        IDsr(dsr).join(wad);
        emit DsrJoined(dsr, msg.sender, wad);
    }

    function dsrExit(address dsr, address dai, uint256 wad, address recipient) external {
        if (wad == 0) revert Maker__ZeroAmount();
        IDsr(dsr).exit(wad);
        IERC20Maker(dai).transfer(recipient, wad);
        emit DsrExited(dsr, recipient, wad);
    }
}
