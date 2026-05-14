# OMEGA Multi-Chain Deployment Guide
## ArbitrageX-V2 — Protocolo OMEGA AGENT TEAMS
### Version: 2026.05-B1 | Classification: OPERATIONAL

---

> **WARNING — OMEGA TERMINOLOGY DOCTRINE**
> Este documento utiliza exclusivamente la nomenclatura OMEGA. Bajo ninguna
> circunstancia se emplearan terminos como "arbitrage", "bot", "snipe",
> "front-run", "sandwich" o cualquier otra denominacion que comprometa la
> integridad operacional del sistema. El lenguaje es parte de la seguridad.

---

## Tabla de Contenidos

1. [Arquitectura Multi-Chain Overview](#1-arquitectura-multi-chain-overview)
2. [Agregar una Nueva Mainnet / Chain](#2-agregar-una-nueva-mainnet--chain)
3. [Agregar un Nuevo DEX](#3-agregar-un-nuevo-dex)
4. [Agregar un Nuevo Pool](#4-agregar-un-nuevo-pool)
5. [Tabla de Referencia: Chains Soportadas](#5-tabla-de-referencia-chains-soportadas)
6. [Troubleshooting por Paso](#6-troubleshooting-por-paso)
7. [Seguridad y Wallets](#7-seguridad-y-wallets)
8. [Apendice: Comandos de Verificacion](#8-apendice-comandos-de-verificacion)

---

## 1. Arquitectura Multi-Chain Overview

### 1.1 Componentes del Sistema

El sistema OMEGA consta de los siguientes componentes interconectados que
participan en la operacion multi-chain:

| Componente | Lenguaje | Rol Multi-Chain |
|-----------|----------|-----------------|
| `sed-core` | Rust | Motor SED (Sequential Equilibrium Dispatcher): filtracion estocastica, eigenstates, allocaciones, hedging |
| `searcher-rs` | Rust | Ejecutable principal: suscriptor Redis, conector RPC, publicador de convergencia |
| `api-server` | TypeScript/Node.js | API REST: trading-config, dexes, pools, runtime-status |
| `pool_sync_worker` | TypeScript/Node.js | Scanner de reservas: escanea pools activos por chain |
| `recon_worker` | TypeScript/Node.js | Reconocimiento automatico de nuevos pools |
| PostgreSQL | SQL | Fuente de verdad: chains, dexes, pools, trading_config, sed_* |
| Redis | Key-Value | Hot-cache: trading_config, serializacion de senales, pub/sub |

### 1.2 Flujo de Datos Multi-Chain

```
Operator Dashboard (frontend)
       |
       v
/api/v1/admin/trading-config/:chain_id   (api-server)
       |
       +--> PostgreSQL (fuente de verdad)
       +--> Redis SET arbx:trading_config:<chain_id>
       +--> Redis PUBLISH arbx:trading_config:changes
       +--> Redis PUBLISH arbx:config:hot_reload   (dual-channel)
       |
       v
searcher-rs (suscriptor Redis)
       |
       +--> Spawnea task `run_chain` por cada chain habilitada
       +--> Conecta RPC HTTP/WSS via `chains_runtime` table
       |
       v
sed-core pipeline:
  filtration (CDC) --> eigenstate --> allocator --> hedger
       |
       v
Redis PUBLISH arbx:signals:convergence   (telemetry_publisher.rs)
       |
       v
api-server WebSocket --> Frontend Dashboard
```

### 1.3 Precedence Rule (CRITICAL)

La precedencia de configuracion sigue esta regla estricta:

```
1. PostgreSQL `chains_runtime`   GANA si existe registro explicito
2. TOML bootstrap `configs/app.toml [[chains]]`   Solo para seed inicial
3. Conflicto PG vs TOML: PG gana; api-server emite warning + metrica
4. Delete = soft-disable (enabled=false), NUNCA DELETE fisico
```

---

## 2. Agregar una Nueva Mainnet / Chain

**Ejemplo practico:** Agregar **Base** (chain_id = 8453).

---

### Paso 2.1: Añadir Entry en `configs/chains.json`

⚠️ **ADVERTENCIA:** Este archivo es SOLO para seed inicial. La fuente de verdad
es PostgreSQL. Si existe conflicto, PG gana.

```bash
# Editar el archivo de configuracion bootstrap
cd /mnt/agents/arbitragex-v2
vim configs/chains.json
```

Añadir al array `chains`:

```json
{
  "chain_id": 8453,
  "name": "Base",
  "rpc_http": "https://mainnet.base.org",
  "rpc_ws": "wss://mainnet.base.org",
  "native_token": "ETH",
  "block_time_ms": 2000,
  "confirmations_required": 12,
  "is_active": true
}
```

**Campos obligatorios y sus constraints:**

| Campo | Tipo | Constraint | Descripcion |
|-------|------|------------|-------------|
| `chain_id` | BIGINT | `> 0`, UNIQUE | Chain ID canonico (EIP-155) |
| `name` | TEXT | NOT NULL | Nombre humano-legible |
| `rpc_http` | TEXT | NOT NULL | Endpoint HTTP RPC |
| `rpc_ws` | TEXT | NULLABLE | Endpoint WebSocket RPC |
| `native_token` | TEXT | NOT NULL | Simbolo del token nativo |
| `block_time_ms` | INT | `> 0` | Tiempo medio entre bloques |
| `confirmations_required` | INT | `> 0` | Confirmaciones para finalidad |
| `is_active` | BOOLEAN | NOT NULL DEFAULT TRUE | Gate operacional |

---

### Paso 2.2: Variables de Entorno en `.env`

⚠️ **ADVERTENCIA DE SEGURIDAD:** Las URLs de RPC pueden contener API keys.
Nunca commitear `.env`. Usar `.env.example` para documentar campos esperados.

```bash
# Editar variables de entorno
cd /mnt/agents/arbitragex-v2/backend/searcher-rs
vim .env
```

Añadir las variables especificas de la nueva chain:

```bash
# === Base (chain_id=8453) ===
RPC_HTTP_8453=https://mainnet.base.org
RPC_WS_8453=wss://mainnet.base.org

# Si se usa un provider con API key (ej: Alchemy, Infura):
# RPC_HTTP_8453=https://base-mainnet.g.alchemy.com/v2/${ALCHEMY_API_KEY}
# RPC_WS_8453=wss://base-mainnet.g.alchemy.com/v2/${ALCHEMY_API_KEY}
```

**Formato estandar de variables RPC:**

```bash
# Patron: RPC_{PROTOCOL}_{CHAIN_ID}
RPC_HTTP_1=https://eth-mainnet.g.alchemy.com/v2/${ALCHEMY_KEY}      # Ethereum
RPC_HTTP_42161=https://arb-mainnet.g.alchemy.com/v2/${ALCHEMY_KEY}  # Arbitrum
RPC_HTTP_10=https://opt-mainnet.g.alchemy.com/v2/${ALCHEMY_KEY}     # Optimism
RPC_HTTP_8453=https://base-mainnet.g.alchemy.com/v2/${ALCHEMY_KEY}  # Base
RPC_HTTP_137=https://polygon-mainnet.g.alchemy.com/v2/${ALCHEMY_KEY} # Polygon
RPC_HTTP_56=https://bsc-dataseed.binance.org/                        # BSC
```

---

### Paso 2.3: Migracion de Base de Datos

#### 2.3a: Tabla `chains_runtime` (Migration 061 — B1)

Esta tabla es la **fuente de verdad operacional**. Todo pasa por aqui.

```sql
BEGIN;

INSERT INTO chains_runtime (
    chain_id,
    name,
    rpc_http_url,
    rpc_ws_url,
    native_currency,
    block_time_ms,
    enabled,
    config_hash,
    notes,
    created_by,
    updated_by
) VALUES (
    8453,                                    -- chain_id
    'Base',                                  -- name
    'https://mainnet.base.org',              -- rpc_http_url
    'wss://mainnet.base.org',                -- rpc_ws_url
    'ETH',                                   -- native_currency
    2000,                                    -- block_time_ms
    true,                                    -- enabled
    NULL,                                    -- config_hash (calculado por searcher-rs)
    'Chain agregada via OMEGA_MULTICHAIN_GUIDE paso 2.3',  -- notes
    'operator_admin',                        -- created_by
    'operator_admin'                         -- updated_by
)
ON CONFLICT (chain_id) DO UPDATE SET
    name = EXCLUDED.name,
    rpc_http_url = EXCLUDED.rpc_http_url,
    rpc_ws_url = EXCLUDED.rpc_ws_url,
    native_currency = EXCLUDED.native_currency,
    block_time_ms = EXCLUDED.block_time_ms,
    enabled = EXCLUDED.enabled,
    updated_at = NOW(),
    updated_by = EXCLUDED.updated_by;

COMMIT;
```

**Verificacion post-insert:**

```sql
SELECT chain_id, name, rpc_http_url, native_currency, block_time_ms, enabled
FROM chains_runtime
WHERE chain_id = 8453;
```

Resultado esperado:
```
 chain_id | name |      rpc_http_url          | native_currency | block_time_ms | enabled
----------+------+----------------------------+-----------------+---------------+---------
 8453     | Base | https://mainnet.base.org   | ETH             | 2000          | t
```

#### 2.3b: Tabla `chains` (Migration 021 — DeFi Registries)

```sql
BEGIN;

INSERT INTO chains (chain_id, name, native_currency, explorer_url, is_active)
VALUES (
    8453,
    'base',
    'ETH',
    'https://basescan.org',
    true
)
ON CONFLICT (chain_id) DO UPDATE SET
    name = EXCLUDED.name,
    native_currency = EXCLUDED.native_currency,
    explorer_url = EXCLUDED.explorer_url,
    is_active = EXCLUDED.is_active;

COMMIT;
```

#### 2.3c: Tabla `rpcs` (endpoints redundantes)

```sql
BEGIN;

-- Endpoint HTTP primario
INSERT INTO rpcs (chain_id, url, type, priority, is_active)
VALUES (8453, 'https://mainnet.base.org', 'HTTP', 1, true)
ON CONFLICT DO NOTHING;

-- Endpoint WSS primario
INSERT INTO rpcs (chain_id, url, type, priority, is_active)
VALUES (8453, 'wss://mainnet.base.org', 'WSS', 1, true)
ON CONFLICT DO NOTHING;

-- Endpoint HTTP de backup (ej: Alchemy)
INSERT INTO rpcs (chain_id, url, type, priority, is_active)
VALUES (8453, 'https://base-mainnet.g.alchemy.com/v2/YOUR_KEY', 'HTTP', 2, true)
ON CONFLICT DO NOTHING;

COMMIT;
```

---

### Paso 2.4: Trading Config por Defecto

El sistema permanece **IDLE** para cualquier chain que no tenga una fila en
`trading_config`. Este es el comportamiento por diseno: nunca se opera sin
configuracion explicita del operador.

#### 2.4a: Via API REST (recomendado)

```bash
# Definir variables de entorno para el CLI
export ARBX_API_URL="http://localhost:8080"
export ARBX_ADMIN_TOKEN="tu_admin_token_aqui"

# Verificar que el token es valido
curl -s -o /dev/null -w "%{http_code}" \
  "${ARBX_API_URL}/admin/trading-config" \
  -H "Authorization: Bearer ${ARBX_ADMIN_TOKEN}"
# Esperado: 200

# Upsert trading config para Base (chain_id=8453)
curl -X PUT "${ARBX_API_URL}/admin/trading-config/8453" \
  -H "Authorization: Bearer ${ARBX_ADMIN_TOKEN}" \
  -H "Content-Type: application/json" \
  -H "X-Arbx-Actor: operator_cli" \
  -d '{
    "capital_usd": 5000.00,
    "base_token_symbol": "WETH",
    "base_token_price_usd": 3200.00,
    "allowed_token_symbols": ["WETH", "USDC", "USDT", "DAI"],
    "token_prices_usd": {
      "WETH": 3200.00,
      "USDC": 1.00,
      "USDT": 1.00,
      "DAI": 1.00
    },
    "simulation_capital_usd": 1000.00,
    "simulation_per_token_amounts_usd": {
      "WETH": 500.00,
      "USDC": 500.00
    },
    "simulation_per_strategy_caps_usd": {},
    "simulation_target_profit_usd": 10.00,
    "simulation_target_roi_pct": 1.00,
    "min_profit_usd": 5.00,
    "min_roi_pct": 0.50,
    "min_landing_probability": 0.50,
    "min_liquidity_confidence": 0.70,
    "max_token_risk_score": 1.00,
    "gas_price_strategy": "dynamic_basefee_plus_tip",
    "gas_estimate_units": 250000,
    "max_slippage_pct": 0.50,
    "failure_risk_buffer_pct": 0.001,
    "flashloan_fee_pct": 0.0009,
    "enabled_strategies": ["dex_arb_v2v2", "triangular"],
    "strategy_configs": {},
    "capital_cost_rate_annual_pct": 0.0,
    "ops_overhead_usd_per_attempt": 0.01,
    "spread_sanity_mult": 3.0,
    "p_copied_volume_threshold_usd": 1000000.0,
    "p_copied_max": 0.5,
    "enabled": true
  }'
```

Respuesta esperada (200 OK):
```json
{
  "ok": true,
  "chain_id": 8453,
  "subscribers_trading_config": 1,
  "subscribers_hot_reload": 1,
  "channels": [
    "arbx:trading_config:changes",
    "arbx:config:hot_reload"
  ],
  "capital_usd": 5000,
  "base_token_symbol": "WETH",
  ...
}
```

#### 2.4b: Via SQL directo (solo en emergencia)

```sql
BEGIN;

INSERT INTO trading_config (
    chain_id, capital_usd, base_token_symbol, base_token_price_usd,
    allowed_token_symbols, token_prices_usd,
    simulation_capital_usd, simulation_per_token_amounts_usd,
    min_profit_usd, min_roi_pct,
    min_landing_probability, min_liquidity_confidence, max_token_risk_score,
    gas_price_strategy, gas_estimate_units, max_slippage_pct,
    failure_risk_buffer_pct, flashloan_fee_pct,
    enabled_strategies, enabled, updated_by,
    capital_cost_rate_annual_pct, ops_overhead_usd_per_attempt,
    spread_sanity_mult, p_copied_volume_threshold_usd, p_copied_max
) VALUES (
    8453,                                  -- chain_id
    5000.00,                               -- capital_usd
    'WETH',                                -- base_token_symbol
    3200.00,                               -- base_token_price_usd
    ARRAY['WETH', 'USDC', 'USDT', 'DAI'],  -- allowed_token_symbols
    '{"WETH": 3200.00, "USDC": 1.00, "USDT": 1.00, "DAI": 1.00}'::jsonb,
    1000.00,                               -- simulation_capital_usd
    '{"WETH": 500.00, "USDC": 500.00}'::jsonb,
    5.00,                                  -- min_profit_usd
    0.50,                                  -- min_roi_pct
    0.50,                                  -- min_landing_probability
    0.70,                                  -- min_liquidity_confidence
    1.00,                                  -- max_token_risk_score
    'dynamic_basefee_plus_tip',            -- gas_price_strategy
    250000,                                -- gas_estimate_units
    0.50,                                  -- max_slippage_pct
    0.001,                                 -- failure_risk_buffer_pct
    0.0009,                                -- flashloan_fee_pct
    ARRAY['dex_arb_v2v2', 'triangular'],  -- enabled_strategies
    true,                                  -- enabled
    'operator_emergency',                  -- updated_by
    0.0,                                   -- capital_cost_rate_annual_pct
    0.01,                                  -- ops_overhead_usd_per_attempt
    3.0,                                   -- spread_sanity_mult
    1000000.0,                             -- p_copied_volume_threshold_usd
    0.5                                    -- p_copied_max
)
ON CONFLICT (chain_id) DO UPDATE SET
    capital_usd = EXCLUDED.capital_usd,
    base_token_symbol = EXCLUDED.base_token_symbol,
    base_token_price_usd = EXCLUDED.base_token_price_usd,
    allowed_token_symbols = EXCLUDED.allowed_token_symbols,
    min_profit_usd = EXCLUDED.min_profit_usd,
    min_roi_pct = EXCLUDED.min_roi_pct,
    gas_price_strategy = EXCLUDED.gas_price_strategy,
    max_slippage_pct = EXCLUDED.max_slippage_pct,
    enabled_strategies = EXCLUDED.enabled_strategies,
    enabled = EXCLUDED.enabled,
    updated_by = EXCLUDED.updated_by,
    updated_at = NOW();

COMMIT;
```

---

### Paso 2.5: Seed DEXes y Factories para la Nueva Chain

Base ya tiene sus DEXes catalogados en `migration 043`. Verificar que existen:

```sql
SELECT d.name, f.address
FROM factories f
JOIN dexes d ON d.id = f.dex_id
WHERE f.chain_id = 8453
ORDER BY d.name;
```

Resultado esperado:
```
    name     |                  address
-----------+------------------------------------------
 Aerodrome  | 0x420dd381b31aef6683db6b902084cb0ffece40da
 BaseSwap   | 0xfda619b6d20975be80a10332cd39b9a4b0faa8bb
 Curve      | 0x4f8846ae9380b90d2e71d5e3d042dff3e7ebb40d
 PancakeSwap V3 | 0x0bfbcf9fa4f9c56b0f40a671ad40e0805a091865
 SushiSwap  | 0x71524b4f93c58fcbf659783284e38825f0622859
 UniswapV3  | 0x33128a8fc17869897dce68ed026d694621f6fdfd
```

Si algun DEX falta, ejecutar los INSERT del migration 043 manualmente.

---

### Paso 2.6: Seed Tokens para la Nueva Chain

```sql
BEGIN;

INSERT INTO tokens (chain_id, address, symbol, decimals, is_stablecoin, is_active)
VALUES
  (8453, '0x4200000000000000000000000000000000000006', 'WETH',  18, FALSE, true),
  (8453, '0x833589fcd6edb6e08f4c7c32d4f71b54bda02913', 'USDC',   6, TRUE,  true),
  (8453, '0x50c5725949a6f0c72e6c4a641f24049a917db0cb', 'DAI',   18, TRUE,  true),
  (8453, '0x68f180fcce6836688e9084f035309e29bf0a2095', 'WBTC',   8, FALSE, true)
ON CONFLICT (chain_id, address) DO NOTHING;

COMMIT;
```

**Verificacion:**
```sql
SELECT symbol, address, decimals, is_stablecoin
FROM tokens
WHERE chain_id = 8453
ORDER BY symbol;
```

---

### Paso 2.7: Reiniciar searcher-rs para Reconectar

```bash
# Opcion A: Reinicio graceful via systemd
sudo systemctl restart arbitragex-searcher-rs

# Opcion B: Reinicio via Docker Compose
cd /mnt/agents/arbitragex-v2
docker-compose restart searcher-rs

# Opcion C: Kill signal + restart (si no hay systemd/docker)
kill -TERM $(pgrep -f "searcher-rs")
# Esperar 30s (CancellationToken timeout)
cd /mnt/agents/arbitragex-v2/backend/searcher-rs
cargo run --release 2>&1 | tee /var/log/arbitragex/searcher-rs.log
```

**Logs de verificacion post-restart:**

```bash
# Verificar que la chain fue detectada
grep -i "chain.*8453\|base\|spawning.*run_chain" /var/log/arbitragex/searcher-rs.log | tail -20

# Verificar conexion RPC exitosa
grep -i "connected.*8453\|ws.*open.*8453\|rpc.*healthy" /var/log/arbitragex/searcher-rs.log | tail -10

# Verificar suscripcion a Redis
grep -i "subscribed\|hot_reload\|trading_config" /var/log/arbitragex/searcher-rs.log | tail -10
```

---

### Paso 2.8: Verificar Estado Operacional

#### 2.8a: Runtime Status via API

```bash
# Verificar runtime status para Base
curl -s "${ARBX_API_URL}/api/v1/strategies/runtime-status?chain_id=8453" \
  -H "Authorization: Bearer ${ARBX_ADMIN_TOKEN}" | jq .
```

Respuesta esperada:
```json
{
  "chain_id": 8453,
  "chain_name": "Base",
  "is_active": true,
  "rpc_http_connected": true,
  "rpc_ws_connected": true,
  "block_height": 12345678,
  "latest_block_time": "2026-05-14T12:00:00Z",
  "trading_configured": true,
  "enabled_strategies": ["dex_arb_v2v2", "triangular"],
  "active_dexes": 6,
  "active_pools": 0,
  "status": "healthy"
}
```

#### 2.8b: Verificacion de Trading Config en Redis

```bash
# Verificar que la config se reflejo en Redis
redis-cli GET "arbx:trading_config:8453" | jq .

# Verificar canales de pub/sub
redis-cli PUBSUB CHANNELS | grep arbx
```

#### 2.8c: Health Check End-to-End

```bash
# Health check general del api-server
curl -s "${ARBX_API_URL}/api/v1/health" | jq .

# Verificar chains activas
curl -s "${ARBX_API_URL}/api/v1/chains?is_active=true" \
  -H "Authorization: Bearer ${ARBX_ADMIN_TOKEN}" | jq '.items[] | {chain_id, name, enabled}'

# Verificar DEXes para Base
curl -s "${ARBX_API_URL}/api/v1/dexes?chain_id=8453" \
  -H "Authorization: Bearer ${ARBX_ADMIN_TOKEN}" | jq '.items[] | {name, protocol_type}'
```

---

## 3. Agregar un Nuevo DEX

**Ejemplo practico:** Agregar **Aerodrome** en Base (ya pre-seedeado en
migration 043, pero documentado aqui como procedimiento operativo).

### 3.1: Arquitectura de Adaptadores

El sistema OMEGA utiliza un patron de adaptadores para integrar DEXes:

```
Executor Contract
       |
       +--> UniswapV2Adapter.sol    (Constant Product AMM)
       +--> UniswapV3Adapter.sol    (Concentrated Liquidity)
       +--> CurveAdapter.sol        (StableSwap)
       +--> BalancerAdapter.sol     (Weighted Pools)
       +--> AerodromeAdapter.sol    (Solidly fork — VELO style)
       +--> TraderJoeAdapter.sol    (Liquidity Book)
       +--> [nuevo adaptador]
```

### Paso 3.1: Verificar/Registrar DEX en DB

```sql
BEGIN;

-- 1. Verificar que el DEX existe a nivel global
SELECT id, name, protocol_type FROM dexes WHERE name = 'Aerodrome';

-- 2. Si NO existe, insertarlo (usar protocol_type correcto)
INSERT INTO dexes (name, protocol_type, is_active)
VALUES ('Aerodrome', 'SOLIDLY', true)
ON CONFLICT (name) DO NOTHING
RETURNING id;

-- 3. Registrar factory para la chain especifica
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 8453, '0x420dd381b31aef6683db6b902084cb0ffece40da'
FROM dexes WHERE name = 'Aerodrome'
ON CONFLICT (chain_id, address) DO NOTHING;

-- 4. Registrar router (si aplica)
INSERT INTO routers (dex_id, chain_id, address, version, is_trusted)
SELECT id, 8453, '0xcF77a3Ba9A809CAe10A0Fc3B4E5C35C0E2e1a5e2', 'v2', true
FROM dexes WHERE name = 'Aerodrome'
ON CONFLICT (chain_id, address) DO NOTHING;

COMMIT;
```

**Verificacion:**
```sql
SELECT d.name, d.protocol_type, f.address as factory, r.address as router
FROM dexes d
LEFT JOIN factories f ON f.dex_id = d.id AND f.chain_id = 8453
LEFT JOIN routers r ON r.dex_id = d.id AND r.chain_id = 8453
WHERE d.name = 'Aerodrome';
```

### Paso 3.2: Crear Adaptador Solidity

⚠️ **ADVERTENCIA:** Todo nuevo adaptador debe heredar de `IDEXAdapter` y
pasar por auditoria interna antes del deployment.

#### 3.2a: Plantilla de Adaptador

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import {IDEXAdapter} from "../interfaces/IDEXAdapter.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";

/// @title AerodromeAdapter
/// @notice Adaptador para Aerodrome (Solidly fork) en Base
/// @dev Referencia: https://aerodrome.finance
contract AerodromeAdapter is IDEXAdapter {

    string public constant override name = "Aerodrome";
    string public constant override version = "v2";

    address public immutable router;
    address public immutable factory;

    constructor(address _router, address _factory) {
        require(_router != address(0), "AerodromeAdapter: router zero");
        require(_factory != address(0), "AerodromeAdapter: factory zero");
        router = _router;
        factory = _factory;
    }

    /// @notice Ejecutar swap en Aerodrome
    /// @param tokenIn  Token de entrada
    /// @param tokenOut Token de salida
    /// @param amountIn  Cantidad de entrada
    /// @param minAmountOut Minimo de salida (slippage protection)
    /// @param to       Destinatario del output
    function swap(
        address tokenIn,
        address tokenOut,
        uint256 amountIn,
        uint256 minAmountOut,
        address to
    ) external override returns (uint256 amountOut) {
        // Aprobar router
        IERC20(tokenIn).approve(router, amountIn);

        // Construir path (Aerodrome usa routes optimizadas)
        IAerodromeRouter.Route[] memory routes = new IAerodromeRouter.Route[](1);
        routes[0] = IAerodromeRouter.Route({
            from: tokenIn,
            to: tokenOut,
            stable: false,  // volatile pool; true para stable pools
            factory: factory
        });

        uint256[] memory amounts = IAerodromeRouter(router).swapExactTokensForTokens(
            amountIn,
            minAmountOut,
            routes,
            to,
            block.timestamp + 300  // 5 min deadline
        );

        amountOut = amounts[amounts.length - 1];
        emit SwapExecuted(tokenIn, tokenOut, amountIn, amountOut);
    }

    /// @notice Obtener quote del router
    function getQuote(
        address tokenIn,
        address tokenOut,
        uint256 amountIn
    ) external view override returns (uint256 amountOut) {
        IAerodromeRouter.Route[] memory routes = new IAerodromeRouter.Route[](1);
        routes[0] = IAerodromeRouter.Route({
            from: tokenIn,
            to: tokenOut,
            stable: false,
            factory: factory
        });
        uint256[] memory amounts = IAerodromeRouter(router).getAmountsOut(amountIn, routes);
        amountOut = amounts[amounts.length - 1];
    }

    event SwapExecuted(
        address indexed tokenIn,
        address indexed tokenOut,
        uint256 amountIn,
        uint256 amountOut
    );
}

/// @notice Interface minima de Aerodrome Router
interface IAerodromeRouter {
    struct Route {
        address from;
        address to;
        bool stable;
        address factory;
    }
    function swapExactTokensForTokens(
        uint256 amountIn,
        uint256 amountOutMin,
        Route[] calldata routes,
        address to,
        uint256 deadline
    ) external returns (uint256[] memory amounts);

    function getAmountsOut(
        uint256 amountIn,
        Route[] memory routes
    ) external view returns (uint256[] memory amounts);
}
```

#### 3.2b: Compilacion y Deployment

```bash
cd /mnt/agents/arbitragex-v2/contracts

# Compilar adaptador
forge build --match-contract AerodromeAdapter

# Verificar tamano del bytecode (debe ser < 24KB)
forge inspect AerodromeAdapter bytecode | wc -c

# Deployment a Base mainnet (usando CREATE2 para direccion determinista)
forge script script/DeployAdapter.s.sol:AerodromeAdapterDeploy \
  --rpc-url $RPC_HTTP_8453 \
  --private-key $DEPLOYER_KEY \
  --verify \
  --broadcast \
  --chain-id 8453
```

### Paso 3.3: Registrar Adaptador en Executor

```solidity
// En el constructor o funcion de setup del Executor:
function _registerAdapters() internal {
    // Adaptadores existentes...
    adapters["UniswapV2"] = address(0x...);
    adapters["UniswapV3"] = address(0x...);
    adapters["Curve"] = address(0x...);

    // Nuevo adaptador Aerodrome (Base only)
    if (block.chainid == 8453) {
        adapters["Aerodrome"] = address(aerodromeAdapter);
    }
}
```

### Paso 3.4: Tests de Integracion

```bash
cd /mnt/agents/arbitragex-v2/contracts

# Test unitario del adaptador
forge test --match-contract AerodromeAdapterTest -vvv

# Test de integracion contra fork de Base mainnet
forge test --match-contract AerodromeIntegrationTest \
  --fork-url $RPC_HTTP_8453 \
  --fork-block-number 12345678 \
  -vvv

# Test de gas (reporte detallado)
forge test --match-contract AerodromeGasBenchmark --gas-report
```

**Criterios de aceptacion:**
- [ ] Swap exitoso con >= 99.5% de eficiencia
- [ ] Slippage protection funciona correctamente
- [ ] Gas usage dentro de presupuesto (target: < 180k units)
- [ ] Revert handling atomic: si falla un swap, todo revierte
- [ ] Compatible con flashloan callback del Executor

---

## 4. Agregar un Nuevo Pool

**Ejemplo practico:** Agregar pool **WETH/USDC** en **Aerodrome** (Base).

### Paso 4.1: Obtener Datos del Pool

Antes de registrar un pool, verificar on-chain:

```bash
# Variables
AERODROME_FACTORY="0x420dd381b31aef6683db6b902084cb0ffece40da"
WETH="0x4200000000000000000000000000000000000006"
USDC="0x833589fcd6edb6e08f4c7c32d4f71b54bda02913"

# Obtener direccion del pool (volatile = false para stable, true para volatile)
cast call $AERODROME_FACTORY \
  "getPool(address,address,bool)(address)" \
  $WETH $USDC false \
  --rpc-url $RPC_HTTP_8453

# Obtener reservas
cast call <POOL_ADDRESS> "getReserves()(uint256,uint256,uint256)" \
  --rpc-url $RPC_HTTP_8453
```

### Paso 4.2: Registrar Pool en DB

```sql
BEGIN;

-- 1. Obtener IDs necesarios
WITH dex_lookup AS (
    SELECT f.id as factory_id, d.id as dex_id
    FROM factories f
    JOIN dexes d ON d.id = f.dex_id
    WHERE d.name = 'Aerodrome' AND f.chain_id = 8453
),
token0_lookup AS (
    SELECT id as token0_id FROM tokens
    WHERE chain_id = 8453 AND address = '0x4200000000000000000000000000000000000006'
),
token1_lookup AS (
    SELECT id as token1_id FROM tokens
    WHERE chain_id = 8453 AND address = '0x833589fcd6edb6e08f4c7c32d4f71b54bda02913'
)

-- 2. Insertar pool
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier, is_active)
SELECT
    8453,
    (SELECT factory_id FROM dex_lookup),
    '0x...pool_address_onchain...',
    (SELECT token0_id FROM token0_lookup),
    (SELECT token1_id FROM token1_lookup),
    NULL,  -- fee_tier NULL para Solidly forks (fee dinamico)
    true
FROM dex_lookup, token0_lookup, token1_lookup
ON CONFLICT (chain_id, address) DO NOTHING;

COMMIT;
```

**Verificacion:**
```sql
SELECT
    p.address as pool_address,
    t0.symbol as token0,
    t1.symbol as token1,
    d.name as dex,
    p.fee_tier,
    p.is_active
FROM pools p
JOIN factories f ON f.id = p.factory_id
JOIN dexes d ON d.id = f.dex_id
JOIN tokens t0 ON t0.id = p.token0_id
JOIN tokens t1 ON t1.id = p.token1_id
WHERE p.chain_id = 8453 AND d.name = 'Aerodrome';
```

### Paso 4.3: Verificar Reservas via API

```bash
# Verificar que el pool aparece en el registro
curl -s "${ARBX_API_URL}/api/v1/dex/registry/pools?chain_id=8453" \
  -H "Authorization: Bearer ${ARBX_ADMIN_TOKEN}" | jq '.items[] | select(.dex == "Aerodrome")'

# Forzar sync de reservas
curl -X POST "${ARBX_API_URL}/api/v1/admin/pools/sync-reserves" \
  -H "Authorization: Bearer ${ARBX_ADMIN_TOKEN}" \
  -H "Content-Type: application/json" \
  -d '{
    "chain_id": 8453,
    "pool_addresses": ["0x...pool_address_onchain..."]
  }'
```

### Paso 4.4: Recon Worker Escaneara Automaticamente

El `recon_worker` escanea periodicamente las factories de DEXes activos en
busca de nuevos pools. Verificar que esta procesando la nueva chain:

```bash
# Verificar logs del recon worker
grep -i "recon.*8453\|aerodrome.*pool.*found" /var/log/arbitragex/recon-worker.log | tail -20

# Verificar metricas de recon
curl -s "${ARBX_API_URL}/api/v1/metrics/recon?chain_id=8453" | jq .
```

---

## 5. Tabla de Referencia: Chains Soportadas

| Chain | Chain ID | Block Time | Confirmations | Native Token | Status | DEXes Pre-seed |
|-------|----------|------------|---------------|--------------|--------|----------------|
| Ethereum | 1 | 12s | 12 | ETH | Operativa | 8 |
| Arbitrum | 42161 | 250ms | 10 | ETH | Operativa | 6 |
| Optimism | 10 | 2s | 10 | ETH | Operativa | 5 |
| Base | 8453 | 2s | 10 | ETH | Documentada en esta guia | 6 |
| Polygon | 137 | 2s | 20 | MATIC | Documentada en esta guia | 6 |
| BSC | 56 | 3s | 15 | BNB | Documentada en esta guia | 6 |

### 5.1 RPCs Recomendados por Chain

| Chain | Provider | HTTP URL | WSS URL |
|-------|----------|----------|---------|
| Ethereum | Alchemy | `https://eth-mainnet.g.alchemy.com/v2/${KEY}` | `wss://eth-mainnet.g.alchemy.com/v2/${KEY}` |
| Arbitrum | Alchemy | `https://arb-mainnet.g.alchemy.com/v2/${KEY}` | `wss://arb-mainnet.g.alchemy.com/v2/${KEY}` |
| Optimism | Alchemy | `https://opt-mainnet.g.alchemy.com/v2/${KEY}` | `wss://opt-mainnet.g.alchemy.com/v2/${KEY}` |
| Base | Alchemy | `https://base-mainnet.g.alchemy.com/v2/${KEY}` | `wss://base-mainnet.g.alchemy.com/v2/${KEY}` |
| Polygon | Alchemy | `https://polygon-mainnet.g.alchemy.com/v2/${KEY}` | `wss://polygon-mainnet.g.alchemy.com/v2/${KEY}` |
| BSC | Public | `https://bsc-dataseed.binance.org/` | `wss://bsc-ws-node.nariox.org:443` |

### 5.2 Configuraciones de Trading por Defecto

| Parametro | Valor Default | Rango Permitido | Notas |
|-----------|--------------|-----------------|-------|
| `capital_usd` | 5000.00 | >= 0 | Capital desplegable diario |
| `min_profit_usd` | 5.00 | >= 0 | Umbral minimo de beneficio neto |
| `min_roi_pct` | 0.50 | >= 0 | ROI minimo en porcentaje |
| `max_slippage_pct` | 0.50 | 0 - 50 | Slippage maximo permitido |
| `gas_estimate_units` | 250000 | > 0 | Estimacion de gas por tx |
| `failure_risk_buffer_pct` | 0.001 | >= 0 | Buffer de riesgo de fallo |
| `flashloan_fee_pct` | 0.0009 | >= 0 | Comision de flashloan (Aave: 0.09%) |

---

## 6. Troubleshooting por Paso

### Error en Paso 2.1: chains.json parse error

**Sintoma:** searcher-rs falla al iniciar con error de parseo JSON.

```bash
# Diagnostico: validar JSON
jq . configs/chains.json
# Si jq retorna error, el JSON esta mal formado

# Fix comun: coma faltante entre objetos del array
# Antes (mal):
#   "is_active": true
# }
# {
#   "chain_id": 42161,

# Despues (correcto):
#   "is_active": true
# },
# {
#   "chain_id": 42161,
```

### Error en Paso 2.2: RPC connection refused

**Sintoma:** searcher-rs logs muestran `Connection refused` o `timeout`.

```bash
# Diagnostico: probar conectividad RPC
curl -X POST $RPC_HTTP_8453 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'

# Si falla, verificar:
# 1. API key valida (Alchemy/Infura)
# 2. Red correcta (mainnet vs testnet)
# 3. Firewall/iptables no bloquea el puerto 443
# 4. Rate limits del provider

# Fix: usar endpoint alternativo
export RPC_HTTP_8453="https://base-mainnet.g.alchemy.com/v2/${ALCHEMY_BACKUP_KEY}"
```

### Error en Paso 2.3: chain_id ya existe en chains_runtime

**Sintoma:** INSERT falla con `duplicate key value violates unique constraint`.

```sql
-- Diagnostico: verificar estado actual
SELECT chain_id, name, enabled, updated_at
FROM chains_runtime
WHERE chain_id = 8453;

-- Fix: usar ON CONFLICT DO UPDATE (ya incluido en el script del paso 2.3)
-- Si necesitas cambiar de soft-disable a enable:
UPDATE chains_runtime
SET enabled = true,
    rpc_http_url = 'https://mainnet.base.org',
    updated_at = NOW(),
    updated_by = 'operator_recovery'
WHERE chain_id = 8453;
```

### Error en Paso 2.4: trading_config validation error

**Sintoma:** API retorna 400 con `invalid_request`.

```bash
# Diagnostico: verificar schema Zod
# Campos comunes que causan error:

# 1. gas_price_strategy = "fixed" pero fixed_gas_price_gwei es null
# Fix: incluir fixed_gas_price_gwei o cambiar a "dynamic_basefee_plus_tip"

# 2. min_landing_probability > 1.0
# Fix: rango valido es [0.0, 1.0]

# 3. max_slippage_pct > 50
# Fix: maximo permitido es 50.00

# Ver response detallada:
curl -X PUT "${ARBX_API_URL}/admin/trading-config/8453" ... 2>&1 | jq '.details'
```

### Error en Paso 2.5: factory no encontrado

**Sintoma:** Query de factories retorna 0 rows.

```sql
-- Diagnostico: verificar que el DEX existe
SELECT id, name FROM dexes WHERE name = 'Aerodrome';

-- Si no existe, insertar (ver Paso 3.1)
-- Si existe pero no tiene factory para Base:
SELECT d.id, d.name, f.chain_id, f.address
FROM dexes d
LEFT JOIN factories f ON f.dex_id = d.id
WHERE d.name = 'Aerodrome';

-- Fix: ejecutar los INSERT del migration 043
```

### Error en Paso 2.7: searcher-rs no detecta nueva chain

**Sintoma:** Logs no muestran `spawning run_chain for 8453`.

```bash
# Diagnostico paso a paso:

# 1. Verificar que chains_runtime tiene enabled=true
psql -c "SELECT chain_id, enabled FROM chains_runtime WHERE chain_id = 8453;"

# 2. Verificar que trading_config existe
psql -c "SELECT chain_id, enabled FROM trading_config WHERE chain_id = 8453;"

# 3. Verificar conexion Redis
redis-cli PING

# 4. Verificar que searcher-rs lee de la DB correcta
grep "DATABASE_URL\|chains_runtime" /var/log/arbitragex/searcher-rs.log | tail -10

# Fix: si la chain esta en chains_runtime pero no tiene trading_config,
# el sistema se mantiene idle por diseno. Ir al paso 2.4.
```

### Error en Paso 3.2: adaptador no compila

**Sintoma:** `forge build` retorna error de compilacion.

```bash
# Diagnostico: compilar con verbose
forge build --match-contract AerodromeAdapter 2>&1 | head -50

# Errores comunes:
# 1. Interface no encontrada -> verificar import path
#    Fix: import {IAerodromeRouter} from "../interfaces/aerodrome/IAerodromeRouter.sol";
#
# 2. Version de pragma incompatible
#    Fix: usar pragma solidity ^0.8.19;
#
# 3. Function override mismatch
#    Fix: verificar que las firmas coinciden con IDEXAdapter
```

### Error en Paso 4.2: pool ya existe

**Sintoma:** ON CONFLICT DO NOTHING no inserta nada.

```sql
-- Diagnostico: verificar pool existente
SELECT p.address, p.is_active, t0.symbol, t1.symbol
FROM pools p
JOIN tokens t0 ON t0.id = p.token0_id
JOIN tokens t1 ON t1.id = p.token1_id
WHERE p.chain_id = 8453 AND p.address = '0x...pool...';

-- Fix: si existe pero is_active=false, reactivar:
UPDATE pools SET is_active = true WHERE address = '0x...pool...';
```

---

## 7. Seguridad y Wallets

### 7.1 Modelo de Wallets Segregadas

El sistema OMEGA utiliza un modelo de **tres wallets segregadas**:

| Rol | Nombre | Funcion | Permisos |
|-----|--------|---------|----------|
| 1 | **Gas Sponsor** | Financia gas de transacciones | Solo envio de ETH para gas |
| 2 | **Execution Signer** | Firma payloads de resolucion | Firma ECDSA de resolutions |
| 3 | **Cold Treasury** | Recibe yield post-ejecucion | Solo recepcion, nunca firma |

### 7.2 Configuracion de Wallets

```bash
# .env - NUNCA COMMITEAR
# ============================================

# Gas Sponsor (hot wallet con balance minimo)
GAS_SPONSOR_PRIVATE_KEY=0x...          # 🔥 HOT — solo para gas
GAS_SPONSOR_ADDRESS=0x...

# Execution Signer (semi-hot, firma solo)
EXECUTION_SIGNER_PRIVATE_KEY=0x...     # 🔐 SEMI-HOT — firma solamente
EXECUTION_SIGNER_ADDRESS=0x...

# Cold Treasury (cold, recepcion unicamente)
COLD_TREASURY_ADDRESS=0x...            # 🧊 COLD — solo recepcion

# Deployer (solo para deployment de contratos)
DEPLOYER_PRIVATE_KEY=0x...             # 🔧 Usar solo en forge script
```

### 7.3 Verificacion de Permisos

```solidity
// En el Executor, verificar que:
// 1. Execution Signer esta autorizado
modifier onlyAuthorizedSigner() {
    require(msg.sender == authorizedSigner, "Executor: signer no autorizado");
    _;
}

// 2. Cold Treasury nunca puede firmar
//    (implementado off-chain: la clave de Cold Treasury NO EXISTE en sistema)

// 3. Gas Sponsor nunca puede llamar funciones del Executor
//    (implementado off-chain: Gas Sponsor no esta en allowlist del Executor)
```

### 7.4 Rotacion de Claves

Procedimiento para rotar el Execution Signer:

```bash
# 1. Generar nuevo par de claves
cast wallet new --json

# 2. Actualizar authorizedSigner en Executor (via multisig/governance)
# SoloOwner: llamar a Executor.updateAuthorizedSigner(newAddress)

# 3. Verificar que el viejo signer ya no puede ejecutar
cast call $EXECUTOR "authorizedSigner()(address)"

# 4. Actualizar .env (nunca commitear)
# 5. Reiniciar searcher-rs
```

---

## 8. Apendice: Comandos de Verificacion

### 8.1 Script de Health Check Completo

```bash
#!/bin/bash
# health_check_multichain.sh — Verificacion operacional multi-chain
# Uso: ./health_check_multichain.sh

set -euo pipefail

API_URL="${ARBX_API_URL:-http://localhost:8080}"
ADMIN_TOKEN="${ARBX_ADMIN_TOKEN:-}"
CHAINS=(1 10 56 137 8453 42161)
CHAIN_NAMES=(Ethereum Optimism BSC Polygon Base Arbitrum)

echo "=== OMEGA Multi-Chain Health Check ==="
echo "Timestamp: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo ""

# 1. API Server health
echo "[1/5] API Server Health..."
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" "${API_URL}/api/v1/health")
if [ "$HTTP_CODE" = "200" ]; then
    echo "  ✅ API Server: OK"
else
    echo "  ❌ API Server: HTTP $HTTP_CODE"
    exit 1
fi

# 2. Database connectivity
echo "[2/5] Database Connectivity..."
psql -c "SELECT COUNT(*) FROM chains_runtime WHERE enabled = true;" > /dev/null 2>&1
if [ $? -eq 0 ]; then
    echo "  ✅ PostgreSQL: Connected"
else
    echo "  ❌ PostgreSQL: Connection failed"
fi

# 3. Redis connectivity
echo "[3/5] Redis Connectivity..."
redis-cli PING > /dev/null 2>&1
if [ $? -eq 0 ]; then
    echo "  ✅ Redis: Connected"
else
    echo "  ❌ Redis: Connection failed"
fi

# 4. Chain status
echo "[4/5] Chain Status..."
for i in "${!CHAINS[@]}"; do
    CHAIN_ID=${CHAINS[$i]}
    CHAIN_NAME=${CHAIN_NAMES[$i]}
    CONFIGURED=$(curl -s "${API_URL}/api/v1/trading-config?chain_id=${CHAIN_ID}" | jq -r '.configured // false')
    if [ "$CONFIGURED" = "true" ]; then
        echo "  ✅ ${CHAIN_NAME} (chain_id=${CHAIN_ID}): Configured"
    else
        echo "  ⚠️  ${CHAIN_NAME} (chain_id=${CHAIN_ID}): Not configured (IDLE)"
    fi
done

# 5. DEX registry
echo "[5/5] DEX Registry..."
for i in "${!CHAINS[@]}"; do
    CHAIN_ID=${CHAINS[$i]}
    CHAIN_NAME=${CHAIN_NAMES[$i]}
    DEX_COUNT=$(curl -s "${API_URL}/api/v1/dexes?chain_id=${CHAIN_ID}" | jq '.items | length')
    echo "  📊 ${CHAIN_NAME} (chain_id=${CHAIN_ID}): ${DEX_COUNT} DEXes"
done

echo ""
echo "=== Health Check Complete ==="
```

### 8.2 Metricas Clave por Chain

| Metrica | Fuente | Query/Endpoint | Threshold Alerta |
|---------|--------|----------------|-------------------|
| Block lag | searcher-rs logs | `grep "block.*lag.*8453"` | > 5 blocks |
| RPC latency | rpcs table | `SELECT latency_ms FROM rpcs` | > 500ms |
| Pool count | pools table | `SELECT COUNT(*) FROM pools WHERE chain_id=X` | < 10 |
| CDC opportunities | sed_filtrations | `SELECT COUNT(*) FROM sed_filtrations WHERE cdc_is_inefficiency` | N/A |
| Pipeline latency | ConvergenceSignal | `pipeline_latency_ms` field | > 500ms |

### 8.3 Glosario OMEGA

| Termino | Definicion |
|---------|------------|
| **SED** | Sequential Equilibrium Dispatcher — motor principal de evaluacion |
| **CDC** | Coefficiente de Divergencia de Estado — metrica de ineficiencia transitoria |
| **Filtration** | Proceso de Markov con saltos que detecta ineficiencias |
| **Eigenstate** | Estado propio del Hamiltoniano efectivo del mercado |
| **Allocation** | Control optimo e impulso Dirac sobre variedad de liquidez |
| **Hedge** | Neutralizacion ortogonal de varianza entre mercados |
| **Resolution** | Payload firmado que el Executor decodifica y ejecuta |
| **Convergence Signal** | Senal de telemetria emitida al final de cada ciclo SED |
| **Topological Yield** | Beneficio neto calculado por el pipeline SED |

---

## Document Control

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2026-05-14 | DevOps OMEGA | Initial release — Base chain procedure |
| 1.0.1 | 2026-05-14 | DevOps OMEGA | Added Polygon, BSC, Optimism, Arbitrum references |

---

*Este documento es propiedad del sistema ArbitrageX-V2 (OMEGA).
Distribucion restringida. Modificaciones solo via PR aprobado por
el equipo de DevOps OMEGA.*
