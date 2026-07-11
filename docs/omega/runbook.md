# OMEGA Pipeline Runbook

**Last Updated:** 2026-07-11
**Version:** 2.0
**Status:** Production

## Quick Reference Commands

### Health Checks

```bash
# Edge health (fastest)
curl -s http://localhost:8787/api/v1/health | jq

# Full system status
curl -s http://localhost:8787/status | jq

# Redis connectivity
docker exec redis redis-cli PING

# PostgreSQL connectivity
docker exec postgres psql -U postgres -d arbitragex -c "SELECT 1"

# Searcher-rs health
curl -s http://localhost:9001/health | jq

# API server health
curl -s http://localhost:8080/api/health | jq

# All upstreams status
curl -s http://localhost:8080/status | jq '.upstreams'
```

### Stream Monitoring

```bash
# Check stream lengths
docker exec redis redis-cli XLEN arbx:hot:detected
docker exec redis redis-cli XLEN arbx:hot:simulated
docker exec redis redis-cli XLEN arbx:hot:paper_executed

# Check consumer group status
docker exec redis redis-cli XINFO GROUPS arbx:hot:detected
docker exec redis redis-cli XINFO CONSUMERS arbx:hot:detected paper-executor-g0

# Pending messages (backlog)
docker exec redis redis-cli XPENDING arbx:hot:detected paper-executor-g0

# Recent stream entries (last 5)
docker exec redis redis-cli XREVRANGE arbx:hot:detected + - COUNT 5
```

### Container Management

```bash
# View all running containers
docker ps --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"

# View logs for a specific service
docker logs searcher-rs --tail 100 -f
docker logs api-server --tail 100 -f
docker logs edge --tail 100 -f

# Restart a service
docker compose -f docker/compose.prod.yml restart searcher-rs

# Full stack restart
docker compose -f docker/compose.prod.yml down
docker compose -f docker/compose.prod.yml up -d

# Resource usage
docker stats --no-stream
```

## Startup Procedures

### Pre-Flight Checklist

Before starting the OMEGA Pipeline, verify:

- [ ] Vault is unsealed (`vault status` returns `sealed: false`)
- [ ] `.env` file is present and contains required variables
- [ ] RPC endpoints are responsive (`RPC_WS_1`, `RPC_HTTP_1`)
- [ ] Docker daemon is running
- [ ] Sufficient disk space (>>10GB free)
- [ ] Network connectivity to external RPC providers

### Startup Sequence

```bash
# 1. Navigate to project directory
cd /opt/arbitragex-v2

# 2. Verify environment
./scripts/verify_env.sh  # If available, else manual check

# 3. Start data plane first (foundation services)
docker compose -f docker/compose.prod.yml up -d postgres redis

# 4. Wait for data plane health
docker compose -f docker/compose.prod.yml exec postgres pg_isready -U postgres
docker compose -f docker/compose.prod.yml exec redis redis-cli PING

# 5. Start core pipeline services
docker compose -f docker/compose.prod.yml up -d searcher-rs selector-api sim-ctl

# 6. Wait for searcher-rs to initialize (30-60s)
sleep 30
docker logs searcher-rs --tail 20 | grep -i "scanner.ready\|boot.complete"

# 7. Start supporting services
docker compose -f docker/compose.prod.yml up -d recon relays-client token-enricher

# 8. Start API layer
docker compose -f docker/compose.prod.yml up -d api-server

# 9. Start edge layer
docker compose -f docker/compose.prod.yml up -d edge

# 10. Start frontend (if applicable)
docker compose -f docker/compose.prod.yml up -d frontend

# 11. Start observability stack
docker compose -f docker/compose.prod.yml up -d prometheus grafana loki promtail

# 12. Verify full startup
curl -s http://localhost:8787/status | jq '.ok'
```

### Startup Verification

```bash
# 1. All containers should be healthy
docker ps --filter "health=healthy" --format "{{.Names}}"

# 2. Redis streams should be accessible
docker exec redis redis-cli XLEN arbx:hot:detected
# Expected: 0 (empty) or positive integer (existing entries)

# 3. API should respond
curl -s http://localhost:8080/api/v1/readiness | jq '.overall'

# 4. Edge should proxy correctly
curl -s http://localhost:8787/api/v1/health | jq '.ok'

# 5. WebSocket should accept connections
# (Use browser DevTools or wscat)
wscat -c "ws://localhost:8080" -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN"
```

## Shutdown Procedures

### Graceful Shutdown

```bash
# 1. Stop the searcher first (prevents partial detections)
docker compose -f docker/compose.prod.yml stop -t 30 searcher-rs

# 2. Stop api-server (closes WebSocket connections gracefully)
docker compose -f docker/compose.prod.yml stop -t 30 api-server

# 3. Stop remaining services
docker compose -f docker/compose.prod.yml down

# 4. Verify all containers stopped
docker ps | grep arbitragex
# Should return nothing
```

### Emergency Shutdown (Kill-Switch)

```bash
# Activate kill-switch immediately (stops new executions)
curl -X POST http://localhost:8787/admin/killswitch \
  -H "Content-Type: application/json" \
  -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
  -d '{"enabled": true, "reason": "emergency_shutdown", "triggered_by": "operator"}'

# Stop all containers immediately
docker compose -f docker/compose.prod.yml down -t 0

# Verify shutdown
docker ps
```

## Health Check Commands

### Component-Specific Health

#### searcher-rs
```bash
# Health endpoint
curl -s http://localhost:9001/health | jq

# Expected response:
# {
#   "ok": true,
#   "service": "searcher-rs",
#   "scanner_state": "running",
#   "mempool_connected": true
# }

# Scanner heartbeat (last funnel state)
curl -s http://localhost:8787/api/scanner/heartbeat?chain_id=1 | jq

# Key metrics to monitor
docker logs searcher-rs --tail 100 | grep -E "(detected|simulated|error|warn)"
```

#### api-server
```bash
# Health endpoint
curl -s http://localhost:8080/api/health | jq

# Readiness (17-item checklist)
curl -s http://localhost:8080/api/v1/readiness | jq '.overall'

# Key metrics
curl -s http://localhost:8080/metrics | grep -E "(websocket_clients|opportunities_archived)"
```

#### Redis
```bash
# Basic connectivity
docker exec redis redis-cli PING
# Expected: PONG

# Memory usage
docker exec redis redis-cli INFO memory | grep used_memory_human

# Connection count
docker exec redis redis-cli INFO clients | grep connected_clients

# Slow queries
docker exec redis redis-cli SLOWLOG GET 10

# Stream info
docker exec redis redis-cli XINFO STREAM arbx:hot:detected
```

#### PostgreSQL
```bash
# Connection test
docker exec postgres psql -U postgres -d arbitragex -c "SELECT version();"

# Active connections
docker exec postgres psql -U postgres -d arbitragex -c "SELECT count(*) FROM pg_stat_activity;"

# Recent opportunities count
docker exec postgres psql -U postgres -d arbitragex -c "SELECT COUNT(*) FROM opportunities WHERE detected_at > NOW() - INTERVAL '1 hour';"

# Table sizes
docker exec postgres psql -U postgres -d arbitragex -c "SELECT schemaname, tablename, pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) AS size FROM pg_tables WHERE schemaname='public' ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC LIMIT 10;"
```

## Monitoring Queries

### Pipeline Funnel Metrics

```sql
-- Opportunities by hour (last 24h)
SELECT 
  date_trunc('hour', detected_at) as hour,
  COUNT(*) as total,
  COUNT(*) FILTER (WHERE rejection_reason IS NULL) as passed,
  COUNT(*) FILTER (WHERE rejection_reason IS NOT NULL) as rejected
FROM opportunities 
WHERE detected_at > NOW() - INTERVAL '24 hours'
GROUP BY 1 
ORDER BY 1 DESC;

-- Rejection reasons distribution (last hour)
SELECT 
  COALESCE(rejection_reason, 'PASSED') as reason,
  COUNT(*) as count
FROM opportunities 
WHERE detected_at > NOW() - INTERVAL '1 hour'
GROUP BY 1 
ORDER BY count DESC;

-- Pipeline latency percentiles (last hour)
SELECT 
  percentile_cont(0.50) WITHIN GROUP (ORDER BY pipeline_latency_ms) as p50,
  percentile_cont(0.95) WITHIN GROUP (ORDER BY pipeline_latency_ms) as p95,
  percentile_cont(0.99) WITHIN GROUP (ORDER BY pipeline_latency_ms) as p99
FROM opportunities 
WHERE detected_at > NOW() - INTERVAL '1 hour'
  AND pipeline_latency_ms IS NOT NULL;
```

### Redis Stream Metrics

```bash
# Stream rates (run twice, 10s apart, calculate delta)
docker exec redis redis-cli XLEN arbx:hot:detected

# Consumer group lag
docker exec redis redis-cli XINFO GROUPS arbx:hot:detected

# Pending messages per consumer
docker exec redis redis-cli XPENDING arbx:hot:detected paper-executor-g0 - + 100
```

### System Metrics

```bash
# Container resource usage
docker stats --no-stream --format "table {{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}\t{{.NetIO}}"

# Disk usage
df -h /var/lib/docker

# Network connections
netstat -an | grep -E "(8080|8787|9001|6379|5432)" | wc -l
```

## Troubleshooting Guide

### Symptom: No Opportunities Detected

**Diagnostic Steps:**

```bash
# 1. Check searcher-rs is running
docker ps | grep searcher-rs

# 2. Check mempool connection
docker logs searcher-rs --tail 50 | grep -i mempool

# 3. Check RPC endpoints are responsive
curl -s -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
  $RPC_HTTP_1

# 4. Check Redis is receiving detections
docker exec redis redis-cli XLEN arbx:hot:detected

# 5. Check token prices are available
docker exec redis redis-cli HGETALL arbx:token_prices:1 | head -20

# 6. Check watchlist is not empty
docker exec postgres psql -U postgres -d arbitragex -c "SELECT COUNT(*) FROM tokens WHERE is_watched = true;"
```

**Common Causes:**
| Cause | Solution |
|-------|----------|
| RPC endpoint down | Switch to backup RPC in `.env` |
| Mempool WS disconnected | Restart searcher-rs |
| Empty token watchlist | Add tokens via `/strategies` tab |
| Missing token prices | Configure price oracles (DexScreener/GeckoTerminal) |
| All tokens filtered | Adjust `min_profit_usd` threshold |

### Symptom: High Latency (>100ms)

**Diagnostic Steps:**

```bash
# 1. Check Redis latency
docker exec redis redis-cli --latency-history -i 1

# 2. Check PostgreSQL latency
docker exec postgres psql -U postgres -d arbitragex -c "SELECT now();"

# 3. Check container resources
docker stats --no-stream searcher-rs api-server redis postgres

# 4. Check network latency between containers
docker exec api-server ping -c 3 redis

# 5. Check for slow queries
docker exec postgres psql -U postgres -d arbitragex -c "SELECT query, mean_exec_time FROM pg_stat_statements ORDER BY mean_exec_time DESC LIMIT 5;"
```

**Solutions:**
- Redis latency >5ms: Check memory usage, consider Redis clustering
- PostgreSQL slow: Add indexes, increase `shared_buffers`
- High CPU: Increase container CPU limits or scale horizontally
- Network latency: Ensure containers on same Docker network

### Symptom: WebSocket Disconnections

**Diagnostic Steps:**

```bash
# 1. Check api-server WebSocket endpoint
curl -s http://localhost:8080/api/health | jq '.websocket_ok'

# 2. Check Redis Pub/Sub
docker exec redis redis-cli PUBSUB CHANNELS

# 3. Check consumer groups
docker exec redis redis-cli XINFO GROUPS arbx:hot:detected

# 4. Check api-server logs for WS errors
docker logs api-server --tail 100 | grep -i websocket
```

**Solutions:**
- Restart api-server: `docker compose restart api-server`
- Clear stuck consumer groups:
  ```bash
  docker exec redis redis-cli XGROUP DESTROY arbx:hot:detected ws-emitter-g0
  docker exec redis redis-cli XGROUP CREATE arbx:hot:detected ws-emitter-g0 $ MKSTREAM
  ```

### Symptom: PostgreSQL Connection Errors

**Diagnostic:**
```bash
# Check connection count
docker exec postgres psql -U postgres -d arbitragex -c "SELECT count(*) FROM pg_stat_activity WHERE state = 'active';"

# Check max connections
docker exec postgres psql -U postgres -d arbitragex -c "SHOW max_connections;"

# Check for idle connections
docker exec postgres psql -U postgres -d arbitragex -c "SELECT pid, usename, state, query_start FROM pg_stat_activity WHERE state = 'idle' AND query_start < NOW() - INTERVAL '5 minutes';"
```

**Solutions:**
- Increase `max_connections` in PostgreSQL config
- Add connection pooling (PgBouncer)
- Restart services with connection leaks

### Symptom: Kill-Switch Won't Disarm

**Diagnostic:**
```bash
# Check kill-switch state
curl -s http://localhost:8787/api/killswitch/status \
  -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" | jq

# Check Redis kill-switch key
docker exec redis redis-cli GET arbx:killswitch:enabled

# Check api-server logs
docker logs api-server --tail 50 | grep -i killswitch
```

**Solutions:**
- Force reset in Redis:
  ```bash
  docker exec redis redis-cli DEL arbx:killswitch:enabled
  docker exec redis redis-cli PUBLISH arbx:killswitch:changes "0"
  ```
- Restart api-server if state is inconsistent

## Incident Response Procedures

### P0 - Complete Pipeline Failure

**Symptoms:** No opportunities, all health checks failing, kill-switch armed

**Response:**
```bash
# 1. Arm kill-switch (if not already)
curl -X POST http://localhost:8787/admin/killswitch \
  -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
  -d '{"enabled": true, "reason": "p0_incident", "triggered_by": "operator"}'

# 2. Check data plane (Redis + PostgreSQL)
docker compose -f docker/compose.prod.yml ps redis postgres

# 3. If data plane is down, restore from backup
# See: docs/runbooks/db-restore.md

# 4. If data plane is up, restart core services
docker compose -f docker/compose.prod.yml restart searcher-rs api-server edge

# 5. Verify recovery
curl -s http://localhost:8787/status | jq '.ok'
```

### P1 - High Latency Degradation

**Symptoms:** Pipeline latency >100ms, opportunities timing out

**Response:**
```bash
# 1. Identify bottleneck component
# Check each service latency
curl -s http://localhost:9001/metrics | grep latency
curl -s http://localhost:8080/metrics | grep latency

# 2. If Redis bottleneck, check for large keys
docker exec redis redis-cli --bigkeys

# 3. If PostgreSQL bottleneck, check for locks
docker exec postgres psql -U postgres -d arbitragex -c "SELECT * FROM pg_locks WHERE NOT granted;"

# 4. Scale up affected service
docker compose -f docker/compose.prod.yml up -d --scale api-server=2
```

### P2 - Memory Exhaustion

**Symptoms:** Containers OOMKilled, high memory usage

**Response:**
```bash
# 1. Check which container is consuming memory
docker stats --no-stream --format "table {{.Name}}\t{{.MemPerc}}" | sort -k2 -nr

# 2. Check for memory leaks in searcher-rs
docker logs searcher-rs --tail 100 | grep -i "memory\|alloc"

# 3. Increase memory limit temporarily
docker compose -f docker/compose.prod.yml stop searcher-rs
# Edit compose.prod.yml to increase memory limit
docker compose -f docker/compose.prod.yml up -d searcher-rs

# 4. Trim Redis streams if too large
docker exec redis redis-cli XTRIM arbx:hot:detected MAXLEN 5000
```

### P3 - RPC Provider Outage

**Symptoms:** Mempool disconnected, block number stale

**Response:**
```bash
# 1. Verify RPC is down
curl -s -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
  $RPC_HTTP_1

# 2. Switch to backup RPC
# Edit .env to use RPC_WS_2, RPC_HTTP_2
docker compose -f docker/compose.prod.yml up -d --force-recreate searcher-rs

# 3. Verify reconnection
docker logs searcher-rs --tail 20 | grep -i "mempool.connected\|chain.ready"
```

## Post-Incident Verification

After any incident resolution, verify:

```bash
# 1. All health checks pass
curl -s http://localhost:8787/status | jq '.ok'  # Should be true

# 2. Opportunities are flowing
docker exec redis redis-cli XLEN arbx:hot:detected
sleep 10
docker exec redis redis-cli XLEN arbx:hot:detected  # Should increase

# 3. Latency is within SLA
curl -s http://localhost:8787/api/scanner/heartbeat | jq '.pipeline_latency_ms'

# 4. PostgreSQL is receiving data
docker exec postgres psql -U postgres -d arbitragex -c "SELECT MAX(detected_at) FROM opportunities;"

# 5. Disarm kill-switch (if appropriate)
curl -X POST http://localhost:8787/admin/killswitch \
  -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
  -d '{"enabled": false, "reason": "incident_resolved", "triggered_by": "operator"}'
```

## Related Documentation

- [Pipeline Architecture](./pipeline-architecture.md) - System design and data flow
- [Deployment Guide](./deployment-guide.md) - Environment setup
- [API Reference](./api-reference.md) - Endpoints and WebSocket events
- [Kill-Switch Runbook](../runbooks/kill-switch-activation.md) - Detailed kill-switch procedures
- [DB Restore Runbook](../runbooks/db-restore.md) - Database recovery

---

*Document maintained by OMEGA Operations Team. Update this runbook after every incident with lessons learned.*
