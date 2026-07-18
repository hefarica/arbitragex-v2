# End-to-End Information Flows

## F1 — Experience/API

**State:** `VERIFIED`

```text
frontend[VERIFIED] → edge[VERIFIED] → api-server[VERIFIED] → postgres[VERIFIED]
```

## F2 — Processing pipeline

**State:** `VERIFIED`

```text
searcher-rs[VERIFIED] → selector-api[VERIFIED] → sim-ctl[VERIFIED] → relays-client[VERIFIED] → recon[VERIFIED]
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

**State:** `VERIFIED`

```text
redis[VERIFIED] → postgres[VERIFIED] → searcher-rs[VERIFIED]
```

## DEP:sim-ctl — Dependencies of sim-ctl

**State:** `VERIFIED`

```text
postgres[VERIFIED] → redis[VERIFIED] → sim-ctl[VERIFIED]
```

## DEP:relays-client — Dependencies of relays-client

**State:** `VERIFIED`

```text
redis[VERIFIED] → relays-client[VERIFIED]
```

## DEP:recon — Dependencies of recon

**State:** `VERIFIED`

```text
postgres[VERIFIED] → recon[VERIFIED]
```

## DEP:selector-api — Dependencies of selector-api

**State:** `VERIFIED`

```text
postgres[VERIFIED] → redis[VERIFIED] → selector-api[VERIFIED]
```

## DEP:api-server — Dependencies of api-server

**State:** `VERIFIED`

```text
redis[VERIFIED] → api-server[VERIFIED]
```

## DEP:edge — Dependencies of edge

**State:** `VERIFIED`

```text
api-server[VERIFIED] → edge[VERIFIED]
```

## DEP:frontend — Dependencies of frontend

**State:** `VERIFIED`

```text
edge[VERIFIED] → frontend[VERIFIED]
```

## DEP:grafana — Dependencies of grafana

**State:** `VERIFIED`

```text
prometheus[VERIFIED] → grafana[VERIFIED]
```

## DEP:token-enricher — Dependencies of token-enricher

**State:** `VERIFIED`

```text
postgres[VERIFIED] → redis[VERIFIED] → token-enricher[VERIFIED]
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

**State:** `VERIFIED`

```text
minio[VERIFIED] → thanos-store[VERIFIED]
```

## DEP:thanos-query — Dependencies of thanos-query

**State:** `VERIFIED`

```text
thanos-sidecar[VERIFIED] → thanos-store[VERIFIED] → thanos-query[VERIFIED]
```
