# OMEGA Integration: sed-core <-> EVM
## ArbitrageX-V2 — Protocolo OMEGA AGENT TEAMS
### Plan de Integracion: Sequential Equilibrium Dispatcher <-> Ethereum Virtual Machine
### Version: 2026.05-B1 | Classification: OPERATIONAL-INTEGRATION

---

> **WARNING — OMEGA TERMINOLOGY DOCTRINE**
> Este documento utiliza exclusivamente la nomenclatura OMEGA. Bajo ninguna
> circunstancia se emplearan terminos como "arbitrage", "bot", "snipe",
> "front-run", "sandwich" o cualquier otra denominacion que comprometa la
> integridad operacional del sistema. El lenguaje es parte de la seguridad.

---

## Tabla de Contenidos

1. [Vision Arquitectonica](#1-vision-arquitectonica)
2. [Estructura del Payload](#2-estructura-del-payload)
3. [Codificacion en Rust (searcher-rs)](#3-codificacion-en-rust-searcher-rs)
4. [Verificacion Criptografica](#4-verificacion-criptografica)
5. [Decodificacion Optima en Yul](#5-decodificacion-optima-en-yul)
6. [Flujo End-to-End](#6-flujo-end-to-end)
7. [Tabla de Compatibilidad](#7-tabla-de-compatibilidad)
8. [Seguridad y Firmas](#8-seguridad-y-firmas)
9. [Troubleshooting de Integracion](#9-troubleshooting-de-integracion)
10. [Apendices](#10-apendices)

---

## 1. Vision Arquitectonica

### 1.1 Contexto del Sistema

El sistema OMEGA consta de dos mundos que deben comunicarse de forma segura y
atomica:

| Mundo | Stack | Responsabilidad |
|-------|-------|-----------------|
| **sed-core** | Rust (backend) | Evaluacion matematica del equilibrio de mercado: filtracion estocastica, eigenstates, allocaciones de control optimo, hedging ortogonal |
| **EVM** | Solidity (on-chain) | Ejecucion atomica de swaps, flashloans, verificacion de firmas, distribucion de yield |

### 1.2 Componentes de la Integracion

```
┌──────────────────────────────────────────────────────────────────────┐
│                         MUNDO RUST (off-chain)                        │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────────┐  │
│  │  sed-core   │  │ searcher-rs │  │  prioritization-spine       │  │
│  │             │  │             │  │                             │  │
│  │ Filtration  │──│  Converge.  │──│  swap_encoder.rs            │  │
│  │ (CDC calc)  │  │  Publisher  │  │  round_trip_executor.rs     │  │
│  │ Eigenstate  │  │             │  │                             │  │
│  │ (Hamilt.)   │  │ Redis Pub   │  │  ABI encoding               │  │
│  │ Allocator   │  │             │  │  Resolution packing         │  │
│  │ (Dirac)     │  │ ECDSA Sign  │  │  Calldata construction      │  │
│  │ Hedger      │  │             │  │                             │  │
│  │ (Orthog.)   │  │ Mempool tx  │  │  FlashLoan pathfinding      │  │
│  └─────────────┘  └──────┬──────┘  └─────────────────────────────┘  │
│                          │                                          │
│                    ┌─────┴─────┐                                    │
│                    │  ECDSA    │                                    │
│                    │  Signature│                                    │
│                    └─────┬─────┘                                    │
└──────────────────────────┼──────────────────────────────────────────┘
                           │ tx (signed ResolutionPayload)
                           v
┌──────────────────────────────────────────────────────────────────────┐
│                      MUNDO EVM (on-chain)                             │
│  ┌──────────────────┐  ┌──────────────────┐  ┌─────────────────┐    │
│  │ FlashLoanExecutor│  │ ArbitrageExecutor│  │   Adaptadores   │    │
│  │                  │  │                  │  │                 │    │
│  │ - Aave V3        │  │ - UUPS proxy     │  │ - UniswapV2     │    │
│  │ - Balancer       │  │ - ReentrancyGuard│  │ - UniswapV3     │    │
│  │ - dYdX           │  │ - Pausable       │  │ - Curve         │    │
│  │ - UniV3 FL       │  │ - AccessControl  │  │ - Aerodrome     │    │
│  │                  │  │                  │  │ - PancakeV3     │    │
│  │ requestFlashLoan │  │ executeArbitrage │  │ - TraderJoe     │    │
│  │ executeOperation │  │ - Selector wh.   │  │ - Balancer      │    │
│  │ receiveFlashLoan │  │ - Slippage guard │  │                 │    │
│  └──────────────────┘  └──────────────────┘  └─────────────────┘    │
│           │                     │                    │              │
│           └─────────────────────┴────────────────────┘              │
│                          Flashloan Atomic Loop                       │
└──────────────────────────────────────────────────────────────────────┘
```

### 1.3 Pipeline SED (de arriba hacia abajo)

```
Mempool Data
    │
    v
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  1. Filtration  │────▶│  2. Eigenstate  │────▶│ 3. Allocator    │
│  (Stochastic    │     │  (Effective     │     │ (Dirac Manifold │
│   Markov +      │ CDC │   Hamiltonian   │Energy│  Optimal       │
│   Jump Diff.)   │────▶│   Solver)       │────▶│  Control)      │
│                 │     │                 │     │                 │
│ cdc_value >     │     │ eigenstate_     │     │ optimal_        │
│ 2.706 threshold │     │ energy > 0      │     │ control vector  │
│                 │     │                 │     │                 │
│ Output: CDC +   │     │ Output: Eq.     │     │ Output: Route   │
│ predicted_state │     │   probability   │     │ + amounts       │
└─────────────────┘     └─────────────────┘     └─────────────────┘
                                                        │
                                                        v
                                               ┌─────────────────┐
                                               │ 4. Hedger       │
                                               │ (Orthogonal     │
                                               │  Variance       │
                                               │  Neutralizer)   │
                                               │                 │
                                               │ Output: Final   │
                                               │   Resolution    │
                                               └─────────────────┘
```

---

## 2. Estructura del Payload

### 2.1 Resolution Payload (Rust → EVM)

El `searcher-rs` empaqueta los bytes para el Executor en una estructura que
atraviesa tres capas: Rust encoding → transmision → Solidity decoding.

### 2.1a: Estructura Rust (en searcher-rs / prioritization-spine)

```rust
use ethers::types::{Address, U256, Bytes};

/// ---------------------------------------------------------------------------
/// ResolutionPayload — mensaje firmado que viaja de Rust al Executor EVM.
/// ---------------------------------------------------------------------------
/// Este struct es la interfaz de contrato entre el mundo Rust (sed-core) y
/// el mundo EVM (ArbitrageExecutor / FlashLoanExecutor).
///
/// Invariantes:
///   - target == address(ArbitrageExecutor) para la chain activa
///   - data contiene el ABI-encoded `executeArbitrage(...)` calldata
///   - value es 0 salvo para swaps que requieren ETH nativo
///   - deadline <= block.timestamp + 300 (5 minutos maximo)
///   - signature es ECDSA(secp256k1) del hash estructurado
///
/// SECURITY: El Execution Signer NUNCA debe tener permisos on-chain.
/// Solo firma payloads; el Executor verifica la firma pero el msg.sender
/// debe tener EXECUTOR_ROLE (relayer/keeper separado).
pub struct ResolutionPayload {
    /// Direccion del contrato Executor en la chain destino.
    /// Verificado on-chain contra la direccion desplegada.
    pub target: Address,

    /// Calldata ABI-encoded para `executeArbitrage(...)`.
    /// Construido por `swap_encoder.rs` + `round_trip_executor.rs`.
    pub data: Vec<u8>,

    /// Valor ETH a enviar con la transaccion (wei).
    /// Usualmente 0. Solo != 0 para `swapExactETHForTokens`.
    pub value: U256,

    /// Deadline de expiracion (unix timestamp).
    /// Must be <= block.timestamp + MAX_DEADLINE_DELTA (300s).
    pub deadline: U256,

    /// Firma ECDSA (secp256k1) del hash estructurado.
    /// v: 1 byte (27 o 28, o 0/1 en formato EIP-155)
    /// r: 32 bytes
    /// s: 32 bytes
    /// Total: 65 bytes
    pub signature: Vec<u8>,
}

/// Hash estructurado que se firma. Incluye chainId para prevenir replay
/// cross-chain.
pub struct ResolutionHash {
    /// keccak256(abi.encode(target, data, value, deadline, chainId))
    pub hash: [u8; 32],
}

impl ResolutionPayload {
    /// Construye el hash a firmar segun el esquema OMEGA.
    ///
    /// Esquema: keccak256(abi.encode(
    ///     target,      // address  — 20 bytes
    ///     keccak256(data), // bytes32 — hash del calldata
    ///     value,       // uint256
    ///     deadline,    // uint256
    ///     chainId      // uint256  — proteccion anti-replay
    /// ))
    pub fn compute_hash(&self, chain_id: u64) -> [u8; 32] {
        use ethers::utils::keccak256;
        use ethers::abi::{encode, Token, Address as EthAddress};

        let data_hash = keccak256(&self.data);

        let encoded = encode(&[
            Token::Address(self.target),
            Token::FixedBytes(data_hash.to_vec()),
            Token::Uint(self.value),
            Token::Uint(self.deadline),
            Token::Uint(U256::from(chain_id)),
        ]);

        keccak256(&encoded).into()
    }

    /// Firma el hash con la clave del Execution Signer.
    pub fn sign(&mut self, wallet: &LocalWallet, chain_id: u64) {
        let hash = self.compute_hash(chain_id);
        let signature = wallet.sign_message(&hash).unwrap();
        self.signature = signature.to_vec();
    }
}
```

### 2.1b: Esquema de Hash (detalle byte-level)

```
Hash Input (abi.encode):
+------------------+------------------+------------------+------------------+------------------+
|     target       |   keccak(data)   |      value       |     deadline     |     chainId      |
|    (20 bytes)    |    (32 bytes)    |    (32 bytes)    |    (32 bytes)    |    (32 bytes)    |
|  address padded  |    bytes32       |     uint256      |     uint256      |     uint256      |
+------------------+------------------+------------------+------------------+------------------+
Total: 160 bytes

keccak256(output) = 32 bytes (hash final a firmar)
```

### 2.1c: Estructura de la Firma (65 bytes)

```
Signature Layout:
+--------+----------------------------------+----------------------------------+
|  v (1B)|              r (32B)              |              s (32B)             |
+--------+----------------------------------+----------------------------------+
|  27/28 |  bytes aleatorios (secp256k1)     |  bytes aleatorios (secp256k1)    |
+--------+----------------------------------+----------------------------------+
Total: 65 bytes

NOTA: En formato EIP-155, v puede ser chainId * 2 + 35/36.
      El contrato acepta ambos formatos.
```

---

## 3. Codificacion en Rust (searcher-rs)

### 3.1 Swap Encoder (prioritization-spine/src/swap_encoder.rs)

Este modulo es la base de la codificacion. Proporciona funciones puras que
producen calldata bytes para routers Uniswap V2/V3 y funciones ERC20.

#### Selectores de funcion pre-calculados:

| Funcion | Selector (hex) | Bytes |
|---------|---------------|-------|
| `swapExactTokensForTokens(uint256,uint256,address[],address,uint256)` | `0x38ed1739` | 4 |
| `swapExactETHForTokens(uint256,address[],address,uint256)` | `0x7ff36ab5` | 4 |
| `approve(address,uint256)` | `0x095ea7b3` | 4 |
| `balanceOf(address)` | `0x70a08231` | 4 |
| `transfer(address,uint256)` | `0xa9059cbb` | 4 |
| `exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160))` | `0x414bf389` | 4 |
| `exactInput((bytes,address,uint256,uint256,uint256))` | `0xc04b8d59` | 4 |

#### Ejemplo de codificacion V2:

```rust
use ethers::types::{U256, Address};

// Codificar swap V2: WETH -> USDC
let calldata = encode_v2_swap_exact_tokens_for_tokens(
    U256::from(1_000_000_000_000_000_000u64), // 1 WETH (18 decimals)
    U256::from(1_800_000_000u64),              // min 1800 USDC (6 decimals)
    &[
        Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(), // WETH
        Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(), // USDC
    ],
    Address::from_str("0x1111111111111111111111111111111111111111").unwrap(), // recipient
    U256::from(1_700_000_000u64), // deadline (unix timestamp)
);

// Resultado: bytes que se envian directamente al router V2
assert_eq!(&calldata[..4], &[0x38, 0xed, 0x17, 0x39]); // selector correcto
```

#### Ejemplo de codificacion V3:

```rust
// Codificar swap V3 single-hop: WETH -> USDC (0.05% fee tier)
let params = V3ExactInputSingleParams {
    token_in: weth_address(),
    token_out: usdc_address(),
    fee: 500,  // 0.05% en unidades V3 (no bps)
    recipient: executor_address(),
    deadline: U256::from(get_unix_timestamp() + 300),
    amount_in: U256::from(1e18 as u64), // 1 WETH
    amount_out_minimum: U256::from(1_800_000_000u64), // 1800 USDC min
    sqrt_price_limit_x96: U256::zero(), // sin limite de precio
};

let calldata = encode_v3_exact_input_single(&params);
assert_eq!(&calldata[..4], &[0x41, 0x4b, 0xf3, 0x89]); // selector correcto
```

### 3.2 Round-Trip Executor (prioritization-spine/src/round_trip_executor.rs)

Este modulo construye la ruta completa (ida y vuelta) para el Executor:

```rust
/// Construye el payload completo para `executeArbitrage`.
///
/// Parameters:
/// - route_hash: Hash unico de la ruta (para eventos/indexacion)
/// - token_in: Token de entrada/salida (ruta circular)
/// - token_out: Token intermedio
/// - amount_in: Cantidad a invertir
/// - min_profit: Beneficio minimo aceptable (slippage guard)
/// - routers: Array de direcciones de routers (uno por swap)
/// - payloads: Array de calldata para cada router
pub fn build_execute_arbitrage_payload(
    route_hash: [u8; 32],
    token_in: Address,
    token_out: Address,
    amount_in: U256,
    min_profit: U256,
    routers: Vec<Address>,
    payloads: Vec<Bytes>,
) -> Vec<u8> {
    use ethers::abi::{encode, Token};

    let router_tokens: Vec<Token> = routers.into_iter()
        .map(Token::Address)
        .collect();

    let payload_tokens: Vec<Token> = payloads.into_iter()
        .map(|b| Token::Bytes(b.to_vec()))
        .collect();

    encode(&[
        Token::FixedBytes(route_hash.to_vec()),
        Token::Address(token_in),
        Token::Address(token_out),
        Token::Uint(amount_in),
        Token::Uint(min_profit),
        Token::Array(router_tokens),
        Token::Array(payload_tokens),
    ])
}
```

### 3.3 Publicacion de la Resolucion

```rust
// En searcher-rs, tras calcular la resolucion:

// 1. Construir el ResolutionPayload
let mut payload = ResolutionPayload {
    target: executor_address_for_chain(chain_id),
    data: build_execute_arbitrage_payload(...),
    value: U256::zero(),
    deadline: U256::from(now + 300), // 5 minutos
    signature: vec![], // se llena en el paso 2
};

// 2. Firmar con el Execution Signer
let execution_wallet = LocalWallet::from_str(&env::var("EXECUTION_SIGNER_KEY")?)?;
payload.sign(&execution_wallet, chain_id);

// 3a. Enviar via mempool (broadcast publico)
let tx = TransactionRequest::new()
    .to(payload.target)
    .data(payload.data.clone())
    .value(payload.value)
    .gas(500_000)
    .max_fee_per_gas(gwei_to_wei(max_gas_gwei))
    .max_priority_fee_per_gas(gwei_to_wei(priority_gwei));

let pending_tx = provider.send_transaction(tx, None).await?;

// 3b. O enviar via Flashbots (private mempool)
let bundle = BundleRequest::new()
    .push_transaction(tx.rlp_signed(&signature)?)
    .set_block(target_block_number);

let response = flashbots_client.send_bundle(&bundle).await?;
```

---

## 4. Verificacion Criptografica

### 4.1 Esquema de Firma en el Executor

El contrato `ArbitrageExecutor` verifica la firma antes de ejecutar cualquier
operacion. El esquema sigue el estandar EIP-191 (version 0x45 — signed data).

### 4.2 Pseudocodigo de Verificacion (Solidity)

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @title ResolutionVerifier — utilidad de verificacion criptografica
/// @notice Verifica firmas ECDSA de payloads enviados desde searcher-rs.
/// @dev Se integra en ArbitrageExecutor como libreria interna.
library ResolutionVerifier {

    /// @notice Reconstruye el hash firmado siguiendo el esquema OMEGA.
    /// @dev keccak256(abi.encode(target, keccak256(data), value, deadline, chainId))
    function getResolutionHash(
        address target,
        bytes calldata data,
        uint256 value,
        uint256 deadline,
        uint256 chainId
    ) internal pure returns (bytes32) {
        return keccak256(abi.encode(
            target,
            keccak256(data),
            value,
            deadline,
            chainId
        ));
    }

    /// @notice Verifica firma ECDSA contra un authorizedSigner.
    /// @param hash      Hash del mensaje (32 bytes).
    /// @param signature Firma ECDSA (65 bytes: v + r + s).
    /// @param signer    Direccion esperada del firmante.
    /// @return valid    True si la firma es valida y del signer esperado.
    function verifySignature(
        bytes32 hash,
        bytes calldata signature,
        address signer
    ) internal pure returns (bool valid) {
        // Prefijo EIP-191: "\x19Ethereum Signed Message:\n32"
        bytes32 ethHash = keccak256(abi.encodePacked(
            "\x19Ethereum Signed Message:\n32",
            hash
        ));

        // Extraer v, r, s de la firma
        require(signature.length == 65, "Invalid signature length");

        bytes32 r;
        bytes32 s;
        uint8 v;

        assembly ("memory-safe") {
            r := calldataload(add(signature.offset, 0x20))
            s := calldataload(add(signature.offset, 0x40))
            v := byte(0, calldataload(add(signature.offset, 0x60)))
        }

        // Proteccion contra malleability (s debe estar en la mitad inferior)
        require(uint256(s) <= 0x7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF5D576E7357A4501DDFE92F46681B20A0,
            "Invalid s value");

        // Recuperar direccion del firmante
        address recovered = ecrecover(ethHash, v, r, s);

        return recovered == signer && recovered != address(0);
    }
}
```

### 4.3 Flujo de Verificacion Completo

```solidity
function validateAndExecute(
    bytes calldata data,
    uint256 value,
    uint256 deadline,
    bytes calldata signature
) external onlyRole(EXECUTOR_ROLE) nonReentrant {

    // ---------- PASO 1: Deadline check (filtration CDC equivalent) ----------
    require(block.timestamp <= deadline, "Resolution expired");

    // ---------- PASO 2: Hash reconstruction ----------
    bytes32 resolutionHash = ResolutionVerifier.getResolutionHash(
        address(this),  // target
        data,           // calldata
        value,          // ETH value
        deadline,       // deadline
        block.chainid   // chainId anti-replay
    );

    // ---------- PASO 3: Signature verification ----------
    bool valid = ResolutionVerifier.verifySignature(
        resolutionHash,
        signature,
        authorizedSigner  // direccion del Execution Signer
    );
    require(valid, "Invalid signature — unauthorized resolution");

    // ---------- PASO 4: Execute (eigenstate/allocator equivalent) ----------
    // Decodificar y ejecutar el calldata
    (bool success, ) = address(this).call(data);
    require(success, "Execution failed");
}
```

### 4.4 Verificacion On-Chain (cast CLI)

```bash
# Verificar que el authorizedSigner esta configurado
cast call $ARBITRAGE_EXECUTOR "authorizedSigner()(address)" \
  --rpc-url $RPC_HTTP_1

# Verificar que EXECUTOR_ROLE esta asignado al relayer
cast call $ARBITRAGE_EXECUTOR "hasRole(bytes32,address)(bool)" \
  $(cast keccak "EXECUTOR_ROLE") $RELAYER_ADDRESS \
  --rpc-url $RPC_HTTP_1

# Verificar firma off-chain (debug)
cast verify-signature $MESSAGE_HASH $SIGNATURE $AUTHORIZED_SIGNER
```

---

## 5. Decodificacion Optima en Yul

### 5.1 Motivacion

La decodificacion del calldata en el Executor es un hotspot de gas. Cada byte
ahorrado en decoding se multiplica por el numero de ejecuciones. Por eso se
utiliza Yul inline para operaciones criticas.

### 5.2 Decodificacion de Selector (A5 — audit 2026-05-10)

```solidity
// En ArbitrageExecutor.executeArbitrage():

// A5: selector whitelist gate.
// Extraer los 4 bytes del selector sin allocacion de memoria.
bytes calldata pld = payload[i];
if (pld.length < 4) revert AE_PayloadTooShort(router);

bytes4 selector;
// Extract the leading 4 bytes without a memory allocation (gas-optimal).
assembly {
    selector := calldataload(pld.offset)
}

// Verificar que el selector esta aprobado para este router
if (!approvedSelectors[router][selector]) {
    revert AE_RouterSelectorNotApproved(router, selector);
}
```

**Analisis de gas:**

| Metodo | Gas (aprox) | Notas |
|--------|-------------|-------|
| `abi.decode()` standard | ~200 + mem alloc | Requiere memoria auxiliar |
| `calldataload` directo | ~6 | Sin memoria, directo de calldata |
| Ahorro | ~97% | Significativo en loops de N swaps |

### 5.3 Decodificacion de la Resolucion Completa (Yul)

```solidity
/// @notice Decodifica una Resolution desde calldata con minimo overhead.
/// @dev Usa directamente calldataload para evitar copias a memoria.
///      Optimizado para el layout especifico de `executeArbitrage`.
function _decodeResolution(bytes calldata _data)
    internal
    pure
    returns (
        bytes32 routeHash,
        address tokenIn,
        address tokenOut,
        uint256 amountIn,
        uint256 minProfit,
        address[] calldata routers,
        bytes[] calldata payload
    )
{
    assembly ("memory-safe") {
        // Layout del calldata de executeArbitrage:
        // [0:4]     selector (0x...)
        // [4:36]    routeHash (bytes32)
        // [36:68]   tokenIn (address, 20 bytes en los ultimos 20)
        // [68:100]  tokenOut (address)
        // [100:132] amountIn (uint256)
        // [132:164] minProfit (uint256)
        // [164:196] offset routers array
        // [196:228] offset payload array

        let dataOffset := _data.offset

        // routeHash: offset 4, 32 bytes
        routeHash := calldataload(add(dataOffset, 4))

        // tokenIn: offset 36, ultimos 20 bytes
        tokenIn := shr(96, calldataload(add(dataOffset, 36)))

        // tokenOut: offset 68, ultimos 20 bytes
        tokenOut := shr(96, calldataload(add(dataOffset, 68)))

        // amountIn: offset 100
        amountIn := calldataload(add(dataOffset, 100))

        // minProfit: offset 132
        minProfit := calldataload(add(dataOffset, 132))

        // routers array: offset dinamico
        let routersOffset := add(dataOffset, add(calldataload(add(dataOffset, 164)), 4))
        routers.offset := add(routersOffset, 0x20) // skip length word
        routers.length := calldataload(routersOffset)

        // payload array: offset dinamico
        let payloadOffset := add(dataOffset, add(calldataload(add(dataOffset, 196)), 4))
        payload.offset := add(payloadOffset, 0x20) // skip length word
        payload.length := calldataload(payloadOffset)
    }
}
```

### 5.4 Acceso a Arrays Dinamicos en Yul

```solidity
/// @notice Acceso indexado a un array dinamico de bytes sin copia.
function _getPayloadAt(bytes[] calldata payloads, uint256 index)
    internal
    pure
    returns (bytes calldata result)
{
    assembly ("memory-safe") {
        // Cada elemento de un bytes[] es un offset relativo
        let baseOffset := payloads.offset
        let elemOffsetPtr := add(baseOffset, mul(index, 0x20))
        let elemRelativeOffset := calldataload(elemOffsetPtr)
        let elemAbsoluteOffset := add(payloads.offset, elemRelativeOffset)

        // El array de bytes empieza con su length
        let len := calldataload(elemAbsoluteOffset)

        result.offset := add(elemAbsoluteOffset, 0x20)
        result.length := len
    }
}
```

---

## 6. Flujo End-to-End

### 6.1 Diagrama de Secuencia Completo

```
searcher-rs (Rust)                          EVM (Solidity)
    │                                            │
    │ 1. Mempool subscription (WSS)              │
    │◄───────────────────────────────────────────│
    │                                            │
    │ 2. sed-core pipeline:                      │
    │    filtration ──▶ eigenstate ──▶           │
    │    allocator ──▶ hedger                    │
    │    Topological Yield > 0 ✅                │
    │                                            │
    │ 3. swap_encoder.rs:                        │
    │    - Encode V2/V3 swaps                    │
    │    - Build executeArbitrage calldata       │
    │                                            │
    │ 4. round_trip_executor.rs:                 │
    │    - Calculate route hash                  │
    │    - Set minProfit (slippage guard)        │
    │                                            │
    │ 5. ResolutionPayload construction          │
    │    - target: ArbitrageExecutor address     │
    │    - data: ABI-encoded calldata            │
    │    - deadline: now + 300                   │
    │    - value: 0 (no ETH)                     │
    │                                            │
    │ 6. Sign with Execution Signer (ECDSA)      │
    │    hash = keccak256(abi.encode(             │
    │      target, keccak256(data),               │
    │      value, deadline, chainId               │
    │    ))                                       │
    │    signature = ecdsa_sign(hash)             │
    │                                            │
    │ 7a. Broadcast via public mempool            │
    │ ───────── tx ─────────────────────────────▶│
    │     (target, data, value, sig)             │
    │                                            │
    │ 7b. O via Flashbots private relay           │
    │ ───────── bundle ────────────────────────▶ │
    │     (tx + target block)                    │
    │                                            │
    │                                     8. FlashLoanExecutor
    │                                        receiveFlashLoan()
    │                                        (callback del provider)
    │                                            │
    │                                     9. Verify callbacks:
    │                                        - msg.sender == vault
    │                                        - initiator == this
    │                                            │
    │                                    10. Approve ArbitrageExecutor
    │                                        asset.forceApprove(executor, amount)
    │                                            │
    │                                    11. Delegate execution:
    │                                        (bool ok, ) = executor.call(params)
    │                                            │
    │                                    12. ArbitrageExecutor:
    │                                        executeArbitrage()
    │                                            │
    │                                    13. Validate guards:
    │                                        - routers.length == payload.length
    │                                        - tokenIn approved ✅
    │                                        - tokenOut approved ✅ (M8)
    │                                        - balance >= amountIn ✅
    │                                        - For each router:
    │                                          * router approved ✅
    │                                          * allowance manager ✅ (SC-5)
    │                                          * selector whitelisted ✅ (A5)
    │                                          * low-level call ✅
    │                                            │
    │                                    14. Post-execution validation:
    │                                        - balanceAfter > balanceBefore
    │                                        - profit >= minProfit ✅
    │                                            │
    │                                    15. Emit event:
    │                                        ArbitrageExecuted(
    │                                          routeHash, tokenIn,
    │                                          tokenOut, profit
    │                                        )
    │                                            │
    │                                    16. Repay flashloan
    │                                        asset.forceApprove(vault, amount+fee)
    │                                        asset.transfer(vault, amount+fee)
    │                                            │
    │                                    17. Yield distribution:
    │                                        profit → Cold Treasury
    │◄───────────────────────────────────────────│
    │ 18. searcher-rs receives confirmation      │
    │     (event monitoring via WSS)             │
    │                                            │
    │ 19. Redis PUBLISH convergence signal       │
    │     arbx:signals:convergence               │
```

### 6.2 Estados de la Transaccion

```
PENDING ──▶ BROADCAST ──▶ MEMPOOL ──▶ MINED ──▶ CONFIRMED
   │            │            │          │           │
   │            │            │          │           └── 12+ confirmaciones
   │            │            │          │               (segun chain)
   │            │            │          └── Inclusion en bloque
   │            │            │              (evento emitido)
   │            │            └── En mempool del validador
   │            │               (vulnerable a MEV)
   │            └── Enviado a relay/mempool
   └── Construccion del ResolutionPayload
```

### 6.3 Estados de Error

```
┌─────────────────────────┬──────────────────────────────┬─────────────┐
│ Estado                  │ Causa                        │ Accion      │
├─────────────────────────┼──────────────────────────────┼─────────────┤
│ REVERT: DeadlineExpired │ block.timestamp > deadline   │ Descartar   │
│ REVERT: NotExecutor     │ msg.sender sin EXECUTOR_ROLE │ Alerta sec. │
│ REVERT: InvalidSig      │ Firma invalida               │ Alerta sec. │
│ REVERT: LengthMismatch  │ routers != payload.len       │ Bug report  │
│ REVERT: TokenNotApproved│ Token no en whitelist        │ Reconfigurar│
│ REVERT: InsufficientBal │ Balance < amountIn           │ Reabastecer │
│ REVERT: RouterNotApproved│ Router no en whitelist      │ Reconfigurar│
│ REVERT: SwapFailed      │ Router reverto               │ Slippage    │
│ REVERT: ZeroGrossProfit │ Sin beneficio bruto          │ Oportunidad │
│                         │                              │ perdida     │
│ REVERT: InsufficientPrft│ profit < minProfit           │ Reajustar   │
│ REVERT: FL_Unauthorized │ Callback no del vault        │ Alerta sec. │
│ REVERT: FL_InvalidInit  │ Initiator incorrecto         │ Alerta sec. │
└─────────────────────────┴──────────────────────────────┴─────────────┘
```

---

## 7. Tabla de Compatibilidad

### 7.1 sed-core Features vs EVM Equivalents

| Feature sed-core | EVM Equivalent | Estado | Detalle Tecnico |
|-----------------|----------------|--------|-----------------|
| `filtration` (CDC) | Deadline check | Operativo | `block.timestamp <= deadline` en `validateAndExecute` |
| `filtration` (CDC threshold > 2.706) | Slippage protection (`minProfit`) | Operativo | `profit >= minProfit` en `executeArbitrage` |
| `eigenstate` (Eq. probability) | Balance validation | Operativo | `balanceBefore >= amountIn` pre-ejecucion |
| `eigenstate` (Energy > 0) | Profit validation | Operativo | `balanceAfter > balanceBefore` post-ejecucion |
| `allocator` (Control vector) | Calldata encoding | Operativo | `swap_encoder.rs` produce bytes para cada swap |
| `allocator` (Dirac impulse) | Atomic execution | Operativo | Todo en una tx atomica: flashloan → swaps → repay |
| `hedger` (Orthogonal null) | Revert on failure | Operativo | `require(success)` revierte toda la tx si un swap falla |
| `hedger` (Entanglement) | Multi-hop route | Operativo | `routers[]` + `payload[]` permiten rutas N-leg |
| CDC confidence score | Selector whitelist | Operativo | `approvedSelectors[router][selector]` (A5) |
| Predicted state | Allowance Manager | Operativo | `IAllowanceManager.isApproved()` (SC-5) |
| Variance envelope | Gas estimation | Operativo | `gas_estimate_units` en trading_config |
| Spectral gap | Flashloan fee check | Operativo | `selectCheapestProvider()` compara fees |

### 7.2 Mapeo de Tablas sed-core a Contratos

| Tabla PostgreSQL | Contrato Solidity | Proposito |
|-----------------|-------------------|-----------|
| `sed_filtrations` | Deadline + `minProfit` | Validacion temporal y de beneficio |
| `sed_eigenstates` | `approvedTokens` + `approvedRouters` | Verificacion de activos/rutas aprobados |
| `sed_allocations` | `executeArbitrage` params | Ejecucion del control optimo on-chain |
| `sed_hedges` | Revert atomic + slippage | Proteccion contra varianza no deseada |

### 7.3 Mapeo de Telemetria a Eventos EVM

| Senal Rust (telemetry) | Evento Solidity | Canal Redis |
|------------------------|-----------------|-------------|
| `ConvergenceSignal` | `ArbitrageExecuted` | `arbx:signals:convergence` |
| `pipeline_latency_ms` | Block timestamp delta | — |
| `opportunities_detected` | Event count (indexer) | — |
| `simulations_success` | Successful txs count | — |
| `mempool_entropy_score` | Gas price oracle | — |

---

## 8. Seguridad y Firmas

### 8.1 Modelo de Segregacion de Wallets

```
┌─────────────────────────────────────────────────────────────────┐
│                    MODELO DE 3 WALLETS                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────────┐                                            │
│  │  GAS SPONSOR    │── HOT ──► Paga gas de transacciones       │
│  │  (0xGAS...)     │           Balance minimo mantenido         │
│  │                 │           NUNCA firma, NUNCA posee fondos  │
│  └─────────────────┘           significativos                   │
│                                                                 │
│  ┌─────────────────┐                                            │
│  │ EXECUTION SIGNER│── SEMI-HOT ──► Firma ResolutionPayloads   │
│  │ (0xEXEC...)     │                Clave en .env (nunca en    │
│  │                 │                codigo)                     │
│  └─────────────────┘                                            │
│                                                                 │
│  ┌─────────────────┐                                            │
│  │  COLD TREASURY  │── COLD ──► Recibe yield post-ejecucion   │
│  │ (0xTREAS...)    │           Hardware wallet / multisig       │
│  │                 │           NUNCA firma transacciones        │
│  └─────────────────┘                                            │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 8.2 Configuracion de Authorized Signer

```solidity
// En el ArbitrageExecutor:

address public authorizedSigner;

function setAuthorizedSigner(address _signer) external onlyRole(ADMIN_ROLE) {
    require(_signer != address(0), "Zero address");
    authorizedSigner = _signer;
    emit AuthorizedSignerUpdated(_signer);
}
```

### 8.3 Verificacion de Permisos

```bash
# 1. Verificar authorizedSigner
cast call $ARBITRAGE_EXECUTOR "authorizedSigner()(address)" \
  --rpc-url $RPC_HTTP_1

# 2. Verificar EXECUTOR_ROLE del relayer
cast call $ARBITRAGE_EXECUTOR "hasRole(bytes32,address)(bool)" \
  $(cast keccak "EXECUTOR_ROLE") $RELAYER_ADDRESS \
  --rpc-url $RPC_HTTP_1

# 3. Verificar ADMIN_ROLE
cast call $ARBITRAGE_EXECUTOR "hasRole(bytes32,address)(bool)" \
  $(cast keccak "DEFAULT_ADMIN_ROLE") $ADMIN_ADDRESS \
  --rpc-url $RPC_HTTP_1

# 4. Verificar UPGRADER_ROLE
cast call $ARBITRAGE_EXECUTOR "hasRole(bytes32,address)(bool)" \
  $(cast keccak "UPGRADER_ROLE") $UPGRADER_ADDRESS \
  --rpc-url $RPC_HTTP_1
```

### 8.4 Rotacion de Execution Signer

```bash
# PASO 1: Generar nueva wallet
cast wallet new --json > /tmp/new_signer.json
NEW_SIGNER=$(jq -r '.address' /tmp/new_signer.json)

# PASO 2: Actualizar en el contrato (via timelock/admin)
cast send $ARBITRAGE_EXECUTOR \
  "setAuthorizedSigner(address)" $NEW_SIGNER \
  --rpc-url $RPC_HTTP_1 \
  --private-key $ADMIN_KEY

# PASO 3: Verificar actualizacion
cast call $ARBITRAGE_EXECUTOR "authorizedSigner()(address)" \
  --rpc-url $RPC_HTTP_1
# Debe retornar: $NEW_SIGNER

# PASO 4: Actualizar .env (nunca en git)
sed -i "s/EXECUTION_SIGNER_ADDRESS=.*/EXECUTION_SIGNER_ADDRESS=$NEW_SIGNER/" .env

# PASO 5: Reiniciar searcher-rs
sudo systemctl restart arbitragex-searcher-rs
```

---

## 9. Troubleshooting de Integracion

### 9.1 Firma Invalida

**Sintoma:** `Invalid signature — unauthorized resolution`

```bash
# Diagnostico:
# 1. Verificar que el hash se computa correctamente
cast keccak $(cast abi-encode "f(address,bytes32,uint256,uint256,uint256)" \
  $TARGET \
  $(cast keccak $DATA) \
  $VALUE \
  $DEADLINE \
  $CHAIN_ID)

# 2. Verificar la firma off-chain
cast verify-signature $HASH $SIGNATURE $AUTHORIZED_SIGNER

# Causas comunes:
# - Chain ID incorrecto en el hash (anti-replay falla)
# - Format EIP-191 no aplicado al hash
# - Execution Signer cambiado pero .env no actualizado
# - Firma hecha con wallet equivocada
```

### 9.2 Deadline Expired

**Sintoma:** `Resolution expired`

```bash
# Diagnostico:
# Verificar deadline vs block timestamp
cast call $RPC_HTTP_1 "eth_blockNumber" | cast to-dec
cast block --rpc-url $RPC_HTTP_1 latest timestamp

# Si deadline < block.timestamp, el searcher-rs esta usando un reloj
# desincronizado. Verificar NTP:
ntpq -p

# Fix: sincronizar reloj del servidor
sudo ntpdate -s pool.ntp.org
```

### 9.3 Swap Failed

**Sintoma:** `SwapFailed` (revert en router call)

```bash
# Diagnostico:
# 1. Simular la transaccion con cast
cast call $ROUTER $CALLDATA \
  --rpc-url $RPC_HTTP_1 \
  --from $EXECUTOR_ADDRESS \
  --trace

# Causas comunes:
# - Slippage excedido (amountOutMin demasiado alto)
# - Pool sin liquidez suficiente
# - Token no aprobado para el router
# - Deadline ya pasado en el router

# Fix: Ajustar minProfit o amountOutMin en trading_config
```

### 9.4 Flash Loan Execution Failed

**Sintoma:** `FL_ArbitrageExecutionFailed`

```bash
# Diagnostico:
# 1. Verificar que el ArbitrageExecutor tiene balance suficiente
#    (flashloan debe haber transferido fondos)
cast call $FLASHLOAN_ASSET "balanceOf(address)(uint256)" $ARBITRAGE_EXECUTOR \
  --rpc-url $RPC_HTTP_1

# 2. Verificar que el allowance esta correcto
cast call $FLASHLOAN_ASSET "allowance(address,address)(uint256)" \
  $FLASHLOAN_EXECUTOR $ARBITRAGE_EXECUTOR \
  --rpc-url $RPC_HTTP_1

# 3. Simular la ejecucion interna
cast call $ARBITRAGE_EXECUTOR \
  "executeArbitrage(bytes32,address,address,uint256,uint256,address[],bytes[])" \
  $ROUTE_HASH $TOKEN_IN $TOKEN_OUT $AMOUNT_IN $MIN_PROFIT \
  "[$ROUTER1,$ROUTER2]" "[$PAYLOAD1,$PAYLOAD2]" \
  --rpc-url $RPC_HTTP_1 \
  --from $FLASHLOAN_EXECUTOR
```

### 9.5 Callback No Autorizado

**Sintoma:** `FL_UnauthorizedCaller` o `FL_InvalidInitiator`

```bash
# Diagnostico:
# 1. Verificar que el callback proviene del vault correcto
cast call $FLASHLOAN_EXECUTOR "balancerVault()(address)" \
  --rpc-url $RPC_HTTP_1
# Debe coincidir con el Balancer Vault canonico:
# Ethereum: 0xBA12222222228d8Ba445958a75a0704d566BF2C8

# 2. Verificar que msg.sender es el vault
# (requiere logging en el contrato o traza de transaccion)

# Causas comunes:
# - Vault address no configurado (address(0))
# - Ataque de callback spoofing (bloqueado por A4)
# - Flashloan provider incorrecto configurado
```

### 9.6 Selector Not Approved

**Sintoma:** `AE_RouterSelectorNotApproved`

```bash
# Diagnostico:
# 1. Verificar que el selector esta aprobado para el router
cast call $ARBITRAGE_EXECUTOR \
  "approvedSelectors(address,bytes4)(bool)" \
  $ROUTER_ADDRESS $SELECTOR \
  --rpc-url $RPC_HTTP_1

# Si retorna false, aprobar el selector:
cast send $ARBITRAGE_EXECUTOR \
  "setRouterSelectorApproval(address,bytes4,bool)" \
  $ROUTER_ADDRESS $SELECTOR true \
  --rpc-url $RPC_HTTP_1 \
  --private-key $ADMIN_KEY

# Selectores comunes a aprobar:
# UniswapV2: 0x38ed1739 (swapExactTokensForTokens)
# UniswapV3: 0x414bf389 (exactInputSingle)
# UniswapV3: 0xc04b8d59 (exactInput)
```

---

## 10. Apendices

### Apendice A: Selectores de Funcion Comunes

| Contrato | Funcion | Selector | Uso |
|----------|---------|----------|-----|
| ArbitrageExecutor | `executeArbitrage(bytes32,address,address,uint256,uint256,address[],bytes[])` | `0x...` | Entrada principal |
| ArbitrageExecutor | `setRouterApproval(address,bool)` | `0x...` | Admin: aprobar router |
| ArbitrageExecutor | `setTokenApproval(address,bool)` | `0x...` | Admin: aprobar token |
| ArbitrageExecutor | `setRouterSelectorApproval(address,bytes4,bool)` | `0x...` | Admin: A5 |
| ArbitrageExecutor | `batchSetRouterSelectorApproval(address,bytes4[],bool)` | `0x...` | Admin: batch A5 |
| ArbitrageExecutor | `setAllowanceManager(address)` | `0x...` | Admin: SC-5 |
| ArbitrageExecutor | `emergencyWithdraw(address)` | `0x...` | Admin: rescate |
| ArbitrageExecutor | `pause()` | `0x...` | Admin: emergencia |
| ArbitrageExecutor | `unpause()` | `0x...` | Admin: reactivar |
| FlashLoanExecutor | `requestFlashLoan(address,uint256,bytes)` | `0x...` | EXECUTOR_ROLE |
| FlashLoanExecutor | `executeOperation(...)` | `0x...` | Callback Aave |
| FlashLoanExecutor | `receiveFlashLoan(...)` | `0x...` | Callback Balancer |
| FlashLoanExecutor | `setFlashLoanProvider(address)` | `0x...` | Admin: SC-1 |
| FlashLoanExecutor | `setBalancerVault(address)` | `0x...` | Admin: A4 |
| FlashLoanExecutor | `setReferralCode(uint16)` | `0x...` | Admin: SC-8 |

### Apendice B: ABI Encoding Reference

```
executeArbitrage(
    bytes32 routeHash,       // 32 bytes — hash de la ruta
    address tokenIn,         // 32 bytes (padded) — token de entrada
    address tokenOut,        // 32 bytes (padded) — token intermedio
    uint256 amountIn,        // 32 bytes — cantidad a invertir
    uint256 minProfit,       // 32 bytes — beneficio minimo
    address[] routers,       // dynamic — array de routers
    bytes[] payload          // dynamic — array de calldata
)

Layout calldata:
[0:4]     selector
[4:36]    routeHash
[36:68]   tokenIn
[68:100]  tokenOut
[100:132] amountIn
[132:164] minProfit
[164:196] offset routers (relative to start of args)
[196:228] offset payload (relative to start of args)
[228:...] routers array data
[...:...] payload array data
```

### Apendice C: Eventos para Indexacion Off-Chain

```solidity
// Evento principal — emitido en ejecucion exitosa
event ArbitrageExecuted(
    bytes32 indexed routeHash,
    address tokenIn,
    address tokenOut,
    uint256 profit
);

// Eventos administrativos
event RouterApproved(address router, bool status);
event TokenApproved(address token, bool status);
event RouterSelectorApproved(address indexed router, bytes4 indexed selector, bool status);
event AllowanceManagerUpdated(address indexed allowanceManager);
event EmergencyWithdrawn(address token, uint256 amount);
event ETHWithdrawn(address indexed to, uint256 amount);

// Eventos de flashloan
event FlashLoanRequested(address indexed asset, uint256 amount, bytes32 paramsHash);
event FlashLoanExecuted(address indexed asset, uint256 amount, uint256 premium, bool success);
event FlashLoanProviderUpdated(address indexed provider);
event BalancerVaultUpdated(address indexed previousVault, address indexed newVault);
```

### Apendice D: Esquema de Telemetria Redis

```rust
// Canal: arbx:signals:convergence
// Publisher: searcher-rs/src/telemetry_publisher.rs

pub struct ConvergenceSignal {
    pub entropy_snapshot: EntropySnapshot,
    pub pipeline_latency_ms: u64,
    pub opportunities_detected: u64,
    pub simulations_run: u64,
    pub simulations_success: u64,
    pub timestamp: String,      // ISO8601 UTC
    pub schema_version: u8,     // actualmente 1
}

pub struct EntropySnapshot {
    pub mempool_tx_per_sec: f64,
    pub mempool_avg_gas_price_gwei: f64,
    pub mempool_entropy_score: f64,     // 0.0 = orden, 1.0 = caos
    pub reserve_divergence_max: f64,
}
```

Ejemplo de payload JSON:
```json
{
  "entropy_snapshot": {
    "mempool_tx_per_sec": 12.5,
    "mempool_avg_gas_price_gwei": 25.3,
    "mempool_entropy_score": 0.78,
    "reserve_divergence_max": 0.04
  },
  "pipeline_latency_ms": 145,
  "opportunities_detected": 3,
  "simulations_run": 3,
  "simulations_success": 2,
  "timestamp": "2026-05-14T00:00:00Z",
  "schema_version": 1
}
```

### Apendice E: Glosario de Terminos OMEGA

| Termino | Definicion |
|---------|------------|
| **SED** | Sequential Equilibrium Dispatcher — motor principal de evaluacion |
| **CDC** | Coefficiente de Divergencia de Estado — metrica de ineficiencia transitoria |
| **Filtration** | Proceso de Markov con saltos que detecta ineficiencias de mercado |
| **Eigenstate** | Estado propio del Hamiltoniano efectivo del mercado |
| **Allocation** | Control optimo e impulso Dirac sobre variedad de liquidez |
| **Hedge** | Neutralizacion ortogonal de varianza entre mercados entrelazados |
| **Resolution** | Payload firmado que el Executor decodifica y ejecuta on-chain |
| **Topological Yield** | Beneficio neto calculado por el pipeline SED |
| **Convergence Signal** | Senal de telemetria emitida al final de cada ciclo SED |
| **Execution Signer** | Wallet semi-hot que firma ResolutionPayloads |
| **Cold Treasury** | Wallet fria que recibe yield post-ejecucion |
| **Selector Whitelist** | Lista blanca de selectores de funcion por router (A5) |
| **Allowance Manager** | Verificacion de aprobaciones de router (SC-5) |

---

## Document Control

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2026-05-14 | DevOps OMEGA | Initial release — sed-core <-> EVM integration plan |
| 1.0.1 | 2026-05-14 | DevOps OMEGA | Added Yul decoding, security model, troubleshooting |

---

*Este documento es propiedad del sistema ArbitrageX-V2 (OMEGA).
Distribucion restringida. Modificaciones solo via PR aprobado por
el equipo de DevOps OMEGA.*
