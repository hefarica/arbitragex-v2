# 00. NÚCLEO DE INGENIERÍA MEV/ARBITRAGE (v1.0.0) — Fundación de la Biblioteca

> **Procedencia**: núcleo v1.0.0 de la skill `arbx-live-engineering`, redactado por orden del
> operador el 2026-09-15. Se preserva íntegro como fundación de la biblioteca de conocimiento
> (referencias 11-22 lo extienden en profundidad sin duplicarlo). La skill operacional
> (SKILL.md §1-§14) gobierna el encargo; esta biblioteca es el cuerpo de conocimiento técnico.
>
> **GOBERNANZA**: todo el conocimiento aquí está subordinado a los gates `arbx-*`
> (paper-trade-first, simulation-mandatory, risk-limits-enforcement, pre-execute-checklist) y a
> CLAUDE.md §34 (LIVE_MAINNET gated, default-deny en `relays-client`). Nada de esta biblioteca
> autoriza un flip a live ni broadcast con capital real por sí mismo.

# SKILL: ARBX-LIVE-ENGINEERING — NÚCLEO
# VERSIÓN: 1.0.0
# ALCANCE: Ingeniería de sistemas live para arbitrage DeFi

## 1. ARQUITECTURA DE MOTORES MEV/ARBITRAGE

### 1.1 Pipeline Clásico de Arbitrage

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   MEMPOOL   │────▶│  DETECTOR   │────▶│  SIMULADOR  │
│   WATCHER   │     │   CICLOS    │     │   EVM/REVM  │
└─────────────┘     └─────────────┘     └──────┬──────┘
                                                │
┌─────────────┐     ┌─────────────┐     ┌─────▼───────┐
│  EJECUTOR   │◀────│  DECISOR    │◀────│  EVALUADOR  │
│  (BUNDLE)   │     │  RIESGO     │     │  P&L/GAS    │
└─────────────┘     └─────────────┘     └─────────────┘
```

### 1.2 Componentes Core

**Mempool Watcher**
- Suscripción a `eth_newPendingTransactions` o `mev_sendBundle` privado
- Filtrado por tokens objetivo (allowlist dinámica)
- Latencia objetivo: <50ms desde tx pendiente a detección

**Detector de Ciclos**
- Graph de pools en memoria (TokenGraph)
- Algoritmo: DFS con detección de ciclos 2-5 hops
- Caché de rutas pre-computadas (warmup en bloques vacíos)

**Simulador EVM**
- REVM para Rust (simulación local sin RPC)
- Anvil fork para validación contra estado real
- State diff validation (post-sim vs pre-sim)

**Evaluador de P&L**
```rust
// Fórmula completa de P&L
net_profit = output_amount - input_amount 
           - gas_cost 
           - flash_loan_fee 
           - protocol_fees 
           - bribe_to_miner
           - slippage_adjustment
```

**Decisor de Riesgo**
- Checklist A.9 (max loss, slippage, gas ceiling)
- Circuit breaker (kill switch por pérdida acumulada)
- Quorum de RPCs (3 proveedores, 2/3 consensus)

**Ejecutor**
- `eth_sendBundle` a Flashbots/MEV-Share
- `eth_sendPrivateTransaction` (protect)
- Fallback a mempool público (último recurso)

### 1.3 Patrones de Integración

**Pattern: Carrier-Based Pipeline**
```rust
// Productor escribe plan validado
redis::set_ex("arbx:validated_plan:{id}", json_plan, 300)?;

// Consumidor lee y re-simula
let plan: ValidatedPlan = redis::get(key)?;
let outcome = execute_multistep_revm(&plan.ctx, &config)?;
```

**Pattern: Event-Driven Architecture**
```rust
// Redis Streams como bus de eventos
XADD arbx:opps:detected * chain_id 1 token_in 0x... token_out 0x... ...

// Workers independientes consumen por tipo
XREADGROUP GROUP sim-ctl consumer-1 STREAMS arbx:opps:detected >
```

**Pattern: Idempotency via Deterministic Key**
```rust
let idempotency_key = keccak256(encode(&[
    chain_id,
    opportunity_id,
    target_block,
    route_hash,
    amount_in,
    mode.as_bytes(),
]));
```

## 2. SMART CONTRACTS PARA ARBITRAGE

### 2.1 ArbitrageExecutor.sol (UUPS Proxy)

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";

contract ArbitrageExecutor is UUPSUpgradeable, AccessControlUpgradeable {
    bytes32 public constant EXECUTOR_ROLE = keccak256("EXECUTOR_ROLE");
    bytes32 public constant ADMIN_ROLE = keccak256("ADMIN_ROLE");
    
    // Events
    event ArbitrageExecuted(
        bytes32 indexed routeHash,
        uint256 inputAmount,
        uint256 outputAmount,
        uint256 gasUsed,
        uint256 netProfit
    );
    
    event EmergencyPause(address indexed triggeredBy);
    event EmergencyUnpause(address indexed triggeredBy);
    
    // State
    mapping(bytes32 => bool) public executedRoutes;
    bool public paused;
    uint256 public maxLossPerTrade;
    uint256 public minProfitThreshold;
    
    modifier whenNotPaused() {
        require(!paused, "Contract paused");
        _;
    }
    
    function initialize() public initializer {
        __AccessControl_init();
        __UUPSUpgradeable_init();
        _grantRole(DEFAULT_ADMIN_ROLE, msg.sender);
        _grantRole(ADMIN_ROLE, msg.sender);
    }
    
    // Flash loan callback (Balancer/Aave compatible)
    function executeArbitrage(
        bytes calldata routeData,
        uint256 amountIn,
        bytes32 routeHash
    ) external whenNotPaused onlyRole(EXECUTOR_ROLE) {
        require(!executedRoutes[routeHash], "Route already executed");
        
        // Decode route: [(dex, tokenIn, tokenOut, fee), ...]
        RouteStep[] memory route = abi.decode(routeData, (RouteStep[]));
        
        uint256 balanceBefore = IERC20(route[0].tokenIn).balanceOf(address(this));
        
        // Execute swaps
        for (uint i = 0; i < route.length; i++) {
            _executeSwap(route[i]);
        }
        
        uint256 balanceAfter = IERC20(route[0].tokenIn).balanceOf(address(this));
        uint256 netProfit = balanceAfter - balanceBefore;
        
        require(netProfit >= minProfitThreshold, "Profit below threshold");
        require(netProfit >= maxLossPerTrade, "Loss exceeds max"); // maxLoss is negative threshold
        
        executedRoutes[routeHash] = true;
        
        emit ArbitrageExecuted(routeHash, amountIn, balanceAfter, gasleft(), netProfit);
    }
    
    function _executeSwap(RouteStep memory step) internal {
        // Integration with DEX routers
        if (step.dex == DexType.UniswapV2) {
            IUniswapV2Router(step.router).swapExactTokensForTokens(...);
        } else if (step.dex == DexType.UniswapV3) {
            IUniswapV3Router(step.router).exactInput(...);
        }
        // ... other DEXs
    }
    
    // Emergency functions
    function emergencyPause() external onlyRole(ADMIN_ROLE) {
        paused = true;
        emit EmergencyPause(msg.sender);
    }
    
    function emergencyWithdraw(address token) external onlyRole(ADMIN_ROLE) {
        IERC20(token).transfer(msg.sender, IERC20(token).balanceOf(address(this)));
    }
    
    function _authorizeUpgrade(address newImplementation) internal override onlyRole(ADMIN_ROLE) {}
}

struct RouteStep {
    DexType dex;
    address router;
    address tokenIn;
    address tokenOut;
    uint24 fee; // For V3
    uint256 minOut; // Slippage protection
}

enum DexType { UniswapV2, UniswapV3, Curve, Balancer }
```

### 2.2 Flash Loan Integration

```solidity
// Balancer Flash Loans
interface IFlashLoanRecipient {
    function receiveFlashLoan(
        IERC20[] memory tokens,
        uint256[] memory amounts,
        uint256[] memory feeAmounts,
        bytes memory userData
    ) external;
}

// Aave Flash Loans
interface IFlashLoanSimpleReceiver {
    function executeOperation(
        address asset,
        uint256 amount,
        uint256 premium,
        address initiator,
        bytes calldata params
    ) external returns (bool);
}
```

## 3. BACKEND ENGINEERING

### 3.1 Rust Async Patterns

**Pattern: Actor Model with Tokio**
```rust
use tokio::sync::mpsc;

struct ArbitrageActor {
    rx: mpsc::Receiver<ArbitrageOpportunity>,
    executor: Arc<dyn BundleExecutor>,
}

impl ArbitrageActor {
    async fn run(mut self) {
        while let Some(opp) = self.rx.recv().await {
            if self.validate(&opp).await {
                self.execute(opp).await;
            }
        }
    }
}
```

**Pattern: Circuit Breaker**
```rust
use std::sync::atomic::{AtomicUsize, Ordering};

struct CircuitBreaker {
    failures: AtomicUsize,
    threshold: usize,
    last_failure: Mutex<Instant>,
}

impl CircuitBreaker {
    async fn call<F, Fut, T>(&self, f: F) -> Result<T, CircuitBreakerOpen>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, Error>>,
    {
        if self.is_open() {
            return Err(CircuitBreakerOpen);
        }
        
        match f().await {
            Ok(v) => {
                self.reset();
                Ok(v)
            }
            Err(e) => {
                self.record_failure();
                Err(e)
            }
        }
    }
}
```

**Pattern: Rate Limiter (Token Bucket)**
```rust
use governor::{Quota, RateLimiter};
use std::num::NonZeroU32;

let limiter = RateLimiter::direct(Quota::per_second(NonZeroU32::new(100).unwrap()));
limiter.until_ready().await;
```

### 3.2 Database Patterns

**PostgreSQL for Arbitrage State**

```sql
-- Opportunities table with partitioning
CREATE TABLE opportunities (
    id UUID PRIMARY KEY,
    detected_at TIMESTAMPTZ NOT NULL,
    chain_id INTEGER NOT NULL,
    strategy_kind VARCHAR(50) NOT NULL,
    token_in VARCHAR(42) NOT NULL,
    token_out VARCHAR(42) NOT NULL,
    amount_in NUMERIC(78, 0),
    expected_profit_wei NUMERIC(78, 0),
    status opportunity_status NOT NULL,
    rejection_reason VARCHAR(100),
    
    CONSTRAINT valid_amount CHECK (amount_in > 0)
) PARTITION BY RANGE (detected_at);

-- Monthly partitions
CREATE TABLE opportunities_y2024m01 PARTITION OF opportunities
    FOR VALUES FROM ('2024-01-01') TO ('2024-02-01');

-- Indexes for hot queries
CREATE INDEX CONCURRENTLY idx_opps_detected_chain 
    ON opportunities(detected_at, chain_id) 
    WHERE status = 'detected';

-- Simulations table
CREATE TABLE simulations (
    id UUID PRIMARY KEY,
    opportunity_id UUID REFERENCES opportunities(id),
    simulated_at TIMESTAMPTZ DEFAULT NOW(),
    simulator VARCHAR(20), -- 'revm', 'anvil', 'prod'
    passed BOOLEAN,
    gas_estimate_wei NUMERIC(78, 0),
    gas_price_wei NUMERIC(78, 0),
    fail_reason VARCHAR(200),
    
    CONSTRAINT fk_opp FOREIGN KEY (opportunity_id) 
        REFERENCES opportunities(id) ON DELETE CASCADE
);

-- Executions with idempotency
CREATE TABLE executions (
    idempotency_key BYTEA PRIMARY KEY,
    opportunity_id UUID NOT NULL,
    executed_at TIMESTAMPTZ DEFAULT NOW(),
    tx_hash VARCHAR(66),
    block_number INTEGER,
    gas_used INTEGER,
    effective_gas_price NUMERIC(78, 0),
    status execution_status, -- 'pending', 'confirmed', 'reverted', 'expired'
    
    CONSTRAINT unique_idempotency UNIQUE (idempotency_key)
);

-- State machine transitions
CREATE TYPE execution_status AS ENUM (
    'CREATED', 'VALIDATED', 'AUTHORIZED', 
    'SUBMITTING', 'SUBMITTED', 
    'INCLUDED', 'REVERTED', 'EXPIRED', 'RECONCILED'
);
```

**Redis Patterns**

```rust
// Connection pool with bb8-redis
use bb8_redis::RedisConnectionManager;

let manager = RedisConnectionManager::new("redis://localhost:6379")?;
let pool = bb8::Pool::builder().max_size(20).build(manager).await?;

// Atomic operations for counters
redis::pipe()
    .atomic()
    .incr("arbx:metrics:simulations:total", 1)
    .incr("arbx:metrics:simulations:passed", passed as i64)
    .query_async(&mut conn).await?;

// Distributed locking for singleton operations
let lock_key = format!("arbx:lock:fee_update:{}", chain_id);
let lock = redis::cmd("SET")
    .arg(&lock_key)
    .arg(uuid::Uuid::new_v4().to_string())
    .arg("NX")
    .arg("EX")
    .arg(30) // 30s TTL
    .query_async::<_, Option<String>>(&mut conn).await?;
```

## 4. DEVOPS & DEPLOYMENT

### 4.1 Docker Multi-Stage Build

```dockerfile
# Dockerfile for arbitrage-engine
FROM rust:1.91-slim-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/

# Build dependencies cache
RUN mkdir -p crates/engine/src && echo "fn main() {}" > crates/engine/src/main.rs
RUN cargo build --release -p arbitrage-engine
RUN rm -rf crates/engine/src

# Build actual binary
COPY . .
RUN cargo build --release -p arbitrage-engine

# Runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/arbitrage-engine /usr/local/bin/
COPY --from=builder /app/config/ /app/config/

USER 1000:1000
WORKDIR /app

ENV RUST_LOG=info
ENV RUST_BACKTRACE=1

ENTRYPOINT ["arbitrage-engine"]
```

### 4.2 Kubernetes Deployment

```yaml
# k8s/arbitrage-engine.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: arbitrage-engine
  namespace: trading
spec:
  replicas: 3
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxSurge: 1
      maxUnavailable: 0
  selector:
    matchLabels:
      app: arbitrage-engine
  template:
    metadata:
      labels:
        app: arbitrage-engine
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "9090"
    spec:
      containers:
      - name: engine
        image: ghcr.io/hefarica/arbitrage-engine:v1.2.3
        resources:
          requests:
            memory: "4Gi"
            cpu: "2000m"
          limits:
            memory: "8Gi"
            cpu: "4000m"
        env:
        - name: RUST_LOG
          value: "info,arbitrage_engine=debug"
        - name: REDIS_URL
          valueFrom:
            secretKeyRef:
              name: arbitrage-secrets
              key: redis-url
        - name: RPC_WS_1
          valueFrom:
            configMapKeyRef:
              name: arbitrage-config
              key: rpc-ws-mainnet
        ports:
        - containerPort: 9090
          name: metrics
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
        volumeMounts:
        - name: config
          mountPath: /app/config
          readOnly: true
      volumes:
      - name: config
        configMap:
          name: arbitrage-config
---
apiVersion: v1
kind: Service
metadata:
  name: arbitrage-engine
  namespace: trading
spec:
  selector:
    app: arbitrage-engine
  ports:
  - port: 8080
    targetPort: 8080
  - port: 9090
    targetPort: 9090
    name: metrics
```

### 4.3 CI/CD Pipeline (GitHub Actions)

```yaml
# .github/workflows/arbitrage-engine.yml
name: Arbitrage Engine CI/CD

on:
  push:
    branches: [main, fix/*, feature/*]
    tags: ['v*']
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-action@stable
      
      - name: Cache dependencies
        uses: Swatinem/rust-cache@v2
      
      - name: Run tests
        run: cargo test --workspace --lib --features test-utils
      
      - name: Run clippy
        run: cargo clippy --workspace --all-targets -- -D warnings
      
      - name: Check formatting
        run: cargo fmt --all -- --check

  integration:
    runs-on: ubuntu-latest
    needs: test
    services:
      postgres:
        image: postgres:16
        env:
          POSTGRES_PASSWORD: postgres
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
        ports:
          - 5432:5432
      
      redis:
        image: redis:7-alpine
        ports:
          - 6379:6379
      
      anvil:
        image: ghcr.io/foundry-rs/foundry:latest
        ports:
          - 8545:8545
        options: >-
          --entrypoint anvil
          --args ["--fork-url", "${{ secrets.MAINNET_RPC }}", "--fork-block-number", "20000000"]

    steps:
      - uses: actions/checkout@v4
      
      - name: Run integration tests
        env:
          DATABASE_URL: postgres://postgres:postgres@localhost:5432/test
          REDIS_URL: redis://localhost:6379
          RPC_HTTP_1: http://localhost:8545
        run: cargo test --workspace --test integration

  deploy:
    runs-on: ubuntu-latest
    needs: [test, integration]
    if: github.ref == 'refs/heads/main'
    steps:
      - name: Deploy to staging
        run: |
          kubectl set image deployment/arbitrage-engine \
            engine=ghcr.io/hefarica/arbitrage-engine:${{ github.sha }} \
            -n trading-staging
          
      - name: Smoke tests
        run: |
          kubectl rollout status deployment/arbitrage-engine -n trading-staging --timeout=300s
          curl -f https://staging-api.arbitrage.io/health
          
      - name: Deploy to production
        if: github.event_name == 'push' && startsWith(github.ref, 'refs/tags/v')
        run: |
          kubectl set image deployment/arbitrage-engine \
            engine=ghcr.io/hefarica/arbitrage-engine:${{ github.ref_name }} \
            -n trading
```

## 5. OBSERVABILIDAD Y MONITOREO

### 5.1 Métricas Prometheus

```rust
use prometheus::{Counter, Gauge, Histogram, Registry, Opts};

lazy_static! {
    static ref REGISTRY: Registry = Registry::new();
    
    // Simulations
    static ref SIMULATIONS_TOTAL: Counter = Counter::with_opts(
        Opts::new("arbx_simulations_total", "Total simulations run")
            .const_label("component", "sim-ctl")
    ).unwrap();
    
    static ref SIMULATIONS_PASSED: Counter = Counter::with_opts(
        Opts::new("arbx_simulations_passed", "Simulations that passed")
    ).unwrap();
    
    static ref SIMULATION_DURATION: Histogram = Histogram::with_opts(
        HistogramOpts::new("arbx_simulation_duration_seconds", "Simulation time")
            .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0])
    ).unwrap();
    
    // Opportunities
    static ref OPPORTUNITIES_DETECTED: CounterVec = CounterVec::new(
        Opts::new("arbx_opportunities_detected", "Opportunities by strategy"),
        &["strategy_kind", "chain_id"]
    ).unwrap();
    
    static ref OPPORTUNITIES_REJECTED: CounterVec = CounterVec::new(
        Opts::new("arbx_opportunities_rejected", "Rejections by reason"),
        &["rejection_reason", "strategy_kind"]
    ).unwrap();
    
    // P&L
    static ref PROFIT_REALIZED: Gauge = Gauge::with_opts(
        Opts::new("arbx_profit_realized_wei", "Realized profit in wei")
    ).unwrap();
    
    static ref GAS_COST: Gauge = Gauge::with_opts(
        Opts::new("arbx_gas_cost_wei", "Gas costs in wei")
    ).unwrap();
    
    // Safety
    static ref SAFETY_SCORE: Gauge = Gauge::with_opts(
        Opts::new("arbx_safety_score", "Current safety score 0-100")
    ).unwrap();
}

// Usage
SIMULATIONS_TOTAL.inc();
let timer = SIMULATION_DURATION.start_timer();
// ... simulation ...
timer.observe_duration();
```

### 5.2 Logging Estructurado

```rust
use tracing::{info, warn, error, instrument};

#[instrument(
    skip(self, opp),
    fields(
        opportunity_id = %opp.id,
        chain_id = opp.chain_id,
        strategy = %opp.strategy_kind,
        expected_profit = ?opp.expected_profit_wei
    )
)]
async fn evaluate_opportunity(&self, opp: Opportunity) -> Result<Decision, Error> {
    info!("Evaluating opportunity");
    
    if opp.expected_profit_wei < self.min_profit_threshold {
        warn!(
            expected = %opp.expected_profit_wei,
            threshold = %self.min_profit_threshold,
            "Opportunity below profit threshold"
        );
        return Ok(Decision::Reject(Reason::BelowThreshold));
    }
    
    // ... evaluation logic
    
    info!("Opportunity approved for execution");
    Ok(Decision::Execute)
}
```

### 5.3 Alertas (Prometheus Alertmanager)

```yaml
# alertmanager/arbitrage-alerts.yml
groups:
  - name: arbitrage-critical
    rules:
      - alert: HighRejectionRate
        expr: |
          (
            rate(arbx_opportunities_rejected_total[5m])
            /
            rate(arbx_opportunities_detected_total[5m])
          ) > 0.95
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High rejection rate (>95%)"
          description: "Rejection rate is {{ $value | humanizePercentage }}"

      - alert: SafetyScoreBelowThreshold
        expr: arbx_safety_score < 50
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Safety score below threshold"
          description: "Current score: {{ $value }}"

      - alert: NoProfitableTrades
        expr: |
          increase(arbx_profit_realized_wei[1h]) == 0
          and
          increase(arbx_opportunities_detected_total[1h]) > 100
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "No profitable trades in 1 hour despite opportunities"

      - alert: CircuitBreakerOpen
        expr: arbx_circuit_breaker_open == 1
        for: 0m
        labels:
          severity: critical
        annotations:
          summary: "Circuit breaker is OPEN"
          description: "Trading halted due to failures"
```

### 5.4 Distributed Tracing (Jaeger/Tempo)

```rust
use opentelemetry::trace::{Tracer, TraceContextExt};
use opentelemetry::global;

// Initialize tracer
fn init_tracer() -> impl Tracer {
    opentelemetry_jaeger::new_agent_pipeline()
        .with_service_name("arbitrage-engine")
        .install_simple()
}

// Create spans
async fn execute_pipeline(ctx: Context) {
    let tracer = global::tracer("arbitrage-engine");
    
    let span = tracer.start("pipeline_execution");
    let cx = Context::current_with_span(span);
    
    async {
        detect_opportunities().await;
        simulate().await;
        execute().await;
    }
    .with_context(cx)
    .await;
}
```

## 6. SEGURIDAD Y HARDENING

### 6.1 Secreto Management

```rust
use aws_sdk_secretsmanager::Client;
use std::env;

async fn load_secrets() -> Result<Secrets, Error> {
    // Local development: env vars
    if env::var("LOCAL_DEV").is_ok() {
        return Ok(Secrets {
            rpc_url: env::var("RPC_URL")?,
            private_key: env::var("PRIVATE_KEY")?,
            redis_url: env::var("REDIS_URL")?,
        });
    }
    
    // Production: AWS Secrets Manager
    let client = Client::new(&aws_config::load_from_env().await);
    let secret = client
        .get_secret_value()
        .secret_id("arbitrage-engine/prod")
        .send()
        .await?;
    
    let payload: Secrets = serde_json::from_str(
        secret.secret_string().unwrap_or("{}")
    )?;
    
    Ok(payload)
}

// Never log secrets
#[derive(Debug)]
struct Secrets {
    #[debug(skip)]
    private_key: String,
    rpc_url: String,
    redis_url: String,
}
```

### 6.2 Transaction Security

```rust
// Slippage protection
fn apply_slippage(amount_out: U256, slippage_bps: u16) -> U256 {
    let basis_points = U256::from(10_000);
    let slippage = U256::from(slippage_bps);
    amount_out * (basis_points - slippage) / basis_points
}

// Deadline protection
fn check_deadline(deadline: U256) -> Result<(), Error> {
    let current_time = U256::from(SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs());
    
    if current_time > deadline {
        return Err(Error::DeadlineExpired);
    }
    Ok(())
}

// Reentrancy guard (for smart contracts)
modifier nonReentrant() {
    require(!_locked, "Reentrant call");
    _locked = true;
    _;
    _locked = false;
}
```

### 6.3 Kill Switch

```rust
use std::sync::atomic::{AtomicBool, Ordering};

static EMERGENCY_STOP: AtomicBool = AtomicBool::new(false);

async fn check_kill_switch() -> Result<(), Error> {
    if EMERGENCY_STOP.load(Ordering::SeqCst) {
        // Close all positions, halt trading
        emergency_liquidate_all().await?;
        return Err(Error::EmergencyStop);
    }
    Ok(())
}

// HTTP endpoint for manual trigger
async fn emergency_stop_handler() -> impl Responder {
    EMERGENCY_STOP.store(true, Ordering::SeqCst);
    HttpResponse::Ok().body("Emergency stop activated")
}
```

## 7. OPTIMIZACIÓN DE PERFORMANCE

### 7.1 Memory Management

```rust
// Object pooling for hot paths
use object_pool::Pool;

static SIMULATION_CONTEXT_POOL: Pool<SimulationContext> = Pool::new(
    100, // max size
    || SimulationContext::new() // factory
);

fn acquire_context() -> Reusable<SimulationContext> {
    SIMULATION_CONTEXT_POOL.try_pull().unwrap_or_else(|| {
        Reusable::new(SIMULATION_CONTEXT_POOL, SimulationContext::new())
    })
}

// Zero-copy deserialization
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize)]
struct Opportunity {
    // Zero-copy access to archived data
}
```

### 7.2 Network Optimization

```rust
// Connection pooling for RPC
use hyper::Client;
use hyper::client::HttpConnector;

let connector = HttpConnector::new();
let client: Client<HttpConnector> = Client::builder()
    .pool_idle_timeout(Duration::from_secs(30))
    .pool_max_idle_per_host(10)
    .build(connector);

// Batch RPC calls
let batch = vec![
    json!({"jsonrpc": "2.0", "id": 1, "method": "eth_getBalance", "params": [addr, "latest"]}),
    json!({"jsonrpc": "2.0", "id": 2, "method": "eth_getCode", "params": [addr, "latest"]}),
];
```

### 7.3 SIMD for Price Calculations

```rust
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

// Vectorized price impact calculation
unsafe fn calculate_impacts_simd(prices: &[f64], amounts: &[f64]) -> Vec<f64> {
    let mut results = Vec::with_capacity(prices.len());
    
    for i in (0..prices.len()).step_by(4) {
        let price_vec = _mm256_loadu_pd(&prices[i]);
        let amount_vec = _mm256_loadu_pd(&amounts[i]);
        let impact = _mm256_mul_pd(price_vec, amount_vec);
        
        // Store results...
    }
    
    results
}
```

## 8. INTEGRACIÓN CON INFRAESTRUCTURA EXTERNA

### 8.1 Flashbots/MEV-Boost

```rust
use flashbots::FlashbotsMiddleware;
use ethers::providers::{Http, Provider};

let flashbots = FlashbotsMiddleware::new(
    Provider::<Http>::try_from("https://rpc.flashbots.net")?,
    "https://relay.flashbots.net",
    // Auth key from env
);

// Send private transaction
let bundle = BundleRequest::new()
    .push_transaction(tx.rlp())
    .set_block(target_block);

let response = flashbots.send_bundle(&bundle).await?;
```

### 8.2 Price Oracles

```rust
// Chainlink
let price_feed = IAggregator::new(price_feed_address, client);
let round_data = price_feed.latest_round_data().call().await?;
let price = round_data.answer;

// Uniswap TWAP
let oracle = IUniswapV3Oracle::new(factory, client);
let twap = oracle.observe(pool, [3600, 0]).await?;
```

### 8.3 Block Builders

```rust
// Eden Network
let eden = EdenClient::new("https://api.edennetwork.io/v1/rpc");

// Send bundle with Eden
let bundle = EdenBundle::new()
    .txs(vec![tx1, tx2])
    .block_number(target_block)
    .min_timestamp(now)
    .max_timestamp(now + 60);

eden.send_bundle(bundle).await?;
```

## 9. CAPACIDAD DE RECUPERACIÓN Y DEBUGGING

### 9.1 Post-Mortem Analysis

```rust
// Structured panic info
#[derive(Debug)]
struct PanicInfo {
    timestamp: DateTime<Utc>,
    thread: String,
    payload: String,
    backtrace: Backtrace,
    system_state: SystemSnapshot,
}

impl PanicInfo {
    fn save_to_disk(&self) -> std::io::Result<()> {
        let path = format!("/var/crash/arbx-{}.json", self.timestamp.timestamp());
        let file = File::create(path)?;
        serde_json::to_writer_pretty(file, self)?;
        Ok(())
    }
}

// Custom panic hook
std::panic::set_hook(Box::new(|info| {
    let panic_info = PanicInfo::capture(info);
    panic_info.save_to_disk().unwrap();
    // Notify on-call
    alert_pagerduty("Critical: Arbitrage engine panic");
}));
```

### 9.2 Replay Debugging

```rust
// Capture state for replay
#[derive(Serialize, Deserialize)]
struct ReplayBundle {
    block_number: u64,
    block_hash: H256,
    opportunities: Vec<Opportunity>,
    simulations: Vec<SimulationResult>,
    execution_trace: ExecutionTrace,
}

// Replay from captured state
async fn replay_bundle(bundle: ReplayBundle) -> Result<ReplayResult, Error> {
    // Fork at specific block
    let anvil = Anvil::new()
        .fork(rpc_url)
        .fork_block_number(bundle.block_number)
        .spawn();
    
    // Replay transactions
    for tx in bundle.execution_trace.txs {
        anvil.send_transaction(tx).await?;
    }
    
    // Compare results
    Ok(ReplayResult::compare(&bundle, &actual))
}
```

## 10. CHECKLIST DE DEPLOYMENT A PRODUCCIÓN

### Pre-Deploy
- [ ] Todos los tests pasan (unit + integration + e2e)
- [ ] Clippy sin warnings (`cargo clippy -- -D warnings`)
- [ ] Formateo correcto (`cargo fmt --check`)
- [ ] Security audit (`cargo audit`)
- [ ] Benchmarks de performance ejecutados
- [ ] Documentación actualizada
- [ ] CHANGELOG.md actualizado
- [ ] Versión bumped (semver)

### Deploy
- [ ] Backup de base de datos
- [ ] Feature flags configurados (gradual rollout)
- [ ] Monitoreo activo (dashboards, alerts)
- [ ] Kill switch verificado
- [ ] Rollback plan documentado

### Post-Deploy
- [ ] Smoke tests pasan
- [ ] Métricas dentro de rangos normales
- [ ] Sin errores en logs (24h)
- [ ] Profit/loss tracking activo

---

**AGENTE: Este núcleo te da la base para construir, desplegar y operar motores de arbitrage de clase mundial. Las referencias 11-22 de la biblioteca (`references/biblioteca/`) extienden cada área en profundidad de nivel productivo.**

## ERRATAS CONOCIDAS DEL NÚCLEO (post-auditoría 2026-09-15, crítico de completitud)

El contenido arriba se preserva íntegro como fundación histórica v1.0.0. Estas son las
correcciones que un lector DEBE aplicar mentalmente (la referencia citada es autoritativa):

1. **§2.1 (`require(netProfit >= maxLossPerTrade, "Loss exceeds max")`)**: línea muerta/confusa
   tal como está — mezcla umbral de ganancia mínima con umbral de pérdida máxima y no cumple
   ninguna de las dos. La guarda real canónica es el delta-check de solvencia de la
   referencia 14 §14.1: `output >= deuda_flash + minProfit + gasBuffer`, con la pérdida
   máxima controlada off-chain por los risk limits (referencia 18). No compiles desde el
   esqueleto del núcleo sin este ajuste.
2. **§4.2 (Kubernetes)**: conocimiento general de despliegue. Para ESTE repo la referencia
   autoritativa de deploy es la 22 (Docker Compose sobre VPS único, `--env-file` explícito,
   rebuild `--no-cache` cuando el env se hornea en build, deploy veraz SHA==HEAD, verificación
   L4). No provisions k8s para este proyecto.
3. **§2.1/§5-§8 vs referencias 11-22**: donde el núcleo y una referencia profunda discrepen
   (firmas de API, parámetros, cifras), gana SIEMPRE la referencia 11-22: fue verificada
   adversarialmente contra fuentes primarias en la pasada de 2026-09-15.
