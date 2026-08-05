# ArbitrageX v2 — Standard Operating Procedure (SOP)

> For operators. All commands assume SSH alias `arbx` → VPS `/opt/arbitragex-v2`.

---

## 1. Daily Health Check

```bash
# Stack health (24/24 expected, 0 unhealthy)
ssh arbx "docker ps --format 'table {{.Names}}\t{{.Status}}' | grep arbitragex"
ssh arbx "echo unhealthy=$(docker ps --filter health=unhealthy --format '{{.Names}}' | grep -c arbitragex)"

# R7 pipeline (searcher → redis → PG → API)
ssh arbx "docker exec arbitragex-v2-redis-1 redis-cli XLEN arbx:opps:detected"
ssh arbx "docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -t -c 'SELECT MAX(detected_at) FROM opportunities'"
curl -sf http://<VPS>:8787/api/health
curl -sf http://<VPS>:8787/api/killswitch/status
```

**Expected:** XLEN growing, MAX(detected_at) recent, killswitch `enabled: true` (armed).

---

## 2. Deploy (via CI/CD)

```bash
# Local: edit → test → commit → push
cd frontend && npm run typecheck    # tsc
cd backend && cargo check -p math-engine --lib  # Rust
git add <files>
git commit -m "feat(...): ..."
git push origin main

# CI runs: e2e (must pass) → auto-deploy (migration gate → build → up → health-wait)
# Monitor:
gh run watch
```

**Rules:**
- NEVER `git add .` (stages noise + churn).
- ALWAYS commit `backend/Cargo.lock` when touching Rust deps.
- `NEXT_PUBLIC_*` changes require `docker compose build --no-cache frontend`.

---

## 3. Enable §IV Motor (after Gaps 1+2 resolved)

```bash
# VPS: enable drift-tracker
ssh arbx "cd /opt/arbitragex-v2 && echo 'ARBX_DRIFT_TRACKER_MODE=on' >> .env"
ssh arbx "cd /opt/arbitragex-v2 && docker compose --env-file .env -f docker/compose.prod.yml up -d --force-recreate recon"

# Verify Y-labels flowing
ssh arbx "docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -c \
  'SELECT COUNT(*) FILTER (WHERE actual_timestamp IS NOT NULL) AS resolved, COUNT(*) AS total FROM paper_trade_runs'"

# Verify evidence capture
ssh arbx "docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -c \
  'SELECT source_context, COUNT(*) FROM scored_opportunities GROUP BY 1'"
# Expected: all 'flat_prior' until calibration runs (Stage 2b)
```

---

## 4. Emergency: Stack Down

```bash
# Restore core (fastest)
ssh arbx "cd /opt/arbitragex-v2 && docker compose --env-file .env -f docker/compose.prod.yml up -d \
  postgres redis searcher-rs api-server edge frontend token-enricher prometheus grafana"

# Then the rest
ssh arbx "cd /opt/arbitragex-v2 && docker compose --env-file .env -f docker/compose.prod.yml up -d \
  recon anvil sim-ctl math-engine relays-client selector-api loki promtail alertmanager \
  minio vault thanos-sidecar thanos-store thanos-query socket-proxy"

# Verify
ssh arbx "docker ps --format '{{.Names}}' | grep -c arbitragex"
# Expected: 24
```

---

## 5. Kill-Switch (Emergency Stop)

```bash
# Arm (fail-closed, <10ms)
TOKEN=$(ssh arbx "grep ARBX_ADMIN_TOKEN /opt/arbitragex-v2/.env | cut -d= -f2")
curl -X POST http://<VPS>:8787/api/killswitch/activate -H "x-arbx-admin-token: $TOKEN"

# Disarm (only when safe)
curl -X POST http://<VPS>:8787/api/killswitch/deactivate -H "x-arbx-admin-token: $TOKEN"
```

---

## 6. Service Control (start/stop)

```bash
# Requires ARBX_SERVICE_CONTROL=on (default OFF)
TOKEN=$(ssh arbx "grep ARBX_ADMIN_TOKEN /opt/arbitragex-v2/.env | cut -d= -f2")

# Stop a non-critical service (confirm dialog in UI)
curl -X POST http://<VPS>:8787/api/v1/admin/services/token-enricher/stop \
  -H "x-arbx-admin-token: $TOKEN"

# Start it back
curl -X POST http://<VPS>:8787/api/v1/admin/services/token-enricher/start \
  -H "x-arbx-admin-token: $TOKEN"

# Verify audit log
ssh arbx "docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -c \
  \"SELECT action, target_id, after_state, created_at FROM audit_log WHERE action LIKE 'service.%' ORDER BY created_at DESC LIMIT 5\""
```

---

## 7. Operational Invariants (daily verify)

| Invariant | Check | Expected |
|---|---|---|
| `source_context` | `SELECT DISTINCT source_context FROM scored_opportunities` | `flat_prior` (until Stage 2b) |
| `math_operator_calibration` | `SELECT COUNT(*) WHERE log_lr != 0` | `0` (until calibration) |
| Kill-switch | `curl /api/killswitch/status` | `enabled: true` |
| Paper invariant | `curl /api/wallet/safe-posture` | `live: false, capital: 0, broadcast: false` |
| Evidence capture | `SELECT COUNT(*) WHERE evidence_vector IS NOT NULL` | Growing (if scoring archiver ON) |

---

## 8. Phase Status

| Phase | Status | Gate |
|---|---|---|
| **Cold** (detection + math + UI) | ✅ Complete | 31/31 operators, 24/24 services |
| **Warm** (Y-oracle + calibration) | 🟡 In progress | Gaps 1+2 → drift-tracker → Stage 2b |
| **Live** (mainnet, capital > $0) | 🔴 Pending | Crucible 72h ≥95% + security + sign-off |

---

*Maintained alongside [HARDENING_AND_ROADMAP.md](HARDENING_AND_ROADMAP.md). Update on phase transitions.*
