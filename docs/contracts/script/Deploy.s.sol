// SPDX-License-Identifier: MIT
pragma solidity 0.8.19;

import "forge-std/Script.sol";
import "forge-std/console2.sol";
import {Executor} from "../src/core/Executor.sol";
import {UniswapV2Adapter} from "../src/adapters/UniswapV2Adapter.sol";
import {UniswapV3Adapter} from "../src/adapters/UniswapV3Adapter.sol";
import {IExecutor} from "../src/interfaces/IExecutor.sol";

// ┌─────────────────────────────────────────────────────────────┐
// │  ArbitrageX-V2 (OMEGA) — CREATE2 Deployment Script          │
// │  Deterministic addresses via CREATE2 factory                │
// │  Registers adapters + verifies deployment                   │
// └─────────────────────────────────────────────────────────────┘

/// @title Deploy
/// @notice Foundry deployment script for ArbitrageX-V2 (OMEGA)
/// @author ArbitrageX Team
/// @dev Uses CREATE2 for deterministic contract addresses
/// @dev All parameters loaded from environment variables
/// @dev Run: forge script Deploy --rpc-url $RPC_URL --broadcast --verify
contract Deploy is Script {

    // ============ CREATE2 Factory ============

    /// @notice Deterministic CREATE2 deployer (e.g., 0x4e59b44847b379578588920cA78FbF26c0B4956C)
    address public constant CREATE2_FACTORY =
        0x4e59b44847b379578588920cA78FbF26c0B4956C;

    // ============ Deployment Salts ============

    /// @notice Salt for Executor contract
    bytes32 public constant EXECUTOR_SALT =
        keccak256("ARBITRAGEX_V2_EXECUTOR_OMEGA");

    /// @notice Salt for UniswapV2Adapter
    bytes32 public constant V2_ADAPTER_SALT =
        keccak256("ARBITRAGEX_V2_ADAPTER_UNISWAPV2");

    /// @notice Salt for UniswapV3Adapter
    bytes32 public constant V3_ADAPTER_SALT =
        keccak256("ARBITRAGEX_V2_ADAPTER_UNISWAPV3");

    // ============ Environment Variables ============

    /// @notice Execution signer address (authorized to submit resolutions)
    address public authorizedSigner;

    /// @notice Treasury address for yield distribution
    address public treasury;

    /// @notice Minimum yield threshold (wei)
    uint256 public minYield;

    /// @notice Contract owner
    address public owner;

    // ============ DEX Addresses ============

    /// @notice Uniswap V2 Router (mainnet: 0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D)
    address public UNISWAP_V2_ROUTER;

    /// @notice SushiSwap Router (mainnet: 0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F)
    address public SUSHISWAP_ROUTER;

    /// @notice Uniswap V3 SwapRouter (mainnet: 0xE592427A0AEce92De3Edee1F18E0157C05861564)
    address public UNISWAP_V3_ROUTER;

    /// @notice Uniswap V3 Factory (mainnet: 0x1F98431c8aD98523631AE4a59f267346ea31F984)
    address public UNISWAP_V3_FACTORY;

    // ============ Flashloan Provider Addresses ============

    /// @notice Aave V3 Pool (mainnet: 0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2)
    address public AAVE_V3_POOL;

    /// @notice Balancer V2 Vault (mainnet: 0xBA12222222228d8Ba445958a75a0704d566BF2C8)
    address public BALANCER_VAULT;

    /// @notice MakerDAO DSS Flash (mainnet: 0x60744434d6339a6B27d73d9Eda62b6F5667f5ae9)
    address public MAKER_DSS_FLASH;

    // ============ Deployed Addresses ============

    address public executorAddress;
    address public v2AdapterAddress;
    address public v3AdapterAddress;

    // ============ Chain Configuration ============

    /// @notice Chain ID => configuration mapping
    mapping(uint256 => ChainConfig) public chainConfigs;

    struct ChainConfig {
        address uniswapV2Router;
        address sushiswapRouter;
        address uniswapV3Router;
        address uniswapV3Factory;
        address aaveV3Pool;
        address balancerVault;
        address makerDssFlash;
    }

    // ============ Setup ============

    constructor() {
        // Mainnet configuration
        chainConfigs[1] = ChainConfig({
            uniswapV2Router: 0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D,
            sushiswapRouter: 0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F,
            uniswapV3Router: 0xE592427A0AEce92De3Edee1F18E0157C05861564,
            uniswapV3Factory: 0x1F98431c8aD98523631AE4a59f267346ea31F984,
            aaveV3Pool: 0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2,
            balancerVault: 0xBA12222222228d8Ba445958a75a0704d566BF2C8,
            makerDssFlash: 0x60744434d6339a6B27d73d9Eda62b6F5667f5ae9
        });

        // Arbitrum configuration
        chainConfigs[42161] = ChainConfig({
            uniswapV2Router: 0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506, // SushiSwap V2 on Arbitrum
            sushiswapRouter: 0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506,
            uniswapV3Router: 0xE592427A0AEce92De3Edee1F18E0157C05861564,
            uniswapV3Factory: 0x1F98431c8aD98523631AE4a59f267346ea31F984,
            aaveV3Pool: 0x794a61358D6845594F94dc1DB02A252b5b4814aD,
            balancerVault: 0xBA12222222228d8Ba445958a75a0704d566BF2C8,
            makerDssFlash: address(0) // Not available on Arbitrum
        });

        // Optimism configuration
        chainConfigs[10] = ChainConfig({
            uniswapV2Router: 0x4A7d4FBE541e84715641cd4fE5D5f63E31007DE4,
            sushiswapRouter: 0x4A7d4FBE541e84715641cd4fE5D5f63E31007DE4,
            uniswapV3Router: 0xE592427A0AEce92De3Edee1F18E0157C05861564,
            uniswapV3Factory: 0x1F98431c8aD98523631AE4a59f267346ea31F984,
            aaveV3Pool: 0x794a61358D6845594F94dc1DB02A252b5b4814aD,
            balancerVault: 0xBA12222222228d8Ba445958a75a0704d566BF2C8,
            makerDssFlash: address(0)
        });
    }

    // ============ Configuration Loading ============

    /// @notice Load configuration from environment variables
    function _loadConfig() internal {
        authorizedSigner = vm.envOr("AUTHORIZED_SIGNER", address(0));
        treasury = vm.envOr("TREASURY", address(0));
        minYield = vm.envOr("MIN_YIELD", uint256(0.001 ether));
        owner = vm.envOr("OWNER", msg.sender);

        // Load chain-specific DEX addresses
        uint256 chainId = block.chainid;
        ChainConfig memory config = chainConfigs[chainId];

        UNISWAP_V2_ROUTER = vm.envOr("UNISWAP_V2_ROUTER", config.uniswapV2Router);
        SUSHISWAP_ROUTER = vm.envOr("SUSHISWAP_ROUTER", config.sushiswapRouter);
        UNISWAP_V3_ROUTER = vm.envOr("UNISWAP_V3_ROUTER", config.uniswapV3Router);
        UNISWAP_V3_FACTORY = vm.envOr("UNISWAP_V3_FACTORY", config.uniswapV3Factory);
        AAVE_V3_POOL = vm.envOr("AAVE_V3_POOL", config.aaveV3Pool);
        BALANCER_VAULT = vm.envOr("BALANCER_VAULT", config.balancerVault);
        MAKER_DSS_FLASH = vm.envOr("MAKER_DSS_FLASH", config.makerDssFlash);

        // Validate required addresses
        require(authorizedSigner != address(0), "AUTHORIZED_SIGNER not set");
        require(treasury != address(0), "TREASURY not set");

        console2.log("=== Configuration ===");
        console2.log("Chain ID:", chainId);
        console2.log("Authorized Signer:", authorizedSigner);
        console2.log("Treasury:", treasury);
        console2.log("Min Yield:", minYield);
        console2.log("Owner:", owner);
    }

    /// @notice Compute deterministic CREATE2 address
    /// @param salt Deployment salt
    /// @param creationCode Contract creation bytecode
    /// @return computed The deterministic address
    function computeAddress(bytes32 salt, bytes memory creationCode)
        internal
        pure
        returns (address computed)
    {
        computed = address(
            uint160(
                uint256(
                    keccak256(
                        abi.encodePacked(
                            bytes1(0xff),
                            CREATE2_FACTORY,
                            salt,
                            keccak256(creationCode)
                        )
                    )
                )
            )
        );
    }

    // ============ Deployment ============

    /// @notice Deploy all contracts deterministically
    function run() external {
        _loadConfig();

        uint256 deployerKey = vm.envUint("PRIVATE_KEY");
        address deployer = vm.addr(deployerKey);

        console2.log("\n=== Deployer ===");
        console2.log("Address:", deployer);
        console2.log("Balance:", deployer.balance);

        // ============ Compute Addresses First ============

        bytes memory executorBytecode = _getExecutorBytecode();
        bytes memory v2AdapterBytecode = _getV2AdapterBytecode();
        bytes memory v3AdapterBytecode = _getV3AdapterBytecode();

        address computedExecutor = computeAddress(EXECUTOR_SALT, executorBytecode);
        address computedV2Adapter = computeAddress(V2_ADAPTER_SALT, v2AdapterBytecode);
        address computedV3Adapter = computeAddress(V3_ADAPTER_SALT, v3AdapterBytecode);

        console2.log("\n=== Computed Addresses (CREATE2) ===");
        console2.log("Executor:", computedExecutor);
        console2.log("V2Adapter:", computedV2Adapter);
        console2.log("V3Adapter:", computedV3Adapter);

        // ============ Execute Deployments ============

        vm.startBroadcast(deployerKey);

        // --- Deploy UniswapV2Adapter ---
        console2.log("\n--- Deploying UniswapV2Adapter ---");
        address[] memory v2Routers = new address[](2);
        v2Routers[0] = UNISWAP_V2_ROUTER;
        v2Routers[1] = SUSHISWAP_ROUTER;

        v2AdapterAddress = _deployCreate2(
            V2_ADAPTER_SALT,
            abi.encodePacked(
                type(UniswapV2Adapter).creationCode,
                abi.encode(v2Routers)
            )
        );
        console2.log("Deployed V2Adapter:", v2AdapterAddress);
        require(v2AdapterAddress == computedV2Adapter, "V2 address mismatch");

        // --- Deploy UniswapV3Adapter ---
        console2.log("\n--- Deploying UniswapV3Adapter ---");
        address[] memory v3Routers = new address[](1);
        v3Routers[0] = UNISWAP_V3_ROUTER;

        v3AdapterAddress = _deployCreate2(
            V3_ADAPTER_SALT,
            abi.encodePacked(
                type(UniswapV3Adapter).creationCode,
                abi.encode(UNISWAP_V3_FACTORY, v3Routers)
            )
        );
        console2.log("Deployed V3Adapter:", v3AdapterAddress);
        require(v3AdapterAddress == computedV3Adapter, "V3 address mismatch");

        // --- Deploy Executor (depends on adapters for registration) ---
        console2.log("\n--- Deploying Executor ---");
        executorAddress = _deployCreate2(
            EXECUTOR_SALT,
            executorBytecode
        );
        console2.log("Deployed Executor:", executorAddress);
        require(executorAddress == computedExecutor, "Executor address mismatch");

        // ============ Post-Deployment Configuration ============

        // Register adapters in Executor
        console2.log("\n--- Registering Adapters ---");
        Executor(payable(executorAddress)).registerAdapter(0, v2AdapterAddress);
        console2.log("Registered V2 adapter for dexType=0");

        Executor(payable(executorAddress)).registerAdapter(1, v3AdapterAddress);
        console2.log("Registered V3 adapter for dexType=1");

        // If owner is different from deployer, transfer ownership
        if (owner != deployer) {
            Executor(payable(executorAddress)).transferOwnership(owner);
            console2.log("Ownership transferred to:", owner);
        }

        vm.stopBroadcast();

        // ============ Verification ============

        console2.log("\n=== Verification ===");
        _verifyDeployment();

        console2.log("\n=== Deployment Complete ===");
        console2.log("Executor:", executorAddress);
        console2.log("V2Adapter:", v2AdapterAddress);
        console2.log("V3Adapter:", v3AdapterAddress);
    }

    /// @notice Deploy via CREATE2 factory
    /// @param salt Deployment salt
    /// @param creationCode Full creation bytecode with constructor args
    /// @return deployed Contract address
    function _deployCreate2(bytes32 salt, bytes memory creationCode)
        internal
        returns (address deployed)
    {
        (bool success, bytes memory returndata) = CREATE2_FACTORY.call(
            abi.encodePacked(salt, creationCode)
        );

        require(success, "CREATE2 deployment failed");

        assembly ("memory-safe") {
            deployed := shr(96, mload(add(returndata, 0x20)))
        }

        require(deployed != address(0), "CREATE2 returned zero address");
    }

    /// @notice Get Executor creation bytecode with constructor args
    function _getExecutorBytecode() internal view returns (bytes memory) {
        return abi.encodePacked(
            type(Executor).creationCode,
            abi.encode(
                authorizedSigner,
                treasury,
                minYield,
                owner,
                AAVE_V3_POOL,
                BALANCER_VAULT,
                MAKER_DSS_FLASH
            )
        );
    }

    /// @notice Get V2 Adapter creation bytecode with constructor args
    function _getV2AdapterBytecode() internal pure returns (bytes memory) {
        // Returns creation code without args — args added at deploy time
        return type(UniswapV2Adapter).creationCode;
    }

    /// @notice Get V3 Adapter creation bytecode with constructor args
    function _getV3AdapterBytecode() internal pure returns (bytes memory) {
        return type(UniswapV3Adapter).creationCode;
    }

    // ============ Verification ============

    /// @notice Verify deployment state matches expectations
    function _verifyDeployment() internal view {
        Executor exec = Executor(payable(executorAddress));

        // Verify ownership
        address actualOwner = exec.owner();
        console2.log("Owner:", actualOwner, "==", owner, "?");
        require(actualOwner == owner, "Owner mismatch");

        // Verify authorized signer
        address actualSigner = exec.authorizedSigner();
        console2.log("Signer:", actualSigner, "==", authorizedSigner, "?");
        require(actualSigner == authorizedSigner, "Signer mismatch");

        // Verify treasury
        address actualTreasury = exec.treasury();
        console2.log("Treasury:", actualTreasury, "==", treasury, "?");
        require(actualTreasury == treasury, "Treasury mismatch");

        // Verify min yield
        uint256 actualMinYield = exec.minYield();
        console2.log("Min Yield:", actualMinYield, "==", minYield, "?");
        require(actualMinYield == minYield, "Min yield mismatch");

        // Verify adapter registrations
        address v2Adapter = exec.adapters(0);
        console2.log("Adapter[0]:", v2Adapter, "==", v2AdapterAddress, "?");
        require(v2Adapter == v2AdapterAddress, "V2 adapter mismatch");

        address v3Adapter = exec.adapters(1);
        console2.log("Adapter[1]:", v3Adapter, "==", v3AdapterAddress, "?");
        require(v3Adapter == v3AdapterAddress, "V3 adapter mismatch");

        // Verify counters are zero
        require(exec.totalResolutions() == 0, "Total resolutions should be 0");
        require(exec.totalReverts() == 0, "Total reverts should be 0");
        require(exec.totalYieldCaptured() == 0, "Total yield should be 0");

        console2.log("All verifications passed!");
    }

    /// @notice Verify a deployed Executor contract matches expected state
    /// @param execAddr Address of deployed Executor
    function verifyExecutor(address execAddr) external view {
        Executor exec = Executor(payable(execAddr));

        console2.log("=== Executor State ===");
        console2.log("Owner:", exec.owner());
        console2.log("Signer:", exec.authorizedSigner());
        console2.log("Treasury:", exec.treasury());
        console2.log("Min Yield:", exec.minYield());
        console2.log("Total Resolutions:", exec.totalResolutions());
        console2.log("Total Reverts:", exec.totalReverts());
        console2.log("Total Yield:", exec.totalYieldCaptured());
        console2.log("V2 Adapter:", exec.adapters(0));
        console2.log("V3 Adapter:", exec.adapters(1));
    }

    /// @notice Estimate deployment gas costs
    function estimateGas() external view {
        bytes memory executorCode = _getExecutorBytecode();
        bytes memory v2Code = _getV2AdapterBytecode();
        bytes memory v3Code = _getV3AdapterBytecode();

        console2.log("=== Deployment Gas Estimates ===");
        console2.log("Executor bytecode size:", executorCode.length, "bytes");
        console2.log("V2Adapter bytecode size:", v2Code.length, "bytes");
        console2.log("V3Adapter bytecode size:", v3Code.length, "bytes");
        console2.log("Total:", executorCode.length + v2Code.length + v3Code.length, "bytes");

        // Rough gas estimate: 200 gas per byte for deployment
        uint256 estimatedGas = (executorCode.length + v2Code.length + v3Code.length) * 200;
        console2.log("Estimated gas:", estimatedGas);
    }

    /// @notice Deploy only the Executor (for upgrades)
    function deployExecutor() external {
        _loadConfig();

        uint256 deployerKey = vm.envUint("PRIVATE_KEY");
        address deployer = vm.addr(deployerKey);

        bytes memory bytecode = _getExecutorBytecode();
        address computed = computeAddress(EXECUTOR_SALT, bytecode);

        console2.log("Computed Executor address:", computed);

        vm.startBroadcast(deployerKey);
        executorAddress = _deployCreate2(EXECUTOR_SALT, bytecode);
        vm.stopBroadcast();

        console2.log("Deployed Executor:", executorAddress);
        require(executorAddress == computed, "Address mismatch");
    }
}
