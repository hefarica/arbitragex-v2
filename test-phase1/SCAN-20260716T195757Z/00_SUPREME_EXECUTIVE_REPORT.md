# Supreme Repository + VPS Architecture Audit

## Verdict

```text
PARTIAL-GO
```

- Scan ID: `SCAN-20260716T195757Z`
- Repository SHA: `aa28fee4ff421bc907b16358418cb1f5505887c4`
- VPS SHA: `UNKNOWN`
- Weighted maturity: **82.4%**
- Previous maturity: **82.4%**
- Delta: **+0.0%**
- Duration: `6.57s`
- Read-only safety: **ENFORCED**

## Conformity

| VERIFIED | DRIFT | MISSING | BROKEN | BLOCKED | UNKNOWN | EXTRA |
|---|---|---|---|---|---|---|
| 117 | 0 | 0 | 0 | 0 | 25 | 0 |

## First actions by architectural unlock

| Priority | Node | Status | Unlock | Blocked by | Recommendation |
|---|---|---|---|---|---|
| P2 | redis | UNKNOWN | 63.0 |  |  |
| P2 | postgres | UNKNOWN | 54.0 |  |  |
| P2 | api-server | UNKNOWN | 16.0 |  |  |
| P2 | minio | UNKNOWN | 15.0 |  |  |
| P2 | edge | UNKNOWN | 12.0 |  |  |
| P2 | HOST:repo-parity | UNKNOWN | 10 |  |  |
| P2 | prometheus | UNKNOWN | 9.0 |  |  |
| P2 | HOST:docker-engine | UNKNOWN | 8 |  |  |
| P2 | VPS:host | UNKNOWN | 8 |  |  |
| P2 | relays-client | UNKNOWN | 8.0 |  |  |
| P2 | searcher-rs | UNKNOWN | 8.0 |  |  |
| P2 | selector-api | UNKNOWN | 8.0 |  |  |
| P2 | sim-ctl | UNKNOWN | 8.0 |  |  |
| P2 | vault | UNKNOWN | 7.0 |  |  |
| P2 | frontend | UNKNOWN | 6.0 |  |  |
| P2 | loki | UNKNOWN | 6.0 |  |  |
| P2 | thanos-sidecar | UNKNOWN | 6.0 |  |  |
| P2 | thanos-store | UNKNOWN | 6.0 |  |  |
| P2 | anvil | UNKNOWN | 5.0 |  |  |
| P2 | recon | UNKNOWN | 5.0 |  |  |
| P2 | token-enricher | UNKNOWN | 5.0 |  |  |
| P2 | alertmanager | UNKNOWN | 3.0 |  |  |
| P2 | grafana | UNKNOWN | 3.0 |  |  |
| P2 | promtail | UNKNOWN | 3.0 |  |  |
| P2 | thanos-query | UNKNOWN | 3.0 |  |  |

## Trust boundary

The audit did not modify the repository, VPS, Docker, database, Redis, services,
CI/CD, secrets, firewall or deployment state. Repository URLs are cloned only
into an isolated audit workspace. Existing local repositories are opened without
checkout, reset, pull, clean or file writes.
