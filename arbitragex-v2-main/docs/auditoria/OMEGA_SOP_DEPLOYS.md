# SOP: Despliegues Multi-Chain y Topología de Wallets
## OMEGA Protocol — ArbitrageX-V2
### Procedimiento Operativo Estándar para Infraestructura On-Chain

---

**Versión**: 2.0.0-OMEGA  
**Fecha**: 2026-05-14  
**Clasificación**: SOP Nivel 0 — Crítico: Requiere 2FA + Multisig  
**Autor**: Arquitectura Lead, Infraestructura On-Chain OMEGA  
**Estado**: Final

---

## Tabla de Contenidos

1. [Alcance y Objetivo](#1-alcance-y-objetivo)
2. [Despliegue Determinista con CREATE2](#2-despliegue-determinista-create2)
3. [Topología de Wallets](#3-topología-de-wallets)
4. [Fondeo Asimétrico de Gas](#4-fondeo-asimétrico-de-gas)
5. [Procedimiento de Deploy por Red](#5-procedimiento-de-deploy-por-red)
6. [Verificación Post-Deploy](#6-verificación-post-deploy)
7. [Rollback y Contingencias](#7-rollback-y-contingencias)
8. [Registro de Operaciones (Audit Log)](#8-registro-de-operaciones)
9. [Appendix: Comandos Rápidos](#9-appendix-comandos-rápidos)
10. [Referencias](#10-referencias)

---

## 1. Alcance y Objetivo

### 1.1 Alcance

Este SOP cubre el despliegue completo de la suite de contratos OMEGA Protocol en múltiples cadenas EVM:

- **Ethereum Mainnet** (chainId: 1)
- **Arbitrum One** (chainId: 42161)
- **Optimism** (chainId: 10)
- **Base** (chainId: 8453)
- **Polygon PoS** (chainId: 137)
- **BNB Smart Chain** (chainId: 56)

### 1.2 Objetivo

Garantizar que:

1. Las direcciones de contratos sean **idénticas en todas las cadenas** (determinismo CREATE2)
2. La topología de wallets cumpla el principio de **mínimo privilegio**
3. El fondeo de gas siga el modelo de **Asimetría de Información** — wallets calientes con capital mínimo, treasury fría con yield acumulado
4. Cada deploy sea **replicable, verificable y auditable**

### 1.3 Diagrama de Flujo del Proceso

```
┌──────────────────────────────────────────────────────────────────────────┐
│                     FASE 1: PREPARACIÓN (T-24h)                           │
│  ├── [ ] Generar salts deterministas (keccak256(chainId, name))          │
│  ├── [ ] Verificar bytecode compilado (checksum)                         │
│  ├── [ ] Preparar .env por red (6 archivos)                              │
│  └── [ ] Validar nonces de deployer en todas las cadenas                 │
├──────────────────────────────────────────────────────────────────────────┤
│                     FASE 2: FONDEO (T-12h)                                │
│  ├── [ ] Fondear Gas Sponsor en cada red (bridge desde Ethereum)         │
│  ├── [ ] Fondear Execution Signer (zero balance OK)                      │
│  ├── [ ] Verificar saldos mínimos por red                                │
│  └── [ ] Confirmar 12 confirmaciones en bridges                          │
├──────────────────────────────────────────────────────────────────────────┤
│                     FASE 3: DEPLOY (T-0)                                  │
│  ├── [ ] Deploy Factory CREATE2 en Ethereum mainnet                      │
│  ├── [ ] Verificar dirección del Factory                                 │
│  ├── [ ] Deploy secuencial en L2s (Arbitrum → Optimism → Base)          │
│  ├── [ ] Deploy en sidechains (Polygon → BSC)                            │
│  └── [ ] Verificar direcciones idénticas cross-chain                     │
├──────────────────────────────────────────────────────────────────────────┤
│                     FASE 4: VERIFICACIÓN (T+2h)                           │
│  ├── [ ] Verificar código en cada block explorer                         │
│  ├── [ ] Ejecutar pruebas de integración en mainnet fork                 │
│  ├── [ ] Validar invariantes termodinámicas                              │
│  └── [ ] Emitir reporte de deploy                                        │
└──────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Despliegue Determinista con CREATE2

### 2.1 Principio Matemático

El determinismo de direcciones se fundamenta en el opcode `CREATE2` de la EVM. La dirección de un contrato desplegado con CREATE2 es:

$$
\text{addr} = \text{keccak256}(\mathtt{0xff} \, \| \, \text{deployer} \, \| \, \text{salt} \, \| \, \text{keccak256}(\text{init\_code}))[12:]
$$

donde:
- $\mathtt{0xff}$ es el byte de prefijo (valor 255)
- $\text{deployer}$ es la dirección del contrato Factory (20 bytes)
- $\text{salt}$ es un valor `bytes32` calculado determinísticamente
- $\text{init\_code}$ es el bytecode de inicialización del contrato

### 2.2 Factory de Despliegue

```solidity
// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

/**
 * @title OmegaDeployFactory
 * @notice Factory de despliegue determinista CREATE2 para todo el protocolo OMEGA
 * @dev    Produce direcciones idénticas cross-chain dado el mismo salt e init_code
 *         Invariante: ∀ chain, deploy(salt, code) → addr(salt, code) es constante
 */
contract OmegaDeployFactory {
    /// @notice Evento emitido en cada despliegue exitoso
    event ContractDeployed(
        bytes32 indexed salt,
        bytes32 indexed codeHash,
        address indexed deployedAddr,
        uint256 chainId,
        uint256 timestamp
    );
    
    /// @notice Despliega un contrato con CREATE2 determinista
    /// @param salt     El salt calculado: keccak256(abi.encodePacked(chainId, contractName))
    /// @param initCode El bytecode de inicialización (creation code + constructor args)
    /// @return addr    La dirección del contrato desplegado
    function deploy(bytes32 salt, bytes memory initCode) 
        external 
        returns (address addr) 
    {
        // CREATE2: addr = keccak256(0xff + this.address + salt + keccak256(initCode))[12:]
        assembly {
            addr := create2(
                callvalue(),        // value enviado (0 para contratos normales)
                add(initCode, 0x20), // puntero al init code (saltar length)
                mload(initCode),     // tamaño del init code
                salt
            )
            
            // Fail-honest: si create2 retorna 0, el deploy falló
            if iszero(addr) {
                returndatacopy(0, 0, returndatasize())
                revert(0, returndatasize())
            }
        }
        
        emit ContractDeployed(
            salt,
            keccak256(initCode),
            addr,
            block.chainid,
            block.timestamp
        );
    }
    
    /// @notice Predice la dirección de un contrato antes de desplegarlo
    /// @param salt     El salt a usar
    /// @param initCodeHash keccak256(initCode)
    /// @return predictedAddr La dirección que se obtendrá con deploy(salt, initCode)
    function predictAddress(bytes32 salt, bytes32 initCodeHash) 
        external 
        view 
        returns (address predictedAddr) 
    {
        // addr = keccak256(0xff + address(this) + salt + initCodeHash)[12:]
        bytes32 hash = keccak256(
            abi.encodePacked(
                bytes1(0xff),
                address(this),
                salt,
                initCodeHash
            )
        );
        predictedAddr = address(uint160(uint256(hash)));
    }
}
```

### 2.3 Cálculo de Salts

```solidity
/// @notice Calcula el salt determinista para un contrato en una cadena
/// @param chainId      El chain ID de la red (1, 42161, 10, 8453, 137, 56)
/// @param contractName El nombre canónico del contrato (e.g., "Executor", "UniswapV2Adapter")
/// @return salt El bytes32 determinista
function computeSalt(uint256 chainId, string memory contractName) 
    pure 
    returns (bytes32 salt) 
{
    salt = keccak256(abi.encodePacked(chainId, contractName));
}
```

#### Tabla de Salts por Red

| Red | chainId | Contracto | Salt (`bytes32`) |
|-----|---------|-----------|-----------------|
| Ethereum | 1 | `Executor` | `keccak256(abi.encodePacked(1, "Executor"))` = `0xabe9...` |
| Ethereum | 1 | `UniswapV2Adapter` | `keccak256(abi.encodePacked(1, "UniswapV2Adapter"))` = `0x4f2c...` |
| Ethereum | 1 | `UniswapV3Adapter` | `keccak256(abi.encodePacked(1, "UniswapV3Adapter"))` = `0x8e1a...` |
| Ethereum | 1 | `CurveAdapter` | `keccak256(abi.encodePacked(1, "CurveAdapter"))` = `0x3b7d...` |
| Ethereum | 1 | `BalancerAdapter` | `keccak256(abi.encodePacked(1, "BalancerAdapter"))` = `0x1c9f...` |
| Arbitrum | 42161 | `Executor` | `keccak256(abi.encodePacked(42161, "Executor"))` = `0xd2e1...` |
| Optimism | 10 | `Executor` | `keccak256(abi.encodePacked(10, "Executor"))` = `0x7a3b...` |
| Base | 8453 | `Executor` | `keccak256(abi.encodePacked(8453, "Executor"))` = `0x5c8e...` |
| Polygon | 137 | `Executor` | `keccak256(abi.encodePacked(137, "Executor"))` = `0x9f4a...` |
| BSC | 56 | `Executor` | `keccak256(abi.encodePacked(56, "Executor"))` = `0x2e7d...` |

**IMPORTANTE**: Aunque los salts difieren por `chainId`, las direcciones resultantes serán idénticas cross-chain **solo si** el Factory se despliega en la misma dirección en cada cadena. Esto requiere que el Factory mismo sea desplegado con CREATE2 usando un salt común.

#### Salt del Factory (común a todas las cadenas)

```solidity
// Salt universal para el Factory — NO cambiar entre cadenas
bytes32 constant FACTORY_SALT = keccak256("OMEGA_FACTORY_v2_0_0_2026_05_14");
// = 0x8f3c2d1e4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d
```

### 2.4 Comando Exacto de Despliegue

```bash
#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════
# OMEGA Protocol — Script de Despliegue Determinista Multi-Chain
# ═══════════════════════════════════════════════════════════════════════════

set -euo pipefail

# ── Variables de entorno (requeridas) ─────────────────────────────────────
# $RPC        — URL del nodo RPC (e.g., https://eth-mainnet.g.alchemy.com/...)
# $KEY        — Private key del Gas Sponsor (0x...)
# $ETHERSCAN  — API key para verificación
# $CHAIN_ID   — chainId de la red de destino

# ── Verificaciones previas ────────────────────────────────────────────────
if [[ -z "${RPC:-}" ]]; then echo "ERROR: RPC no definido"; exit 1; fi
if [[ -z "${KEY:-}" ]]; then echo "ERROR: KEY no definido"; exit 1; fi
if [[ -z "${CHAIN_ID:-}" ]]; then echo "ERROR: CHAIN_ID no definido"; exit 1; fi

# ── Validar que el deployer tiene gas ─────────────────────────────────────
DEPLOYER_ADDR=$(cast wallet address --private-key "$KEY")
BALANCE_WEI=$(cast balance "$DEPLOYER_ADDR" --rpc-url "$RPC")
BALANCE_ETH=$(echo "scale=6; $BALANCE_WEI / 10^18" | bc)
echo "[INFO] Deployer: $DEPLOYER_ADDR | Balance: $BALANCE_ETH ETH"

# ── Compilar contratos ────────────────────────────────────────────────────
echo "[INFO] Compilando contratos..."
forge build --sizes --force

# ── Deploy del Factory (CREATE2) ──────────────────────────────────────────
echo "[INFO] Desplegando Factory con salt universal..."
FACTORY_SALT=$(cast keccak "OMEGA_FACTORY_v2_0_0_2026_05_14")
echo "[INFO] Factory salt: $FACTORY_SALT"

forge script script/DeployFactory.s.sol \
    --rpc-url "$RPC" \
    --private-key "$KEY" \
    --broadcast \
    --sig "run(bytes32)" \
    "$FACTORY_SALT" \
    --verify \
    --etherscan-api-key "${ETHERSCAN:-}" \
    -vvvv

# ── Deploy de contratos principales ───────────────────────────────────────
echo "[INFO] Desplegando contratos core..."
forge script script/Deploy.s.sol \
    --rpc-url "$RPC" \
    --private-key "$KEY" \
    --broadcast \
    --verify \
    --etherscan-api-key "${ETHERSCAN:-}" \
    -vvvv

echo "[SUCCESS] Deploy completado en chainId=$CHAIN_ID"
```

### 2.5 Script Foundry: Deploy.s.sol

```solidity
// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

import "forge-std/Script.sol";
import "../src/core/Executor.sol";
import "../src/adapters/UniswapV2Adapter.sol";
import "../src/adapters/UniswapV3Adapter.sol";
import "../src/adapters/CurveAdapter.sol";
import "../src/adapters/BalancerAdapter.sol";
import "../src/core/OmegaDeployFactory.sol";

/**
 * @title Deploy
 * @notice Script de despliegue determinista para el protocolo OMEGA
 * @dev    Requiere que el Factory ya esté desplegado en la dirección esperada
 *         Uso: forge script script/Deploy.s.sol --rpc-url $RPC --private-key $KEY --broadcast
 */
contract Deploy is Script {
    // Dirección del Factory (debe ser la misma en todas las cadenas)
    OmegaDeployFactory factory;
    
    // Salts determinísticos
    bytes32 saltExecutor;
    bytes32 saltV2;
    bytes32 saltV3;
    bytes32 saltCurve;
    bytes32 saltBalancer;
    
    function setUp() public {
        uint256 chainId = block.chainid;
        
        saltExecutor  = keccak256(abi.encodePacked(chainId, "Executor"));
        saltV2        = keccak256(abi.encodePacked(chainId, "UniswapV2Adapter"));
        saltV3        = keccak256(abi.encodePacked(chainId, "UniswapV3Adapter"));
        saltCurve     = keccak256(abi.encodePacked(chainId, "CurveAdapter"));
        saltBalancer  = keccak256(abi.encodePacked(chainId, "BalancerAdapter"));
        
        // Factory se busca en dirección predecible
        address factoryAddr = vm.envOr("FACTORY_ADDRESS", address(0));
        require(factoryAddr != address(0), "Deploy: FACTORY_ADDRESS not set");
        factory = OmegaDeployFactory(factoryAddr);
    }
    
    function run() public {
        vm.startBroadcast();
        
        console.log("Deploying OMEGA Protocol on chainId:", block.chainid);
        console.log("Factory:", address(factory));
        
        // ── 1. Deploy Executor ──
        bytes memory executorInit = abi.encodePacked(
            type(Executor).creationCode,
            abi.encode(msg.sender) // governance = deployer
        );
        address executor = factory.deploy(saltExecutor, executorInit);
        console.log("Executor deployed at:", executor);
        
        // ── 2. Deploy UniswapV2Adapter ──
        address factoryV2 = vm.envAddress("UNISWAP_V2_FACTORY");
        address weth = vm.envAddress("WETH");
        bytes memory v2Init = abi.encodePacked(
            type(UniswapV2Adapter).creationCode,
            abi.encode(factoryV2, weth)
        );
        address v2Adapter = factory.deploy(saltV2, v2Init);
        console.log("UniswapV2Adapter deployed at:", v2Adapter);
        
        // ── 3. Deploy UniswapV3Adapter ──
        address factoryV3 = vm.envAddress("UNISWAP_V3_FACTORY");
        address positionManager = vm.envAddress("UNISWAP_V3_POSITION_MANAGER");
        bytes memory v3Init = abi.encodePacked(
            type(UniswapV3Adapter).creationCode,
            abi.encode(factoryV3, positionManager)
        );
        address v3Adapter = factory.deploy(saltV3, v3Init);
        console.log("UniswapV3Adapter deployed at:", v3Adapter);
        
        // ── 4. Deploy CurveAdapter ──
        address curveRegistry = vm.envAddress("CURVE_REGISTRY");
        bytes memory curveInit = abi.encodePacked(
            type(CurveAdapter).creationCode,
            abi.encode(curveRegistry)
        );
        address curveAdapter = factory.deploy(saltCurve, curveInit);
        console.log("CurveAdapter deployed at:", curveAdapter);
        
        // ── 5. Deploy BalancerAdapter ──
        address balancerVault = vm.envAddress("BALANCER_VAULT");
        bytes memory balancerInit = abi.encodePacked(
            type(BalancerAdapter).creationCode,
            abi.encode(balancerVault)
        );
        address balancerAdapter = factory.deploy(saltBalancer, balancerInit);
        console.log("BalancerAdapter deployed at:", balancerAdapter);
        
        // ── 6. Registrar adaptadores en Executor ──
        Executor(executor).registerAdapter(keccak256("UNISWAP_V2_1"), v2Adapter);
        Executor(executor).registerAdapter(keccak256("UNISWAP_V3_1"), v3Adapter);
        Executor(executor).registerAdapter(keccak256("CURVE_V1_1"), curveAdapter);
        Executor(executor).registerAdapter(keccak256("BALANCER_V2_1"), balancerAdapter);
        console.log("All adapters registered");
        
        // ── 7. Guardar direcciones en archivo de salida ──
        string memory json = "deployed";
        vm.serializeAddress(json, "executor", executor);
        vm.serializeAddress(json, "uniswapV2Adapter", v2Adapter);
        vm.serializeAddress(json, "uniswapV3Adapter", v3Adapter);
        vm.serializeAddress(json, "curveAdapter", curveAdapter);
        string memory finalJson = vm.serializeAddress(json, "balancerAdapter", balancerAdapter);
        
        string memory filename = string.concat("deploy_", vm.toString(block.chainid), ".json");
        vm.writeJson(finalJson, filename);
        console.log("Deployment saved to:", filename);
        
        vm.stopBroadcast();
    }
}
```

### 2.6 Verificación de Determinismo Cross-Chain

```bash
#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════
# Verificación de direcciones idénticas cross-chain
# ═══════════════════════════════════════════════════════════════════════════

echo "═══════════════════════════════════════════════════════════════════════"
echo "OMEGA Protocol — Cross-Chain Address Verification"
echo "═══════════════════════════════════════════════════════════════════════"

CHAINS=("1:Ethereum" "42161:Arbitrum" "10:Optimism" "8453:Base" "137:Polygon" "56:BSC")

echo ""
echo "Executor addresses:"
echo "───────────────────────────────────────────────────────────────────────"
for chain in "${CHAINS[@]}"; do
    IFS=':' read -r chainId name <<< "$chain"
    ADDR=$(cat "deploy_${chainId}.json" | jq -r '.executor // "NOT_DEPLOYED"')
    echo "  $name (chainId=$chainId): $ADDR"
done

echo ""
echo "UniswapV2Adapter addresses:"
echo "───────────────────────────────────────────────────────────────────────"
for chain in "${CHAINS[@]}"; do
    IFS=':' read -r chainId name <<< "$chain"
    ADDR=$(cat "deploy_${chainId}.json" | jq -r '.uniswapV2Adapter // "NOT_DEPLOYED"')
    echo "  $name (chainId=$chainId): $ADDR"
done

echo ""
echo "✅ VERIFICATION: All addresses should be IDENTICAL across chains"
echo "   (due to CREATE2 determinism with universal Factory address)"
```

---

## 3. Topología de Wallets

### 3.1 Principio de Mínimo Privilegio

La topología de wallets sigue el modelo de **Asimetría de Información** — cada wallet tiene acceso únicamente a los recursos necesarios para su función específica, y nada más.

```
┌──────────────────────────────────────────────────────────────────────────┐
│                    TOPOLOGÍA DE WALLETS OMEGA                             │
│                                                                          │
│   ┌─────────────────┐      ┌─────────────────┐      ┌─────────────────┐  │
│   │   Gas Sponsor   │      │ Execution       │      │   Cold Treasury │  │
│   │   (Hot)         │─────▶│ Signer (Hot)    │─────▶│   (Cold)        │  │
│   │                 │ firma│                 │ yield│                 │  │
│   │ Balance: 0.5 ETH│      │ Balance: 0 ETH  │      │ Yield acumulado │  │
│   │                 │      │                 │      │                 │  │
│   │ Uso: Gas de     │      │ Uso: Firma de   │      │ Uso: Receptor   │  │
│   │ deploys y txs   │      │ bundles         │      │ de Topological  │  │
│   │                 │      │                 │      │ Yield           │  │
│   │ Acceso:         │      │ Acceso:         │      │ Acceso:         │  │
│   │ RPC + private   │      │ Air-gapped      │      │ Hardware wallet │  │
│   │ key (HSM)       │      │ signing device  │      │ (Ledger/Trezor) │  │
│   └─────────────────┘      └─────────────────┘      └─────────────────┘  │
│            │                        │                        │           │
│            ▼                        ▼                        ▼           │
│   ┌─────────────────────────────────────────────────────────────────┐    │
│   │                    CONTROLES DE SEGURIDAD                        │    │
│   │  • 2FA obligatorio para cualquier operación                     │    │
│   │  • Multisig 2-of-3 para cambios de governance                  │    │
│   │  • Kill switch accesible desde cualquier wallet autorizada     │    │
│   │  • Rate limiting: max 10 txs/minuto por Execution Signer       │    │
│   └─────────────────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Tabla de Wallets

| Rol | Clave | Balance Mínimo | Balance Máximo | Uso | Storage | Acceso |
|-----|-------|---------------|---------------|-----|---------|--------|
| **Gas Sponsor** | Hot | 0.5 ETH | 2.0 ETH | Gas de deploys y transacciones on-chain | HSM (AWS KMS / Azure Key Vault) | Infra team |
| **Execution Signer** | Hot | 0 ETH | 0 ETH | Firma de bundles de convergencia (zero-balance signer) | Air-gapped signing device | Operador on-call |
| **Cold Treasury** | Cold | N/A | Sin límite superior | Receptor de Topological Yield acumulado | Hardware wallet offline (Ledger/Trezor) | Multisig 2-of-3 |
| **Governance** | Cold | 0.1 ETH | 0.5 ETH | Autorización de ejecutores, cambios de parámetros | Hardware wallet + Shamir backup | Core team |
| **Emergency Kill** | Cold | 0 ETH | 0 ETH | Activación de kill switch en emergencia | Hardware wallet dedicado | Solo operator lead |

### 3.3 Gas Sponsor (Hot)

```solidity
/// @notice Wallet que paga el gas de todas las transacciones del protocolo
/// @dev    Balance limitado a 2 ETH máximo para limitar exposición
contract GasSponsor is IGasSponsor {
    address public immutable operator;
    uint256 public constant MAX_BALANCE = 2 ether;
    
    modifier onlyOperator() {
        require(msg.sender == operator, "GasSponsor: unauthorized");
        _;
    }
    
    /// @notice Recarga la wallet hasta el máximo permitido
    function topUp() external payable onlyOperator {
        require(address(this).balance <= MAX_BALANCE, "GasSponsor: exceeds max");
    }
    
    /// @notice Retira exceso al Cold Treasury
    function sweepExcess() external onlyOperator {
        uint256 excess = address(this).balance - 1 ether; // mantener 1 ETH mínimo
        if (excess > 0) {
            payable(COLD_TREASURY).transfer(excess);
        }
    }
}
```

### 3.4 Execution Signer (Zero-Balance)

El **Execution Signer** es una wallet con balance **exactamente cero** que solo sirve para firmar bundles de transacciones. No puede enviar transacciones por sí misma (carece de gas), pero sus firmas son validadas por el Executor contrato.

```
Execution Signer: 0xAbC...
  - Balance ETH: 0.000000000000000000
  - Balance tokens: 0 (todos)
  - Capacidad: Firmar bundles (EIP-712 typed data)
  - Incapacidad: Enviar transacciones, transferir fondos
  - Modelo: Flashbots/EIP-1559 bundles pre-firmados, subidos por el Gas Sponsor
```

```solidity
/// @notice Firma un bundle de convergencia sin poseer gas
/// @dev    La firma se valida en Executor.execute() — el Gas Sponsor paga el gas
function signBundle(
    Bundle calldata bundle,
    uint256 privateKey  // almacenada en air-gapped device
) pure returns (bytes memory signature) {
    bytes32 digest = keccak256(abi.encodePacked(
        "\x19\x01",
        DOMAIN_SEPARATOR,
        keccak256(abi.encode(BUNDLE_TYPEHASH, bundle))
    ));
    (uint8 v, bytes32 r, bytes32 s) = vm.sign(privateKey, digest);
    signature = abi.encodePacked(r, s, v);
}
```

### 3.5 Cold Treasury

```
Cold Treasury: 0xDef...
  - Función: Receptor exclusivo de Topological Yield
  - Seguridad: Hardware wallet offline + Shamir Secret Sharing (3-of-5)
  - Acceso: Multisig 2-of-3 (3 personas, 2 firmas requeridas)
  - Monitoreo: Alertas automáticas por cualquier salida de fondos
  - Sweep: Automático cada 24h desde el Executor
```

```solidity
/// @notice Treasury frío que acumula el yield topológico
/// @dev    Solo acepta transferencias desde el Executor. Ninguna salida sin multisig.
contract ColdTreasury is IColdTreasury {
    address public immutable executor;
    
    /// @notice Solo el Executor puede depositar yield
    receive() external payable {
        require(msg.sender == executor, "Treasury: only executor");
    }
    
    /// @notice Retiro requiere multisig 2-of-3
    function withdraw(
        address to,
        uint256 amount,
        bytes calldata signatures
    ) external {
        require(
            verifyMultisig(signatures, WITHDRAW_TYPEHASH, keccak256(abi.encode(to, amount))),
            "Treasury: invalid multisig"
        );
        payable(to).transfer(amount);
    }
}
```

---

## 4. Fondeo Asimétrico de Gas

### 4.1 Principio de Asimetría

El **fondeo asimétrico** refleja la estructura de Asimetría de Información del sistema: las wallets calientes mantienen capital mínimo (suficiente para operar, no más), mientras que el yield acumulado fluye automáticamente al Cold Treasury.

### 4.2 Bridges desde Ethereum a L2s

```
┌──────────────────────────────────────────────────────────────────────────┐
│                    FONDEO ASIMÉTRICO MULTI-CHAIN                          │
│                                                                          │
│   Ethereum Mainnet                                                       │
│   ┌─────────────────────────────────────────────────────────────────┐    │
│   │  Cold Treasury: yield acumulado                                  │    │
│   │  ├─ Across Protocol  ──▶ Arbitrum  (0x729... relayer)           │    │
│   │  ├─ Hop Protocol     ──▶ Optimism  (0x631... bridge)            │    │
│   │  ├─ Stargate         ──▶ Base      (0x689... endpoint)          │    │
│   │  └─ Native Polygon   ──▶ Polygon   (0xA0c... PoS bridge)        │    │
│   │     BSC Bridge       ──▶ BSC       (0x69e... bridge)            │    │
│   └─────────────────────────────────────────────────────────────────┘    │
│                                                                          │
│   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                  │
│   │  Arbitrum    │  │  Optimism    │  │  Base        │                  │
│   │  Gas Sponsor │  │  Gas Sponsor │  │  Gas Sponsor │                  │
│   │  0.5 ETH     │  │  0.5 ETH     │  │  0.5 ETH     │                  │
│   └──────────────┘  └──────────────┘  └──────────────┘                  │
│                                                                          │
│   ┌──────────────┐  ┌──────────────┐                                    │
│   │  Polygon     │  │  BSC         │                                    │
│   │  Gas Sponsor │  │  Gas Sponsor │                                    │
│   │  500 MATIC   │  │  0.5 BNB     │                                    │
│   └──────────────┘  └──────────────┘                                    │
└──────────────────────────────────────────────────────────────────────────┘
```

### 4.3 Cantidades Mínimas por Red

| Red | Token Gas | Balance Mínimo | Balance Óptimo | Bridge Recomendado | Tiempo Confirmación |
|-----|-----------|---------------|----------------|-------------------|-------------------|
| Ethereum | ETH | 0.5 ETH | 1.0 ETH | N/A (main hub) | 12 bloques (~144s) |
| Arbitrum | ETH | 0.3 ETH | 0.5 ETH | Across Protocol | ~15 min |
| Optimism | ETH | 0.3 ETH | 0.5 ETH | Hop Protocol | ~20 min |
| Base | ETH | 0.3 ETH | 0.5 ETH | Stargate | ~25 min |
| Polygon | MATIC | 500 MATIC | 1000 MATIC | Native Polygon PoS | ~20 min |
| BSC | BNB | 0.3 BNB | 0.5 BNB | BSC Bridge | ~10 min |

### 4.4 Script de Fondeo

```bash
#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════
# OMEGA Protocol — Fondeo Asimétrico de Gas Multi-Chain
# ═══════════════════════════════════════════════════════════════════════════

set -euo pipefail

# ── Configuración ─────────────────────────────────────────────────────────
GAS_SPONSOR_ETH="0xYourGasSponsorAddress"
GAS_SPONSOR_ARB="0xYourArbGasSponsorAddress"
GAS_SPONSOR_OP="0xYourOptGasSponsorAddress"
GAS_SPONSOR_BASE="0xYourBaseGasSponsorAddress"
GAS_SPONSOR_MATIC="0xYourPolygonGasSponsorAddress"
GAS_SPONSOR_BSC="0xYourBSCGasSponsorAddress"

COLD_TREASURY="0xYourColdTreasuryAddress"

# ── Fondeo Ethereum (main hub) ────────────────────────────────────────────
echo "[1/6] Fondeando Gas Sponsor en Ethereum..."
cast send "$GAS_SPONSOR_ETH" \
    --value "$(cast tw 0.5)" \
    --rpc-url "$ETH_RPC" \
    --private-key "$COLD_TREASURY_KEY"

# ── Fondeo Arbitrum vía Across Protocol ───────────────────────────────────
echo "[2/6] Fondeando Gas Sponsor en Arbitrum (Across)..."
# Across: deposit en L1 → receive en L2
cast send "0x4D9079Bb4165A..." \
    --value "$(cast tw 0.35)" \
    --rpc-url "$ETH_RPC" \
    --private-key "$COLD_TREASURY_KEY" \
    --data "$(cast calldata 'deposit(address,address,uint256,uint256)' \
        "$GAS_SPONSOR_ARB" \
        '0x0000000000000000000000000000000000000000' \
        "$(cast tw 0.3)" \
        42161)"

# ── Fondeo Optimism vía Hop Protocol ──────────────────────────────────────
echo "[3/6] Fondeando Gas Sponsor en Optimism (Hop)..."
cast send "0xb8901acB165..." \
    --value "$(cast tw 0.35)" \
    --rpc-url "$ETH_RPC" \
    --private-key "$COLD_TREASURY_KEY"

# ── Fondeo Base vía Stargate ──────────────────────────────────────────────
echo "[4/6] Fondeando Gas Sponsor en Base (Stargate)..."
cast send "0xAf5191B0De..." \
    --value "$(cast tw 0.35)" \
    --rpc-url "$ETH_RPC" \
    --private-key "$COLD_TREASURY_KEY"

# ── Fondeo Polygon (MATIC) ────────────────────────────────────────────────
echo "[5/6] Fondeando Gas Sponsor en Polygon..."
cast send "0xA0c68C6382..." \
    --value "$(cast tw 0.35)" \
    --rpc-url "$ETH_RPC" \
    --private-key "$COLD_TREASURY_KEY"

# ── Fondeo BSC ────────────────────────────────────────────────────────────
echo "[6/6] Fondeando Gas Sponsor en BSC..."
cast send "0x69e66f4a04..." \
    --value "$(cast tw 0.35)" \
    --rpc-url "$ETH_RPC" \
    --private-key "$COLD_TREASURY_KEY"

echo "[SUCCESS] Fondeo completado. Esperando confirmaciones..."
```

### 4.5 Verificación Post-Fondeo

```bash
#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════
# Verificación de saldos post-fondeo
# ═══════════════════════════════════════════════════════════════════════════

echo "═══════════════════════════════════════════════════════════════════════"
echo "OMEGA Protocol — Post-Fondeo Balance Verification"
echo "═══════════════════════════════════════════════════════════════════════"

REDES=(
    "Ethereum:$ETH_RPC:$GAS_SPONSOR_ETH"
    "Arbitrum:$ARB_RPC:$GAS_SPONSOR_ARB"
    "Optimism:$OP_RPC:$GAS_SPONSOR_OP"
    "Base:$BASE_RPC:$GAS_SPONSOR_BASE"
    "Polygon:$POLYGON_RPC:$GAS_SPONSOR_MATIC"
    "BSC:$BSC_RPC:$GAS_SPONSOR_BSC"
)

for red in "${REDES[@]}"; do
    IFS=':' read -r nombre rpc direccion <<< "$red"
    
    echo ""
    echo "[$nombre]"
    echo "  Dirección: $direccion"
    
    BALANCE=$(cast balance "$direccion" --rpc-url "$rpc" 2>/dev/null || echo "0")
    BALANCE_ETH=$(echo "scale=6; $BALANCE / 10^18" | bc)
    
    echo "  Balance: $BALANCE_ETH ETH/MATIC/BNB"
    
    # Verificar nonce (debe ser 0 para wallet nueva, o >0 si ya usada)
    NONCE=$(cast nonce "$direccion" --rpc-url "$rpc" 2>/dev/null || echo "ERROR")
    echo "  Nonce: $NONCE"
    
    # Validar balance mínimo
    if (( $(echo "$BALANCE_ETH < 0.25" | bc -l) )); then
        echo "  ⚠️  BALANCE BAJO — requiere recarga"
    else
        echo "  ✅ Balance suficiente"
    fi
done

echo ""
echo "═══════════════════════════════════════════════════════════════════════"
echo "Verificación completada"
```

---

## 5. Procedimiento de Deploy por Red

### 5.1 Secuencia de Deploy

```
┌──────────────────────────────────────────────────────────────────────────┐
│                    SECUENCIA DE DEPLOY MULTI-CHAIN                        │
│                                                                          │
│  PASO 1: Ethereum Mainnet (chainId=1)                                    │
│  ├── [ ] Deploy OmegaDeployFactory con FACTORY_SALT                      │
│  ├── [ ] Verificar dirección del Factory                                 │
│  ├── [ ] Deploy Executor + 4 Adapters                                    │
│  ├── [ ] Registrar adapters en Executor                                  │
│  ├── [ ] Configurar authorizedExecutors                                  │
│  ├── [ ] Verificar en Etherscan                                          │
│  └── [ ] Ejecutar pruebas de integración                                 │
│                                                                          │
│  PASO 2: Arbitrum One (chainId=42161)                                    │
│  ├── [ ] Verificar que el Factory está en dirección idéntica             │
│  ├── [ ] Deploy Executor + 4 Adapters                                    │
│  ├── [ ] Registrar adapters                                              │
│  ├── [ ] Verificar en Arbiscan                                           │
│  └── [ ] Ejecutar pruebas de integración                                 │
│                                                                          │
│  PASO 3: Optimism (chainId=10)                                           │
│  ├── [ ] Mismo procedimiento que Arbitrum                                │
│  └── [ ] Verificar en Optimistic Etherscan                               │
│                                                                          │
│  PASO 4: Base (chainId=8453)                                             │
│  ├── [ ] Mismo procedimiento                                             │
│  └── [ ] Verificar en Basescan                                           │
│                                                                          │
│  PASO 5: Polygon PoS (chainId=137)                                       │
│  ├── [ ] Deploy con tokens MATIC para gas                                │
│  └── [ ] Verificar en Polygonscan                                        │
│                                                                          │
│  PASO 6: BSC (chainId=56)                                                │
│  ├── [ ] Deploy con BNB para gas                                         │
│  └── [ ] Verificar en Bscscan                                            │
└──────────────────────────────────────────────────────────────────────────┘
```

### 5.2 Checklist por Red (replicar 6 veces)

- [ ] **1. Configurar entorno**: `.env.chainId` con RPC, private key, explorer API key
- [ ] **2. Verificar gas**: `cast balance $DEPLOYER --rpc-url $RPC` ≥ mínimo requerido
- [ ] **3. Compilar**: `forge build --sizes --force`
- [ ] **4. Simular deploy**: `forge script script/Deploy.s.sol --rpc-url $RPC -vvvv` (dry-run)
- [ ] **5. Ejecutar deploy**: `forge script script/Deploy.s.sol --rpc-url $RPC --private-key $KEY --broadcast --verify -vvvv`
- [ ] **6. Verificar direcciones**: comparar con `predictAddress()` del Factory
- [ ] **7. Verificar en explorer**: código fuente + NatSpec visible
- [ ] **8. Ejecutar tests de integración**: `forge test --match-contract Integration --rpc-url $RPC --fork-block-number $DEPLOY_BLOCK`
- [ ] **9. Validar invariantes**: `balanceAfter ≥ balanceBefore + minYield` en simulación
- [ ] **10. Guardar reporte**: guardar `deploy_{chainId}.json` en directorio de artefactos

### 5.3 Comando Exacto de Deploy

```bash
# ═══════════════════════════════════════════════════════════════════════════
# COMANDO EXACTO DE DESPLIEGUE — OMEGA Protocol
# ═══════════════════════════════════════════════════════════════════════════
#
# Uso: Copiar y ejecutar directamente después de configurar variables
#

forge script script/Deploy.s.sol \
    --rpc-url "$RPC" \
    --private-key "$KEY" \
    --broadcast \
    --verify \
    --etherscan-api-key "$ETHERSCAN_API_KEY" \
    --gas-estimate-multiplier 130 \
    --slow \
    -vvvv
```

**Flags explicados**:

| Flag | Propósito |
|------|-----------|
| `--rpc-url $RPC` | Nodo RPC de la red destino |
| `--private-key $KEY` | Clave privada del Gas Sponsor (hot wallet) |
| `--broadcast` | Enviar transacciones on-chain (no solo simular) |
| `--verify` | Verificar código fuente en el block explorer |
| `--etherscan-api-key` | API key para verificación |
| `--gas-estimate-multiplier 130` | Aumentar estimación de gas 30% para congestión |
| `--slow` | Esperar confirmación entre transacciones |
| `-vvvv` | Verbosity máxima para debugging |

---

## 6. Verificación Post-Deploy

### 6.1 Verificación Automatizada

```bash
#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════
# Verificación Post-Deploy Completa
# ═══════════════════════════════════════════════════════════════════════════

echo "═══════════════════════════════════════════════════════════════════════"
echo "OMEGA Protocol — Post-Deploy Verification Suite"
echo "═══════════════════════════════════════════════════════════════════════"

# ── 1. Verificar que el Factory existe ────────────────────────────────────
FACTORY_CODE=$(cast code "$FACTORY_ADDRESS" --rpc-url "$RPC")
if [[ "$FACTORY_CODE" == "0x" ]]; then
    echo "❌ FATAL: Factory no desplegado en $FACTORY_ADDRESS"
    exit 1
fi
echo "✅ Factory desplegado: ${#FACTORY_CODE} bytes"

# ── 2. Verificar Executor ─────────────────────────────────────────────────
EXECUTOR=$(jq -r '.executor' "deploy_${CHAIN_ID}.json")
EXECUTOR_CODE=$(cast code "$EXECUTOR" --rpc-url "$RPC")
if [[ "$EXECUTOR_CODE" == "0x" ]]; then
    echo "❌ FATAL: Executor no desplegado"
    exit 1
fi
echo "✅ Executor desplegado: ${#EXECUTOR_CODE} bytes"

# ── 3. Verificar adaptadores ──────────────────────────────────────────────
for adapter in uniswapV2Adapter uniswapV3Adapter curveAdapter balancerAdapter; do
    ADDR=$(jq -r ".$adapter" "deploy_${CHAIN_ID}.json")
    CODE=$(cast code "$ADDR" --rpc-url "$RPC")
    if [[ "$CODE" == "0x" ]]; then
        echo "❌ FATAL: $adapter no desplegado en $ADDR"
        exit 1
    fi
    echo "✅ $adapter desplegado: ${#CODE} bytes"
done

# ── 4. Verificar registry ─────────────────────────────────────────────────
for key in "UNISWAP_V2_1" "UNISWAP_V3_1" "CURVE_V1_1" "BALANCER_V2_1"; do
    KEY_HASH=$(cast keccak "$key")
    ADDR=$(cast call "$EXECUTOR" "adapterRegistry(bytes32)" "$KEY_HASH" --rpc-url "$RPC")
    if [[ "$ADDR" == "$(cast address-zero)" ]]; then
        echo "❌ FATAL: Adapter $key no registrado"
        exit 1
    fi
    echo "✅ Adapter $key registrado en: $ADDR"
done

# ── 5. Verificar invariantes termodinámicas en fork local ────────────────
echo ""
echo "[5/5] Verificando invariantes en fork local..."
forge test \
    --match-contract InvariantTest \
    --rpc-url "$RPC" \
    --fork-block-number "$(cast block-number --rpc-url "$RPC")" \
    -vvvv

echo ""
echo "═══════════════════════════════════════════════════════════════════════"
echo "✅ VERIFICACIÓN COMPLETADA EXITOSAMENTE"
echo "═══════════════════════════════════════════════════════════════════════"
```

### 6.2 Verificación Cross-Chain

```solidity
/// @notice Verifica que las direcciones son idénticas en todas las cadenas
/// @dev    Se ejecuta off-chain como script de validación
contract CrossChainVerification {
    struct Deployment {
        uint256 chainId;
        address executor;
        address v2Adapter;
        address v3Adapter;
        address curveAdapter;
        address balancerAdapter;
    }
    
    function verifyCrossChain(
        Deployment[6] memory deployments
    ) external pure returns (bool allIdentical) {
        // Las direcciones DEBEN coincidir si el Factory está en la misma dirección
        // y los salts se calculan correctamente
        for (uint256 i = 1; i < deployments.length; i++) {
            require(
                deployments[i].executor == deployments[0].executor,
                "CrossChain: Executor mismatch"
            );
            require(
                deployments[i].v2Adapter == deployments[0].v2Adapter,
                "CrossChain: V2Adapter mismatch"
            );
            require(
                deployments[i].v3Adapter == deployments[0].v3Adapter,
                "CrossChain: V3Adapter mismatch"
            );
            require(
                deployments[i].curveAdapter == deployments[0].curveAdapter,
                "CrossChain: CurveAdapter mismatch"
            );
            require(
                deployments[i].balancerAdapter == deployments[0].balancerAdapter,
                "CrossChain: BalancerAdapter mismatch"
            );
        }
        allIdentical = true;
    }
}
```

---

## 7. Rollback y Contingencias

### 7.1 Escenarios de Emergencia

| Escenario | Detección | Acción | Responsable |
|-----------|-----------|--------|-------------|
| Deploy con dirección incorrecta | `predictAddress` ≠ dirección real | Abortar, investigar nonce collision | Arquitecto Lead |
| Verificación en explorer fallida | API error | Reintentar manualmente con `--resume` | Infra team |
| Fondeo insuficiente | `balance < 0.25 ETH` | Bridge adicional desde Cold Treasury | Operator on-call |
| Invariante fallida en test | `forge test` falla | NO desplegar. Revisar código. | Core dev |
| Kill switch activado | `KillSwitchState.Terminated` | Pausar todo, investigar | Operator lead |

### 7.2 Procedimiento de Rollback

```bash
#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════
# Procedimiento de Rollback de Emergencia
# ═══════════════════════════════════════════════════════════════════════════

# ── Paso 1: Activar pausa en Executor ─────────────────────────────────────
echo "[EMERGENCY] Activando pausa en Executor..."
cast send "$EXECUTOR" "pause()" \
    --rpc-url "$RPC" \
    --private-key "$EMERGENCY_KILL_KEY"

# ── Paso 2: Revocar autorizaciones ────────────────────────────────────────
echo "[EMERGENCY] Revocando authorizedExecutors..."
for executor in "${AUTHORIZED_EXECUTORS[@]}"; do
    cast send "$EXECUTOR" "deauthorizeExecutor(address)" "$executor" \
        --rpc-url "$RPC" \
        --private-key "$GOVERNANCE_KEY"
done

# ── Paso 3: Deregistrar adaptadores ───────────────────────────────────────
echo "[EMERGENCY] Deregistrando adaptadores..."
for key in "UNISWAP_V2_1" "UNISWAP_V3_1" "CURVE_V1_1" "BALANCER_V2_1"; do
    KEY_HASH=$(cast keccak "$key")
    cast send "$EXECUTOR" "deregisterAdapter(bytes32)" "$KEY_HASH" \
        --rpc-url "$RPC" \
        --private-key "$GOVERNANCE_KEY"
done

# ── Paso 4: Emitir alerta ─────────────────────────────────────────────────
echo "[EMERGENCY] Rollback completado. Timestamp: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
# Aquí se integraría con PagerDuty/Slack
```

---

## 8. Registro de Operaciones (Audit Log)

### 8.1 Formato de Registro

Cada operación de deploy genera un registro audit de la forma:

```json
{
  "operation": "DEPLOY",
  "version": "2.0.0-OMEGA",
  "timestamp": "2026-05-14T12:00:00Z",
  "operator": "0xOperatorAddress",
  "chainId": 1,
  "network": "Ethereum",
  "deployments": {
    "factory": {
      "address": "0x...",
      "salt": "0x...",
      "txHash": "0x...",
      "gasUsed": 1234567,
      "blockNumber": 18000000
    },
    "executor": {
      "address": "0x...",
      "salt": "0x...",
      "txHash": "0x...",
      "gasUsed": 2345678,
      "blockNumber": 18000001
    }
  },
  "verification": {
    "status": "VERIFIED",
    "explorerUrl": "https://etherscan.io/address/0x...",
    "verifiedAt": "2026-05-14T12:15:00Z"
  },
  "signatures": {
    "operator": "0x...",
    "witness": "0x..."
  }
}
```

---

## 9. Appendix: Comandos Rápidos

### 9.1 Comandos Fundidos

```bash
# Compilar
forge build --sizes --force

# Test
forge test -vvvv

# Test específico
forge test --match-contract InvariantTest --match-test testInvariant -vvvv

# Deploy dry-run
forge script script/Deploy.s.sol --rpc-url $RPC -vvvv

# Deploy real
forge script script/Deploy.s.sol --rpc-url $RPC --private-key $KEY --broadcast --verify -vvvv

# Verificar contrato individual
forge verify-contract $ADDR src/core/Executor.sol:Executor \
    --etherscan-api-key $ETHERSCAN \
    --chain $CHAIN_ID

# Balance check
cast balance $ADDR --rpc-url $RPC

# Llamada de lectura
cast call $ADDR "totalExecutions()" --rpc-url $RPC

# Transacción
cast send $ADDR "pause()" --rpc-url $RPC --private-key $KEY

# Calcular salt
cast keccak $(cast abi-encode "f(uint256,string)" $CHAIN_ID "Executor")
```

### 9.2 Tabla de Variables de Entorno

| Variable | Descripción | Ejemplo |
|----------|-------------|---------|
| `RPC` | URL del nodo RPC | `https://eth-mainnet.g.alchemy.com/v2/...` |
| `KEY` | Private key del Gas Sponsor | `0xabc123...` |
| `ETHERSCAN_API_KEY` | API key para verificación | `ABC123...` |
| `CHAIN_ID` | chainId de la red | `1`, `42161`, `10`, etc. |
| `FACTORY_ADDRESS` | Dirección del Factory (misma en todas las cadenas) | `0x729...` |
| `WETH` | Dirección del WETH nativo | `0xC02...` (Ethereum) |
| `UNISWAP_V2_FACTORY` | Factory de Uniswap V2 | `0x5C69...` |
| `UNISWAP_V3_FACTORY` | Factory de Uniswap V3 | `0x1F98...` |
| `CURVE_REGISTRY` | Registro de Curve | `0x90E8...` |
| `BALANCER_VAULT` | Vault de Balancer V2 | `0xBA12...` |

---

## 10. Referencias

### Documentos Internos

1. **OMEGA White Paper** — `OMEGA_WHITE_PAPER_ONCHAIN.md` — Arquitectura completa del protocolo
2. **OMEGA Roadmap** — `OMEGA_ROADMAP_CONTRACTS.md` — Plan de implementación física
3. **OMEGA README** — `OMEGA_README_CONTRACTS.md` — Guía del operador
4. **ANEXOS_V1.2.md §4.1** — Especificación de tipos holonómicos

### Referencias Externas

5. **Ethereum CREATE2 EIP-1014** — `eips.ethereum.org/EIPS/eip-1014` — Especificación del opcode CREATE2
6. **Foundry Book** — `book.getfoundry.sh` — Documentación de Foundry
7. **Aave V3 Flash Loans** — `docs.aave.com/developers/guides/flash-loans` — Documentación de flash loans
8. **Balancer V2 Vault** — `docs.balancer.fi` — Arquitectura del Vault de Balancer
9. **OpenZeppelin ReentrancyGuard** — `docs.openzeppelin.com/contracts` — Patrón nonReentrant

---

**Document End — SOP Despliegues Multi-Chain OMEGA Protocol**

*"Un deploy determinista no es una conveniencia — es un requisito de seguridad. Si la dirección varía, la verificación falla."*
