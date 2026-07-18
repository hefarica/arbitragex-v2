# Docker and Runtime Conformity

| Service/Container | Status | Health | Path | Requires | Runtime evidence | Priority |
|---|---|---|---|---|---|---|
| redis | UNKNOWN | UNKNOWN |  |  | repo_path=True \| dockerfile=True \| compose=True \| container=not-observed \| health=UNKNOWN | P2 |
| postgres | UNKNOWN | UNKNOWN |  |  | repo_path=True \| dockerfile=True \| compose=True \| container=not-observed \| health=UNKNOWN | P2 |
| api-server | UNKNOWN | UNKNOWN |  | redis | repo_path=True \| dockerfile=True \| compose=True \| container=not-observed \| health=UNKNOWN | P2 |
| minio | UNKNOWN | UNKNOWN |  |  | repo_path=True \| dockerfile=True \| compose=True \| container=not-observed \| health=UNKNOWN | P2 |
| edge | UNKNOWN | UNKNOWN |  | api-server | repo_path=True \| dockerfile=True \| compose=True \| container=not-observed \| health=UNKNOWN | P2 |
| prometheus | UNKNOWN | UNKNOWN |  |  | repo_path=True \| dockerfile=True \| compose=True \| container=not-observed \| health=UNKNOWN | P2 |
| relays-client | UNKNOWN | UNKNOWN | backend | redis | repo_path=True \| dockerfile=True \| compose=True \| container=not-observed \| health=UNKNOWN | P2 |
| searcher-rs | UNKNOWN | UNKNOWN | backend | redis, postgres | repo_path=True \| dockerfile=True \| compose=True \| container=not-observed \| health=UNKNOWN | P2 |
| selector-api | UNKNOWN | UNKNOWN |  | postgres, redis | repo_path=True \| dockerfile=True \| compose=True \| container=not-observed \| health=UNKNOWN | P2 |
| sim-ctl | UNKNOWN | UNKNOWN | backend | postgres, redis | repo_path=True \| dockerfile=True \| compose=True \| container=not-observed \| health=UNKNOWN | P2 |
| vault | UNKNOWN | UNKNOWN |  |  | repo_path=True \| dockerfile=True \| compose=True \| container=not-observed \| health=UNKNOWN | P2 |
| frontend | UNKNOWN | UNKNOWN |  | edge | repo_path=True \| dockerfile=True \| compose=True \| container=not-observed \| health=UNKNOWN | P2 |
| loki | UNKNOWN | UNKNOWN |  |  | repo_path=True \| dockerfile=True \| compose=True \| container=not-observed \| health=UNKNOWN | P2 |
| thanos-sidecar | UNKNOWN | UNKNOWN |  | prometheus, minio | repo_path=True \| dockerfile=True \| compose=True \| container=not-observed \| health=UNKNOWN | P2 |
| thanos-store | UNKNOWN | UNKNOWN |  | minio | repo_path=True \| dockerfile=True \| compose=True \| container=not-observed \| health=UNKNOWN | P2 |
| anvil | UNKNOWN | UNKNOWN |  |  | repo_path=True \| dockerfile=True \| compose=True \| container=not-observed \| health=UNKNOWN | P2 |
| recon | UNKNOWN | UNKNOWN | backend | postgres | repo_path=True \| dockerfile=True \| compose=True \| container=not-observed \| health=UNKNOWN | P2 |
| token-enricher | UNKNOWN | UNKNOWN | backend | postgres, redis | repo_path=True \| dockerfile=True \| compose=True \| container=not-observed \| health=UNKNOWN | P2 |
| alertmanager | UNKNOWN | UNKNOWN |  |  | repo_path=True \| dockerfile=True \| compose=True \| container=not-observed \| health=UNKNOWN | P2 |
| grafana | UNKNOWN | UNKNOWN |  | prometheus | repo_path=True \| dockerfile=True \| compose=True \| container=not-observed \| health=UNKNOWN | P2 |
| promtail | UNKNOWN | UNKNOWN |  | loki | repo_path=True \| dockerfile=True \| compose=True \| container=not-observed \| health=UNKNOWN | P2 |
| thanos-query | UNKNOWN | UNKNOWN |  | thanos-sidecar, thanos-store | repo_path=True \| dockerfile=True \| compose=True \| container=not-observed \| health=UNKNOWN | P2 |