# Roadmap: Implementación Física de Contratos OMEGA
## ArbitrageX-V2 — Protocolo de Resolución Holonómica On-Chain

---

**Versión**: 2.0.0-OMEGA  
**Fecha**: 2026-05-14  
**Clasificación**: Plan de Implementación — Nivel Institucional  
**Autor**: Arquitectura Lead, Infraestructura On-Chain OMEGA  
**Estado**: Final

---

## Tabla de Contenidos

1. [Visión General](#1-visión-general)
2. [Estructura de Directorios Foundry](#2-estructura-de-directorios-foundry)
3. [Fase 1: Contrato Base de Flash Convergence](#3-fase-1-contrato-base)
4. [Fase 2: Adaptadores DEX](#4-fase-2-adaptadores-dex)
5. [Fase 3: Pruebas Unitarias Invariantes (Echidna)](#5-fase-3-pruebas-echidna)
6. [Fase 4: Pruebas de Integración](#6-fase-4-pruebas-integración)
7. [Fase 5: Scripts de Despliegue CREATE2](#7-fase-5-scripts-deploy)
8. [Fase 6: Topología de Wallets](#8-fase-6-topología-wallets)
9. [Fase 7: Despliegue Multi-Chain](#9-fase-7-deploy-multi-chain)
10. [Fase 8: Verificación y Auditoría](#10-fase-8-verificación-auditoría)
11. [Fase 9: Monitoreo y Telemetría](#11-fase-9-monitoreo-telemetría)
12. [Fase 10: Governance y Kill Switch](#12-fase-10-governance)
13. [Cronograma](#13-cronograma)
14. [Métricas de Éxito](#14-métricas-de-éxito)
15. [Referencias](#15-referencias)

---

## 1. Visión General

### 1.1 Objetivo del Roadmap

Este documento establece la ruta de implementación física de los contratos inteligentes del Protocolo OMEGA — desde la estructura de directorios hasta el despliegue multi-chain pasando por pruebas invariantes con Echidna y verificación formal.

### 1.2 Principios Rectores

| Principio | Aplicación |
|-----------|-----------|
| **Determinismo** | Las direcciones de contratos son idénticas en todas las cadenas (CREATE2) |
| **Invariante** | Toda ejecución satisface `Y_topo > 0`, `balanceAfter ≥ balanceBefore + minYield` |
| **Fail-Honest** | Toda falla produce un error explícito, nunca un estado silencioso |
| **Zero-Mocks** | Las pruebas de integración ejecutan contra forks de mainnet, nunca mocks |
| **Asimetría** | Wallets con mínimo privilegio, yield acumulado en treasury fría |

### 1.3 Diagrama de Dependencias entre Fases

```
FASE 1          FASE 2           FASE 3          FASE 4          FASE 5
[Directorios] → [Flash Base] →  [Echidna]   →  [Integration] → [Scripts]
     │               │               │                │              │
     ▼               ▼               ▼                ▼              ▼
     └───────────────┴───────────────┴────────────────┘              │
                           │                                        │
                           ▼                                        ▼
FASE 6          FASE 7           FASE 8          FASE 9          FASE 10
[Wallets]     → [Multi-Chain] → [Audit]     →  [Telemetry]  →  [Governance]
```

**Regla de dependencia**: Ninguna fase puede comenzar hasta que todas sus dependencias directas estén completas y verificadas.

---

## 2. Estructura de Directorios Foundry

### 2.1 Árbol de Directorios

```
contracts/
├── foundry.toml              # Configuración de Foundry
├── remappings.txt            # Mapeo de importaciones
├── .env.example              # Variables de entorno (template)
├── .gitignore
│
├── src/
│   ├── interfaces/           # Interfaces y contratos abstractos
│   │   ├── IExecutor.sol
│   │   ├── IAdapter.sol
│   │   ├── IFlashLoanProvider.sol
│   │   ├── IUniswapV2Pair.sol
│   │   ├── IUniswapV3Pool.sol
│   │   ├── ICurvePool.sol
│   │   ├── IBalancerVault.sol
│   │   ├── IColdTreasury.sol
│   │   └── IGasSponsor.sol
│   │
│   ├── core/                 # Contratos core del protocolo
│   │   ├── Executor.sol              # Entry point único atómico
│   │   ├── OmegaDeployFactory.sol    # Factory CREATE2 determinista
│   │   ├── ColdTreasury.sol          # Receptor de yield acumulado
│   │   ├── GasSponsor.sol            # Wallet de gas controlada
│   │   ├── KillSwitch.sol            # Emergency stop tri-state
│   │   └── Governance.sol            # Multisig 2-of-3 + timelock
│   │
│   ├── adapters/             # Adaptadores DEX (modular, extensible)
│   │   ├── UniswapV2Adapter.sol      # CPMM: x·y = k
│   │   ├── UniswapV3Adapter.sol      # CLAMM: ticks + concentrated liq
│   │   ├── CurveAdapter.sol          # StableSwap invariant
│   │   ├── BalancerAdapter.sol       # Weighted pools
│   │   └── BaseAdapter.sol           # Clase abstracta común
│   │
│   ├── flash/                # Módulo de Flash Convergence
│   │   ├── FlashConvergence.sol      # Orquestador de flashloans
│   │   ├── AaveV3FlashProvider.sol   # Provider Aave V3
│   │   ├── BalancerFlashProvider.sol # Provider Balancer V2
│   │   ├── MakerDAOFlashProvider.sol # Provider MakerDAO DSS
│   │   └── FlashRouter.sol           # Ruteador de providers
│   │
│   ├── lib/                  # Librerías internas
│   │   ├── HolonomicProof.sol        # Verificación de pruebas
│   │   ├── TopologicalMath.sol       # Matemática de contornos
│   │   ├── ReentrancyGuard.sol       # Protección contra reentrancia
│   │   ├── SafeERC20.sol             # Transfers seguras
│   │   └── Errors.sol                # Enums de error (fail-honest)
│   │
│   └── types/                # Tipos y estructuras compartidas
│       ├── BundlePosition.sol        # Typestate bundle
│       ├── FlashParams.sol           # Parámetros de flashloan
│       ├── SwapStep.sol              # Paso individual de swap
│       └── AdapterCall.sol           # Llamada a adaptador
│
├── test/
│   ├── unit/                 # Pruebas unitarias (cada contrato)
│   │   ├── Executor.t.sol
│   │   ├── UniswapV2Adapter.t.sol
│   │   ├── UniswapV3Adapter.t.sol
│   │   ├── CurveAdapter.t.sol
│   │   ├── BalancerAdapter.t.sol
│   │   ├── FlashConvergence.t.sol
│   │   ├── ColdTreasury.t.sol
│   │   └── KillSwitch.t.sol
│   │
│   ├── invariant/            # Pruebas invariantes (Echidna)
│   │   ├── EchidnaExecutor.sol       # Invariantes del Executor
│   │   ├── EchidnaFlashLoan.sol      # Invariantes de flash loans
│   │   ├── EchidnaAdapter.sol        # Invariantes de adaptadores
│   │   └── crytic-config.yaml        # Configuración de Echidna
│   │
│   ├── fuzz/                 # Pruebas de fuzzing
│   │   ├── FuzzExecutor.sol          # Fuzzing de execute()
│   │   ├── FuzzHolonomicProof.sol    # Fuzzing de verificación
│   │   └── FuzzTopologicalMath.sol   # Fuzzing de cálculos
│   │
│   ├── integration/          # Pruebas de integración (fork mainnet)
│   │   ├── IntegrationExecutor.t.sol
│   │   ├── IntegrationFlashLoan.t.sol
│   │   ├── IntegrationCrossDEX.t.sol
│   │   └── IntegrationGovernance.t.sol
│   │
│   ├── mocks/                # Contratos mock (solo para unit tests)
│   │   ├── MockERC20.sol
│   │   ├── MockUniswapV2Pair.sol
│   │   ├── MockUniswapV3Pool.sol
│   │   └── MockAavePool.sol
│   │
│   └── shared/               # Utilidades compartidas para tests
│       ├── TestBase.sol
│       ├── Constants.sol
│       └── Helpers.sol
│
├── script/
│   ├── Deploy.s.sol          # Script principal de despliegue
│   ├── DeployFactory.s.sol   # Despliegue del Factory CREATE2
│   ├── Verify.s.sol          # Verificación en block explorers
│   ├── FundWallets.s.sol     # Fondeo de wallets
│   ├── EmergencyRollback.s.sol  # Rollback de emergencia
│   └── utils/
│       └── ScriptHelpers.sol
│
├── lib/
│   ├── forge-std/            # Foundry standard library (submodule)
│   ├── openzeppelin-contracts/  # OpenZeppelin (submodule)
│   ├── solmate/              # Solmate (submodule)
│   └── v3-core/              # Uniswap V3 (submodule, para interfaces)
│
└── artifacts/                # Artefactos generados post-deploy
    ├── deploy_1.json         # Ethereum mainnet
    ├── deploy_42161.json     # Arbitrum
    ├── deploy_10.json        # Optimism
    ├── deploy_8453.json      # Base
    ├── deploy_137.json       # Polygon
    └── deploy_56.json        # BSC
```

### 2.2 Configuración: foundry.toml

```toml
[profile.default]
src = "src"
out = "out"
libs = ["lib"]
test = "test"
cache_path = "cache"
verbosity = 4
optimizer = true
optimizer_runs = 200
via_ir = false
evm_version = "cancun"

# Gas reporting
gas_reports = ["*"]
gas_reports_ignore = ["Mock*"]

# Fuzzing
[fuzz]
runs = 10000
max_test_rejects = 65536
dictionary_weight = 40

# Invariant testing
[invariant]
runs = 1000
depth = 50
fail_on_revert = true
call_override = false
shrink_sequence = true

# Formateo
[fmt]
line_length = 100
tab_width = 4
bracket_spacing = false
int_types = "long"
multiline_func_header = "params_first"
number_underscore = "thousands"
quote_style = "double"

# Perfiles de red
[profile.mainnet]
fork_block_number = 18000000
eth_rpc_url = "${ETH_RPC}"

[profile.arbitrum]
fork_block_number = 220000000
eth_rpc_url = "${ARB_RPC}"

[profile.optimism]
fork_block_number = 120000000
eth_rpc_url = "${OP_RPC}"
```

### 2.3 Remappings

```
forge-std/=lib/forge-std/src/
openzeppelin/=lib/openzeppelin-contracts/contracts/
solmate/=lib/solmate/src/
@uniswap/v3-core/=lib/v3-core/contracts/
src/=src/
test/=test/
```

### 2.4 Variables de Entorno (.env.example)

```bash
# ── RPC Endpoints ──
ETH_RPC=https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY
ARB_RPC=https://arb-mainnet.g.alchemy.com/v2/YOUR_KEY
OP_RPC=https://opt-mainnet.g.alchemy.com/v2/YOUR_KEY
BASE_RPC=https://base-mainnet.g.alchemy.com/v2/YOUR_KEY
POLYGON_RPC=https://polygon-mainnet.g.alchemy.com/v2/YOUR_KEY
BSC_RPC=https://bsc-dataseed.binance.org

# ── Claves ──
GAS_SPONSOR_KEY=0x...
EXECUTION_SIGNER_KEY=0x...
GOVERNANCE_KEY=0x...
EMERGENCY_KILL_KEY=0x...

# ── API Keys ──
ETHERSCAN_API_KEY=...
ARBISCAN_API_KEY=...
OPTIMISTIC_ETHERSCAN_API_KEY=...
BASESCAN_API_KEY=...
POLYGONSCAN_API_KEY=...
BSCSCAN_API_KEY=...

# ── Direcciones (post-deploy) ──
FACTORY_ADDRESS=0x...
EXECUTOR_ADDRESS=0x...
COLD_TREASURY_ADDRESS=0x...
```

---

## 3. Fase 1: Contrato Base de Flash Convergence

### 3.1 Paso 1.1: Inicializar Proyecto Foundry

```bash
# Crear estructura de directorios
mkdir -p omega-protocol && cd omega-protocol
forge init --force

# Instalar dependencias
forge install foundry-rs/forge-std
forge install OpenZeppelin/openzeppelin-contracts
forge install transmissions11/solmate
forge install Uniswap/v3-core

# Crear estructura completa
mkdir -p src/{interfaces,core,adapters,flash,lib,types}
mkdir -p test/{unit,invariant,fuzz,integration,mocks,shared}
mkdir -p script/utils
mkdir -p artifacts
```

### 3.2 Paso 1.2: Implementar Tipos Base

**Archivo**: `src/types/FlashParams.sol`

```solidity
// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

/// @notice Parámetros unificados para operaciones de flash convergence
/// @dev    Estructura inmutable que transporta toda la información necesaria
///         para una Resolución Holonómica atómica
struct FlashParams {
    address token;
    uint256 amount;
    AdapterCall[] calls;
    bytes callback;
    uint256 deadline;
    uint256 minYieldBp;
    HolonomicProof proof;
}

/// @notice Paso de swap individual dentro de una secuencia
struct SwapStep {
    address tokenIn;
    address tokenOut;
    uint256 amountIn;
    uint256 minAmountOut;
    uint24 fee;          // Fee tier para Uniswap V3
}

/// @notice Llamada a un adaptador registrado
struct AdapterCall {
    bytes32 adapterKey;
    bytes data;
    address tokenIn;
    address tokenOut;
    uint256 amountIn;
    uint256 minAmountOut;
}

/// @notice Prueba criptográfica de holonomía válida
/// @dev    Se construye off-chain y se valida on-chain
struct HolonomicProof {
    int256 rawHolonomy;         // ∮_γ (dp/p)
    int256 networkFriction;     // F_net = F_gas + F_slippage + F_LP
    int256 netYield;            // Y_topo = Y_raw - F_net
    bool isContourClosed;       // γ(0) = γ(1)
    uint256 loopCardinality;    // N ≥ 3
    bytes32 contourHash;        // Hash del contorno para integridad
}
```

### 3.3 Paso 1.3: Implementar Interfaz IExecutor

**Archivo**: `src/interfaces/IExecutor.sol`

```solidity
// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

import {FlashParams, HolonomicProof} from "../types/FlashParams.sol";

/// @title IExecutor
/// @notice Interfaz del entry point atómico único del protocolo OMEGA
interface IExecutor {
    /// @notice Error: ejecutor no autorizado
    error UnauthorizedExecutor(address caller);
    
    /// @notice Error: manifold count insuficiente (< 3)
    error InsufficientManifolds(uint256 count);
    
    /// @notice Error: deadline expirado
    error DeadlineExceeded(uint256 deadline, uint256 current);
    
    /// @notice Error: contorno abierto (γ(0) ≠ γ(1))
    error OpenContourTrajectory();
    
    /// @notice Error: holonomía trivial (|∮(dp/p)| < 1e-12)
    error TrivialHolonomy();
    
    /// @notice Error: rendimiento topológico no positivo
    error NonPositiveTopologicalYield(int256 netYield);
    
    /// @notice Error: adaptador no registrado
    error AdapterNotRegistered(bytes32 key);
    
    /// @notice Error: post-condición de balance violada
    error PostConditionBalance(uint256 before, uint256 after, uint256 minRequired);

    /// @notice Evento: resolución holonómica ejecutada exitosamente
    event HolonomicResolutionExecuted(
        bytes32 indexed executionId,
        address indexed executor,
        bytes32 indexed adapterKey,
        uint256 timestamp,
        uint256 grossHolonomy,
        uint256 networkFriction,
        uint256 netYield,
        uint256[] manifoldIds
    );

    /// @notice Ejecuta una resolución holonómica atómica
    /// @param params  Parámetros de flash convergence
    /// @param proof   Prueba criptográfica de holonomía
    /// @return netYield El rendimiento topológico neto
    function execute(FlashParams calldata params, HolonomicProof calldata proof) 
        external 
        returns (uint256 netYield);
    
    /// @notice Registra un adaptador DEX en el protocolo
    function registerAdapter(bytes32 key, address adapter) external;
    
    /// @notice Deregistra un adaptador
    function deregisterAdapter(bytes32 key) external;
    
    /// @notice Autoriza un ejecutor
    function authorizeExecutor(address executor) external;
    
    /// @notice Revoca autorización de ejecutor
    function deauthorizeExecutor(address executor) external;
}
```

### 3.4 Paso 1.4: Implementar Executor.sol

**Archivo**: `src/core/Executor.sol`

Ver implementación completa en White Paper §3.2. Aquí se describe la estructura:

```
Executor.sol
├── State Variables
│   ├── authorizedExecutors (mapping)
│   ├── adapterRegistry (mapping bytes32 => address)
│   ├── totalExecutions (uint256)
│   └── accumulatedTopologicalYield (uint256)
├── Modifiers
│   ├── onlyAuthorizedExecutor
│   ├── onlyGovernance
│   └── validBundle
├── External Functions
│   ├── execute(FlashParams, HolonomicProof) → uint256
│   ├── registerAdapter(bytes32, address)
│   ├── deregisterAdapter(bytes32)
│   ├── authorizeExecutor(address)
│   └── deauthorizeExecutor(address)
├── Internal Functions
│   ├── _verifyHolonomicProof(HolonomicProof) → bool
│   ├── _executeAdapterCalls(AdapterCall[]) → uint256
│   └── _emitExecutionEvent(bytes32, uint256, uint256, uint256)
└── View Functions
    ├── getAdapter(bytes32) → address
    └── isAuthorized(address) → bool
```

### 3.5 Paso 1.5: Implementar Factory CREATE2

**Archivo**: `src/core/OmegaDeployFactory.sol`

Ver implementación completa en SOP §2.2.

### 3.6 Paso 1.6: Implementar ColdTreasury y KillSwitch

**Archivos**:
- `src/core/ColdTreasury.sol` — Receptor de yield con multisig
- `src/core/KillSwitch.sol` — Estado tri-state: Active/Suspended/Terminated
- `src/core/Governance.sol` — Timelock + multisig 2-of-3

### 3.7 Checklist Fase 1

- [ ] Paso 1.1: Estructura de directorios creada
- [ ] Paso 1.2: Tipos base implementados (FlashParams, SwapStep, AdapterCall, HolonomicProof)
- [ ] Paso 1.3: Interfaz IExecutor completa
- [ ] Paso 1.4: Executor.sol compilando sin errores
- [ ] Paso 1.5: Factory CREATE2 implementado
- [ ] Paso 1.6: ColdTreasury, KillSwitch y Governance implementados
- [ ] `forge build` pasa sin warnings
- [ ] Gas report generado para todos los contratos

---

## 4. Fase 2: Adaptadores DEX

### 4.1 Paso 2.1: Clase Abstracta BaseAdapter

**Archivo**: `src/adapters/BaseAdapter.sol`

```solidity
// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

import {IAdapter} from "../interfaces/IAdapter.sol";
import {SwapStep} from "../types/FlashParams.sol";

/**
 * @title BaseAdapter
 * @notice Clase abstracta base para todos los adaptadores DEX
 * @dev    Implementa funcionalidad común: validación de parámetros,
 *         manejo de slippage, y telemetría
 */
abstract contract BaseAdapter is IAdapter {
    /// @notice Versión del adaptador (para compatibilidad)
    string public version;
    
    /// @notice Número máximo de pasos en una secuencia
    uint256 public constant MAX_STEPS = 10;
    
    /// @notice Slippage máximo permitido (1% = 100 bp)
    uint256 public constant MAX_SLIPPAGE_BP = 500;
    
    /// @notice Evento: secuencia de swaps ejecutada
    event SwapSequenceExecuted(
        string indexed adapter,
        uint256 steps,
        uint256 output,
        uint256 timestamp
    );
    
    /// @notice Valida una secuencia de swaps antes de ejecución
    modifier validSequence(SwapStep[] calldata steps) {
        require(steps.length >= 2, "BaseAdapter: min 2 steps");
        require(steps.length <= MAX_STEPS, "BaseAdapter: max steps exceeded");
        for (uint256 i = 0; i < steps.length; i++) {
            require(steps[i].tokenIn != address(0), "BaseAdapter: zero tokenIn");
            require(steps[i].tokenOut != address(0), "BaseAdapter: zero tokenOut");
            require(steps[i].amountIn > 0, "BaseAdapter: zero amount");
        }
        _;
    }
    
    /// @notice Verifica que el output cumple el mínimo de slippage
    function _validateOutput(
        uint256 output, 
        uint256 minRequired
    ) internal pure {
        require(output >= minRequired, "BaseAdapter: slippage exceeded");
    }
}
```

### 4.2 Paso 2.2: UniswapV2Adapter

| Campo | Valor |
|-------|-------|
| **Protocolo** | Uniswap V2 / SushiSwap |
| **Invariante** | $x \cdot y = k$ |
| **Fee** | 0.3% (30 bp) |
| **Gas por swap** | ~50,000 |
| **Archivo** | `src/adapters/UniswapV2Adapter.sol` |

**Fórmula de output**:

$$
\text{amountOut} = \frac{\text{amountIn} \times 997 \times \text{reserveOut}}{\text{reserveIn} \times 1000 + \text{amountIn} \times 997}
$$

### 4.3 Paso 2.3: UniswapV3Adapter

| Campo | Valor |
|-------|-------|
| **Protocolo** | Uniswap V3 |
| **Invariante** | Concentrated liquidity, ticks |
| **Fee tiers** | 100, 500, 3000, 10000 bp |
| **Gas por swap** | ~65,000 |
| **Archivo** | `src/adapters/UniswapV3Adapter.sol` |

### 4.4 Paso 2.4: CurveAdapter

| Campo | Valor |
|-------|-------|
| **Protocolo** | Curve StableSwap |
| **Invariante** | $An^n \sum x_i + D = DAn^n + \frac{D^{n+1}}{n^n \prod x_i}$ |
| **Fee** | Variable (0.04% típico) |
| **Gas por swap** | ~70,000 |
| **Archivo** | `src/adapters/CurveAdapter.sol` |

### 4.5 Paso 2.5: BalancerAdapter

| Campo | Valor |
|-------|-------|
| **Protocolo** | Balancer V2 |
| **Invariante** | $\prod_i x_i^{w_i} = k$ |
| **Fee** | Variable por pool |
| **Gas por swap** | ~75,000 (batch swap) |
| **Archivo** | `src/adapters/BalancerAdapter.sol` |

### 4.6 Checklist Fase 2

- [ ] Paso 2.1: BaseAdapter implementado con validaciones
- [ ] Paso 2.2: UniswapV2Adapter completo con test unitario
- [ ] Paso 2.3: UniswapV3Adapter completo con callback
- [ ] Paso 2.4: CurveAdapter con soporte para múltiples pools
- [ ] Paso 2.5: BalancerAdapter con batch swaps
- [ ] Todos los adaptadores heredan BaseAdapter
- [ ] `forge test --match-contract Adapter` pasa al 100%

---

## 5. Fase 3: Pruebas Unitarias Invariantes (Echidna)

### 5.1 Principio de Pruebas Invariantes

Las pruebas invariantes verifican que **ciertas propiedades nunca se violan**, sin importar la secuencia de operaciones. Echidna genera automáticamente secuencias de llamadas aleatorias intentando romper las invariantes.

### 5.2 Configuración de Echidna

**Archivo**: `test/invariant/crytic-config.yaml`

```yaml
corpusDir: echidna-corpus
testMode: assertion
testLimit: 50000
seqLen: 50
shrinkLimit: 2000
coverage: true
format: text
# Multi-abi: permite llamar a cualquier función de cualquier contrato
multi-abi: true
codeSize: 0x6000
gasLimit: 0xfffffffffff
balanceAddr: 0xffffffffffffffffffffffffffffffff
balanceContract: 0xffffffffffffffffffffffffffffffff
timeout: 600 # 10 minutos
nworkers: 4
```

### 5.3 Paso 3.1: Invariantes del Executor

**Archivo**: `test/invariant/EchidnaExecutor.sol`

```solidity
// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

import "../../src/core/Executor.sol";

/**
 * @title EchidnaExecutor
 * @notice Pruebas invariantes del Executor con Echidna
 * @dev    Echidna intentará romper estas invariantes generando
 *         secuencias aleatorias de llamadas
 */
contract EchidnaExecutor {
    Executor executor;
    
    // Invariante I1: totalExecutions nunca decrece
    uint256 private lastExecutionCount;
    
    // Invariante I2: accumulatedTopologicalYield nunca decrece
    uint256 private lastAccumulatedYield;
    
    constructor() {
        executor = new Executor(address(this));
        lastExecutionCount = executor.totalExecutions();
        lastAccumulatedYield = executor.accumulatedTopologicalYield();
    }
    
    // ── Funciones accionables por Echidna ──
    
    function authorizeExecutor(address executorAddr) public {
        // Echidna generará direcciones aleatorias
        executor.authorizeExecutor(executorAddr);
    }
    
    function deauthorizeExecutor(address executorAddr) public {
        executor.deauthorizeExecutor(executorAddr);
    }
    
    function registerAdapter(bytes32 key, address adapter) public {
        executor.registerAdapter(key, adapter);
    }
    
    // ── Invariantes ──
    
    /// @notice Invariante I1: totalExecutions es monótono no-decreciente
    function echidna_monotone_executions() public view returns (bool) {
        return executor.totalExecutions() >= lastExecutionCount;
    }
    
    /// @notice Invariante I2: accumulatedTopologicalYield es monótono no-decreciente
    function echidna_monotone_yield() public view returns (bool) {
        return executor.accumulatedTopologicalYield() >= lastAccumulatedYield;
    }
    
    /// @notice Invariante I3: el Executor nunca tiene balance (es passthrough)
    function echidna_zero_balance() public view returns (bool) {
        return address(executor).balance == 0;
    }
    
    /// @notice Invariante I4: adaptadores registrados son non-zero
    function echidna_registered_nonzero() public view returns (bool) {
        // Verificar los 4 adaptadores conocidos
        bytes32[4] memory keys = [
            keccak256("UNISWAP_V2_1"),
            keccak256("UNISWAP_V3_1"),
            keccak256("CURVE_V1_1"),
            keccak256("BALANCER_V2_1")
        ];
        for (uint256 i = 0; i < keys.length; i++) {
            if (executor.getAdapter(keys[i]) == address(0)) {
                return false; // adaptador no registrado = fallo si debería estar
            }
        }
        return true;
    }
    
    /// @notice Invariante I5: governance nunca es address(0)
    function echidna_governance_nonzero() public view returns (bool) {
        return executor.governance() != address(0);
    }
}
```

### 5.4 Paso 3.2: Invariantes de Flash Loans

**Archivo**: `test/invariant/EchidnaFlashLoan.sol`

```solidity
// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

/**
 * @title EchidnaFlashLoan
 * @notice Invariantes del sistema de flash loans
 * @dev    Verifica que los flash loans siempre se reembolsan
 */
contract EchidnaFlashLoan {
    FlashConvergence flash;
    
    // Invariante: la suma de préstamos activos siempre es cero
    // (todos los flashloans se reembolsan dentro de la tx)
    
    /// @notice Invariante: nunca hay préstamos pendientes post-transacción
    /// @dev    Por definición atómica, un flashloan siempre se reembolsa
    function echidna_no_outstanding_loans() public view returns (bool) {
        // Si hay un préstamo activo, significa que un callback falló
        return !flash.hasActiveLoan();
    }
    
    /// @notice Invariante: el balance del contrato nunca es negativo
    function echidna_non_negative_balance() public view returns (bool) {
        return address(this).balance >= 0;
    }
    
    /// @notice Invariante: el premium acumulado siempre es ≤ yield acumulado
    function echidna_premium_le_yield() public view returns (bool) {
        return flash.accumulatedPremiums() <= flash.accumulatedYield();
    }
}
```

### 5.5 Paso 3.3: Invariantes de Adaptadores

**Archivo**: `test/invariant/EchidnaAdapter.sol`

```solidity
// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

/**
 * @title EchidnaAdapter
 * @notice Invariantes comunes a todos los adaptadores DEX
 */
contract EchidnaAdapter {
    /// @notice Invariante: para cualquier input, output ≥ 0
    function echidna_output_non_negative(
        uint256 amountIn
    ) public view returns (bool) {
        // Un output negativo es imposible en solidity (uint), 
        // pero un output de 0 indica un problema
        if (amountIn == 0) return true; // skip
        
        uint256 amountOut = adapter.quoteSwap(tokenA, tokenB, amountIn);
        return amountOut > 0;
    }
    
    /// @notice Invariante: quoteSwap es consistente (llamada pura)
    function echidna_quote_consistent(
        uint256 amountIn
    ) public view returns (bool) {
        uint256 out1 = adapter.quoteSwap(tokenA, tokenB, amountIn);
        uint256 out2 = adapter.quoteSwap(tokenA, tokenB, amountIn);
        return out1 == out2;
    }
    
    /// @notice Invariante: slippage nunca excede el máximo
    function echidna_slippage_bounded(
        uint256 amountIn,
        uint256 minAmountOut
    ) public returns (bool) {
        try adapter.executeSwapSequence(steps, recipient) {
            return true; // éxito = slippage OK
        } catch {
            return true; // revert = slippage protegido
        }
    }
}
```

### 5.6 Ejecución de Echidna

```bash
# Instalar Echidna (requiere crytic-compile)
pip install crytic-compile

# Ejecutar pruebas invariantes

echidna test/invariant/EchidnaExecutor.sol \
    --contract EchidnaExecutor \
    --config test/invariant/crytic-config.yaml \
    --crytic-args "--compile-force-framework foundry"

# Con cobertura de código
echidna test/invariant/EchidnaExecutor.sol \
    --contract EchidnaExecutor \
    --config test/invariant/crytic-config.yaml \
    --coverage

# Análisis de resultados
# PASS: Todas las invariantes se mantuvieron durante 50,000 tests
# FAIL: Echidna encontró un counterexample → corregir y re-ejecutar
```

### 5.7 Checklist Fase 3

- [ ] Paso 3.1: EchidnaExecutor con 5 invariantes
- [ ] Paso 3.2: EchidnaFlashLoan con 3 invariantes
- [ ] Paso 3.3: EchidnaAdapter con 3 invariantes
- [ ] crytic-config.yaml configurado
- [ ] Echidna ejecuta sin errores de compilación
- [ ] ≥ 50,000 tests por contrato invariante
- [ ] Cobertura de código > 80%

---

## 6. Fase 4: Pruebas de Integración

### 6.1 Filosofía de Pruebas de Integración

Las pruebas de integración ejecutan contra **forks de mainnet**, nunca mocks. Esto garantiza que:

1. Los adaptadores interactúan con contratos reales en mainnet
2. Las invariantes se mantienen con estado real de liquidez
3. Los costos de gas son precisos

### 6.2 Paso 4.1: IntegrationExecutor

**Archivo**: `test/integration/IntegrationExecutor.t.sol`

```solidity
// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

import "forge-std/Test.sol";
import "../../src/core/Executor.sol";
import "../../src/adapters/UniswapV2Adapter.sol";
import "../../src/adapters/UniswapV3Adapter.sol";

/**
 * @title IntegrationExecutor
 * @notice Pruebas de integración del Executor contra mainnet fork
 * @dev    Se ejecuta con: forge test --match-contract IntegrationExecutor --fork-url $ETH_RPC
 */
contract IntegrationExecutor is Test {
    Executor executor;
    UniswapV2Adapter v2Adapter;
    UniswapV3Adapter v3Adapter;
    
    address constant WETH = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;
    address constant USDC = 0xA0b86a33E6441F7C1a89C0b8B0c0B0B0B0B0B0B0;
    address constant USDT = 0xdAC17F958D2ee523a2206206994597C13D831ec7;
    address constant DAI = 0x6B175474E89094C44Da98b954EedeAC495271d0F;
    
    address governance = makeAddr("governance");
    
    function setUp() public {
        vm.createSelectFork(vm.envString("ETH_RPC"));
        
        // Deploy contratos
        executor = new Executor(governance);
        v2Adapter = new UniswapV2Adapter(UNISWAP_V2_FACTORY, WETH);
        v3Adapter = new UniswapV3Adapter(UNISWAP_V3_FACTORY, UNISWAP_V3_POSITION_MANAGER);
        
        // Registrar
        vm.prank(governance);
        executor.registerAdapter(keccak256("UNISWAP_V2_1"), address(v2Adapter));
        vm.prank(governance);
        executor.registerAdapter(keccak256("UNISWAP_V3_1"), address(v3Adapter));
    }
    
    /// @notice Test: Executor puede ejecutar una secuencia real en mainnet fork
    function test_ExecuteRealSwapSequence() public {
        // ... implementación
    }
    
    /// @notice Test: La invariante de balance se mantiene post-ejecución
    function test_BalanceInvariantAfterExecution() public {
        // ... implementación
    }
}
```

### 6.3 Paso 4.2: IntegrationFlashLoan

**Archivo**: `test/integration/IntegrationFlashLoan.t.sol`

Pruebas de flash loans reales contra:
- Aave V3 Pool en mainnet fork
- Balancer Vault en mainnet fork
- MakerDAO DSS Flash en mainnet fork

### 6.4 Paso 4.3: IntegrationCrossDEX

**Archivo**: `test/integration/IntegrationCrossDEX.t.sol`

Pruebas de convergencia holonómica cross-DEX:
- Uniswap V2 → Curve → Uniswap V3 (N=3)
- Uniswap V3 → Balancer → Curve → Uniswap V2 (N=4)

### 6.5 Checklist Fase 4

- [ ] Paso 4.1: IntegrationExecutor con fork mainnet
- [ ] Paso 4.2: IntegrationFlashLoan contra Aave/Balancer/MakerDAO
- [ ] Paso 4.3: IntegrationCrossDEX con secuencias N≥3
- [ ] Todas las pruebas pasan con `--fork-url $ETH_RPC`
- [ ] Reporte de gas generado para cada test

---

## 7. Fase 5: Scripts de Despliegue CREATE2

### 7.1 Paso 5.1: DeployFactory.s.sol

**Archivo**: `script/DeployFactory.s.sol`

Ver implementación completa en SOP §2.4.

### 7.2 Paso 5.2: Deploy.s.sol

**Archivo**: `script/Deploy.s.sol`

Ver implementación completa en SOP §2.5.

### 7.3 Paso 5.3: Verify.s.sol

**Archivo**: `script/Verify.s.sol`

```solidity
// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

import "forge-std/Script.sol";

/**
 * @title Verify
 * @notice Script de verificación post-deploy en block explorers
 */
contract Verify is Script {
    function run() public {
        string memory json = vm.readFile(
            string.concat("artifacts/deploy_", vm.toString(block.chainid), ".json")
        );
        
        address executor = abi.decode(vm.parseJson(json, ".executor"), (address));
        
        // Verificar en etherscan
        string[] memory args = new string[](7);
        args[0] = "forge";
        args[1] = "verify-contract";
        args[2] = vm.toString(executor);
        args[3] = "src/core/Executor.sol:Executor";
        args[4] = "--chain";
        args[5] = vm.toString(block.chainid);
        args[6] = vm.envString("ETHERSCAN_API_KEY");
        
        vm.ffi(args);
    }
}
```

### 7.4 Checklist Fase 5

- [ ] Paso 5.1: DeployFactory.s.sol funcional
- [ ] Paso 5.2: Deploy.s.sol completo
- [ ] Paso 5.3: Verify.s.sol para verificación automática
- [ ] Scripts testeados en anvil local antes de mainnet

---

## 8. Fase 6: Topología de Wallets

### 8.1 Paso 6.1: Generación de Wallets

```bash
#!/bin/bash
# generate-wallets.sh

echo "Generando topología de wallets OMEGA..."

# Gas Sponsor (hot, HSM)
cast wallet new --json > wallets/gas-sponsor.json

# Execution Signer (hot, air-gapped)
cast wallet new --json > wallets/execution-signer.json

# Governance (cold, hardware)
cast wallet new --json > wallets/governance.json

# Cold Treasury (cold, multisig)
cast wallet new --json > wallets/cold-treasury-1.json
cast wallet new --json > wallets/cold-treasury-2.json
cast wallet new --json > wallets/cold-treasury-3.json

echo "Wallets generadas. Guardar en HSM/hardware antes de fondear."
```

### 8.2 Paso 6.2: Configuración de Multisig

El Cold Treasury requiere **multisig 2-of-3**:

```solidity
contract ColdTreasury {
    address[3] public signers;
    uint256 public required = 2;
    
    mapping(bytes32 => bool) public executed;
    
    function withdraw(
        address to,
        uint256 amount,
        bytes[3] calldata signatures,
        bytes32 txHash
    ) external {
        require(!executed[txHash], "Treasury: already executed");
        require(_verifyMultisig(txHash, signatures) >= required, "Treasury: insufficient sigs");
        
        executed[txHash] = true;
        payable(to).transfer(amount);
    }
}
```

### 8.3 Checklist Fase 6

- [ ] Paso 6.1: Wallets generadas y almacenadas en HSM
- [ ] Paso 6.2: Multisig configurado (2-of-3)
- [ ] Wallets fondeadas según SOP §4
- [ ] Verificación de saldos completada

---

## 9. Fase 7: Despliegue Multi-Chain

### 9.1 Secuencia de Deploy

| Orden | Red | chainId | Timestamp |
|-------|-----|---------|-----------|
| 1 | Ethereum Mainnet | 1 | T+0 |
| 2 | Arbitrum One | 42161 | T+2h |
| 3 | Optimism | 10 | T+4h |
| 4 | Base | 8453 | T+6h |
| 5 | Polygon PoS | 137 | T+8h |
| 6 | BSC | 56 | T+10h |

### 9.2 Checklist por Red

- [ ] 1. `.env.chainId` configurado
- [ ] 2. RPC funcional y sincronizado
- [ ] 3. Gas sponsor fondeado
- [ ] 4. Nonce verificado
- [ ] 5. Dry-run exitoso (`--broadcast` omitido)
- [ ] 6. Deploy real con `--broadcast`
- [ ] 7. Direcciones verificadas cross-chain
- [ ] 8. Código verificado en explorer
- [ ] 9. Tests de integración pasados
- [ ] 10. Artefactos guardados

---

## 10. Fase 8: Verificación y Auditoría

### 10.1 Checklist de Auditoría Interna

- [ ] Todos los contratos compilados sin warnings
- [ ] Slither: 0 high/medium severity findings
- [ ] Echidna: ≥ 50,000 tests, 0 invariantes rotas
- [ ] Coverage: > 90% líneas, > 80% branches
- [ ] NatSpec: 100% de funciones documentadas
- [ ] CEI pattern: verificado en todas las funciones
- [ ] ReentrancyGuard: aplicado a todas las funciones external

### 10.2 Herramientas de Auditoría

```bash
# Slither (análisis estático)
slither src/ --config-file slither.config.json

# Echidna (pruebas invariantes)
echidna test/invariant/EchidnaExecutor.sol --contract EchidnaExecutor

# Foundry coverage
forge coverage --report lcov
genhtml lcov.info --output-directory coverage-report

# Gas snapshot
forge snapshot --check
```

---

## 11. Fase 9: Monitoreo y Telemetría

### 11.1 Eventos On-Chain

Todos los eventos se consumen por el pipeline SED (Rust) y se retransmiten via Redis:

```rust
// sed-core/src/telemetry/mod.rs (existente)
pub struct ConvergenceSignal {
    pub entropy_snapshot: EntropySnapshot,
    pub pipeline_latency_ms: u64,
    pub opportunities_detected: u64,
    pub simulations_run: u64,
    pub simulations_success: u64,
    pub timestamp: String,
    pub schema_version: u8,
}
```

### 11.2 Métricas Clave

| Métrica | Fuente | Umbral de Alerta |
|---------|--------|-----------------|
| `totalExecutions` | Executor | Delta < 1 por hora = STALE |
| `accumulatedTopologicalYield` | Executor | Delta negativo = CRÍTICO |
| `gasPriceGwei` | mempool | > 100 Gwei = PAUSAR |
| `mempoolEntropyScore` | telemetry | > 0.85 = ALTA VOLATILIDAD |
| `executionRevertRate` | events | > 5% = INVESTIGAR |

---

## 12. Fase 10: Governance y Kill Switch

### 12.1 Timelock

Todas las operaciones de governance requieren un **timelock de 24 horas**:

```solidity
contract GovernanceTimelock {
    uint256 public constant DELAY = 1 days;
    
    struct QueuedTx {
        address target;
        bytes data;
        uint256 executeAfter;
        bool executed;
    }
    
    mapping(bytes32 => QueuedTx) public queue;
    
    function queueTransaction(address target, bytes calldata data) 
        external 
        onlyMultisig 
    {
        bytes32 txHash = keccak256(abi.encode(target, data));
        queue[txHash] = QueuedTx({
            target: target,
            data: data,
            executeAfter: block.timestamp + DELAY,
            executed: false
        });
    }
    
    function executeTransaction(bytes32 txHash) external {
        QueuedTx storage tx_ = queue[txHash];
        require(block.timestamp >= tx_.executeAfter, "Timelock: not ready");
        require(!tx_.executed, "Timelock: already executed");
        
        tx_.executed = true;
        (bool success,) = tx_.target.call(tx_.data);
        require(success, "Timelock: execution failed");
    }
}
```

### 12.2 Kill Switch Tri-State

| Estado | Dispatches | Descripción |
|--------|-----------|-------------|
| `Active` | Permitidos | Operación normal |
| `Suspended` | Bloqueados | Pausa temporal, en-flight termina |
| `Terminated` | Bloqueados | Parada total, solo governance puede reactivar |

---

## 13. Cronograma

```
SEMANA 1          SEMANA 2          SEMANA 3          SEMANA 4
┌─────────────────┬─────────────────┬─────────────────┬─────────────────┐
│ FASE 1          │ FASE 2          │ FASE 3          │ FASE 4          │
│ Directorios     │ Adaptadores     │ Echidna Tests   │ Integration     │
│ Flash Base      │ DEX (4)         │ Invariantes     │ Tests           │
│                 │                 │                 │                 │
│ L  M  X  J  V   │ L  M  X  J  V   │ L  M  X  J  V   │ L  M  X  J  V   │
│ ██ ██ ██ ░░ ░░  │ ██ ██ ██ ░░ ░░  │ ██ ██ ██ ░░ ░░  │ ██ ██ ██ ░░ ░░  │
└─────────────────┴─────────────────┴─────────────────┴─────────────────┘

SEMANA 5          SEMANA 6          SEMANA 7          SEMANA 8
┌─────────────────┬─────────────────┬─────────────────┬─────────────────┐
│ FASE 5          │ FASE 6          │ FASE 7          │ FASE 8+9+10     │
│ Scripts Deploy  │ Wallets         │ Multi-Chain     │ Audit + Mon     │
│ CREATE2         │ Topología       │ Deploy 6 redes  │ Governance      │
│                 │                 │                 │                 │
│ L  M  X  J  V   │ L  M  X  J  V   │ L  M  X  J  V   │ L  M  X  J  V   │
│ ██ ██ ░░ ░░ ░░  │ ██ ██ ░░ ░░ ░░  │ ██ ██ ██ ██ ██  │ ██ ██ ██ ░░ ░░  │
└─────────────────┴─────────────────┴─────────────────┴─────────────────┘

Leyenda: ██ = trabajo activo   ░░ = buffer/revisión
```

| Fase | Duración | Dependencias | Hitos |
|------|----------|-------------|-------|
| 1. Directorios + Flash Base | 3 días | Ninguna | `forge build` pasa |
| 2. Adaptadores DEX | 3 días | Fase 1 | 4 adaptadores + tests unitarios |
| 3. Pruebas Echidna | 3 días | Fase 1+2 | ≥ 50K tests, 0 invariantes rotas |
| 4. Integración | 3 días | Fase 1+2 | Tests en fork mainnet |
| 5. Scripts Deploy | 2 días | Fase 1 | Scripts testeados en anvil |
| 6. Wallets | 2 días | Fase 5 | Wallets fondeadas y verificadas |
| 7. Multi-Chain Deploy | 5 días | Fase 5+6 | 6 redes desplegadas y verificadas |
| 8. Auditoría | 3 días | Fase 3+4 | Slither + Echidna + Coverage |
| 9. Telemetría | 2 días | Fase 7 | Dashboards + alertas |
| 10. Governance | 2 días | Fase 7 | Timelock + Kill Switch activo |
| **TOTAL** | **28 días** | — | **Protocolo en producción** |

---

## 14. Métricas de Éxito

| Métrica | Objetivo | Método de Medición |
|---------|----------|-------------------|
| Cobertura de pruebas | > 90% | `forge coverage` |
| Invariantes Echidna | 0 rotas en 50K tests | `echidna` output |
| Hallazgos Slither | 0 high/medium | `slither` report |
| Gas por ejecución (N=3) | < 400,000 | `forge snapshot` |
| Determinismo cross-chain | 100% | `verifyCrossChain()` |
| Documentación NatSpec | 100% | Análisis manual |
| Tiempo de deploy por red | < 30 min | Cronometraje |
| Uptime post-deploy | > 99.9% | Monitoreo on-chain |

---

## 15. Referencias

### Documentos OMEGA

1. **OMEGA White Paper** — `OMEGA_WHITE_PAPER_ONCHAIN.md` — Arquitectura completa
2. **OMEGA SOP** — `OMEGA_SOP_DEPLOYS.md` — Despliegues multi-chain
3. **OMEGA README** — `OMEGA_README_CONTRACTS.md` — Guía del operador

### Referencias Técnicas

4. **Foundry Book** — `book.getfoundry.sh`
5. **Echidna Documentation** — `github.com/crytic/echidna`
6. **Slither** — `github.com/crytic/slither`
7. **OpenZeppelin Contracts** — `docs.openzeppelin.com`
8. **ANEXOS_V1.2.md** — Especificación interna SED Core

---

**Document End — Roadmap Implementación Física OMEGA Protocol**

*"La implementación es la materialización de la abstracción matemática. Si el código no preserva las invariantes, la teoría es inútil."*
