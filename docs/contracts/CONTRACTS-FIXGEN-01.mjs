#!/usr/bin/env node
// =============================================================================
// CONTRACTS-FIXGEN-01 — generate the callback-binding fix from the ANCHORED bytes
// =============================================================================
// Reads docs/contracts/anchored/FlashLoanExecutor.sol (bytes proven equal to the
// audited rev bceb31ef by git blob hash), applies the five remediation edits, and
// ASSERTS each edit matched exactly once. A missed/ambiguous edit aborts with a
// non-zero exit rather than emitting a silently partial "fixed" contract.
//
// Output: docs/contracts/proposed/FlashLoanExecutor.fixed.sol   (NOT compiled, NOT applied)
//
// Why generate instead of hand-editing: forge(1) is blocked on this host by Windows
// AppControl (os error 4551), so a hand-edited contract that never compiles is an
// unverifiable claim. The generated file is byte-diffable against the anchored
// original, which makes the change reviewable line by line, and `git diff --no-index`
// turns it into a real patch that Release can review and apply on a host with forge.
// =============================================================================

import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { join } from 'node:path';

const REPO = process.argv.includes('--repo') ? process.argv[process.argv.indexOf('--repo') + 1] : '.';
const SRC = join(REPO, 'docs', 'contracts', 'anchored', 'FlashLoanExecutor.sol');
const OUTDIR = join(REPO, 'docs', 'contracts', 'proposed');
const EXPECTED_BLOB = '96be27838d149d3a3f1b74b5bd575c89d25017a9';

const buf = readFileSync(SRC);
const blob = createHash('sha1').update(Buffer.concat([Buffer.from(`blob ${buf.length}\0`), buf])).digest('hex');
if (blob !== EXPECTED_BLOB) {
  console.error(`ANCHOR MISMATCH: ${SRC} is ${blob}, expected ${EXPECTED_BLOB}`);
  process.exit(1);
}
let s = buf.toString('utf8');

const edits = [];
function edit(id, label, from, to) {
  const hits = s.split(from).length - 1;
  if (hits !== 1) {
    console.error(`EDIT ${id} (${label}) matched ${hits} times, expected exactly 1 — ABORT, no output written.`);
    process.exit(1);
  }
  s = s.replace(from, to);
  edits.push({ id, label, ok: true });
}

// ── Fix A (F25, FL-3): transient request binding for callbacks with no initiator ──
edit('A1', 'append storage slot for the pending request commitment',
`    uint256 private _reentrancyStatus = _NOT_ENTERED;`,
`    uint256 private _reentrancyStatus = _NOT_ENTERED;

    // FIX A (F25, arbx-flash-loan-discipline rule 3). Balancer does not expose an
    // \`initiator\`, so the discipline requires a transient session commitment instead.
    // The request records a digest of the FULL request (chain, this contract, provider,
    // asset, amount, userData) before calling out; the callback recomputes and compares
    // it, then clears it. An unsolicited callback from the genuine Vault finds the slot
    // empty (or mismatched) and reverts. APPENDED below _reentrancyStatus per the
    // storage-layout append-only rule (test/StorageLayout.t.sol pins slots 0..3).
    bytes32 private _pendingRequestHash;

    /// @notice Operator-set treasury that receives swept profit (FIX D).
    address public treasury;
    /// @notice Operator-set minimum net profit (asset units) required beyond the
    ///         repayment. 0 disables the extra floor. FIX C.
    uint256 public minProfitFloor;`);

edit('A2', 'add FIX A/B/C/D errors',
`error FL_RepaymentShortfall();`,
`error FL_RepaymentShortfall();
/// @dev FIX A (F25): thrown when a callback does not correspond to a request this
///      contract initiated (no request in flight, or the digest does not match).
error FL_RequestMismatch();
/// @dev FIX C: thrown when the round trip did not clear repayment + minProfitFloor.
error FL_BelowProfitFloor();
/// @dev FIX D: thrown when sweepProfit is called with no treasury configured.
error FL_NoTreasury();`);

edit('A3', 'FIX A: record the commitment before the outgoing request, clear it after',
`    function requestFlashLoan(address asset, uint256 amount, bytes calldata params) external onlyRole(EXECUTOR_ROLE) {
        address provider = flashLoanProvider;
        if (provider != address(0)) {`,
`    function requestFlashLoan(address asset, uint256 amount, bytes calldata params)
        external
        onlyRole(EXECUTOR_ROLE)
        whenNotPaused
    {
        address provider = flashLoanProvider;
        // FIX A (F25): bind THIS request to the callback that will follow. Set before the
        // outbound call, cleared after it returns (or the whole tx reverts, which also
        // clears it — EVM atomicity). FIX E: whenNotPaused stops new exposure here while
        // the callback path stays callable so an already-issued provider call can always
        // settle or revert inside its own transaction.
        _pendingRequestHash = _requestDigest(asset, amount, provider, address(0), params);
        if (provider != address(0)) {`);

edit('A4', 'FIX A: clear the commitment after the provider call returns',
`        // SC-06: emit after the call so the event is only logged on success
        emit FlashLoanRequested(asset, amount, keccak256(params));`,
`        _pendingRequestHash = bytes32(0);
        // SC-06: emit after the call so the event is only logged on success
        emit FlashLoanRequested(asset, amount, keccak256(params));`);

edit('A5', 'FIX A: commit the Aave request too (parity of the binding)',
`            // Legacy path: direct Aave V3 call (backward compat for existing deployments)
            aavePool.flashLoanSimple(address(this), asset, amount, params, referralCode);`,
`            // Legacy path: direct Aave V3 call (backward compat for existing deployments)
            _pendingRequestHash = _requestDigest(asset, amount, address(0), address(aavePool), params);
            aavePool.flashLoanSimple(address(this), asset, amount, params, referralCode);`);

edit('A6', 'FIX A + B: bind and delta-scope the Aave callback',
`        if (msg.sender != address(aavePool)) revert FL_UnauthorizedCaller();
        if (initiator != address(this)) revert FL_InvalidInitiator();

        _executeAndRepayAave(asset, amount, premium, params);`,
`        if (msg.sender != address(aavePool)) revert FL_UnauthorizedCaller();
        if (initiator != address(this)) revert FL_InvalidInitiator();
        // FIX A: the provider's initiator check is necessary but not sufficient — it
        // proves the loan names this contract, not that THIS call corresponds to the
        // request we are currently paying for. Consume the commitment.
        if (_pendingRequestHash != _requestDigest(asset, amount, address(0), address(aavePool), params)) {
            revert FL_RequestMismatch();
        }
        _pendingRequestHash = bytes32(0);

        _executeAndRepayAave(asset, amount, premium, params);`);

edit('A7', 'FIX A + B: bind and delta-scope the Balancer callback',
`        // Layer 3: flashLoanProvider must be set, confirming the Balancer path
        //          was intentionally activated by the operator.
        if (flashLoanProvider == address(0)) revert FL_NoProviderConfigured();

        IERC20 asset = tokens[0];
        uint256 amount = amounts[0];
        uint256 premium = feeAmounts[0]; // 0 for Balancer V2`,
`        // Layer 3: flashLoanProvider must be set, confirming the Balancer path
        //          was intentionally activated by the operator.
        if (flashLoanProvider == address(0)) revert FL_NoProviderConfigured();
        // FIX A (F25): Layer 4 — the layers above prove only that the CONFIGURED Vault
        // called. They do not prove that we asked. Anyone may call
        // vault.flashLoan(address(this), tokens, amounts, attackerData) on the REAL
        // Vault, which then calls this function legitimately. Require the request
        // commitment recorded by requestFlashLoan and consume it.
        if (_pendingRequestHash
            != _requestDigest(asset_candidate(tokens), amounts[0], flashLoanProvider, address(0), userData)) {
            revert FL_RequestMismatch();
        }
        _pendingRequestHash = bytes32(0);

        IERC20 asset = tokens[0];
        uint256 amount = amounts[0];
        uint256 premium = feeAmounts[0]; // 0 for Balancer V2`);

edit('A8', 'FIX B: scope the Balancer solvency check to THIS operation delta',
`        // 3. Repay Balancer Vault (amount + fee = amount + 0 = amount). Fail-closed with a
        // clear named error if the round trip did not leave enough to repay.
        uint256 amountOwed = amount + premium;
        if (asset.balanceOf(address(this)) < amountOwed) revert FL_RepaymentShortfall();
        asset.safeTransfer(msg.sender, amountOwed);`,
`        // 3. Repay Balancer Vault (amount + fee = amount + 0 = amount). Fail-closed with a
        // clear named error if the round trip did not leave enough to repay.
        // FIX B (F26): the check is scoped to THIS operation's inflow (balBefore + owed),
        // not to the contract's total balance. A pre-existing reserve can no longer be
        // silently consumed to cover a losing route; the same delta identity that
        // ArbitrageExecutor already enforces one layer down (balBeforePull) now holds here.
        uint256 amountOwed = amount + premium;
        uint256 required = balBeforeCallback + amountOwed + minProfitFloor;
        if (asset.balanceOf(address(this)) < required) revert FL_BelowProfitFloor();
        asset.safeTransfer(msg.sender, amountOwed);`);

edit('A9', 'FIX B: capture the pre-callback baseline for the delta check',
`        IERC20 asset = tokens[0];
        uint256 amount = amounts[0];
        uint256 premium = feeAmounts[0]; // 0 for Balancer V2

        // 1. Approve funds to ArbitrageExecutor`,
`        IERC20 asset = tokens[0];
        uint256 amount = amounts[0];
        uint256 premium = feeAmounts[0]; // 0 for Balancer V2
        // FIX B (F26): baseline taken BEFORE the executor call so the repayment gate
        // measures what THIS operation added, never what the wrapper already held.
        uint256 balBeforeCallback = asset.balanceOf(address(this));

        // 1. Approve funds to ArbitrageExecutor`);

edit('A10', 'FIX B: scope the Aave solvency check to THIS operation delta',
`        uint256 amountToOwe = amount + premium;
        if (IERC20(asset).balanceOf(address(this)) < amountToOwe) revert FL_RepaymentShortfall();
        IERC20(asset).forceApprove(address(aavePool), amountToOwe);`,
`        uint256 amountToOwe = amount + premium;
        // FIX B (F26): delta-scoped, including the operator profit floor.
        uint256 required = balBeforeCallback + amountToOwe + minProfitFloor;
        if (IERC20(asset).balanceOf(address(this)) < required) revert FL_BelowProfitFloor();
        IERC20(asset).forceApprove(address(aavePool), amountToOwe);`);

edit('A11', 'FIX B: capture the baseline in the shared Aave helper',
`    function _executeAndRepayAave(address asset, uint256 amount, uint256 premium, bytes calldata params) internal {
        // 1. Approve funds to ArbitrageExecutor`,
`    function _executeAndRepayAave(address asset, uint256 amount, uint256 premium, bytes calldata params) internal {
        // FIX B (F26): baseline before the executor call (see receiveFlashLoan).
        uint256 balBeforeCallback = IERC20(asset).balanceOf(address(this));

        // 1. Approve funds to ArbitrageExecutor`);

edit('A12', 'FIX C/D/E: digest helper, profit floor setter, treasury sweep, scoped guardian pause',
`    /// @dev Only UPGRADER_ROLE can authorize a new implementation.
    function _authorizeUpgrade(address newImplementation) internal override onlyRole(UPGRADER_ROLE) {}`,
`    // -------------------------------------------------------------------------
    // FIX A: request digest
    // -------------------------------------------------------------------------

    /// @dev Canonical digest of ONE flash-loan request. Binds chain, this contract, the
    ///      chosen provider (or the Aave pool), the asset, the amount and the exact
    ///      payload. Deliberately excludes msg.sender (any relayer may submit the tx);
    ///      the binding is between a request and a callback, not between a request and
    ///      a transaction sender.
    function _requestDigest(address asset, uint256 amount, address provider, address pool, bytes calldata params)
        internal
        view
        returns (bytes32)
    {
        return keccak256(abi.encode(block.chainid, address(this), provider, pool, asset, amount, keccak256(params)));
    }

    /// @dev Address of the first token. Extracted so the callback can build the digest
    ///      before the local variables are declared.
    function asset_candidate(IERC20[] calldata tokens) internal pure returns (address) {
        return address(tokens[0]);
    }

    // -------------------------------------------------------------------------
    // FIX D (F27): operational exit for retained profit
    // -------------------------------------------------------------------------

    /// @notice Set the treasury that receives swept profit. Operator-set, never a literal.
    function setTreasury(address _treasury) external onlyRole(DEFAULT_ADMIN_ROLE) {
        treasury = _treasury;
        emit TreasuryUpdated(_treasury);
    }

    /// @notice Set the minimum net profit (asset units) required beyond repayment. 0 disables.
    function setMinProfitFloor(uint256 _floor) external onlyRole(DEFAULT_ADMIN_ROLE) {
        minProfitFloor = _floor;
        emit MinProfitFloorUpdated(_floor);
    }

    /// @notice Move retained profit out of this contract to the operator treasury.
    /// @dev FIX D (F27): the wrapper is where the flash-funded spread accumulates, and it
    ///      previously had NO ERC-20 exit at all. Deliberately admin-only (in production
    ///      that is the timelock), NOT callable by the execution role, and it cannot
    ///      touch an amount the current callback still owes because repayment happens in
    ///      the same transaction that produced the profit.
    function sweepProfit(address asset, uint256 amount) external onlyRole(DEFAULT_ADMIN_ROLE) {
        address to = treasury;
        if (to == address(0)) revert FL_NoTreasury();
        IERC20(asset).safeTransfer(to, amount);
        emit ProfitSwept(asset, to, amount);
    }

    // -------------------------------------------------------------------------
    // FIX E (F28): pause that stops new exposure without needing the timelock
    // -------------------------------------------------------------------------

    /// @notice Scoped guardian role: may pause, may NOT unpause, upgrade or move funds.
    bytes32 public constant GUARDIAN_ROLE = keccak256("GUARDIAN_ROLE");

    /// @notice Halt new flash-loan requests immediately. Callable by GUARDIAN_ROLE,
    ///         which is intended to be a hot key on a monitoring process — not the
    ///         timelock-held admin. Unpausing stays with DEFAULT_ADMIN_ROLE.
    function pause() external onlyRole(GUARDIAN_ROLE) {
        _pause();
    }

    /// @notice Resume. Admin-only on purpose: stopping is urgent, resuming is a decision.
    function unpause() external onlyRole(DEFAULT_ADMIN_ROLE) {
        _unpause();
    }

    /// @notice Grant the scoped guardian role (admin-only).
    function setGuardian(address guardian, bool enabled) external onlyRole(DEFAULT_ADMIN_ROLE) {
        if (enabled) _grantRole(GUARDIAN_ROLE, guardian); else _revokeRole(GUARDIAN_ROLE, guardian);
        emit GuardianUpdated(guardian, enabled);
    }

    /// @dev Only UPGRADER_ROLE can authorize a new implementation.
    function _authorizeUpgrade(address newImplementation) internal override onlyRole(UPGRADER_ROLE) {}`);

edit('A13', 'FIX D/E: events',
`    event BalancerVaultUpdated(address indexed previousVault, address indexed newVault);`,
`    event BalancerVaultUpdated(address indexed previousVault, address indexed newVault);
    /// @notice FIX D: emitted when the profit treasury is updated.
    event TreasuryUpdated(address indexed treasury);
    /// @notice FIX D: emitted when retained profit leaves the contract.
    event ProfitSwept(address indexed asset, address indexed to, uint256 amount);
    /// @notice FIX C: emitted when the operator profit floor changes.
    event MinProfitFloorUpdated(uint256 floor);
    /// @notice FIX E: emitted when the scoped guardian role changes.
    event GuardianUpdated(address indexed guardian, bool enabled);`);

edit('A14', 'FIX E: make the contract pausable (namespaced storage, no re-init needed)',
`import "./interfaces/IFlashLoanProvider.sol";`,
`import "@openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol";
import "./interfaces/IFlashLoanProvider.sol";`);

edit('A15', 'FIX E: inherit PausableUpgradeable',
`contract FlashLoanExecutor is Initializable, AccessControlUpgradeable, UUPSUpgradeable {`,
`contract FlashLoanExecutor is Initializable, AccessControlUpgradeable, PausableUpgradeable, UUPSUpgradeable {`);

edit('A16', 'FIX E: initialize the pausable namespace',
`        __AccessControl_init();
        __UUPSUpgradeable_init();`,
`        __AccessControl_init();
        __PausableUpgradeable_init();
        __UUPSUpgradeable_init();
        // NOTE for existing proxies: PausableUpgradeable stores its flag in a namespaced
        // (ERC-7201) slot, which reads as 0 == not paused on an already-initialized
        // proxy, so no re-initialization is required to adopt this upgrade. Granting
        // GUARDIAN_ROLE is a separate post-deploy step (see the deploy plan).`);

mkdirSync(OUTDIR, { recursive: true });
const dst = join(OUTDIR, 'FlashLoanExecutor.fixed.sol');
writeFileSync(dst, s, 'utf8');

console.log(`CONTRACTS-FIXGEN-01`);
console.log(`source (anchored) : ${SRC}`);
console.log(`source blob       : ${blob} (== audited rev bceb31ef)`);
console.log(`output            : ${dst}`);
console.log(`edits applied     : ${edits.length}/${edits.length} (each matched exactly once)`);
for (const e of edits) console.log(`  ${e.id}  ${e.label}`);
console.log(`bytes: ${buf.length} -> ${Buffer.byteLength(s, 'utf8')}`);
console.log(`STATUS: generated, NOT compiled, NOT applied. forge(1) is blocked on this host`);
console.log(`        (Windows AppControl, os error 4551); compile+fuzz+invariant+fork must run where forge works.`);
