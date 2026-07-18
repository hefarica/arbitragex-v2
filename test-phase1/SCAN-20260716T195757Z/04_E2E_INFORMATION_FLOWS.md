# End-to-End Information Flows

## F1 — Experience/API

**State:** `DRIFT`

```text
frontend[UNKNOWN] → edge[UNKNOWN] → api-server[UNKNOWN] → postgres[UNKNOWN]
```

## F2 — Processing pipeline

**State:** `DRIFT`

```text
searcher-rs[UNKNOWN] → selector-api[UNKNOWN] → sim-ctl[UNKNOWN] → relays-client[UNKNOWN] → recon[UNKNOWN]
```

## F3 — Observability

**State:** `DRIFT`

```text
promtail[UNKNOWN] → loki[UNKNOWN] → grafana[UNKNOWN]
```

## F4 — Metrics

**State:** `DRIFT`

```text
prometheus[UNKNOWN] → thanos-sidecar[UNKNOWN] → minio[UNKNOWN] → thanos-query[UNKNOWN] → grafana[UNKNOWN]
```

## DEP:searcher-rs — Dependencies of searcher-rs

**State:** `DRIFT`

```text
redis[UNKNOWN] → postgres[UNKNOWN] → searcher-rs[UNKNOWN]
```

## DEP:sim-ctl — Dependencies of sim-ctl

**State:** `DRIFT`

```text
postgres[UNKNOWN] → redis[UNKNOWN] → sim-ctl[UNKNOWN]
```

## DEP:relays-client — Dependencies of relays-client

**State:** `DRIFT`

```text
redis[UNKNOWN] → relays-client[UNKNOWN]
```

## DEP:recon — Dependencies of recon

**State:** `DRIFT`

```text
postgres[UNKNOWN] → recon[UNKNOWN]
```

## DEP:selector-api — Dependencies of selector-api

**State:** `DRIFT`

```text
postgres[UNKNOWN] → redis[UNKNOWN] → selector-api[UNKNOWN]
```

## DEP:api-server — Dependencies of api-server

**State:** `DRIFT`

```text
redis[UNKNOWN] → api-server[UNKNOWN]
```

## DEP:edge — Dependencies of edge

**State:** `DRIFT`

```text
api-server[UNKNOWN] → edge[UNKNOWN]
```

## DEP:frontend — Dependencies of frontend

**State:** `DRIFT`

```text
edge[UNKNOWN] → frontend[UNKNOWN]
```

## DEP:grafana — Dependencies of grafana

**State:** `DRIFT`

```text
prometheus[UNKNOWN] → grafana[UNKNOWN]
```

## DEP:token-enricher — Dependencies of token-enricher

**State:** `DRIFT`

```text
postgres[UNKNOWN] → redis[UNKNOWN] → token-enricher[UNKNOWN]
```

## DEP:promtail — Dependencies of promtail

**State:** `DRIFT`

```text
loki[UNKNOWN] → promtail[UNKNOWN]
```

## DEP:thanos-sidecar — Dependencies of thanos-sidecar

**State:** `DRIFT`

```text
prometheus[UNKNOWN] → minio[UNKNOWN] → thanos-sidecar[UNKNOWN]
```

## DEP:thanos-store — Dependencies of thanos-store

**State:** `DRIFT`

```text
minio[UNKNOWN] → thanos-store[UNKNOWN]
```

## DEP:thanos-query — Dependencies of thanos-query

**State:** `DRIFT`

```text
thanos-sidecar[UNKNOWN] → thanos-store[UNKNOWN] → thanos-query[UNKNOWN]
```
