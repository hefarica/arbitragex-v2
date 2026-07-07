# Cartridge API Reference

## Cartridge Structure

Every cartridge (strategy) must implement exactly 3 required functions:

```rhai
fn init_strategy() -> object
fn evaluate_opportunity(market_data: object) -> object | null
fn build_payload(opportunity: object) -> object
```

## Function Specifications

### 1. init_strategy()

**Purpose:** Initialize the strategy with configuration and metadata.

**Returns:** Object with strategy configuration

**Required Fields:**
- `name` (string) - Unique strategy identifier
- `version` (string) - Semantic version (e.g., "1.0.0")
- `description` (string) - Human-readable description

**Optional Fields:**
- `chains` (array) - Supported chains (default: ["ethereum"])
- `tokens` (array) - Supported tokens
- `min_profit_bps` (int) - Minimum profit in basis points
- `max_gas_usd` (float) - Maximum acceptable gas cost
- `timeout_ms` (int) - Execution timeout in milliseconds
- `enabled` (bool) - Whether strategy is enabled (default: true)
- `metadata` (map) - Custom metadata

**Example:**

```rhai
fn init_strategy() {
    return #{
        name: "dex_arbitrage_v1",
        version: "1.0.0",
        description: "DEX arbitrage between Uniswap and SushiSwap",
        chains: ["ethereum", "polygon"],
        tokens: ["USDC", "ETH", "DAI"],
        min_profit_bps: 50,
        max_gas_usd: 100.0,
        timeout_ms: 3000,
        enabled: true,
        metadata: #{
            author: "arbitrage_team",
            risk_level: "medium"
        }
    };
}
```

### 2. evaluate_opportunity(market_data)

**Purpose:** Evaluate market data and identify arbitrage opportunities.

**Parameters:**
- `market_data` (object) - Current market data (prices, liquidity, gas prices)

**Returns:** 
- Object describing the opportunity (if found)
- `null` (if no opportunity found)

**Opportunity Object Fields:**
- `type` (string) - Type of opportunity (e.g., "dex_arb", "triangular")
- `profit_bps` (int) - Expected profit in basis points
- `profit_usd` (float) - Expected profit in USD
- `confidence` (float) - Confidence score (0.0-1.0)
- `tokens` (array) - Tokens involved
- `path` (string) - Description of the arbitrage path
- `metadata` (map) - Additional opportunity data

**Example:**

```rhai
fn evaluate_opportunity(market_data) {
    let uniswap_price = market_data.uniswap_eth_usdc;
    let sushiswap_price = market_data.sushiswap_eth_usdc;
    
    if uniswap_price <= 0 || sushiswap_price <= 0 {
        return null;
    }
    
    let price_diff_bps = ((uniswap_price - sushiswap_price) / sushiswap_price) * 10000;
    
    if price_diff_bps.abs() > 50 {
        return #{
            type: "dex_arb",
            profit_bps: price_diff_bps.abs(),
            profit_usd: calculate_profit(price_diff_bps, market_data.eth_price),
            confidence: 0.95,
            tokens: ["ETH", "USDC"],
            path: "Buy on SushiSwap, Sell on Uniswap",
            metadata: #{
                uniswap_price: uniswap_price,
                sushiswap_price: sushiswap_price
            }
        };
    }
    
    return null;
}
```

### 3. build_payload(opportunity)

**Purpose:** Build the transaction payload for executing the opportunity.

**Parameters:**
- `opportunity` (object) - Opportunity object from evaluate_opportunity()

**Returns:** Object describing the transaction payload

**Payload Object Fields:**
- `swaps` (array) - Array of swap operations
- `amount_in` (float) - Input amount
- `min_amount_out` (float) - Minimum output amount (slippage protection)
- `deadline` (int) - Transaction deadline (unix timestamp)
- `gas_estimate` (int) - Estimated gas units
- `metadata` (map) - Additional payload data

**Example:**

```rhai
fn build_payload(opportunity) {
    let amount_in = 10.0; // 10 ETH
    let min_amount_out = amount_in * 0.995; // 0.5% slippage
    let deadline = get_current_timestamp() + 300; // 5 minutes
    
    return #{
        swaps: [
            #{
                dex: "sushiswap",
                action: "swap_exact_tokens_for_tokens",
                token_in: "ETH",
                token_out: "USDC",
                amount_in: amount_in,
                min_amount_out: amount_in * 1800 * 0.995
            },
            #{
                dex: "uniswap",
                action: "swap_exact_tokens_for_tokens",
                token_in: "USDC",
                token_out: "ETH",
                amount_in: amount_in * 1800 * 0.995,
                min_amount_out: min_amount_out
            }
        ],
        amount_in: amount_in,
        min_amount_out: min_amount_out,
        deadline: deadline,
        gas_estimate: 300000,
        metadata: #{
            slippage_tolerance: 0.005,
            max_hops: 2
        }
    };
}
```

## Host Bindings (Native Functions)

### Price Functions

#### fetch_price(chain, token) -> float

Fetch current price of a token.

```rhai
let eth_price = fetch_price("ethereum", "ETH");
let usdc_price = fetch_price("polygon", "USDC");
```

**Parameters:**
- `chain` (string) - Blockchain name
- `token` (string) - Token symbol or address

**Returns:** Price as float (in USD)

#### fetch_prices_batch(chain, tokens) -> map

Fetch prices for multiple tokens efficiently.

```rhai
let prices = fetch_prices_batch("ethereum", ["ETH", "DAI", "USDC"]);
// Returns: { "ETH": 1500.0, "DAI": 1.0, "USDC": 1.0 }
```

#### get_price_history(token, hours) -> array

Get historical price data.

```rhai
let history = get_price_history("ETH", 24);
// Returns: [1500.0, 1502.5, 1498.0, ...]
```

### Liquidity Functions

#### check_liquidity(pool_id) -> float

Check available liquidity in a pool.

```rhai
let liquidity = check_liquidity("uniswap_eth_usdc_3000");
```

#### get_pool_info(pool_id) -> object

Get detailed pool information.

```rhai
let pool = get_pool_info("uniswap_eth_usdc_3000");
// Returns: {
//   reserve_0: 1000000.0,
//   reserve_1: 1500000000.0,
//   fee: 0.003,
//   liquidity: 50000000.0
// }
```

#### find_pools(token_a, token_b) -> array

Find all pools for a token pair.

```rhai
let pools = find_pools("ETH", "USDC");
// Returns: [
//   { dex: "uniswap", fee: 0.003, liquidity: 50000000.0 },
//   { dex: "sushiswap", fee: 0.003, liquidity: 30000000.0 }
// ]
```

### Gas Functions

#### estimate_gas(chain, tx_type) -> float

Estimate gas cost for a transaction type.

```rhai
let gas_cost = estimate_gas("ethereum", "swap");
```

#### get_gas_price(chain) -> float

Get current gas price.

```rhai
let gwei = get_gas_price("ethereum");
```

#### calculate_total_cost(gas_used, gas_price) -> float

Calculate total gas cost in USD.

```rhai
let cost_usd = calculate_total_cost(300000, 50.0);
```

### Execution Functions

#### execute_swap(payload) -> object

Execute a swap transaction.

```rhai
let result = execute_swap(payload);
// Returns: {
//   success: true,
//   transaction_hash: "0x...",
//   amount_out: 10.05,
//   gas_used: 298500
// }
```

#### execute_multi_swap(payloads) -> array

Execute multiple swaps atomically.

```rhai
let results = execute_multi_swap([payload1, payload2]);
```

#### simulate_transaction(payload) -> object

Simulate transaction without executing.

```rhai
let simulation = simulate_transaction(payload);
// Returns: {
//   success: true,
//   amount_out: 10.05,
//   gas_estimate: 298500,
//   slippage_percent: 0.5
// }
```

### Logging Functions

#### log_event(message) -> void

Log an event.

```rhai
log_event("Found arbitrage opportunity: ETH/USDC");
```

#### log_error(error) -> void

Log an error.

```rhai
log_error("Insufficient liquidity for swap");
```

#### log_metric(name, value) -> void

Log a metric.

```rhai
log_metric("opportunity_profit_bps", 75.5);
```

### Data Functions

#### get_cached_data(key) -> any

Retrieve cached data.

```rhai
let cached = get_cached_data("last_eth_price");
```

#### set_cached_data(key, value, ttl) -> void

Store data in cache.

```rhai
set_cached_data("last_eth_price", 1500.0, 60); // 60 second TTL
```

#### delete_cached_data(key) -> void

Delete cached data.

```rhai
delete_cached_data("last_eth_price");
```

## Rhai Data Types

### Primitive Types

```rhai
let str = "hello";           // string
let num = 42;                // int
let float = 3.14;            // float
let bool = true;             // bool
let null_val = null;         // null
```

### Collections

```rhai
// Array
let arr = [1, 2, 3];
arr.push(4);
let first = arr[0];

// Map (Object)
let obj = #{
    name: "strategy",
    version: "1.0.0",
    enabled: true
};
obj.name = "new_name";
let value = obj["name"];
```

### Type Checking

```rhai
if obj.type_of() == "object" {
    // ...
}

if arr.type_of() == "array" {
    // ...
}
```

## Common Patterns

### Conditional Logic

```rhai
if opportunity.profit_bps > 50 {
    log_event("High profit opportunity");
} else if opportunity.profit_bps > 30 {
    log_event("Medium profit opportunity");
} else {
    return null;
}
```

### Loops

```rhai
// For loop
for token in tokens {
    let price = fetch_price("ethereum", token);
    log_event("Price of " + token + ": " + price);
}

// While loop
let i = 0;
while i < 10 {
    i += 1;
}
```

### Maps and Filtering

```rhai
let prices = fetch_prices_batch("ethereum", ["ETH", "DAI", "USDC"]);

// Iterate over map
for key in prices.keys() {
    let value = prices[key];
}

// Filter
let high_value = prices.filter(|k, v| v > 1000.0);
```

### Error Handling

```rhai
try {
    let result = execute_swap(payload);
    if !result.success {
        log_error("Swap failed: " + result.error);
        return null;
    }
} catch (error) {
    log_error("Exception: " + error);
    return null;
}
```

### Calculations

```rhai
let price_diff = (price_a - price_b).abs();
let profit_bps = (price_diff / price_b) * 10000;
let profit_usd = profit_bps * amount / 10000;
```

## Complete Examples

### Example 1: DEX Arbitrage

```rhai
fn init_strategy() {
    return #{
        name: "dex_arbitrage",
        version: "1.0.0",
        description: "Arbitrage between Uniswap and SushiSwap",
        chains: ["ethereum"],
        min_profit_bps: 50,
        max_gas_usd: 100.0
    };
}

fn evaluate_opportunity(market_data) {
    let uni_price = market_data.uniswap_eth_usdc;
    let sushi_price = market_data.sushiswap_eth_usdc;
    
    if uni_price <= 0 || sushi_price <= 0 {
        return null;
    }
    
    let buy_price = sushi_price;
    let sell_price = uni_price;
    
    if buy_price >= sell_price {
        return null;
    }
    
    let profit_bps = ((sell_price - buy_price) / buy_price) * 10000;
    
    if profit_bps < 50 {
        return null;
    }
    
    let gas_cost = estimate_gas("ethereum", "swap");
    let profit_after_gas = profit_bps - (gas_cost / 1500.0) * 10000;
    
    if profit_after_gas < 30 {
        return null;
    }
    
    return #{
        type: "dex_arb",
        profit_bps: profit_bps,
        profit_usd: (profit_bps / 10000) * 1500.0,
        confidence: 0.95,
        tokens: ["ETH", "USDC"],
        path: "Buy SushiSwap → Sell Uniswap"
    };
}

fn build_payload(opportunity) {
    let amount_in = 10.0;
    let min_amount_out = amount_in * 0.995;
    
    return #{
        swaps: [
            #{
                dex: "sushiswap",
                action: "buy",
                token_in: "USDC",
                token_out: "ETH",
                amount_in: amount_in * 1500
            },
            #{
                dex: "uniswap",
                action: "sell",
                token_in: "ETH",
                token_out: "USDC",
                amount_in: amount_in
            }
        ],
        amount_in: amount_in * 1500,
        min_amount_out: min_amount_out * 1500,
        deadline: get_current_timestamp() + 300,
        gas_estimate: 300000
    };
}
```

### Example 2: Triangular Arbitrage

```rhai
fn init_strategy() {
    return #{
        name: "triangular_arbitrage",
        version: "1.0.0",
        description: "Triangular arbitrage: USDC → ETH → DAI → USDC",
        chains: ["ethereum"],
        tokens: ["USDC", "ETH", "DAI"],
        min_profit_bps: 30
    };
}

fn evaluate_opportunity(market_data) {
    let usdc_to_eth = market_data.usdc_eth_price;
    let eth_to_dai = market_data.eth_dai_price;
    let dai_to_usdc = market_data.dai_usdc_price;
    
    if usdc_to_eth <= 0 || eth_to_dai <= 0 || dai_to_usdc <= 0 {
        return null;
    }
    
    // Calculate round-trip profit
    let path_profit = (usdc_to_eth * eth_to_dai * dai_to_usdc) - 1.0;
    let profit_bps = path_profit * 10000;
    
    if profit_bps < 30 {
        return null;
    }
    
    return #{
        type: "triangular_arb",
        profit_bps: profit_bps,
        profit_usd: profit_bps / 100,
        confidence: 0.90,
        tokens: ["USDC", "ETH", "DAI"],
        path: "USDC → ETH → DAI → USDC",
        metadata: #{
            leg1_rate: usdc_to_eth,
            leg2_rate: eth_to_dai,
            leg3_rate: dai_to_usdc
        }
    };
}

fn build_payload(opportunity) {
    let start_amount = 10000.0; // 10k USDC
    
    return #{
        swaps: [
            #{
                dex: "uniswap",
                action: "swap",
                token_in: "USDC",
                token_out: "ETH",
                amount_in: start_amount
            },
            #{
                dex: "sushiswap",
                action: "swap",
                token_in: "ETH",
                token_out: "DAI",
                amount_in: "auto"
            },
            #{
                dex: "curve",
                action: "swap",
                token_in: "DAI",
                token_out: "USDC",
                amount_in: "auto"
            }
        ],
        amount_in: start_amount,
        min_amount_out: start_amount * 1.0003,
        deadline: get_current_timestamp() + 600,
        gas_estimate: 450000
    };
}
```

## Constraints and Limits

| Constraint | Limit | Notes |
|-----------|-------|-------|
| Execution Timeout | 5 seconds | Hard limit, cartridge will be terminated |
| Memory per Instance | 256 MB | Soft limit, may cause slowdown |
| Host Binding Calls | 100 per execution | Prevent excessive external calls |
| Payload Size | 10 MB | Maximum transaction payload size |
| String Length | 1 MB | Maximum string size |
| Array Length | 100,000 items | Maximum array size |
| Map Keys | 100,000 keys | Maximum map size |
| Recursion Depth | 128 levels | Maximum recursion depth |

## Best Practices

### 1. Validate Inputs

Always validate market data before using:

```rhai
fn evaluate_opportunity(market_data) {
    if market_data == null {
        log_error("Market data is null");
        return null;
    }
    
    if market_data.type_of() != "object" {
        log_error("Market data is not an object");
        return null;
    }
    
    // Validate prices
    if market_data.price <= 0 {
        log_error("Invalid price: " + market_data.price);
        return null;
    }
}
```

### 2. Use Slippage Protection

Always set minimum output amounts:

```rhai
let min_amount_out = amount_in * expected_rate * 0.995; // 0.5% slippage
```

### 3. Log Decisions

Log key decision points for debugging:

```rhai
log_event("Evaluating opportunity with profit: " + profit_bps + " bps");
log_event("Gas cost: " + gas_cost + " USD");
log_event("Net profit: " + net_profit + " USD");
```

### 4. Handle Edge Cases

Consider market conditions and edge cases:

```rhai
// Check for zero prices
if price <= 0 {
    return null;
}

// Check for insufficient liquidity
if liquidity < min_required {
    return null;
}

// Check for extreme slippage
if slippage_percent > 5.0 {
    return null;
}
```

### 5. Optimize Gas

Minimize on-chain operations:

```rhai
// Batch multiple operations
let results = execute_multi_swap([swap1, swap2, swap3]);

// Use cached data when possible
let cached_price = get_cached_data("eth_price");
if cached_price != null {
    // use cached price
}
```

### 6. Test Thoroughly

Always simulate before executing:

```rhai
let simulation = simulate_transaction(payload);
if !simulation.success {
    log_error("Simulation failed: " + simulation.error);
    return null;
}

if simulation.slippage_percent > 1.0 {
    log_error("Slippage too high: " + simulation.slippage_percent);
    return null;
}
```
