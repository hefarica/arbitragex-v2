# Redis governance — OMEGA-8 / M3 Capa 2

> Status: locked in for the M3 milestone (2026-05-15).
> Owner: api-server + searcher-rs + selector-api maintainers.

This runbook documents the **Redis usage contract** that every backend
service must honor. It is the single-page reference operators consult
before changing a key, channel, stream, or TTL.

---

## TLS / multi-host posture

Current deployment: **single-instance Redis on a private docker network**.
URLs use `redis://` (no TLS). This is acceptable because:

- All clients (api-server, searcher-rs, recon, relays-client, selector-api,
  token-enricher) live in the same docker compose project.
- The Redis container is **not** exposed on a host port in production
  compose files (verify with `docker compose -f docker/compose.prod.yml
  config | grep -A2 redis`).

**If multi-host Redis is ever needed:**
- Switch to `rediss://`.
- Provision TLS certs via the existing `age`-encrypted secret pipeline
  (`docs/runbooks/rotate-secrets.md`).
- Enable AUTH (`requirepass`) in `redis.conf`.
- Update boot validators in each Rust binary so the absence of TLS in a
  multi-host config is a fail-fast condition (mirrors the `assertSecureBootTokens`
  pattern in shared-ts).

---

## Channels (pub/sub)

| Channel                                  | Producer                              | Consumer(s)                                     | Payload notes                                       |
|------------------------------------------|---------------------------------------|-------------------------------------------------|-----------------------------------------------------|
| `arbx:killswitch:changes`                | `shared-rs/src/killswitch.rs`         | every Rust binary, api-server                   | `KillSwitchState` JSON                              |
| `arbx:cb:admin`                          | `api-server` admin endpoints          | selector-api                                    | `{action, name, reason}`                            |
| `arbx:trading_config:changes`            | `api-server` trading-config routes    | searcher-rs, sim-ctl                            | `TradingConfigState`                                |
| `arbx:config:hot_reload`                 | `api-server` trading-config routes    | legacy SOP-EDGE-001 subs                        | bridge alias of the previous channel                |
| `arbx:config:chains:reload`              | `api-server` admin-chains             | searcher-rs `config_reload.rs`                  | `ChainReloadEvent`                                  |
| `arbx:signals:convergence`               | searcher-rs telemetry_publisher       | api-server (→ WSS `convergence` room)            | `ConvergenceSignal`                                 |
| `arbx:papermode:changes`                 | `api-server` admin paper-mode         | searcher-rs via TradingConfigClient             | `{enabled, updated_at, updated_by, chain_id}`       |

**Rule:** every channel name MUST start with `arbx:` and use colon
separators. Channels MUST NOT carry secrets in the payload — only state
labels and IDs.

---

## Streams (pipeline)

| Stream                  | Producer        | Consumer group  | Consumer       | Semantics                                       |
|-------------------------|-----------------|-----------------|----------------|-------------------------------------------------|
| `arbx:opps:detected`    | searcher-rs     | `selector-g0`   | selector-1     | at-least-once; XACK only after PG persist       |
| `arbx:opps:validated`   | selector-api    | `sim-g0`        | sim-ctl        | idem                                            |
| `arbx:opps:simulated`   | sim-ctl         | `relays-g0`     | relays-client  | idem                                            |
| `arbx:opps:executed`    | relays-client   | `recon-g0`      | recon          | idem                                            |

**Trim policy:** `MAXLEN ~ 10000` on `arbx:opps:detected` (other streams
inherit by audit). Validation errors → XACK + drop. XPENDING is monitored
by the heartbeat worker; alerts at > 500 entries.

---

## Cache keys

All keys live under the `arbx:` prefix. The current set:

| Pattern                                                | TTL                          | Purpose                                    |
|--------------------------------------------------------|------------------------------|--------------------------------------------|
| `arbx:killswitch`                                      | none (state, not cache)      | kill-switch persisted state                |
| `arbx:trading_config:{chain_id}`                       | none                         | per-chain operator config                  |
| `arbx:papermode:{chain_id}`                            | none                         | per-chain paper-mode flag                  |
| `arbx:papermode`                                       | none (deprecated)            | legacy global flag                         |
| `arbx:blacklist:tokens:{chain_id}`                     | none (Set)                   | token blacklist                            |
| `arbx:blacklist:tokens:{chain_id}:{addr}:ttl`          | user-supplied SETEX          | per-entry TTL mirror                       |
| `arbx:heartbeat:scanner:{chain_id}:latest`             | 3× heartbeat (default 180s)  | operator readiness                         |
| `arbx:circuit_breaker:state`                           | none (reserved)              | future CB state persistence                |

**Rules:**

1. Keys with `:state` or `:latest` in the name MUST be operator-persisted
   (no TTL). Loss = config gone, not just cold cache.
2. Keys with `:ttl` or `:cache` MUST have a TTL via `SETEX` or `EXPIRE`
   alongside the `SET`.
3. **`KEYS *` is FORBIDDEN in hot paths.** Use `SCAN MATCH` with a cursor.
   Verified at audit: grep `\.keys\(` returns zero Redis hits in
   backend/api-server and backend/selector-api (only JS `Object.keys`).
4. New keys MUST be added to the table above before merge.

---

## OMEGA-8 / M3 (2026-05-15) recap

- **No Redis schema changes** were required to close Capa 2 P0/P1. The
  POST/GET `/api/system/runtime-ack` flow uses PostgreSQL exclusively.
- **No new channels** were added.
- This document is the resolution for FASE 10 of the M3 plan.

If a future PR adds a new Redis surface, the author MUST update this
document and the channel/stream tables above. CI grep gates (FASE 11)
enforce the `arbx:` prefix on string literals matching `redis://`-like
patterns.
