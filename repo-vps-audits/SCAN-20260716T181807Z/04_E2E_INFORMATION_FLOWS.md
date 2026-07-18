# End-to-End Information Flows

## F1 — Experience/API

**State:** `BROKEN`

```text
frontend[MISSING] → edge[MISSING] → api-server[MISSING] → postgres[VERIFIED]
```

## F2 — Processing pipeline

**State:** `BROKEN`

```text
searcher-rs[MISSING] → selector-api[MISSING] → sim-ctl[MISSING] → relays-client[MISSING] → recon[MISSING]
```

## F3 — Observability

**State:** `VERIFIED`

```text
promtail[VERIFIED] → loki[VERIFIED] → grafana[VERIFIED]
```

## F4 — Metrics

**State:** `VERIFIED`

```text
prometheus[VERIFIED] → thanos-sidecar[VERIFIED] → minio[VERIFIED] → thanos-query[VERIFIED] → grafana[VERIFIED]
```

## DEP:searcher-rs — Dependencies of searcher-rs

**State:** `BROKEN`

```text
redis[VERIFIED] → postgres[VERIFIED] → searcher-rs[MISSING]
```

## DEP:sim-ctl — Dependencies of sim-ctl

**State:** `BROKEN`

```text
postgres[VERIFIED] → redis[VERIFIED] → sim-ctl[MISSING]
```

## DEP:relays-client — Dependencies of relays-client

**State:** `BROKEN`

```text
redis[VERIFIED] → relays-client[MISSING]
```

## DEP:recon — Dependencies of recon

**State:** `BROKEN`

```text
postgres[VERIFIED] → recon[MISSING]
```

## DEP:selector-api — Dependencies of selector-api

**State:** `BROKEN`

```text
postgres[VERIFIED] → redis[VERIFIED] → selector-api[MISSING]
```

## DEP:api-server — Dependencies of api-server

**State:** `BROKEN`

```text
redis[VERIFIED] → api-server[MISSING]
```

## DEP:edge — Dependencies of edge

**State:** `BROKEN`

```text
api-server[MISSING] → edge[MISSING]
```

## DEP:frontend — Dependencies of frontend

**State:** `BROKEN`

```text
edge[MISSING] → frontend[MISSING]
```

## DEP:grafana — Dependencies of grafana

**State:** `VERIFIED`

```text
prometheus[VERIFIED] → grafana[VERIFIED]
```

## DEP:token-enricher — Dependencies of token-enricher

**State:** `BROKEN`

```text
postgres[VERIFIED] → redis[VERIFIED] → token-enricher[MISSING]
```

## DEP:promtail — Dependencies of promtail

**State:** `VERIFIED`

```text
loki[VERIFIED] → promtail[VERIFIED]
```

## DEP:thanos-sidecar — Dependencies of thanos-sidecar

**State:** `VERIFIED`

```text
prometheus[VERIFIED] → minio[VERIFIED] → thanos-sidecar[VERIFIED]
```

## DEP:thanos-store — Dependencies of thanos-store

**State:** `BROKEN`

```text
minio[VERIFIED] → thanos-store[BROKEN]
```

## DEP:thanos-query — Dependencies of thanos-query

**State:** `BROKEN`

```text
thanos-sidecar[VERIFIED] → thanos-store[BROKEN] → thanos-query[VERIFIED]
```
