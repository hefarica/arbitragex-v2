# ArbitrageX V2 — Cartridge API Reference (FASE OMEGA)

## Universal Cartridge Contract

Todo script `.rhai` DEBE cumplir el contrato universal para ser admitido en el runtime.

### Required Functions

| Signature | Returns | Purpose |
|-----------|---------|---------|
| `fn init_strategy()` | Map (metadata) | Declarar identidad y capacidades del cartucho |
| `fn evaluate_opportunity(pool_data)` | Map (eval result) | Evaluar si existe oportunidad rentable |
| `fn build_payload(opportunity)` | Map (tx payload) | Construir payload de ejecucion Web3 |

### Optional Lifecycle Hooks

| Signature | Purpose |
|-----------|---------|
| `fn on_activate()` | Llamado cuando el cartucho se activa |
| `fn on_deactivate()` | Llamado cuando el cartucho se pausa |
| `fn on_new_block(block_number)` | Llamado en cada nuevo bloque |

### Required Metadata Keys (init_strategy)

El Map retornado por `init_strategy()` DEBE contener:

| Key | Type | Descripcion |
|-----|------|-------------|
| `name` | String | Nombre humano del cartucho |
| `version` | String | Semver (e.g., "2.0.0") |
| `author` | String | Autor/equipo |
| `description` | String | Descripcion funcional |

Campos operacionales recomendados: `category`, `target_chains` ([] = todas), `min_eval_interval_ms`, `supported_protocols`, `risk_profile`, `capital_requirement`.

### Contract Validation (Rust Side)

```rust
pub const REQUIRED_FUNCTIONS: &[&str] = &[
    "init_strategy",
    "evaluate_opportunity",
    "build_payload",
];

pub const REQUIRED_METADATA_KEYS: &[&str] = &[
    "name", "version", "author", "description",
];
```

Validation errors: `MissingFunction`, `WrongArity`, `MissingMetadataKey`, `ForbiddenOperation`.

## Sandboxing Limits

```rust
MAX_OPERATIONS: 1_000_000        // Previene loops infinitos
MAX_CALL_STACK_DEPTH: 64         // Previene recursion infinita
MAX_STRING_SIZE: 65_536          // 64 KB
MAX_ARRAY_SIZE: 4_096
MAX_MAP_SIZE: 1_024
MAX_MODULES: 0                   // Sin imports externos
// eval deshabilitado
```

Un error en un cartucho NUNCA crashea el nodo. Se marca como Failed y se excluye.

## Host Bindings (Funciones Nativas)

### Chain and Block Context

| Funcion | Retorna | Fuente |
|---------|---------|--------|
| `get_chain_id()` | Integer | Config inyectado |
| `get_block_number()` | Integer | Redis/chain |
| `get_timestamp()` | Integer (unix) | System clock |
| `get_base_fee()` | Float (gwei) | Redis/mempool |
| `get_block_time_estimate(chain_id)` | Integer (seconds) | Config table |

### Pool and Token Data

| Funcion | Retorna | Fuente |
|---------|---------|--------|
| `get_reserves(pool_addr)` | Map `#{r0, r1, block, ts}` o `()` | Redis cache |
| `get_token_meta(token_addr)` | Map `#{symbol, decimals, name}` o `()` | Redis cache |
| `get_pool_index(symbol_a, symbol_b)` | Array de pool addresses | Redis cache |

### Math and Conversion

| Funcion | Retorna | Descripcion |
|---------|---------|-------------|
| `calculate_price_v2(r0, r1, dec_in, dec_out)` | Float | Precio V2 constant-product |
| `calculate_amount_out_v2(amount_in, r0, r1)` | String (wei) | Output con fee 0.3% |
| `calculate_amount_out_v3(amount, sqrt_price, liquidity, fee)` | String (wei) | Output V3 |
| `estimate_gas_cost(protocol_type, chain_id)` | Float | Gas estimado en gwei |
| `estimate_gas_limit(protocol_type, route_type)` | Integer | Gas limit conservador |
| `calculate_priority_fee(urgency, net_profit, base_fee)` | Float | Priority fee optimo |
| `to_wei(amount, decimals)` | String | Conversion a wei |
| `from_wei(amount_str, decimals)` | Float | Conversion desde wei |
| `math_sqrt(x)` | Float | Raiz cuadrada |
| `math_abs(x)` | Float | Valor absoluto |
| `math_min(a, b)` | Float | Minimo |
| `math_max(a, b)` | Float | Maximo |
| `math_pow(base, exp)` | Float | Potencia |
| `math_log(x)` | Float | Logaritmo natural |

### Simulation

| Funcion | Retorna | Latencia |
|---------|---------|----------|
| `simulate_swap(amount, path)` | Map `#{success, amount_out, gas_used}` o `()` | 5-50ms |
| `simulate_multicall(calls)` | Array de results | 10-100ms |

### Telemetry and Signals

| Funcion | Retorna | Descripcion |
|---------|---------|-------------|
| `log_quantum(level, message)` | () | Log a Redis telemetry (fire and forget) |
| `emit_signal(signal_type, data)` | () | Emitir senal a Redis PubSub |
| `encode_arb_calldata(opportunity)` | String (hex) | Encode calldata para executor |

Niveles de log: `"debug"`, `"info"`, `"warn"`, `"error"`

Signal types: `"opportunity_detected"`, `"risk_alert"`, `"state_change"`

## pool_data Shape (evaluate_opportunity input)

### DEX Arbitrage

```rhai
pool_data = #{
    chain_id: 1,
    source_pool: "0x...",
    token_in: "0x...",
    token_out: "0x...",
    amount_in: "1000000000",
    reserves_source: #{ r0: "...", r1: "...", block: N, ts: N },
    protocol_type: "v2",
    gas_price_gwei: 30.0,
    block_number: 12345678
}
```

### Triangular Arbitrage

```rhai
pool_data = #{
    chain_id: 1,
    token_a: "0x...",
    token_b: "0x...",
    token_c: "0x...",
    pools_ab: [...],
    pools_bc: [...],
    pools_ca: [...],
    amount_in: "...",
    gas_price_gwei: N
}
```

### Liquidation

```rhai
pool_data = #{
    chain_id: 1,
    protocol: "aave_v3",
    borrower: "0x...",
    health_factor: 0.95,
    debt_token: "0x...",
    debt_amount: "...",
    collateral_token: "0x...",
    collateral_amount: "...",
    liquidation_bonus_bps: 500,
    close_factor_bps: 5000
}
```

## evaluate_opportunity Return Shape

```rhai
#{
    is_opportunity: true,
    estimated_profit: 0.05,
    confidence: 0.87,
    urgency: "immediate",
    source_pool: "0x...",
    target_pool: "0x...",
    token_in: "0x...",
    token_out: "0x...",
    amount_in: "...",
    gas_cost_estimate: 0.01,
    net_profit: 0.04,
    protocol_type: "v2",
    chain_id: 1,
    block_number: 12345678,
    route_type: "two_leg_arb",
    reason: "below_gas_threshold"
}
```

## build_payload Return Shape

```rhai
#{
    target_contract: "EXECUTOR",
    calldata: "0x...",
    value_wei: "0",
    gas_limit: 300000,
    max_priority_fee_gwei: 2.5,
    deadline_ts: 1717200000,
    route: #{
        source_pool: "0x...",
        target_pool: "0x...",
        token_in: "0x...",
        token_out: "0x...",
        amount_in: "...",
        direction: "buy_source_sell_target",
        protocol: "v2"
    },
    risk: #{
        max_slippage_bps: 50,
        min_profit_after_gas: 0.032,
        revert_protection: true,
        sandwich_guard: true
    },
    flash_loan: #{
        protocol: "auto",
        token: "0x...",
        amount: "..."
    },
    meta: #{
        cartridge: "dex_arb_universal",
        version: "2.0.0",
        chain_id: 1,
        generated_at: 1717200000,
        block_number: 12345678
    }
}
```

## Hot-Reload Pipeline

### Redis PubSub Contract

Channel: `arbx:cartridge:injection`

Payload (JSON):
```json
{
  "cartridge_id": "uuid-v4",
  "event_type": "inject",
  "content_hash": "sha256-hex",
  "chain_id": 1,
  "timestamp": "2026-05-31T12:00:00Z",
  "actor": "operator@arbitragex.io"
}
```

Event types: `inject`, `update`, `remove`, `pause`, `resume`

### Deduplication

El subscriber trackea `content_hash` por cartridge_id. Si llega un reload con el mismo hash, se ignora (sin recompilacion). Previene thundering-herd en clusters multi-nodo.

### Inject via API

```bash
curl -X POST http://localhost:8080/admin/cartridges/inject \
  -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
  -H "content-type: application/json" \
  -d '{"slug":"mi_estrategia","source":"...","target_chains":[],"category":"custom"}'
```

### Cartridge States

| State | Significado |
|-------|-------------|
| `active` | En evaluacion normal |
| `paused` | Detenido temporalmente (operador) |
| `failed` | Error de compilacion o runtime |
| `archived` | Retirado permanentemente |

## Rhai Language Quick Reference

### Data Types

| Tipo | Ejemplo |
|------|---------|
| Integer | `42`, `-1`, `0xFF` |
| Float | `3.14`, `1.0e-5` |
| String | `"hello"` |
| Boolean | `true`, `false` |
| Array | `[1, 2, 3]` |
| Map | `#{ key: "value", num: 42 }` |
| Unit (null) | `()` |

### Control Flow

```rhai
if x > 0 { "positive" } else { "non-positive" }
for item in array { /* ... */ }
while condition { /* ... */ }
loop { if done { break; } }
return #{ is_opportunity: false, estimated_profit: 0.0, confidence: 0.0 };
```
