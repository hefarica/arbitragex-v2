# ArbitrageX V2 Architecture

## System Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                        CLOUDFLARE CDN                            │
│                  (Security & Global Distribution)                │
└────────────────────────────┬────────────────────────────────────┘
                             │
┌────────────────────────────▼────────────────────────────────────┐
│                      NGINX REVERSE PROXY                         │
│              (Load Balancing & SSL Termination)                  │
└────────────────────────────┬────────────────────────────────────┘
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
┌───────▼────────┐  ┌────────▼────────┐  ┌──────▼──────────┐
│  EDGE/FRONTEND │  │   API SERVICE   │  │  SEARCHER-RS    │
│  (React/Next)  │  │  (Node.js/Rust) │  │  (Rust Engine)  │
└────────────────┘  └────────┬────────┘  └──────┬──────────┘
                             │                   │
                    ┌────────┴───────────────────┘
                    │
        ┌───────────▼──────────────┐
        │   CARTRIDGE RUNTIME      │
        │   (FASE OMEGA - Rhai)    │
        │   - Sandboxing           │
        │   - Host Bindings        │
        │   - Hot-Reload Support   │
        └───────────┬──────────────┘
                    │
        ┌───────────┴──────────────┐
        │                          │
┌───────▼──────────┐  ┌───────────▼────────┐
│   POSTGRESQL     │  │   REDIS CACHE      │
│   (Main DB)      │  │   (PubSub & Cache) │
└──────────────────┘  └────────────────────┘
```

## Data Flow: Strategy Execution

```
1. Market Data Collection
   ├─ Fetch prices from DEX APIs
   ├─ Fetch liquidity pools
   └─ Fetch gas prices

2. Cartridge Evaluation
   ├─ Load active cartridges from registry
   ├─ Execute evaluate_opportunity() for each
   └─ Filter opportunities by profit threshold

3. Opportunity Ranking
   ├─ Calculate expected profit
   ├─ Estimate gas costs
   └─ Sort by profit/risk ratio

4. Payload Building
   ├─ For top opportunity, execute build_payload()
   ├─ Generate transaction data
   └─ Estimate final profit

5. Execution
   ├─ Submit transaction to blockchain
   ├─ Monitor confirmation
   └─ Log results to audit_log

6. Hot-Reload (if needed)
   ├─ Redis PubSub notification
   ├─ Reload cartridge from storage
   └─ Update registry
```

## Cartridge Runtime Details

### Sandboxing Model

Each cartridge runs in an isolated Rhai VM:

- **Memory isolation:** 256 MB per instance
- **Execution timeout:** 5 seconds
- **No file system access:** Only through host bindings
- **No network access:** Only through host bindings

### Host Bindings

Native functions exposed to cartridges:

```
Price Functions:
├─ fetch_price(chain: string, token: string) -> float
├─ fetch_prices_batch(chain: string, tokens: [string]) -> map
└─ get_price_history(token: string, hours: int) -> [float]

Liquidity Functions:
├─ check_liquidity(pool_id: string) -> float
├─ get_pool_info(pool_id: string) -> object
└─ find_pools(token_a: string, token_b: string) -> [object]

Gas Functions:
├─ estimate_gas(chain: string, tx_type: string) -> float
├─ get_gas_price(chain: string) -> float
└─ calculate_total_cost(gas_used: int, gas_price: float) -> float

Execution Functions:
├─ execute_swap(payload: object) -> object
├─ execute_multi_swap(payloads: [object]) -> [object]
└─ simulate_transaction(payload: object) -> object

Logging Functions:
├─ log_event(message: string) -> void
├─ log_error(error: string) -> void
└─ log_metric(name: string, value: float) -> void

Data Functions:
├─ get_cached_data(key: string) -> any
├─ set_cached_data(key: string, value: any, ttl: int) -> void
└─ delete_cached_data(key: string) -> void
```

### Multi-Chain Support

Cartridges can operate across multiple chains:

```rhai
fn init_strategy() {
    return #{
        name: "multi_chain_arb",
        chains: ["ethereum", "polygon", "arbitrum"],
        supported_tokens: ["USDC", "ETH", "DAI"]
    };
}
```

Chains supported:
- Ethereum (mainnet)
- Polygon
- Arbitrum
- Optimism
- Avalanche

## Hot-Reload Pattern

### Redis PubSub Architecture

```
1. Code Change Detected
   └─ New cartridge committed to git

2. Deployment Trigger
   └─ Pull latest code

3. Redis Notification
   ├─ Publish to channel: "cartridge:reload"
   └─ Message: { cartridge: "name", version: "hash" }

4. API Service Receives
   ├─ Unload old cartridge from memory
   ├─ Load new cartridge from disk
   ├─ Update registry status
   └─ Resume execution with new version

5. Zero Downtime
   └─ In-flight transactions complete with old version
   └─ New transactions use new version
```

### Implementation Details

- **Channel:** `cartridge:reload`
- **Message Format:** JSON with cartridge name and version hash
- **Subscriber:** API service (always listening)
- **Latency:** <100ms from notification to active use

## Database Schema

### cartridge_registry

```sql
CREATE TABLE cartridge_registry (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) UNIQUE NOT NULL,
    path VARCHAR(512) NOT NULL,
    version_hash VARCHAR(64),
    status ENUM('active', 'inactive', 'error') DEFAULT 'active',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    last_executed_at TIMESTAMP,
    execution_count INT DEFAULT 0,
    error_count INT DEFAULT 0,
    metadata JSONB
);
```

### audit_log

```sql
CREATE TABLE audit_log (
    id SERIAL PRIMARY KEY,
    cartridge_name VARCHAR(255) NOT NULL,
    chain VARCHAR(50),
    status ENUM('success', 'failed', 'partial') NOT NULL,
    opportunity_type VARCHAR(100),
    profit_usd DECIMAL(18, 8),
    gas_cost_usd DECIMAL(18, 8),
    net_profit_usd DECIMAL(18, 8),
    execution_time_ms INT,
    error_message TEXT,
    transaction_hash VARCHAR(66),
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    metadata JSONB,
    FOREIGN KEY (cartridge_name) REFERENCES cartridge_registry(name)
);

CREATE INDEX idx_audit_log_cartridge ON audit_log(cartridge_name);
CREATE INDEX idx_audit_log_timestamp ON audit_log(timestamp);
CREATE INDEX idx_audit_log_status ON audit_log(status);
```

### metrics

```sql
CREATE TABLE metrics (
    id SERIAL PRIMARY KEY,
    metric_name VARCHAR(255) NOT NULL,
    metric_value FLOAT NOT NULL,
    cartridge_name VARCHAR(255),
    chain VARCHAR(50),
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    tags JSONB
);

CREATE INDEX idx_metrics_name_time ON metrics(metric_name, timestamp);
```

## Deployment Pipeline

### Stages

```
1. Development
   ├─ Create feature branch
   ├─ Write/modify cartridges
   └─ Test locally

2. Testing
   ├─ Run unit tests
   ├─ Deploy to testnet
   ├─ Validate execution
   └─ Performance benchmarking

3. Staging
   ├─ Deploy to staging environment
   ├─ Run integration tests
   ├─ Monitor for 24 hours
   └─ Validate against production data

4. Production
   ├─ Create pull request
   ├─ Code review
   ├─ Merge to main
   ├─ Trigger deployment
   ├─ Health checks
   └─ Monitor metrics

5. Rollback (if needed)
   ├─ Revert commit
   ├─ Trigger deployment
   └─ Verify recovery
```

### Deployment Commands

```bash
# Build Docker images
docker-compose build

# Push to registry
docker push arbitragex/api:latest
docker push arbitragex/searcher:latest

# Pull and restart
docker-compose pull
docker-compose up -d

# Health verification
curl https://arbitragex.example.com/health
```

## Performance Characteristics

### Execution Metrics

- **Average cartridge execution time:** 50-200ms
- **Throughput:** 100-500 opportunities evaluated per second
- **Latency (market data to execution):** 500ms-2s
- **Success rate:** 95-99% (depending on market conditions)

### Resource Usage

- **API Service:** 2-4 CPU cores, 4-8 GB RAM
- **Searcher-RS:** 4-8 CPU cores, 8-16 GB RAM
- **PostgreSQL:** 2-4 CPU cores, 8-16 GB RAM
- **Redis:** 1-2 CPU cores, 2-4 GB RAM
- **Total:** ~12-20 CPU cores, 32-64 GB RAM

### Scaling Considerations

- Horizontal scaling: Add more API instances behind load balancer
- Vertical scaling: Increase resources for Searcher-RS
- Database: Consider read replicas for high query volume
- Cache: Increase Redis memory for better hit rates

## Security Model

### Authentication & Authorization

- **API Access:** JWT tokens with role-based access control
- **Admin Operations:** Separate admin token with elevated privileges
- **SSH Access:** ED25519 keys, no password authentication
- **Database:** Separate credentials per service

### Data Protection

- **In Transit:** TLS 1.3 for all external communications
- **At Rest:** PostgreSQL encryption for sensitive data
- **Secrets:** Stored in Manus secrets, never in code or config files

### Audit Trail

- **All operations logged:** cartridge_registry updates, executions, errors
- **Immutable audit log:** audit_log table with timestamps
- **Monitoring:** Continuous health checks and alerting

### Threat Mitigation

- **Cartridge Sandboxing:** Prevents malicious code execution
- **Rate Limiting:** Protects against DoS attacks
- **Input Validation:** All inputs validated before processing
- **Network Isolation:** Services communicate over private network
