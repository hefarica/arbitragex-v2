// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// DeployMainnet — AdminTimelock DEFAULT_ADMIN_ROLE custody regression
//
// Closes the sibling gap to the UUPS UPGRADER_ROLE finding: the mainnet deploy
// script grants the deployer EOA a *bootstrap* DEFAULT_ADMIN_ROLE over the
// AdminTimelock proxy (OZ TimelockController init grants the `admin` arg the role
// IN ADDITION to the timelock's own self-administration). If the deployer's copy
// is not renounced, the deployer keeps DEFAULT_ADMIN_ROLE over the timelock and
// can grantRole(PROPOSER/EXECUTOR, deployer) instantly — a direct AccessControl
// call that does NOT pass through the 24h delay — defeating the multisig
// separation of duties.
//
// This suite runs the REAL DeployMainnet().run() (satisfying the env / chainid /
// multisig / balance gates via cheatcodes), robustly recovers the deployed
// AdminTimelock proxy from the broadcast logs, and asserts the fixed custody:
//   - deployer does NOT hold DEFAULT_ADMIN_ROLE on the timelock  (gap closed)
//   - timelock self-holds DEFAULT_ADMIN_ROLE (address(this))      (anti-brick)
//   - multisig retains PROPOSER_ROLE, EXECUTOR_ROLE, CANCELLER_ROLE
//   - anti-brick control: the multisig can still administer the timelock via
//     schedule + execute (warp 24h) of a grantRole targeting the timelock itself.
//
// NAMING: distinct from the Rama B custody suite (DeployMainnetRoleCustody.t.sol);
// this suite is scoped to the AdminTimelock's own DEFAULT_ADMIN_ROLE custody.
// =============================================================================

import "forge-std/Test.sol";
import "../script/DeployMainnet.s.sol";
import "../src/AdminTimelock.sol";
import "@openzeppelin/contracts/access/IAccessControl.sol";

contract DeployMainnetTimelockAdminCustodyTest is Test {
    // DEFAULT_ADMIN_ROLE == 0x00 in OZ AccessControl.
    bytes32 internal constant DEFAULT_ADMIN_ROLE = 0x00;

    // RoleGranted(bytes32 indexed role, address indexed account, address indexed sender)
    bytes32 internal constant ROLE_GRANTED_SIG =
        keccak256("RoleGranted(bytes32,address,address)");

    DeployMainnet internal script;

    uint256 internal deployerKey;
    address internal deployer;
    address internal multisig;
    address internal aavePool;

    function setUp() public {
        script = new DeployMainnet();

        // Deployer EOA — deterministic key, funded above the 0.5 ETH gate.
        // IDENTICAL derivation to DeployMainnetRoleCustody.t.sol on purpose: vm.setEnv
        // mutates the shared OS process env, which forge does NOT revert between suites.
        // If the two DeployMainnet suites set MULTISIG_ADDRESS / DEPLOYER_PRIVATE_KEY /
        // AAVE_V3_POOL to different values, the combined `forge test` run collides (one
        // suite reads the other's env and trips a code/balance gate in its own fork).
        // Using the SAME values makes the shared env harmless — whichever setUp wins,
        // each suite has satisfied the gate (funded deployer / etched code) in its fork.
        deployerKey = uint256(keccak256("arbx.test.mainnet-deployer"));
        deployer = vm.addr(deployerKey);
        vm.deal(deployer, 1 ether);

        // Multisig + Aave pool must be contracts (code.length > 0); neither is called
        // during run(), so any nonempty runtime bytecode satisfies the gate.
        multisig = makeAddr("multisig");
        aavePool = makeAddr("aavePool");
        vm.etch(multisig, hex"01");
        vm.etch(aavePool, hex"01");

        // Satisfy the script's env-var gates.
        vm.setEnv("CONFIRM_MAINNET_DEPLOY", "true");
        vm.setEnv("DEPLOYER_PRIVATE_KEY", vm.toString(deployerKey));
        vm.setEnv("MULTISIG_ADDRESS", vm.toString(multisig));
        vm.setEnv("AAVE_V3_POOL", vm.toString(aavePool));

        // Chain-id gate: must be Ethereum mainnet.
        vm.chainId(1);
    }

    // -----------------------------------------------------------------------
    // Robustly recover the AdminTimelock proxy from broadcast logs.
    //
    // Anchor: only TimelockController self-administers, i.e. it emits
    // RoleGranted(DEFAULT_ADMIN_ROLE, address(this), address(this)) where the
    // *emitter* equals the *account* being granted. AE/AM/FL only ever grant
    // DEFAULT_ADMIN_ROLE to the deployer or to the timelock proxy — never to
    // themselves — so the self-grant is unique to the timelock. We also pin that
    // exactly one such event exists (sanity), then verify the recovered address
    // is a TimelockController by role/minDelay probes.
    // -----------------------------------------------------------------------
    function _recoverTimelock(Vm.Log[] memory logs) internal view returns (address tl) {
        uint256 hits;
        for (uint256 i = 0; i < logs.length; i++) {
            Vm.Log memory e = logs[i];
            if (e.topics.length != 4) continue;
            if (e.topics[0] != ROLE_GRANTED_SIG) continue;
            if (e.topics[1] != DEFAULT_ADMIN_ROLE) continue;
            address account = address(uint160(uint256(e.topics[2])));
            // Self-administration grant: emitter granted the admin role to itself.
            if (account == e.emitter) {
                tl = e.emitter;
                hits++;
            }
        }
        require(hits == 1, "expected exactly one timelock self-admin grant");
        require(tl != address(0), "timelock proxy not found in logs");
    }

    // -----------------------------------------------------------------------
    // testDeploy_TimelockAdminCustody
    //
    // GREEN with the fix (deployer's bootstrap admin renounced in-script);
    // RED against the pre-fix script (deployer keeps DEFAULT_ADMIN_ROLE).
    // -----------------------------------------------------------------------
    function testDeploy_TimelockAdminCustody() public {
        vm.recordLogs();
        script.run();
        Vm.Log[] memory logs = vm.getRecordedLogs();

        AdminTimelock tl = AdminTimelock(payable(_recoverTimelock(logs)));

        // Sanity pins: recovered address really is the configured timelock.
        assertEq(tl.getMinDelay(), 86_400, "recovered timelock minDelay must be 24h (sanity pin)");
        assertTrue(
            tl.hasRole(tl.PROPOSER_ROLE(), multisig),
            "recovered timelock must have multisig as proposer (sanity pin)"
        );

        // --- Gap closed: deployer must NOT hold DEFAULT_ADMIN_ROLE on the timelock ---
        assertFalse(
            tl.hasRole(DEFAULT_ADMIN_ROLE, deployer),
            "deployer must NOT retain DEFAULT_ADMIN_ROLE over the AdminTimelock"
        );

        // --- Anti-brick: timelock self-administration must remain intact ---
        assertTrue(
            tl.hasRole(DEFAULT_ADMIN_ROLE, address(tl)),
            "timelock must retain self-administration (DEFAULT_ADMIN_ROLE over itself)"
        );

        // --- Multisig operational roles must be preserved ---
        assertTrue(tl.hasRole(tl.PROPOSER_ROLE(), multisig), "multisig must retain PROPOSER_ROLE");
        assertTrue(tl.hasRole(tl.EXECUTOR_ROLE(), multisig), "multisig must retain EXECUTOR_ROLE");
        assertTrue(tl.hasRole(tl.CANCELLER_ROLE(), multisig), "multisig must retain CANCELLER_ROLE");
    }

    // -----------------------------------------------------------------------
    // testDeploy_TimelockStillAdministrableByMultisig
    //
    // Positive anti-brick control: prove renouncing the deployer's admin does
    // NOT strand the timelock. The multisig schedules + (after 24h) executes a
    // grantRole targeting the timelock itself — the documented self-administration
    // model. If self-admin had been damaged, this execute would revert.
    // -----------------------------------------------------------------------
    function testDeploy_TimelockStillAdministrableByMultisig() public {
        vm.recordLogs();
        script.run();
        Vm.Log[] memory logs = vm.getRecordedLogs();

        AdminTimelock tl = AdminTimelock(payable(_recoverTimelock(logs)));

        // A fresh account the multisig will grant PROPOSER_ROLE to, purely via the
        // timelock's own governance path (schedule -> warp -> execute).
        address newProposer = makeAddr("newProposer");
        assertFalse(
            tl.hasRole(tl.PROPOSER_ROLE(), newProposer),
            "precondition: newProposer must not already hold PROPOSER_ROLE"
        );

        // The admin operation: timelock grants PROPOSER_ROLE to newProposer.
        // target == the timelock itself; only its self-admin can satisfy this.
        bytes memory callData = abi.encodeWithSelector(
            IAccessControl.grantRole.selector,
            tl.PROPOSER_ROLE(),
            newProposer
        );
        bytes32 predecessor = bytes32(0);
        bytes32 salt = keccak256("anti-brick-selfadmin");
        uint256 minDelay = tl.getMinDelay();

        // Multisig holds PROPOSER_ROLE -> can schedule.
        vm.prank(multisig);
        tl.schedule(address(tl), 0, callData, predecessor, salt, minDelay);

        // Wait out the delay.
        vm.warp(block.timestamp + minDelay + 1);

        // Multisig holds EXECUTOR_ROLE -> can execute. Succeeds only because the
        // timelock retained DEFAULT_ADMIN_ROLE over itself (grantRole is admin-gated).
        vm.prank(multisig);
        tl.execute(address(tl), 0, callData, predecessor, salt);

        assertTrue(
            tl.hasRole(tl.PROPOSER_ROLE(), newProposer),
            "multisig must be able to administer the timelock via self-admin schedule+execute"
        );
    }
}
