// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// §IV blocker A2 — ArbitrageExecutor on the sim-ctl Anvil fork (2026-08-18)
// =============================================================================
// Deploys the `ArbitrageExecutor` UUPS stack (implementation + ERC1967Proxy)
// to the EPHEMERAL Anvil mainnet fork used by sim-ctl (container `anvil`,
// RPC http://anvil:8545). The fork resets on container restart, so this
// deployment is repeated at sim-ctl boot by
// `backend/sim-ctl/src/executor_deploy.rs` (raw eth_sendRawTransaction path —
// no forge in the runtime image). This script is the human/CI-facing twin of
// that boot deployer: same signer, same pinned nonces, same canonical
// addresses.
//
// SAFETY GATES (this script must NEVER reach a live chain):
//   1. Explicit opt-in: CONFIRM_ANVIL_FORK_DEPLOY=true (mirrors
//      CONFIRM_SEPOLIA_DEPLOY in DeploySepolia.s.sol).
//   2. The RPC must identify itself as an Anvil node via
//      `web3_clientVersion`. The fork reports chainid 1 (mainnet), so a
//      chainid check alone cannot distinguish the fork from real mainnet —
//      the clientVersion check is what makes accidental mainnet broadcast
//      physically impossible.
//   3. The signer is the well-known Anvil dev account #0
//      (0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266). It only ever holds
//      value on a local Anvil node; the key is public (Hardhat/Anvil default
//      mnemonic) and is NOT a real key.
//
// DETERMINISM (why the pinned nonce):
//   Verified empirically 2026-08-18 (anvil 1.7.x mainnet fork): the fork
//   OVERRIDES the dev accounts' balance (10000 ETH) but does NOT reset their
//   nonces — account #0 reported its live mainnet nonce (7576 at block
//   25779033). A bare first-deploy address therefore drifts as mainnet
//   advances. Pinning the deployer nonce to 0 with `anvil_setNonce` before
//   broadcasting makes the deployment addresses canonical and stable across
//   every boot:
//     impl  = create1(acct#0, 0) = 0x5FbDB2315678afecb367f032d93F642f64180aa3
//     proxy = create1(acct#0, 1) = 0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512
//   The Rust boot deployer asserts these exact addresses from the receipts.
//
// IDEMPOTENCY:
//   If sim-ctl (or a previous run of this script) already deployed the proxy
//   on a still-live fork, the script detects code at the canonical proxy
//   address and skips — safe to re-run after container restarts.
//
// USAGE (against the sim-ctl fork):
//   CONFIRM_ANVIL_FORK_DEPLOY=true \
//   forge script script/DeployArbitrageExecutor.s.sol \
//     --rpc-url http://localhost:8545 --broadcast -vvvv
//
// The ArbitrageExecutor source is NOT modified by this change — deploy only.
// =============================================================================

import "forge-std/Script.sol";
import "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import "../src/ArbitrageExecutor.sol";

contract DeployArbitrageExecutor is Script {
    /// @dev Well-known Anvil dev account #0 private key (public test key from
    ///      the default Anvil/Hardhat mnemonic — holds no real funds).
    uint256 constant ANVIL_DEV_KEY_0 =
        0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80;

    /// @dev Canonical deployment addresses (create1 from account #0 with the
    ///      nonce pinned to 0/1). Asserted post-broadcast; kept in sync with
    ///      backend/sim-ctl/src/executor_deploy.rs.
    address constant EXPECTED_IMPL = 0x5FbDB2315678afecb367f032d93F642f64180aa3;
    address constant EXPECTED_PROXY = 0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512;

    function run() external {
        // Safety gate 1: explicit opt-in.
        require(
            vm.envBool("CONFIRM_ANVIL_FORK_DEPLOY"),
            "DeployArbitrageExecutor: set CONFIRM_ANVIL_FORK_DEPLOY=true (Anvil fork only)"
        );

        // Signer = Anvil dev account #0 (0xf39F...2266 by construction).
        // Canonical addresses below are asserted for this deployer ONLY.
        uint256 key = ANVIL_DEV_KEY_0;
        address deployer = vm.addr(key);

        // Safety gate 2: the RPC must be an Anvil node. This is the gate that
        // makes the script safe despite the fork reporting chainid 1.
        bytes memory clientVersion = vm.rpc("web3_clientVersion", "[]");
        require(_contains(clientVersion, "anvil"), "DeployArbitrageExecutor: RPC is not an Anvil node - refusing to deploy");

        console2.log("=== ArbitrageExecutor fork deploy (sim-ctl Anvil fork) ===");
        console2.log("Deployer      :", deployer);
        console2.log("ClientVersion :", string(clientVersion));

        // Determinism: pin the deployer nonce to 0 so the canonical addresses
        // below hold regardless of the forked mainnet nonce. BOTH states must
        // be pinned: forge simulates against a local EVM forked at a pinned
        // block (where the nonce is still the live mainnet value), while the
        // broadcast transactions land on the remote anvil. vm.setNonceUnsafe
        // mutates the local sim EVM (setNonce is increment-only and refuses
        // 7576 -> 0); the vm.rpc anvil_setNonce call mutates the remote node
        // so it accepts the low-nonce broadcast transactions.
        vm.setNonceUnsafe(deployer, 0);
        vm.rpc("anvil_setNonce", string.concat('["', vm.toString(deployer), '","0x0"]'));

        address expectedImpl = vm.computeCreateAddress(deployer, 0);
        address expectedProxy = vm.computeCreateAddress(deployer, 1);
        require(expectedImpl == EXPECTED_IMPL, "impl address precheck failed - unexpected deployer/nonce");
        require(expectedProxy == EXPECTED_PROXY, "proxy address precheck failed - unexpected deployer/nonce");

        // Idempotency: proxy already deployed on this (still-live) fork.
        if (expectedProxy.code.length > 0) {
            console2.log("ArbitrageExecutor proxy already deployed at", expectedProxy);
            console2.log("ARBITRAGE_EXECUTOR=", expectedProxy);
            return;
        }

        vm.startBroadcast(key);

        // 1. ArbitrageExecutor implementation (UUPS — constructor only
        //    disables initializers; no args).
        ArbitrageExecutor implAE = new ArbitrageExecutor();
        require(
            address(implAE) == expectedImpl,
            "impl landed at unexpected address - nonce not pinned on this RPC?"
        );
        console2.log("ArbitrageExecutor impl  :", address(implAE));

        // 2. ERC1967Proxy -> initialize(admin = anvil account #0). Admin holds
        //    DEFAULT_ADMIN_ROLE + UPGRADER_ROLE on the ephemeral fork (roles
        //    used by the simulation's storage-override path; nothing live).
        ERC1967Proxy proxyAE = new ERC1967Proxy(
            address(implAE),
            abi.encodeWithSelector(ArbitrageExecutor.initialize.selector, deployer)
        );
        require(
            address(proxyAE) == expectedProxy,
            "proxy landed at unexpected address - nonce not pinned on this RPC?"
        );

        vm.stopBroadcast();

        console2.log("ArbitrageExecutor proxy :", address(proxyAE));
        console2.log("ARBITRAGE_EXECUTOR=", address(proxyAE));
        console2.log("");
        console2.log("NOTE: ephemeral fork deployment. sim-ctl redeploys at boot via");
        console2.log("backend/sim-ctl/src/executor_deploy.rs (same canonical addresses).");
    }

    /// @dev Case-insensitive ASCII substring check on raw RPC result bytes.
    function _contains(bytes memory haystack, string memory needle) internal pure returns (bool) {
        bytes memory n = bytes(needle);
        if (n.length == 0 || haystack.length < n.length) return false;
        for (uint256 i = 0; i <= haystack.length - n.length; i++) {
            bool matched = true;
            for (uint256 j = 0; j < n.length; j++) {
                bytes1 h = _lower(haystack[i + j]);
                bytes1 nj = _lower(n[j]);
                if (h != nj) {
                    matched = false;
                    break;
                }
            }
            if (matched) return true;
        }
        return false;
    }

    function _lower(bytes1 b) internal pure returns (bytes1) {
        if (b >= 0x41 && b <= 0x5A) {
            return bytes1(uint8(b) + 0x20);
        }
        return b;
    }
}
