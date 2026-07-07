# Cartridge API Reference

## Cartridge Structure

Every cartridge is a Rhai script with three required functions and optional initialization.

### Required Functions

#### `fn init_strategy()`

Called once when the cartridge boots. Initialize any state needed for strategy evaluation.

```rhai
fn init_strategy() {
  // Example: Initialize tracking variables
  global_state = #{
    min_profit_threshold: 0.01,
    max_slippage_bps: 50,
    last_evaluated_at: 0
  };
}
```

**Constraints:**
- Max 100,000 operations
- No external calls
- Must complete within 5 seconds

---

#### `fn evaluate_opportunity(opportunity)`

Called for each opportunity. Determine if it's worth executing.

**Input:** `opportunity` (map)
```rhai
{
  chain_id: 1,
  block_number: 19500000,
  dex: "uniswap_v2",
  tokens: ["0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"],
  amounts: [1000000000000, 50000000000000000],
  reserves: [
    { token: "0xA0b...", amount: 1000000000000000000, price: 1.0 },
    { token: "0xC02...", amount: 50000000000000000, price: 2000.0 }
  ],
  metadata: { pool_address: "0x...", fee_tier: 3000 }
}
```

**Output:** Map with required fields
```rhai
{
  is_opportunity: true,           // bool: is this worth executing?
  estimated_profit: 0.5,          // f64: profit in USD (or base token)
  confidence: 0.95,               // f64: 0.0-1.0 confidence score
  metadata: #{                    // map: arbitrary metadata
    route: "token_a -> token_b -> token_a",
    hops: 2,
    slippage_estimated: 0.02
  }
}
```

**Constraints:**
- Max 1,000,000 operations
- Must complete within 10 seconds
- Called for every opportunity (high frequency)

**Example:**
```rhai
fn evaluate_opportunity(opp) {
  if opp.chain_id != 1 {
    return #{
      is_opportunity: false,
      estimated_profit: 0.0,
      confidence: 0.0,
      metadata: #{}
    };
  }

  let profit = calculate_profit(opp);
  
  return #{
    is_opportunity: profit > 0.01,
    estimated_profit: profit,
    confidence: 0.85,
    metadata: #{
      profit_breakdown: "swap_fees: 0.005, slippage: 0.003"
    }
  };
}
```

---

#### `fn build_payload(opportunity)`

Called only if `evaluate_opportunity` returned `is_opportunity: true`. Build the transaction payload.

**Input:** Same as `evaluate_opportunity` + additional context

**Output:** Map with transaction details
```rhai
{
  tx_data: "0x...",               // string: encoded transaction data
  gas_estimate: 150000,           // i64: estimated gas units
  slippage_bps: 50,               // i64: max slippage in basis points (50 = 0.5%)
  metadata: #{                    // map: arbitrary metadata
    router: "0x...",
    deadline: 1234567890,
    min_amount_out: "1000000000000000000"
  }
}
```

**Constraints:**
- Max 500,000 operations
- Must complete within 5 seconds
- Called only for valid opportunities (lower frequency)

**Example:**
```rhai
fn build_payload(opp) {
  let router = "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D"; // Uniswap V2 Router
  let path = [opp.tokens[0], opp.tokens[1]];
  let amounts_out = get_amounts_out(opp.amounts[0], path);
  let min_amount_out = amounts_out * 0.995; // 0.5% slippage
  
  return #{
    tx_data: encode_swap(router, path, opp.amounts[0], min_amount_out),
    gas_estimate: 150000,
    slippage_bps: 50,
    metadata: #{
      min_amount_out: min_amount_out.to_string(),
      deadline: now() + 300
    }
  };
}
```

---

## Host Bindings (Native Functions)

Functions available to cartridges (no imports needed).

### Reserve Data

#### `get_reserves(chain_id) → array`

Get all reserves for a chain.

```rhai
let reserves = get_reserves(1);
// Returns: [
//   { token: "0xA0b...", amount: 1000000000000000000, price: 1.0 },
//   { token: "0xC02...", amount: 50000000000000000, price: 2000.0 },
//   ...
// ]
```

#### `get_token_metadata(chain_id, token_address) → map`

Get metadata for a token.

```rhai
let meta = get_token_metadata(1, "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
// Returns: {
//   decimals: 6,
//   symbol: "USDC",
//   name: "USD Coin",
//   total_supply: "1000000000000000"
// }
```

### Pool Data

#### `get_pool_index(chain_id, pool_address) → map`

Get pool metadata.

```rhai
let pool = get_pool_index(1, "0x...");
// Returns: {
//   dex: "uniswap_v2",
//   tokens: ["0xA0b...", "0xC02..."],
//   fee: 3000,
//   liquidity: "1000000000000000000",
//   created_at: 1234567890
// }
```

#### `get_pool_reserves(chain_id, pool_address) → map`

Get current reserves for a pool.

```rhai
let reserves = get_pool_reserves(1, "0x...");
// Returns: {
//   token0: "0xA0b...",
//   token1: "0xC02...",
//   reserve0: "1000000000000000000",
//   reserve1: "50000000000000000",
//   updated_at: 1234567890
// }
```

### Math Utilities

#### `calculate_output_amount(input, reserve_in, reserve_out) → f64`

Calculate output for a swap (Uniswap V2 formula).

```rhai
let output = calculate_output_amount(1000000, 1000000000000, 50000000000000);
// Returns: 0.04998... (approximate output amount)
```

#### `calculate_price_impact(input, reserve_in, reserve_out) → f64`

Calculate price impact as a percentage.

```rhai
let impact = calculate_price_impact(1000000, 1000000000000, 50000000000000);
// Returns: 0.001 (0.1% price impact)
```

#### `sqrt(x) → f64`

Square root.

```rhai
let root = sqrt(16.0);
// Returns: 4.0
```

#### `pow(x, y) → f64`

Power function.

```rhai
let result = pow(2.0, 3.0);
// Returns: 8.0
```

### Logging

#### `log_info(message)`

Log an info message.

```rhai
log_info("Cartridge initialized for chain 1");
```

#### `log_warn(message)`

Log a warning message.

```rhai
log_warn("Profit below threshold");
```

#### `log_error(message)`

Log an error message.

```rhai
log_error("Failed to calculate output amount");
```

### Chain Info

#### `get_chain_id() → i64`

Get the current chain ID.

```rhai
let chain = get_chain_id();
// Returns: 1 (Ethereum mainnet)
```

#### `get_chain_name() → string`

Get the chain name.

```rhai
let name = get_chain_name();
// Returns: "ethereum"
```

#### `get_block_number() → i64`

Get the current block number.

```rhai
let block = get_block_number();
// Returns: 19500000
```

---

## Data Types

### Rhai Primitives

```rhai
// Numbers
let int_val = 42;           // i64
let float_val = 3.14;       // f64

// Strings
let str_val = "hello";      // string

// Booleans
let bool_val = true;        // bool

// Arrays
let arr = [1, 2, 3];        // array

// Maps (objects)
let map = #{
  key1: "value1",
  key2: 42,
  nested: #{
    inner: true
  }
};
```

### Common Patterns

#### Conditional Logic

```rhai
if profit > 0.01 {
  return true;
} else if profit > 0.005 {
  return confidence > 0.9;
} else {
  return false;
}
```

#### Loops

```rhai
let total = 0;
for token in tokens {
  total += token.amount;
}

let i = 0;
while i < 10 {
  log_info(`Iteration ${i}`);
  i += 1;
}
```

#### String Interpolation

```rhai
let chain = get_chain_id();
let block = get_block_number();
log_info(`Processing chain ${chain} at block ${block}`);
```

#### Map Access

```rhai
let opp = #{
  chain_id: 1,
  tokens: ["0xA0b...", "0xC02..."]
};

let chain = opp.chain_id;
let token = opp.tokens[0];
```

---

## Example Cartridges

### Simple DEX Arbitrage

```rhai
fn init_strategy() {
  log_info("DEX Arbitrage cartridge initialized");
}

fn evaluate_opportunity(opp) {
  if opp.chain_id != 1 {
    return #{
      is_opportunity: false,
      estimated_profit: 0.0,
      confidence: 0.0,
      metadata: #{}
    };
  }

  // Calculate profit from price difference
  let profit = (opp.amounts[1] - opp.amounts[0]) / opp.amounts[0];
  
  if profit > 0.01 {
    return #{
      is_opportunity: true,
      estimated_profit: profit,
      confidence: 0.9,
      metadata: #{
        profit_pct: profit * 100
      }
    };
  }

  return #{
    is_opportunity: false,
    estimated_profit: 0.0,
    confidence: 0.0,
    metadata: #{}
  };
}

fn build_payload(opp) {
  return #{
    tx_data: "0x...",
    gas_estimate: 200000,
    slippage_bps: 50,
    metadata: #{}
  };
}
```

### Triangular Arbitrage

```rhai
fn init_strategy() {
  log_info("Triangular arbitrage initialized");
}

fn evaluate_opportunity(opp) {
  // Requires 3 tokens
  if opp.tokens.len() < 3 {
    return #{
      is_opportunity: false,
      estimated_profit: 0.0,
      confidence: 0.0,
      metadata: #{}
    };
  }

  // Calculate round-trip profit
  let out1 = calculate_output_amount(1.0, opp.reserves[0].amount, opp.reserves[1].amount);
  let out2 = calculate_output_amount(out1, opp.reserves[1].amount, opp.reserves[2].amount);
  let out3 = calculate_output_amount(out2, opp.reserves[2].amount, opp.reserves[0].amount);

  let profit = (out3 - 1.0) / 1.0;

  return #{
    is_opportunity: profit > 0.005,
    estimated_profit: profit,
    confidence: 0.85,
    metadata: #{
      hops: 3,
      route: `${opp.tokens[0]} -> ${opp.tokens[1]} -> ${opp.tokens[2]} -> ${opp.tokens[0]}`
    }
  };
}

fn build_payload(opp) {
  return #{
    tx_data: "0x...",
    gas_estimate: 300000,
    slippage_bps: 75,
    metadata: #{}
  };
}
```

---

## Constraints & Limits

| Constraint | Limit | Notes |
|---|---|---|
| **Operations per init** | 100,000 | Prevents initialization loops |
| **Operations per evaluate** | 1,000,000 | Typical: 10K-100K |
| **Operations per build** | 500,000 | Typical: 50K-200K |
| **Array size** | 4,096 elements | Prevents memory bombs |
| **String size** | 65,536 bytes | Per string |
| **Execution timeout** | 10 seconds | Per function call |
| **Memory** | 256 MB | Per cartridge instance |

---

## Error Handling

Cartridges should handle errors gracefully:

```rhai
fn evaluate_opportunity(opp) {
  // Validate input
  if opp == null || opp.chain_id == null {
    log_error("Invalid opportunity object");
    return #{
      is_opportunity: false,
      estimated_profit: 0.0,
      confidence: 0.0,
      metadata: #{}
    };
  }

  // Try-catch pattern (if supported)
  let profit = 0.0;
  
  try {
    profit = calculate_profit(opp);
  } catch (err) {
    log_error(`Profit calculation failed: ${err}`);
    return #{
      is_opportunity: false,
      estimated_profit: 0.0,
      confidence: 0.0,
      metadata: #{ error: err.to_string() }
    };
  }

  return #{
    is_opportunity: profit > 0.01,
    estimated_profit: profit,
    confidence: 0.9,
    metadata: #{}
  };
}
```

---

## Best Practices

1. **Validate inputs** - Check for null/invalid data
2. **Log strategically** - Use log_info/warn/error for debugging
3. **Handle edge cases** - Division by zero, empty arrays, etc.
4. **Optimize for speed** - Keep evaluate_opportunity fast (called frequently)
5. **Cache calculations** - Reuse results across function calls
6. **Test locally** - Use the test harness before deployment
7. **Document metadata** - Include useful info in metadata maps
8. **Monitor errors** - Track error rates in cartridge_metrics_hourly
