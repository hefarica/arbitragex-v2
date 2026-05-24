name: arbx-i-i006-applied-prometheus-histogram-buckets-in-arbitragex-v2
description: "Staff Engineer skill alineada al monorepo hefarica/arbitragex-v2 (Observability & SRE). Se activa cuando: p50/p95/p99 per strategy. Doctrina OMEGA Zero-Mocks · Fail-Closed · El Remoto Manda."
---

# I006 — Applied Prometheus Histogram Buckets in arbitragex-v2

> **Dominio:** Observability & SRE
> **Nivel:** Staff Engineer
> **Trigger:** p50/p95/p99 per strategy
> **Repos de referencia:** open-telemetry/opentelemetry-rust · prometheus/prometheus · grafana/{grafana,loki,tempo,mimir} · vectordotdev/vector · fluent/fluent-bit · cilium/cilium
> **Archivos del repo:** ops/grafana/dashboards/* · ops/prometheus/rules/* · services/*/src/observability/*

## Quick Start

```bash
docker compose -f ops/observability/compose.yml up -d && curl -s :9090/-/ready
```

## Core Workflow

1. Cargar contexto del repo `hefarica/arbitragex-v2` (rama actual, último PR, estado de CI).
2. Identificar el subsistema impactado dentro de los archivos del repo listados arriba.
3. Aplicar la doctrina del nivel **Staff Engineer** sin desviaciones.
4. Validar contra: `lint → typecheck → build → tests → audit → E2E → deploy`.
5. Reportar evidencia en formato Markdown forense (logs, traces, métricas, hash de commit).

## Doctrina del Nivel

- Cableado E2E real en `arbitragex-v2` (Zero-Mocks).
- Métricas RED expuestas en `/metrics`.
- Runbook en `docs/runbooks/` con paso de rollback.

## Reglas de Ejecución

| Condición | Acción |
|-----------|--------|
| CI rojo en `main` | ABORT — fix-forward, nunca skip |
| Drift schema producer↔consumer | ABORT — restaurar isomorfismo Zod/struct |
| Cobertura E2E < contrato iter 18 | ABORT — añadir testid + assertion DEGRADED/UP/DOWN |
| Secret en plaintext fuera de Vault/age | ABORT — rotar y reportar |
| Profit neto ≤ gas + flashloan_fee | ABORT (skill C/E) |
| Slippage > banda RiskGate | RESIZE o ABORT (skill C/E) |

## Activation Triggers (regex parciales)

- `p50/p95/p99 per strategy`
- `applied prometheus histogram buckets in arbitragex-v2`
- `arbitragex-v2 i`

## Output Esperado

- Diff atómico (rustfmt aparte de semántica).
- Evidencia: `gh pr checks <N>`, `cargo nextest`, traces OTel, screenshots Playwright.
- ADR si la decisión cambia contrato (`docs/adr/NNNN-*.md`).


---