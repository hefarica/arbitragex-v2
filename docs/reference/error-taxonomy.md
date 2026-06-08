# Error Taxonomy

ArbitrageX v2 uses a structured error taxonomy based on the **R8 Fail-Honest** doctrine. Every error carries an `R8` code prefix, a category, a severity level, and recovery guidance.

---

## Error Code Format

```
R8-[CATEGORY]-[SEQUENCE]
```

| Segment | Format | Example |
|---------|--------|---------|
| Prefix | `R8` | R8 (always present) |
| Category | Single letter | `E` = Execution, `N` = Network, `S` = Strategy |
| Sequence | 3-digit number | `001`, `002` |

Full example: `R8-E-001` = Execution error, sequence 1.

---

## Error Categories

| Category | Code | Description | Examples |
|----------|------|-------------|----------|
| **Execution** | `E` | Trade execution failures | Reverts, gas errors, slippage exceeded |
| **Network** | `N` | RPC and connectivity issues | Timeout, 429 rate limit, connection drop |
| **Strategy** | `S` | Strategy logic errors | Invalid parameters, threshold violations |
| **Simulation** | `M` | Ghost Protocol / REVM errors | State fork failure, cache miss |
| **System** | `Y` | Infrastructure failures | Database error, Redis down, container crash |
| **Input** | `I` | Request validation errors | Invalid parameters, missing fields |
| **Security** | `C` | Authentication/authorization | Invalid API key, rate limit exceeded |

---

## Error Registry

### Execution Errors (`R8-E-xxx`)

| Code | Message | Severity | Recovery |
|------|---------|----------|----------|
| `R8-E-001` | Transaction reverted on-chain | High | Check `revert_reason`; verify pool liquidity |
| `R8-E-002` | Slippage tolerance exceeded | Medium | Increase `max_slippage_bps` or skip opportunity |
| `R8-E-003` | Gas estimation failed | Medium | Check `gas_price_gwei`; verify contract bytecode |
| `R8-E-004` | Bundle submission rejected by builder | High | Retry with higher priority fee or switch builder |
| `R8-E-005` | Nonce mismatch detected | Medium | Synchronize nonce with `eth_getTransactionCount` |
| `R8-E-006` | Insufficient balance for gas | Critical | Fund wallet or reduce trade size |
| `R8-E-007` | Opportunity expired before execution | Low | Normal; increase evaluation frequency |

### Network Errors (`R8-N-xxx`)

| Code | Message | Severity | Recovery |
|------|---------|----------|----------|
| `R8-N-001` | RPC endpoint timeout | High | Rotate to fallback; see [Rotate RPC](../how-to/rotate-rpc.md) |
| `R8-N-002` | RPC rate limited (429) | Medium | Backoff and retry; consider endpoint rotation |
| `R8-N-003` | RPC returned invalid JSON | Medium | Retry; if persistent, rotate endpoint |
| `R8-N-004` | WebSocket connection dropped | Medium | Auto-reconnect with exponential backoff |
| `R8-N-005` | Block drift exceeds threshold | High | Switch to backup RPC; check chain health |
| `R8-N-006` | All RPC endpoints unavailable | Critical | Enter degraded mode; queue operations |

### Strategy Errors (`R8-S-xxx`)

| Code | Message | Severity | Recovery |
|------|---------|----------|----------|
| `R8-S-001` | Strategy evaluation panicked | High | Disable strategy; check logs |
| `R8-S-002` | Invalid strategy configuration | Medium | Validate config against schema |
| `R8-S-003` | Profit below minimum threshold | Low | Normal filtering; no action needed |
| `R8-S-004` | Required protocol not available | Medium | Enable protocol in configuration |

### Simulation Errors (`R8-M-xxx`)

| Code | Message | Severity | Recovery |
|------|---------|----------|----------|
| `R8-M-001` | REVM state fork failed | High | Retry with latest block; check RPC |
| `R8-M-002` | Simulation result cache miss | Low | Recalculate; normal operation |
| `R8-M-003` | Contract bytecode not found in fork | Medium | Verify contract deployment; refresh state |
| `R8-M-004` | Simulation timeout | Medium | Reduce complexity; increase timeout |

### System Errors (`R8-Y-xxx`)

| Code | Message | Severity | Recovery |
|------|---------|----------|----------|
| `R8-Y-001` | Database connection lost | Critical | Restart `ax-postgres`; check network |
| `R8-Y-002` | Redis connection lost | High | Restart `ax-redis-primary`; check cluster |
| `R8-Y-003` | Container out of memory | Critical | Increase memory limit; reduce parallelism |
| `R8-Y-004` | Disk space critically low | Critical | Prune logs; expand volume |
| `R8-Y-005` | Health check failure | High | Check container logs; restart if needed |

### Input Errors (`R8-I-xxx`)

| Code | Message | Severity | Recovery |
|------|---------|----------|----------|
| `R8-I-001` | Invalid opportunity ID format | Low | Correct ID format |
| `R8-I-002` | Missing required field | Low | Provide required parameter |
| `R8-I-003` | Numeric value out of range | Low | Provide value within valid range |
| `R8-I-004` | Invalid chain identifier | Low | Use `ethereum`, `arbitrum`, or `base` |

### Security Errors (`R8-C-xxx`)

| Code | Message | Severity | Recovery |
|------|---------|----------|----------|
| `R8-C-001` | Invalid API key | Medium | Provide valid `X-API-Key` header |
| `R8-C-002` | Rate limit exceeded | Low | Wait for reset; reduce request rate |
| `R8-C-003` | Insufficient permissions | Medium | Request appropriate API key scope |

---

## Severity Levels

| Level | Color | Meaning | Response |
|-------|-------|---------|----------|
| **Low** | Green | Expected/normal condition | Log only; no action required |
| **Medium** | Yellow | Degraded but functional | Monitor; investigate if persistent |
| **High** | Orange | Significant impact | Immediate attention required |
| **Critical** | Red | System non-functional | Escalate immediately; automated alerts |

---

## Recovery Procedures

### Automatic Recovery

The platform attempts automatic recovery for these error categories:

| Error Category | Auto-Recovery Action | Max Retries | Backoff |
|----------------|---------------------|-------------|---------|
| `R8-N-xxx` | Switch to next RPC endpoint | 3 endpoints | Immediate |
| `R8-N-004` | WebSocket reconnection | Infinite | Exponential (1s → 60s max) |
| `R8-Y-002` | Redis reconnection | 10 | Linear (5s) |
| `R8-E-003` | Retry with fresh gas estimate | 2 | Immediate |
| `R8-E-004` | Retry with next block builder | 2 | 2s delay |

### Manual Recovery

For errors that require manual intervention:

```bash
# Check error details
curl http://localhost:3000/api/v1/system/errors?code=R8-E-001&limit=10

# Check container health
docker compose ps

# Review recent logs for the failing component
docker compose logs --tail=100 <container-name>

# Restart a specific container
docker compose restart <container-name>

# Full stack restart (last resort)
docker compose down && docker compose up -d
```

---

## Error Response Format

All API errors include the full taxonomy:

```json
{
  "error": {
    "code": "R8-N-001",
    "category": "Network",
    "severity": "High",
    "message": "RPC endpoint timeout after 5000ms",
    "details": {
      "endpoint": "https://eth-mainnet.g.alchemy.com/v2/...",
      "timeout_ms": 5000,
      "attempt": 3,
      "fallback_available": true
    },
    "recovery": {
      "auto": true,
      "action": "Switched to fallback endpoint",
      "fallback_endpoint": "https://mainnet.infura.io/v3/..."
    },
    "request_id": "req-uuid-5678",
    "timestamp": "2024-01-15T09:23:47Z"
  }
}
```
