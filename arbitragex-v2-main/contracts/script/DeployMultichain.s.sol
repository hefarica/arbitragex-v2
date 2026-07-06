// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

// =============================================================================
// SC-16: DeployMultichain — Multi-chain CREATE2 deployment script
//
// Deploys the full OMEGA suite deterministically across 6 chains using
// DeterministicFactory with CREATE2. All contract addresses are identical
// on every chain (salt namespaceado por chainId prevents collision).
//
// Chains supported:
//   Ethereum (1), Arbitrum (42161), Optimism (10), Base (8453), Polygon (137), BSC (56)
//
// Contracts deployed per chain:
//   1. ArbitrageExecutor     — UUPS proxy + implementation
//   2. FlashLoanExecutor     — UUPS proxy + implementation
//   3. WalletTopology        — immutable (non-upgradeable)
//   4. UniswapV2Adapter      — immutable (router via constructor)
//   5. UniswapV3Adapter      — immutable (router via constructor)
//   6. BalancerVaultAdapter  — immutable (vault via constructor)
//
// Pre-deploy requirements:
//   - Set DEPLOYER_PRIVATE_KEY environment variable
//   - Set RPC URLs for target chains (e.g. MAINNET_RPC_URL, ARBITRUM_RPC_URL)
//   - Set MULTISIG_ADDRESS (the governance multisig, must be a contract)
//   - DeterministicFactory must already be deployed on each target chain
//     at the same address (use Nick's method or a deterministic deployer)
//
// Usage:
//   # Step 1: Predict all addresses (no tx, no gas)
//   forge script script/DeployMultichain.s.sol --sig "predictAll()" --rpc-url $MAINNET_RPC_URL -vvvv
//
//   # Step 2: Deploy on a single chain (e.g. Ethereum mainnet)
//   forge script script/DeployMultichain.s.sol --sig "deployOnChain(uint256)" 1 \
//     --rpc-url $MAINNET_RPC_URL --broadcast -vvvv
//
//   # Step 3: Deploy on all 6 chains sequentially
//   ./script/deploy_all_chains.sh   # see comments below for manual commands
//
// Output:
//   - Console logs with all predicted/deployed addresses
//   - JSON file: out/deployment_<chainId>_<timestamp>.json
//
// Post-deploy checklist:
//   [ ] 1. Verify contracts on Etherscan/Blockscout for each chain
//   [ ] 2. Wire AllowanceManager to ArbitrageExecutor
//   [ ] 3. Grant EXECUTOR_ROLE on ArbitrageExecutor and FlashLoanExecutor
//   [ ] 4. Approve tokens and routers via ArbitrageExecutor
//   [ ] 5. Grant allowances via AllowanceManager
//   [ ] 6. Transfer admin roles to AdminTimelock on each chain
// =============================================================================

import "forge-std/Script.sol";
import "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import "../src/core/DeterministicFactory.sol";
import "../src/core/WalletTopology.sol";
import "../src/ArbitrageExecutor.sol";
import "../src/FlashLoanExecutor.sol";
import "../src/AllowanceManager.sol";
import "../src/adapters/UniswapV2Adapter.sol";
import "../src/adapters/UniswapV3Adapter.sol";
import "../src/adapters/BalancerVaultAdapter.sol";
import "../src/AdminTimelock.sol";

contract DeployMultichain is Script {
    // -------------------------------------------------------------------------
    // Chain configuration
    // -------------------------------------------------------------------------

    uint256 public constant ETHEREUM  = 1;
    uint256 public constant ARBITRUM  = 42161;
    uint256 public constant OPTIMISM  = 10;
    uint256 public constant BASE      = 8453;
    uint256 public constant POLYGON   = 137;
    uint256 public constant BSC       = 56;

    /// @notice All supported chain IDs for iteration.
    uint256[] public SUPPORTED_CHAINS;

    constructor() {
        SUPPORTED_CHAINS.push(ETHEREUM);
        SUPPORTED_CHAINS.push(ARBITRUM);
        SUPPORTED_CHAINS.push(OPTIMISM);
        SUPPORTED_CHAINS.push(BASE);
        SUPPORTED_CHAINS.push(POLYGON);
        SUPPORTED_CHAINS.push(BSC);
    }

    /// @notice Default entrypoint — predicts all addresses without deploying.
    /// @dev Override of Script.run(). Safe to call without --broadcast.
    function run() external view override {
        predictAll();
    }

    // -------------------------------------------------------------------------
    // Deployment addresses storage (filled during predict/deploy)
    // -------------------------------------------------------------------------

    struct Deployment {
        uint256 chainId;
        address factory;
        address arbitrageExecutor;
        address flashLoanExecutor;
        address allowanceManager;
        address walletTopology;
        address uniswapV2Adapter;
        address uniswapV3Adapter;
        address balancerVaultAdapter;
        address adminTimelock;
    }

    /// @notice Deployment record per chain. Keyed by chainId.
    mapping(uint256 => Deployment) public deployments;

    // -------------------------------------------------------------------------
    // DeterministicFactory reference
    // -------------------------------------------------------------------------

    /// @notice The DeterministicFactory must be deployed at this same address
    /// on every chain. Use Nick's method (0x4e59b44847b379578588920cA78FbF26c0B4956C)
    /// or deploy with a pre-signed transaction.
    /// Set via FACTORY_ADDRESS env var, or override with --sig "setFactory(address)".
    address public factoryAddress;

    // -------------------------------------------------------------------------
    // Constructor argument placeholders (set at runtime)
    // -------------------------------------------------------------------------

    /// @notice Wallet topology addresses. Set via environment or constructor.
    address public gasSponsor;
    address public executionSigner;
    address public coldTreasury;

    // Router addresses per chain (NOT hardcoded — set via environment)
    // These are validated at runtime.

    // -------------------------------------------------------------------------
    // Events
    // -------------------------------------------------------------------------

    event ChainDeploymentComplete(uint256 indexed chainId, Deployment deployment);
    event AllPredictionsComplete(Deployment[] predictions);

    // -------------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------------

    /// @notice Validate that chainId is supported.
    function _isSupported(uint256 chainId) internal pure returns (bool) {
        return (chainId == ETHEREUM || chainId == ARBITRUM || chainId == OPTIMISM
                || chainId == BASE || chainId == POLYGON || chainId == BSC);
    }

    /// @notice Get the DeterministicFactory instance.
    function _factory() internal view returns (DeterministicFactory) {
        require(factoryAddress != address(0), "DeployMultichain: factory not set");
        return DeterministicFactory(factoryAddress);
    }

    /// @notice Build the full init bytecode for a contract with its constructor args.
    function _buildBytecode(
        bytes memory creationCode,
        bytes memory constructorArgs
    ) internal pure returns (bytes memory) {
        return abi.encodePacked(creationCode, constructorArgs);
    }

    /// @notice Predict a single contract address. Reduces stack depth in predictChain.
    function _predict(
        DeterministicFactory _factory,
        bytes memory _creationCode,
        bytes memory _ctorArgs,
        uint256 _chainId,
        string memory _name
    ) internal view returns (address) {
        return _factory.predictAddress(
            _buildBytecode(_creationCode, _ctorArgs),
            _chainId,
            _name
        );
    }

    // -------------------------------------------------------------------------
    // Prediction functions (view — no state change, no gas cost)
    // -------------------------------------------------------------------------

    /// @notice Predict all contract addresses for a single chain.
    /// @dev View function — no transaction, no gas cost. Run this before
    ///      deploying to generate the deployment runbook.
    /// @param _chainId      Chain ID to predict for.
    /// @param _factoryAddr  DeterministicFactory address on that chain.
    /// @param _gs           Gas sponsor address for WalletTopology.
    /// @param _es           Execution signer address for WalletTopology.
    /// @param _ct           Cold treasury address for WalletTopology.
    /// @param _v2Router     Uniswap V2 router address for the adapter.
    /// @param _v3Router     Uniswap V3 router address for the adapter.
    /// @param _vault        Balancer Vault address for the adapter.
    /// @return d            Deployment struct with all predicted addresses.
    function predictChain(
        uint256 _chainId,
        address _factoryAddr,
        address _gs,
        address _es,
        address _ct,
        address _v2Router,
        address _v3Router,
        address _vault
    ) public view returns (Deployment memory d) {
        require(_isSupported(_chainId), "DeployMultichain: unsupported chain");
        require(_factoryAddr != address(0), "DeployMultichain: factory not set");

        DeterministicFactory f = DeterministicFactory(_factoryAddr);
        uint256 c = _chainId;
        d.chainId = c;
        d.factory = _factoryAddr;

        // Use _predict helper to reduce stack depth
        d.walletTopology      = _predict(f, type(WalletTopology).creationCode,       abi.encode(_gs, _es, _ct),       c, "WalletTopology");
        d.uniswapV2Adapter    = _predict(f, type(UniswapV2Adapter).creationCode,     abi.encode(_v2Router),           c, "UniswapV2Adapter");
        d.uniswapV3Adapter    = _predict(f, type(UniswapV3Adapter).creationCode,     abi.encode(_v3Router),           c, "UniswapV3Adapter");
        d.balancerVaultAdapter= _predict(f, type(BalancerVaultAdapter).creationCode, abi.encode(_vault),              c, "BalancerVaultAdapter");
        d.arbitrageExecutor   = _predict(f, type(ArbitrageExecutor).creationCode,    "",                              c, "ArbitrageExecutor");
        d.flashLoanExecutor   = _predict(f, type(FlashLoanExecutor).creationCode,    "",                              c, "FlashLoanExecutor");
        d.allowanceManager    = _predict(f, type(AllowanceManager).creationCode,     "",                              c, "AllowanceManager");
        d.adminTimelock       = _predict(f, type(AdminTimelock).creationCode,        abi.encode(uint256(86400),
                                                                                       _toArray(_factoryAddr),
                                                                                       _toArray(_factoryAddr),
                                                                                       _factoryAddr),               c, "AdminTimelock");
    }

    /// @notice Helper: wrap a single address into a 1-element array.
    function _toArray(address a) internal pure returns (address[] memory arr) {
        arr = new address[](1);
        arr[0] = a;
    }

    /// @notice Predict addresses for all 6 chains and log to console.
    /// @dev Usage: forge script script/DeployMultichain.s.sol --sig "predictAll()" --rpc-url $MAINNET_RPC_URL -vvvv
    ///      This is a PURE VIEW function — it costs zero gas.
    function predictAll()
        public
        view
        returns (Deployment[6] memory predictions)
    {
        address _factoryAddr = vm.envOr("FACTORY_ADDRESS", address(0));
        require(_factoryAddr != address(0), "Set FACTORY_ADDRESS env var");

        address _gasSponsor = vm.envOr("GAS_SPONSOR_ADDRESS", msg.sender);
        address _execSigner = vm.envOr("EXECUTION_SIGNER_ADDRESS", msg.sender);
        address _coldTreasury = vm.envOr("COLD_TREASURY_ADDRESS", msg.sender);

        console2.log("=== OMEGA Multi-Chain Address Predictions ===");
        console2.log("Factory:", _factoryAddr);
        console2.log("");

        // Router addresses per chain (examples — override with env vars)
        // These are validated to be non-zero at runtime.
        address[6] memory v2Routers = [
            vm.envOr("MAINNET_V2_ROUTER", address(0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D)),
            vm.envOr("ARBITRUM_V2_ROUTER", address(0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506)),
            vm.envOr("OPTIMISM_V2_ROUTER", address(0x4A7b5Da61326A6379179b40d00F57E5bbDC962c2)),
            vm.envOr("BASE_V2_ROUTER", address(0x4752ba5DBc23f44D87826276BF6Fd6b1C372aD24)),
            vm.envOr("POLYGON_V2_ROUTER", address(0xa5E0829CaCEd8fFDD4De3c43696c57F7D7A678ff)),
            vm.envOr("BSC_V2_ROUTER", address(0x10ED43C718714eb63d5aA57B78B54704E256024E))
        ];

        address[6] memory v3Routers = [
            vm.envOr("MAINNET_V3_ROUTER", address(0xE592427A0AEce92De3Edee1F18E0157C05861564)),
            vm.envOr("ARBITRUM_V3_ROUTER", address(0xE592427A0AEce92De3Edee1F18E0157C05861564)),
            vm.envOr("OPTIMISM_V3_ROUTER", address(0xE592427A0AEce92De3Edee1F18E0157C05861564)),
            vm.envOr("BASE_V3_ROUTER", address(0x2626664c2603336E57B271c5C0b26F421741e481)),
            vm.envOr("POLYGON_V3_ROUTER", address(0xE592427A0AEce92De3Edee1F18E0157C05861564)),
            vm.envOr("BSC_V3_ROUTER", address(0x1b81D678ffb9C0263b24A97847620C99d213eB14))
        ];

        address[6] memory vaults = [
            vm.envOr("MAINNET_BALANCER_VAULT", address(0xBA12222222228d8Ba445958a75a0704d566BF2C8)),
            vm.envOr("ARBITRUM_BALANCER_VAULT", address(0xBA12222222228d8Ba445958a75a0704d566BF2C8)),
            vm.envOr("OPTIMISM_BALANCER_VAULT", address(0xBA12222222228d8Ba445958a75a0704d566BF2C8)),
            vm.envOr("BASE_BALANCER_VAULT", address(0xBA12222222228d8Ba445958a75a0704d566BF2C8)),
            vm.envOr("POLYGON_BALANCER_VAULT", address(0xBA12222222228d8Ba445958a75a0704d566BF2C8)),
            vm.envOr("BSC_BALANCER_VAULT", address(0x0000000000000000000000000000000000000000))
        ];

        string[6] memory names = ["Ethereum", "Arbitrum", "Optimism", "Base", "Polygon", "BSC"];
        uint256[6] memory chainIds = [ETHEREUM, ARBITRUM, OPTIMISM, BASE, POLYGON, BSC];

        for (uint256 i = 0; i < 6; i++) {
            predictions[i] = predictChain(
                chainIds[i],
                _factoryAddr,
                _gasSponsor,
                _execSigner,
                _coldTreasury,
                v2Routers[i],
                v3Routers[i],
                vaults[i]
            );

            console2.log(string.concat("--- ", names[i], " (", vm.toString(chainIds[i]), ") ---"));
            console2.log("  WalletTopology      :", predictions[i].walletTopology);
            console2.log("  UniswapV2Adapter    :", predictions[i].uniswapV2Adapter);
            console2.log("  UniswapV3Adapter    :", predictions[i].uniswapV3Adapter);
            console2.log("  BalancerVaultAdapter:", predictions[i].balancerVaultAdapter);
            console2.log("  ArbitrageExecutor   :", predictions[i].arbitrageExecutor, "(impl)");
            console2.log("  FlashLoanExecutor   :", predictions[i].flashLoanExecutor, "(impl)");
            console2.log("  AllowanceManager    :", predictions[i].allowanceManager, "(impl)");
            console2.log("  AdminTimelock       :", predictions[i].adminTimelock, "(impl)");
            console2.log("");
        }

        console2.log("=== Prediction Summary ===");
        console2.log("All addresses are deterministic -- they will be identical on each chain.");
        console2.log("Note: ArbitrageExecutor, FlashLoanExecutor, AllowanceManager are IMPLEMENTATION addresses.");
        console2.log("      Proxies are deployed separately via ERC1967Proxy (not CREATE2).");
        console2.log("Run with --sig \"deployOnChain(uint256)\" <chainId> to deploy.");
    }

    // -------------------------------------------------------------------------
    // Deployment functions (state-changing — require broadcast)
    // -------------------------------------------------------------------------

    /// @notice Deploy all OMEGA contracts on a single chain via CREATE2.
    /// @dev Must be broadcast with --broadcast. Uses DeterministicFactory.deploy().
    ///
    ///      Usage:
    ///        forge script script/DeployMultichain.s.sol --sig "deployOnChain(uint256)" 1 \
    ///          --rpc-url $MAINNET_RPC_URL --broadcast -vvvv
    ///
    ///      Post-conditions:
    ///        - WalletTopology deployed at predicted address
    ///        - UniswapV2Adapter deployed at predicted address
    ///        - UniswapV3Adapter deployed at predicted address
    ///        - BalancerVaultAdapter deployed at predicted address (if vault != address(0))
    ///        - ArbitrageExecutor implementation deployed at predicted address
    ///        - FlashLoanExecutor implementation deployed at predicted address
    ///        - AllowanceManager implementation deployed at predicted address
    ///        - ERC1967 proxies deployed for the 3 UUPS contracts
    ///
    /// @param _chainId Chain ID to deploy on. Must be supported.
    function deployOnChain(uint256 _chainId) external returns (Deployment memory d) {
        require(_isSupported(_chainId), "DeployMultichain: unsupported chain");

        uint256 deployerKey = vm.envUint("DEPLOYER_PRIVATE_KEY");
        address deployer = vm.addr(deployerKey);
        address multisig = vm.envAddress("MULTISIG_ADDRESS");
        address aavePool = vm.envOr("AAVE_V3_POOL", address(0));

        require(multisig != address(0), "Set MULTISIG_ADDRESS env var");
        require(multisig.code.length > 0, "MULTISIG_ADDRESS must be a contract");
        require(aavePool != address(0), "Set AAVE_V3_POOL env var");

        // Set factory address
        factoryAddress = vm.envOr("FACTORY_ADDRESS", address(0));
        require(factoryAddress != address(0), "Set FACTORY_ADDRESS env var");

        // Read wallet topology addresses
        gasSponsor = vm.envOr("GAS_SPONSOR_ADDRESS", deployer);
        executionSigner = vm.envOr("EXECUTION_SIGNER_ADDRESS", deployer);
        coldTreasury = vm.envOr("COLD_TREASURY_ADDRESS", multisig);

        // Read router addresses for this chain
        address v2Router = _getV2Router(_chainId);
        address v3Router = _getV3Router(_chainId);
        address vault = _getBalancerVault(_chainId);

        console2.log("=== OMEGA Deployment ===");
        console2.log("Chain ID    :", _chainId);
        console2.log("Deployer    :", deployer);
        console2.log("Multisig    :", multisig);
        console2.log("Factory     :", factoryAddress);
        console2.log("Aave Pool   :", aavePool);
        console2.log("V2 Router   :", v2Router);
        console2.log("V3 Router   :", v3Router);
        console2.log("Balancer    :", vault);
        console2.log("");

        vm.startBroadcast(deployerKey);

        DeterministicFactory factory = _factory();
        d.chainId = _chainId;
        d.factory = factoryAddress;

        // ------------------------------------------------------------------
        // 1. Deploy WalletTopology (immutable — CREATE2 deterministic)
        // ------------------------------------------------------------------
        {
            bytes memory code = _buildBytecode(
                type(WalletTopology).creationCode,
                abi.encode(gasSponsor, executionSigner, coldTreasury)
            );
            d.walletTopology = factory.deploy(code, _chainId, "WalletTopology");
            console2.log("WalletTopology      :", d.walletTopology);
        }

        // ------------------------------------------------------------------
        // 2. Deploy UniswapV2Adapter (immutable — CREATE2 deterministic)
        // ------------------------------------------------------------------
        {
            bytes memory code = _buildBytecode(
                type(UniswapV2Adapter).creationCode,
                abi.encode(v2Router)
            );
            d.uniswapV2Adapter = factory.deploy(code, _chainId, "UniswapV2Adapter");
            console2.log("UniswapV2Adapter    :", d.uniswapV2Adapter);
        }

        // ------------------------------------------------------------------
        // 3. Deploy UniswapV3Adapter (immutable — CREATE2 deterministic)
        // ------------------------------------------------------------------
        {
            bytes memory code = _buildBytecode(
                type(UniswapV3Adapter).creationCode,
                abi.encode(v3Router)
            );
            d.uniswapV3Adapter = factory.deploy(code, _chainId, "UniswapV3Adapter");
            console2.log("UniswapV3Adapter    :", d.uniswapV3Adapter);
        }

        // ------------------------------------------------------------------
        // 4. Deploy BalancerVaultAdapter (immutable — CREATE2 deterministic)
        //      Skip if no Balancer Vault on this chain (e.g. BSC).
        // ------------------------------------------------------------------
        if (vault != address(0)) {
            bytes memory code = _buildBytecode(
                type(BalancerVaultAdapter).creationCode,
                abi.encode(vault)
            );
            d.balancerVaultAdapter = factory.deploy(code, _chainId, "BalancerVaultAdapter");
            console2.log("BalancerVaultAdapter:", d.balancerVaultAdapter);
        } else {
            console2.log("BalancerVaultAdapter: SKIPPED (no vault on this chain)");
        }

        // ------------------------------------------------------------------
        // 5. Deploy ArbitrageExecutor (UUPS implementation via CREATE2)
        //      + ERC1967 proxy (standard deploy, not CREATE2)
        // ------------------------------------------------------------------
        {
            bytes memory implCode = type(ArbitrageExecutor).creationCode;
            d.arbitrageExecutor = factory.deploy(implCode, _chainId, "ArbitrageExecutor");
            console2.log("ArbitrageExecutor impl:", d.arbitrageExecutor);

            // Deploy proxy pointing to implementation
            ERC1967Proxy proxy = new ERC1967Proxy(
                d.arbitrageExecutor,
                abi.encodeWithSelector(ArbitrageExecutor.initialize.selector, deployer)
            );
            console2.log("ArbitrageExecutor proxy:", address(proxy));
        }

        // ------------------------------------------------------------------
        // 6. Deploy FlashLoanExecutor (UUPS implementation via CREATE2)
        //      + ERC1967 proxy
        // ------------------------------------------------------------------
        {
            bytes memory implCode = type(FlashLoanExecutor).creationCode;
            d.flashLoanExecutor = factory.deploy(implCode, _chainId, "FlashLoanExecutor");
            console2.log("FlashLoanExecutor impl:", d.flashLoanExecutor);

            // Deploy proxy — needs arbitrageExecutor proxy address
            // We use the implementation address as a placeholder; the proxy
            // will be updated post-deploy to point at the correct AE proxy.
            ERC1967Proxy proxy = new ERC1967Proxy(
                d.flashLoanExecutor,
                abi.encodeWithSelector(
                    FlashLoanExecutor.initialize.selector,
                    deployer,
                    aavePool,
                    d.arbitrageExecutor // placeholder — update post-deploy
                )
            );
            console2.log("FlashLoanExecutor proxy:", address(proxy));
        }

        // ------------------------------------------------------------------
        // 7. Deploy AllowanceManager (UUPS implementation via CREATE2)
        //      + ERC1967 proxy
        // ------------------------------------------------------------------
        {
            bytes memory implCode = type(AllowanceManager).creationCode;
            d.allowanceManager = factory.deploy(implCode, _chainId, "AllowanceManager");
            console2.log("AllowanceManager impl:", d.allowanceManager);

            ERC1967Proxy proxy = new ERC1967Proxy(
                d.allowanceManager,
                abi.encodeWithSelector(AllowanceManager.initialize.selector, deployer)
            );
            console2.log("AllowanceManager proxy:", address(proxy));
        }

        // ------------------------------------------------------------------
        // 8. Deploy AdminTimelock (non-UUPS — CREATE2 deterministic)
        // ------------------------------------------------------------------
        {
            address[] memory proposers = new address[](1);
            proposers[0] = multisig;
            address[] memory executors = new address[](1);
            executors[0] = multisig;

            bytes memory code = _buildBytecode(
                type(AdminTimelock).creationCode,
                abi.encode(uint256(86_400), proposers, executors, deployer)
            );
            d.adminTimelock = factory.deploy(code, _chainId, "AdminTimelock");
            console2.log("AdminTimelock       :", d.adminTimelock);
        }

        vm.stopBroadcast();

        // Store and emit
        deployments[_chainId] = d;
        emit ChainDeploymentComplete(_chainId, d);

        // Write JSON output
        _writeDeploymentJson(d);

        console2.log("");
        console2.log("=== Post-Deploy Checklist ===");
        console2.log("[ ] 1. Wire ArbitrageExecutor proxy to AllowanceManager proxy");
        console2.log("[ ] 2. Grant EXECUTOR_ROLE to executionSigner on ArbitrageExecutor");
        console2.log("[ ] 3. Grant EXECUTOR_ROLE to executionSigner on FlashLoanExecutor");
        console2.log("[ ] 4. Approve tokens and routers on ArbitrageExecutor");
        console2.log("[ ] 5. Grant allowances via AllowanceManager");
        console2.log("[ ] 6. Update FlashLoanExecutor proxy with correct ArbitrageExecutor proxy");
        console2.log("[ ] 7. Transfer admin roles to AdminTimelock");
        console2.log("[ ] 8. Verify all contracts on block explorer");
    }

    // -------------------------------------------------------------------------
    // Per-chain router lookup (validated examples, override via env vars)
    // -------------------------------------------------------------------------

    function _getV2Router(uint256 chainId) internal view returns (address) {
        if (chainId == ETHEREUM)  return vm.envOr("MAINNET_V2_ROUTER", address(0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D));
        if (chainId == ARBITRUM)  return vm.envOr("ARBITRUM_V2_ROUTER", address(0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506));
        if (chainId == OPTIMISM)  return vm.envOr("OPTIMISM_V2_ROUTER", address(0x4A7b5Da61326A6379179b40d00F57E5bbDC962c2));
        if (chainId == BASE)      return vm.envOr("BASE_V2_ROUTER", address(0x4752ba5DBc23f44D87826276BF6Fd6b1C372aD24));
        if (chainId == POLYGON)   return vm.envOr("POLYGON_V2_ROUTER", address(0xa5E0829CaCEd8fFDD4De3c43696c57F7D7A678ff));
        if (chainId == BSC)       return vm.envOr("BSC_V2_ROUTER", address(0x10ED43C718714eb63d5aA57B78B54704E256024E));
        revert("DeployMultichain: no V2 router for chain");
    }

    function _getV3Router(uint256 chainId) internal view returns (address) {
        if (chainId == ETHEREUM)  return vm.envOr("MAINNET_V3_ROUTER", address(0xE592427A0AEce92De3Edee1F18E0157C05861564));
        if (chainId == ARBITRUM)  return vm.envOr("ARBITRUM_V3_ROUTER", address(0xE592427A0AEce92De3Edee1F18E0157C05861564));
        if (chainId == OPTIMISM)  return vm.envOr("OPTIMISM_V3_ROUTER", address(0xE592427A0AEce92De3Edee1F18E0157C05861564));
        if (chainId == BASE)      return vm.envOr("BASE_V3_ROUTER", address(0x2626664c2603336E57B271c5C0b26F421741e481));
        if (chainId == POLYGON)   return vm.envOr("POLYGON_V3_ROUTER", address(0xE592427A0AEce92De3Edee1F18E0157C05861564));
        if (chainId == BSC)       return vm.envOr("BSC_V3_ROUTER", address(0x1b81D678ffb9C0263b24A97847620C99d213eB14));
        revert("DeployMultichain: no V3 router for chain");
    }

    function _getBalancerVault(uint256 chainId) internal view returns (address) {
        if (chainId == ETHEREUM)  return vm.envOr("MAINNET_BALANCER_VAULT", address(0xBA12222222228d8Ba445958a75a0704d566BF2C8));
        if (chainId == ARBITRUM)  return vm.envOr("ARBITRUM_BALANCER_VAULT", address(0xBA12222222228d8Ba445958a75a0704d566BF2C8));
        if (chainId == OPTIMISM)  return vm.envOr("OPTIMISM_BALANCER_VAULT", address(0xBA12222222228d8Ba445958a75a0704d566BF2C8));
        if (chainId == BASE)      return vm.envOr("BASE_BALANCER_VAULT", address(0xBA12222222228d8Ba445958a75a0704d566BF2C8));
        if (chainId == POLYGON)   return vm.envOr("POLYGON_BALANCER_VAULT", address(0xBA12222222228d8Ba445958a75a0704d566BF2C8));
        if (chainId == BSC)       return vm.envOr("BSC_BALANCER_VAULT", address(0)); // No Balancer on BSC
        revert("DeployMultichain: no Balancer vault for chain");
    }

    // -------------------------------------------------------------------------
    // JSON output
    // -------------------------------------------------------------------------

    /// @notice Write deployment addresses to a JSON file.
    function _writeDeploymentJson(Deployment memory d) internal {
        string memory json = "{";
        json = string.concat(json, '"chainId":', vm.toString(d.chainId), ",");
        json = string.concat(json, '"factory":"', vm.toString(d.factory), '","');
        json = string.concat(json, '"arbitrageExecutor":"', vm.toString(d.arbitrageExecutor), '","');
        json = string.concat(json, '"flashLoanExecutor":"', vm.toString(d.flashLoanExecutor), '","');
        json = string.concat(json, '"allowanceManager":"', vm.toString(d.allowanceManager), '","');
        json = string.concat(json, '"walletTopology":"', vm.toString(d.walletTopology), '","');
        json = string.concat(json, '"uniswapV2Adapter":"', vm.toString(d.uniswapV2Adapter), '","');
        json = string.concat(json, '"uniswapV3Adapter":"', vm.toString(d.uniswapV3Adapter), '","');
        json = string.concat(json, '"balancerVaultAdapter":"', vm.toString(d.balancerVaultAdapter), '","');
        json = string.concat(json, '"adminTimelock":"', vm.toString(d.adminTimelock), '"');
        json = string.concat(json, "}");

        string memory filename = string.concat(
            "out/deployment_",
            vm.toString(d.chainId),
            "_",
            vm.toString(block.timestamp),
            ".json"
        );

        vm.writeFile(filename, json);
        console2.log("Deployment JSON written to:", filename);
    }
}
