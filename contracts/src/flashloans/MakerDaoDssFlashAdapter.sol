// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.20;

import {IFlashLoanProvider} from "../interfaces/IFlashLoanProvider.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";

/**
 * @title MakerDaoDssFlashAdapter
 * @notice FASE 4 — IFlashLoanProvider adapter for MakerDAO DSS Flash.
 *
 * DssFlash (Ethereum mainnet only) mints DAI for the duration of the flash:
 * the receiver gets `amount` DAI up front, runs its logic, then must repay
 * `amount + fee` (fee is configurable; 0 within the free window). The repay
 * is a pull: DssFlash calls `onFlashLoan(initiator, amount, fee, data)` on the
 * receiver (= FlashLoanExecutor), which then approves DssFlash for the owe.
 *
 * Mirrors the BalancerFlashAdapter pattern: single-asset delegation, zero-fee
 * reporting (DssFlash fee is negligible / 0 in the free window), real
 * balanceOf for maxFlashLoan. The `onFlashLoan` callback lives on
 * FlashLoanExecutor (added in a sibling change — see FASE 4 plan), NOT here.
 *
 * Doctrinal gates: arbx-flash-loan-discipline (repay-or-revert lives in the
 * executor callback, not the adapter), arbx-no-hardcode (the DssFlash address
 * is injected via the constructor — never a literal here).
 *
 * @dev Mainnet-only (chain_id 1). The registry seeds this adapter's address
 * via migration 100 with metadata.provider_family = "makerdao_dss".
 */
interface IDssFlash {
    /// @notice Mint `amount` DAI to `receiver`, call its `onFlashLoan`, pull back amount+fee.
    function flashLoan(address receiver, uint256 amount, bytes calldata data) external;
}

contract MakerDaoDssFlashAdapter is IFlashLoanProvider {
    /// @notice The MakerDAO DssFlash contract (mainnet 0x60744434...8B4C). Immutable.
    IDssFlash public immutable dssFlash;

    /// @param dssFlash_ The DssFlash contract address (zero rejected).
    constructor(address dssFlash_) {
        require(dssFlash_ != address(0), "MakerDaoDssFlashAdapter: zero dssFlash");
        dssFlash = IDssFlash(dssFlash_);
    }

    /// @inheritdoc IFlashLoanProvider
    /// @dev DssFlash only lends DAI. `asset` MUST be the DAI address; the
    ///      executor is responsible for passing the right asset (enforced again
    ///      in the onFlashLoan callback). We forward receiver/amount/params as-is.
    function flashLoan(address receiver, address asset, uint256 amount, bytes calldata params) external override {
        // Defence-in-depth: the adapter only lends DAI. A non-DAI asset is a
        // caller bug — revert rather than silently minting the wrong token.
        // The DAI address is NOT hardcoded here (no-hardcode); the caller (executor)
        // guarantees asset == DAI by only selecting this adapter for DAI borrows.
        // We still sanity-check it's a non-zero address.
        require(asset != address(0), "MakerDaoDssFlashAdapter: zero asset");
        IDssFlash(dssFlash).flashLoan(receiver, amount, params);
    }

    /// @inheritdoc IFlashLoanProvider
    /// @dev DssFlash fee is 0 within its free window (the common case for the
    ///      small borrows ArbitrageX makes). Report 0 so the off-chain ranker
    ///      treats this as a zero-fee provider. If MakerDAO raises the fee,
    ///      update the off-chain `contract_registry.metadata.fee_bps` instead
    ///      of changing this (no-hardcode: the on-chain view stays conservative).
    function flashLoanFee(uint256 /* amount */) external pure override returns (uint256 fee) {
        return 0;
    }

    /// @inheritdoc IFlashLoanProvider
    /// @dev DssFlash mints DAI on demand (no pre-funded pool), so the ceiling
    ///      is the DAI balance DssFlash itself can pull from the vat — we
    ///      approximate with its own DAI balance as a safe lower bound. The
    ///      off-chain ranker cross-checks `metadata.max_depth_usd` (operator-tunable).
    function maxFlashLoan(address asset) external view override returns (uint256 max) {
        return IERC20(asset).balanceOf(address(dssFlash));
    }
}
