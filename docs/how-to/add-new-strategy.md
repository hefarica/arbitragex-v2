---
title: Add a New Strategy
description: Implement, register, and test a new arbitrage strategy in the ArbitrageX v2 Rust engine.
tags: [strategy, rust, development]
---

# How to Add a New Strategy

This guide walks you through implementing a new arbitrage strategy in ArbitrageX v2. You will create the strategy logic, register it with the strategy evaluator, and verify it through the paper trade pipeline.

---

## Prerequisites

| Requirement | Purpose |
|-------------|---------|
| Rust 1.75+ | Strategy implementation |
| Foundry | Contract testing if strategy uses new interactions |
| Local stack running | Paper mode testing |
| Understanding of AMM mechanics | Strategy design |

---

## Step 1: Understand the Strategy Interface

Every strategy implements the `Strategy` trait defined in `crates/ax-strategy-eval/src/strategy.rs`:

```rust
use async_trait::async_trait;
use crate::types::{Opportunity, StrategyContext, StrategyResult};

#[async_trait]
pub trait Strategy: Send + Sync {
    /// Unique strategy identifier (kebab-case)
    fn id(&self) -> &'static str;

    /// Human-readable strategy name
    fn name(&self) -> &'static str;

    /// Strategy version (semver)
    fn version(&self) -> &'static str;

    /// Required DEX protocols
    fn required_protocols(&self) -> Vec<Protocol>;

    /// Evaluate whether an opportunity exists in the given context
    async fn evaluate(&self, ctx: &StrategyContext) -> StrategyResult;

    /// Minimum profit threshold in USD for this strategy
    fn min_profit_usd(&self) -> f64;

    /// Maximum gas cost tolerance in USD
    fn max_gas_cost_usd(&self) -> f64;
}
```

### Key Types

| Type | Purpose |
|------|---------|
| `StrategyContext` | Current market state: pool prices, gas costs, block number |
| `StrategyResult` | `Opportunity` vector or `Skip` with reason |
| `Opportunity` | Normalized opportunity struct consumed by execution |
| `Protocol` | Enum of supported DEX protocols (Uniswap V2/V3, Curve, SushiSwap, etc.) |

---

## Step 2: Create the Strategy File

Create a new file in the strategies directory:

```bash
touch crates/ax-strategy-eval/src/strategies/my_strategy.rs
```

### Strategy Template

```rust
use async_trait::async_trait;
use crate::strategy::{Strategy, StrategyContext, StrategyResult};
use crate::types::{Opportunity, Protocol, PoolSnapshot};
use tracing::{debug, info};

/// MyNewStrategy detects arbitrage opportunities across
/// Uniswap V3 and Curve stablecoin pools.
pub struct MyNewStrategy {
    min_profit_usd: f64,
    max_gas_cost_usd: f64,
}

impl MyNewStrategy {
    pub fn new(config: &StrategyConfig) -> Self {
        Self {
            min_profit_usd: config.min_profit_usd,
            max_gas_cost_usd: config.max_gas_cost_usd,
        }
    }
}

#[async_trait]
impl Strategy for MyNewStrategy {
    fn id(&self) -> &'static str {
        "my-new-strategy"
    }

    fn name(&self) -> &'static str {
        "My New Strategy"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn required_protocols(&self) -> Vec<Protocol> {
        vec![
            Protocol::UniswapV3,
            Protocol::Curve,
        ]
    }

    fn min_profit_usd(&self) -> f64 {
        self.min_profit_usd
    }

    fn max_gas_cost_usd(&self) -> f64 {
        self.max_gas_cost_usd
    }

    async fn evaluate(&self, ctx: &StrategyContext) -> StrategyResult {
        debug!("Evaluating my-new-strategy at block {}", ctx.block_number);

        // 1. Filter pools to relevant protocols
        let relevant_pools: Vec<&PoolSnapshot> = ctx.pools.iter()
            .filter(|p| self.required_protocols().contains(&p.protocol))
            .collect();

        if relevant_pools.len() < 2 {
            return StrategyResult::Skip("Insufficient relevant pools".into());
        }

        // 2. Detect price divergence between similar pairs
        let mut opportunities = Vec::new();

        for pool_a in &relevant_pools {
            for pool_b in &relevant_pools {
                if pool_a.address >= pool_b.address {
                    continue; // Avoid duplicate pairs
                }

                // Calculate price ratio
                let price_a = pool_a.price(ctx.base_token);
                let price_b = pool_b.price(ctx.base_token);
                let divergence = price_a.abs_diff(price_b) / price_a;

                if divergence > self.min_divergence() {
                    let estimated_profit = self.estimate_profit(
                        pool_a, pool_b, ctx.gas_price_gwei
                    );

                    if estimated_profit.net_usd >= self.min_profit_usd {
                        opportunities.push(Opportunity {
                            op_id: ctx.generate_op_id(),
                            strategy: self.id().to_string(),
                            chain: ctx.chain.clone(),
                            pools: vec![pool_a.clone(), pool_b.clone()],
                            input_amount: self.optimal_input(pool_a, pool_b),
                            expected_output: estimated_profit.output_amount,
                            expected_profit_usd: estimated_profit.gross_usd,
                            gas_estimate: estimated_profit.gas_units,
                            gas_cost_usd: estimated_profit.gas_usd,
                            net_profit_usd: estimated_profit.net_usd,
                            confidence: self.confidence_score(divergence),
                            ttl_ms: self.ttl_ms(),
                        });
                    }
                }
            }
        }

        if opportunities.is_empty() {
            StrategyResult::Skip("No profitable opportunities found".into())
        } else {
            info!(
                "my-new-strategy found {} opportunities",
                opportunities.len()
            );
            StrategyResult::Opportunities(opportunities)
        }
    }
}

impl MyNewStrategy {
    fn min_divergence(&self) -> f64 {
        0.001 // 0.1% minimum price divergence
    }

    fn ttl_ms(&self) -> u64 {
        5000 // 5 second opportunity lifetime
    }

    fn optimal_input(&self, pool_a: &PoolSnapshot, pool_b: &PoolSnapshot) -> String {
        // Calculate optimal input amount based on pool depth
        let min_liquidity = pool_a.liquidity_usd.min(pool_b.liquidity_usd);
        let optimal = min_liquidity * 0.01; // Use 1% of smaller pool depth
        (optimal * 1e18).to_string() // Convert to wei string
    }

    fn estimate_profit(
        &self,
        pool_a: &PoolSnapshot,
        pool_b: &PoolSnapshot,
        gas_price_gwei: f64,
    ) -> ProfitEstimate {
        // Calculate profit after accounting for swap fees and gas
        let gross = self.calculate_gross(pool_a, pool_b);
        let gas_units = 280_000u64; // Estimated gas for 2-hop swap
        let gas_cost_eth = (gas_units as f64) * gas_price_gwei * 1e-9;
        let gas_usd = gas_cost_eth * ctx.eth_price_usd;

        ProfitEstimate {
            gross_usd: gross,
            gas_units,
            gas_usd,
            net_usd: gross - gas_usd,
            output_amount: self.calculate_output(pool_a, pool_b),
        }
    }

    fn confidence_score(&self, divergence: f64) -> f64 {
        // Map divergence to confidence (0.0 - 1.0)
        (divergence * 100.0).min(1.0)
    }
}

struct ProfitEstimate {
    gross_usd: f64,
    gas_units: u64,
    gas_usd: f64,
    net_usd: f64,
    output_amount: String,
}
```

---

## Step 3: Register the Strategy

Add your strategy to the evaluator's registry in `crates/ax-strategy-eval/src/registry.rs`:

```rust
use crate::strategies::{
    triangular_arb::TriangularArbStrategy,
    cycle_arb::CycleArbStrategy,
    sandwich::SandwichStrategy,
    my_strategy::MyNewStrategy, // Add import
};

pub fn build_registry(config: &StrategyConfig) -> StrategyRegistry {
    let mut registry = StrategyRegistry::new();

    // Existing strategies
    registry.register(Box::new(TriangularArbStrategy::new(config)));
    registry.register(Box::new(CycleArbStrategy::new(config)));
    registry.register(Box::new(SandwichStrategy::new(config)));

    // Register new strategy
    registry.register(Box::new(MyNewStrategy::new(config)));

    registry
}
```

Expose the module in `crates/ax-strategy-eval/src/strategies/mod.rs`:

```rust
pub mod triangular_arb;
pub mod cycle_arb;
pub mod sandwich;
pub mod my_strategy; // Add module declaration
```

---

## Step 4: Build and Test

### Compile the Strategy

```bash
cargo build -p ax-strategy-eval
```

### Unit Tests

Add tests in `crates/ax-strategy-eval/src/strategies/my_strategy.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_strategy_id() {
        let strategy = MyNewStrategy::new(&StrategyConfig::default());
        assert_eq!(strategy.id(), "my-new-strategy");
    }

    #[tokio::test]
    async fn test_evaluate_finds_opportunity() {
        let ctx = StrategyContext::test_fixture()
            .with_pool(PoolSnapshot::test_uniswap_v3_weth_usdc())
            .with_pool(PoolSnapshot::test_curve_weth_usdc())
            .with_gas_price(20.0)
            .with_eth_price(2200.0);

        let strategy = MyNewStrategy::new(&StrategyConfig::default());
        let result = strategy.evaluate(&ctx).await;

        match result {
            StrategyResult::Opportunities(ops) => {
                assert!(!ops.is_empty());
                assert!(ops[0].net_profit_usd > 0.0);
            }
            StrategyResult::Skip(reason) => {
                panic!("Expected opportunities, got skip: {}", reason);
            }
        }
    }

    #[tokio::test]
    async fn test_evaluate_skips_low_divergence() {
        let ctx = StrategyContext::test_fixture()
            .with_pool(PoolSnapshot::test_identical_prices());

        let strategy = MyNewStrategy::new(&StrategyConfig::default());
        let result = strategy.evaluate(&ctx).await;

        assert!(matches!(result, StrategyResult::Skip(_)));
    }
}
```

Run the tests:

```bash
cargo test -p ax-strategy-eval my_strategy
```

### Paper Mode Integration Test

After unit tests pass, rebuild the container and verify in Paper mode:

```bash
docker compose up -d --build ax-strategy-eval
docker compose logs -f ax-strategy-eval
```

Watch for log output confirming your strategy is loaded:

```
ax-strategy-eval  | [INFO] Registered strategy: my-new-strategy (v1.0.0)
ax-strategy-eval  | [INFO] Strategy evaluator: 9 strategies active
```

Submit a test and verify your strategy produces opportunities:

```bash
curl http://localhost:3000/api/v1/opportunities?strategy=my-new-strategy&limit=5
```

---

## Step 5: Tune Parameters

Configure strategy-specific parameters via environment variables:

```bash
# .env
AX_STRATEGY_MY_NEW_STRATEGY_MIN_PROFIT=5.0
AX_STRATEGY_MY_NEW_STRATEGY_MAX_GAS=25.0
AX_STRATEGY_MY_NEW_STRATEGY_DIVERGENCE=0.001
```

Read these in your strategy constructor:

```rust
impl MyNewStrategy {
    pub fn new(config: &StrategyConfig) -> Self {
        Self {
            min_profit_usd: std::env::var("AX_STRATEGY_MY_NEW_STRATEGY_MIN_PROFIT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5.0),
            max_gas_cost_usd: std::env::var("AX_STRATEGY_MY_NEW_STRATEGY_MAX_GAS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(25.0),
        }
    }
}
```

---

## Strategy Performance Checklist

Before deploying a new strategy, verify:

| Check | Criteria | How to Verify |
|-------|----------|---------------|
| Unit tests pass | > 90% line coverage | `cargo tarpaulin` |
| Paper profit positive | Net P&L > 0 over 100 trades | Paper trade history |
| Latency acceptable | Evaluation < 10 ms | Jaeger traces |
| False positive rate | < 5% of opportunities revert | Ghost Protocol metrics |
| No panics | Zero panics in 24h paper run | Container logs |
