// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

// =============================================================================
// OMEGA CRUCIBLE - Testnet Deployment Script
// Fase: Validacion antes de Mainnet
// Redes: Holesky (17000) | Arbitrum Sepolia (421614) | Polygon Amoy (80002)
//
// Este script despliega el core del protocolo en testnets para validacion
// completa antes del despliegue en mainnet. Incluye:
//   1. DeterministicFactory - factory CREATE2 (deploy directo en testnet)
//   2. WalletTopology       - segregacion de fondos y roles
//   3. UniswapV2Adapter     - adapter para DEX V2
//   4. UniswapV3Adapter     - adapter para DEX V3
//   5. BalancerVaultAdapter - adapter para Balancer (si existe vault)
//
// PRE-DEPLOY:
//   1. source .env.crucible
//   2. Fundar GAS_SPONSOR via faucet en cada testnet
//   3. Verificar balance: cast balance $GAS_SPONSOR_ADDRESS --rpc-url <RPC>
//
// USAGE:
//   # Holesky
//   forge script script/DeployCrucible.s.sol --rpc-url $RPC_HTTP_17000 --broadcast -vvvv
//
//   # Arbitrum Sepolia
//   forge script script/DeployCrucible.s.sol --rpc-url $RPC_HTTP_421614 --broadcast -vvvv
//
//   # Polygon Amoy
//   forge script script/DeployCrucible.s.sol --rpc-url $RPC_HTTP_80002 --broadcast -vvvv
//
// POST-DEPLOY:
//   1. Verificar contratos en el explorer correspondiente
//   2. Guardar las direcciones deployadas en el runbook
//   3. Ejecutar CRUCIBLE_CHECKLIST.md
// =============================================================================

import "forge-std/Script.sol";
import "../src/core/DeterministicFactory.sol";
import "../src/core/WalletTopology.sol";
import "../src/adapters/UniswapV2Adapter.sol";
import "../src/adapters/UniswapV3Adapter.sol";
import "../src/adapters/BalancerVaultAdapter.sol";

contract DeployCrucible is Script {
    // -------------------------------------------------------------------------
    // Chain IDs de testnet
    // -------------------------------------------------------------------------
    uint256 public constant HOLESKY = 17000;
    uint256 public constant ARBITRUM_SEPOLIA = 421614;
    uint256 public constant POLYGON_AMOY = 80002;

    // -------------------------------------------------------------------------
    // Estructura para almacenar resultado del deploy
    // -------------------------------------------------------------------------
    struct Deployment {
        uint256 chainId;
        string chainName;
        address gasSponsor;
        address executionSigner;
        address coldTreasury;
        address factory;
        address walletTopology;
        address uniswapV2Adapter;
        address uniswapV3Adapter;
        address balancerVaultAdapter;
    }

    Deployment public deployment;

    // -------------------------------------------------------------------------
    // Events
    // -------------------------------------------------------------------------
    event CrucibleDeployed(
        uint256 indexed chainId,
        address indexed factory,
        address indexed walletTopology,
        address uniswapV2Adapter,
        address uniswapV3Adapter,
        address balancerVaultAdapter
    );

    // -------------------------------------------------------------------------
    // Entry point
    // -------------------------------------------------------------------------
    function run() external {
        uint256 pk = vm.envUint("GAS_SPONSOR_PRIVATE_KEY");
        address deployer = vm.addr(pk);

        console2.log("============================================================");
        console2.log("  OMEGA CRUCIBLE -- Testnet Deploy");
        console2.log("============================================================");
        console2.log("Deployer       :", deployer);
        console2.log("Chain ID       :", block.chainid);
        console2.log("Block number   :", block.number);
        console2.log("Timestamp      :", block.timestamp);
        console2.log("");

        // Leer configuracion de wallets del .env
        address gasSponsor = vm.envAddress("GAS_SPONSOR_ADDRESS");
        address executionSigner = vm.envAddress("EXECUTION_SIGNER_ADDRESS");
        address coldTreasury = vm.envAddress("COLD_TREASURY_ADDRESS");

        // Validar direcciones
        require(gasSponsor != address(0), "CRUCIBLE: GAS_SPONSOR_ADDRESS no configurado");
        require(executionSigner != address(0), "CRUCIBLE: EXECUTION_SIGNER_ADDRESS no configurado");
        require(coldTreasury != address(0), "CRUCIBLE: COLD_TREASURY_ADDRESS no configurado");
        require(deployer == gasSponsor, "CRUCIBLE: deployer debe ser el gas sponsor");

        // Verificar que el deployer tiene balance suficiente
        uint256 deployerBalance = deployer.balance;
        console2.log("Deployer balance:", deployerBalance);
        require(deployerBalance > 0.001 ether, "CRUCIBLE: deployer sin balance. Fundar via faucet primero.");

        console2.log("Gas Sponsor     :", gasSponsor);
        console2.log("Execution Signer:", executionSigner);
        console2.log("Cold Treasury   :", coldTreasury);
        console2.log("");

        // Inicializar registro
        deployment.chainId = block.chainid;
        deployment.gasSponsor = gasSponsor;
        deployment.executionSigner = executionSigner;
        deployment.coldTreasury = coldTreasury;

        // Ejecutar deploy segun la red
        vm.startBroadcast(pk);

        if (block.chainid == HOLESKY) {
            deployment.chainName = "Holesky";
            _deployHolesky(gasSponsor, executionSigner, coldTreasury);
        } else if (block.chainid == ARBITRUM_SEPOLIA) {
            deployment.chainName = "Arbitrum Sepolia";
            _deployArbitrumSepolia(gasSponsor, executionSigner, coldTreasury);
        } else if (block.chainid == POLYGON_AMOY) {
            deployment.chainName = "Polygon Amoy";
            _deployPolygonAmoy(gasSponsor, executionSigner, coldTreasury);
        } else {
            revert(string.concat("CRUCIBLE: chainId ", vm.toString(block.chainid), " no es una testnet soportada"));
        }

        vm.stopBroadcast();

        // Log resultados finales
        _logDeployment();
    }

    // -------------------------------------------------------------------------
    // Deploy en Holesky (17000)
    //
    // Nota: Holesky tiene pocos protocolos DeFi desplegados.
    // Los adapters se despliegan solo si las direcciones de router/vault
    // estan configuradas (diferentes de address(0)).
    // -------------------------------------------------------------------------
    function _deployHolesky(address _gasSponsor, address _executionSigner, address _coldTreasury) internal {
        console2.log("--- Deploying on Holesky ---");

        // 1. DeterministicFactory (deploy directo, no via CREATE2)
        DeterministicFactory factory = new DeterministicFactory();
        deployment.factory = address(factory);
        console2.log("[OK] DeterministicFactory:", address(factory));

        // 2. WalletTopology
        WalletTopology walletTopology = new WalletTopology(_gasSponsor, _executionSigner, _coldTreasury);
        deployment.walletTopology = address(walletTopology);
        console2.log("[OK] WalletTopology      :", address(walletTopology));

        // 3. UniswapV2Adapter (solo si hay router configurado)
        address v2Router = vm.envOr("HOLESKY_V2_ROUTER", address(0));
        if (v2Router != address(0)) {
            UniswapV2Adapter v2Adapter = new UniswapV2Adapter(v2Router);
            deployment.uniswapV2Adapter = address(v2Adapter);
            console2.log("[OK] UniswapV2Adapter    :", address(v2Adapter));
        } else {
            console2.log("[SKIP] UniswapV2Adapter  : no router configured");
        }

        // 4. UniswapV3Adapter (solo si hay router configurado)
        address v3Router = vm.envOr("HOLESKY_V3_ROUTER", address(0));
        if (v3Router != address(0)) {
            UniswapV3Adapter v3Adapter = new UniswapV3Adapter(v3Router);
            deployment.uniswapV3Adapter = address(v3Adapter);
            console2.log("[OK] UniswapV3Adapter    :", address(v3Adapter));
        } else {
            console2.log("[SKIP] UniswapV3Adapter  : no router configured");
        }

        // 5. BalancerVaultAdapter (solo si hay vault configurado)
        address balancerVault = vm.envOr("HOLESKY_BALANCER_VAULT", address(0));
        if (balancerVault != address(0)) {
            BalancerVaultAdapter balAdapter = new BalancerVaultAdapter(balancerVault);
            deployment.balancerVaultAdapter = address(balAdapter);
            console2.log("[OK] BalancerVaultAdapter:", address(balAdapter));
        } else {
            console2.log("[SKIP] BalancerVaultAdapter: no vault configured");
        }

        emit CrucibleDeployed(
            HOLESKY,
            deployment.factory,
            deployment.walletTopology,
            deployment.uniswapV2Adapter,
            deployment.uniswapV3Adapter,
            deployment.balancerVaultAdapter
        );
    }

    // -------------------------------------------------------------------------
    // Deploy en Arbitrum Sepolia (421614)
    // -------------------------------------------------------------------------
    function _deployArbitrumSepolia(address _gasSponsor, address _executionSigner, address _coldTreasury) internal {
        console2.log("--- Deploying on Arbitrum Sepolia ---");

        // 1. DeterministicFactory
        DeterministicFactory factory = new DeterministicFactory();
        deployment.factory = address(factory);
        console2.log("[OK] DeterministicFactory:", address(factory));

        // 2. WalletTopology
        WalletTopology walletTopology = new WalletTopology(_gasSponsor, _executionSigner, _coldTreasury);
        deployment.walletTopology = address(walletTopology);
        console2.log("[OK] WalletTopology      :", address(walletTopology));

        // 3. UniswapV2Adapter
        address v2Router = vm.envOr("ARBITRUM_SEPOLIA_V2_ROUTER", address(0));
        if (v2Router != address(0)) {
            UniswapV2Adapter v2Adapter = new UniswapV2Adapter(v2Router);
            deployment.uniswapV2Adapter = address(v2Adapter);
            console2.log("[OK] UniswapV2Adapter    :", address(v2Adapter));
        } else {
            console2.log("[SKIP] UniswapV2Adapter  : no router configured");
        }

        // 4. UniswapV3Adapter
        address v3Router = vm.envOr("ARBITRUM_SEPOLIA_V3_ROUTER", address(0));
        if (v3Router != address(0)) {
            UniswapV3Adapter v3Adapter = new UniswapV3Adapter(v3Router);
            deployment.uniswapV3Adapter = address(v3Adapter);
            console2.log("[OK] UniswapV3Adapter    :", address(v3Adapter));
        } else {
            console2.log("[SKIP] UniswapV3Adapter  : no router configured");
        }

        // 5. BalancerVaultAdapter
        address balancerVault = vm.envOr("ARBITRUM_SEPOLIA_BALANCER_VAULT", address(0));
        if (balancerVault != address(0)) {
            BalancerVaultAdapter balAdapter = new BalancerVaultAdapter(balancerVault);
            deployment.balancerVaultAdapter = address(balAdapter);
            console2.log("[OK] BalancerVaultAdapter:", address(balAdapter));
        } else {
            console2.log("[SKIP] BalancerVaultAdapter: no vault configured");
        }

        emit CrucibleDeployed(
            ARBITRUM_SEPOLIA,
            deployment.factory,
            deployment.walletTopology,
            deployment.uniswapV2Adapter,
            deployment.uniswapV3Adapter,
            deployment.balancerVaultAdapter
        );
    }

    // -------------------------------------------------------------------------
    // Deploy en Polygon Amoy (80002)
    // -------------------------------------------------------------------------
    function _deployPolygonAmoy(address _gasSponsor, address _executionSigner, address _coldTreasury) internal {
        console2.log("--- Deploying on Polygon Amoy ---");

        // 1. DeterministicFactory
        DeterministicFactory factory = new DeterministicFactory();
        deployment.factory = address(factory);
        console2.log("[OK] DeterministicFactory:", address(factory));

        // 2. WalletTopology
        WalletTopology walletTopology = new WalletTopology(_gasSponsor, _executionSigner, _coldTreasury);
        deployment.walletTopology = address(walletTopology);
        console2.log("[OK] WalletTopology      :", address(walletTopology));

        // 3. UniswapV2Adapter
        address v2Router = vm.envOr("AMOY_V2_ROUTER", address(0));
        if (v2Router != address(0)) {
            UniswapV2Adapter v2Adapter = new UniswapV2Adapter(v2Router);
            deployment.uniswapV2Adapter = address(v2Adapter);
            console2.log("[OK] UniswapV2Adapter    :", address(v2Adapter));
        } else {
            console2.log("[SKIP] UniswapV2Adapter  : no router configured");
        }

        // 4. UniswapV3Adapter
        address v3Router = vm.envOr("AMOY_V3_ROUTER", address(0));
        if (v3Router != address(0)) {
            UniswapV3Adapter v3Adapter = new UniswapV3Adapter(v3Router);
            deployment.uniswapV3Adapter = address(v3Adapter);
            console2.log("[OK] UniswapV3Adapter    :", address(v3Adapter));
        } else {
            console2.log("[SKIP] UniswapV3Adapter  : no router configured");
        }

        // 5. BalancerVaultAdapter
        address balancerVault = vm.envOr("AMOY_BALANCER_VAULT", address(0));
        if (balancerVault != address(0)) {
            BalancerVaultAdapter balAdapter = new BalancerVaultAdapter(balancerVault);
            deployment.balancerVaultAdapter = address(balAdapter);
            console2.log("[OK] BalancerVaultAdapter:", address(balAdapter));
        } else {
            console2.log("[SKIP] BalancerVaultAdapter: no vault configured");
        }

        emit CrucibleDeployed(
            POLYGON_AMOY,
            deployment.factory,
            deployment.walletTopology,
            deployment.uniswapV2Adapter,
            deployment.uniswapV3Adapter,
            deployment.balancerVaultAdapter
        );
    }

    // -------------------------------------------------------------------------
    // Logging final del deployment
    // -------------------------------------------------------------------------
    function _logDeployment() internal {
        console2.log("");
        console2.log("============================================================");
        console2.log("  CRUCIBLE DEPLOY COMPLETE");
        console2.log("============================================================");
        console2.log(
            string.concat("Chain           : ", deployment.chainName, " (", vm.toString(deployment.chainId), ")")
        );
        console2.log("Gas Sponsor     :", deployment.gasSponsor);
        console2.log("Execution Signer:", deployment.executionSigner);
        console2.log("Cold Treasury   :", deployment.coldTreasury);
        console2.log("");
        console2.log("--- Deployed Contracts ---");
        console2.log("Factory           :", deployment.factory);
        console2.log("WalletTopology    :", deployment.walletTopology);
        if (deployment.uniswapV2Adapter != address(0)) {
            console2.log("UniswapV2Adapter  :", deployment.uniswapV2Adapter);
        }
        if (deployment.uniswapV3Adapter != address(0)) {
            console2.log("UniswapV3Adapter  :", deployment.uniswapV3Adapter);
        }
        if (deployment.balancerVaultAdapter != address(0)) {
            console2.log("BalancerVaultAdptr:", deployment.balancerVaultAdapter);
        }
        console2.log("");
        console2.log("--- JSON Output ---");
        console2.log("deployment_", vm.toString(deployment.chainId), ".json written to out/");
        console2.log("");
        console2.log("--- Next Steps ---");
        console2.log("[ ] 1. Verificar contratos en explorer");
        console2.log("[ ] 2. Guardar direcciones en runbook");
        console2.log("[ ] 3. Ejecutar CRUCIBLE_CHECKLIST.md");
        console2.log("[ ] 4. Ejecutar simulaciones E2E");
        console2.log("============================================================");

        // Escribir JSON
        string memory json = "{";
        json = string.concat(json, '"chainId":', vm.toString(deployment.chainId), ",");
        json = string.concat(json, '"chainName":"', deployment.chainName, '","');
        json = string.concat(json, '"gasSponsor":"', vm.toString(deployment.gasSponsor), '","');
        json = string.concat(json, '"executionSigner":"', vm.toString(deployment.executionSigner), '","');
        json = string.concat(json, '"coldTreasury":"', vm.toString(deployment.coldTreasury), '","');
        json = string.concat(json, '"factory":"', vm.toString(deployment.factory), '","');
        json = string.concat(json, '"walletTopology":"', vm.toString(deployment.walletTopology), '","');
        json = string.concat(json, '"uniswapV2Adapter":"', vm.toString(deployment.uniswapV2Adapter), '","');
        json = string.concat(json, '"uniswapV3Adapter":"', vm.toString(deployment.uniswapV3Adapter), '","');
        json = string.concat(json, '"balancerVaultAdapter":"', vm.toString(deployment.balancerVaultAdapter), '"');
        json = string.concat(json, "}");

        string memory filename = string.concat(
            "out/deployment_crucible_", vm.toString(deployment.chainId), "_", vm.toString(block.timestamp), ".json"
        );

        vm.writeFile(filename, json);
    }
}
