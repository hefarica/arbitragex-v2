name: arbx-i-i011-advanced-loki-logql-promtail
description: "Post-Doc skill alineada al monorepo hefarica/arbitragex-v2 (Observability & SRE). Se activa cuando: pattern parsing. Doctrina OMEGA Zero-Mocks · Fail-Closed · El Remoto Manda."
---

# I011 — Advanced Loki LogQL + Promtail

> **Dominio:** Observability & SRE
> **Nivel:** Post-Doc
> **Trigger:** pattern parsing
> **Repos de referencia:** open-telemetry/opentelemetry-rust · prometheus/prometheus · grafana/{grafana,loki,tempo,mimir} · vectordotdev/vector · fluent/fluent-bit · cilium/cilium
> **Archivos del repo:** ops/grafana/dashboards/* · ops/prometheus/rules/* · services/*/src/observability/*

## Quick Start

```bash
docker compose -f ops/observability/compose.yml up -d && curl -s :9090/-/ready
```

## Core Workflow

1. Cargar contexto del repo `hefarica/arbitragex-v2` (rama actual, último PR, estado de CI).
2. Identificar el subsistema impactado dentro de los archivos del repo listados arriba.
3. Aplicar la doctrina del nivel **Post-Doc** sin desviaciones.
4. Validar contra: `lint → typecheck → build → tests → audit → E2E → deploy`.
5. Reportar evidencia en formato Markdown forense (logs, traces, métricas, hash de commit).

## Doctrina del Nivel

- Aporte de investigación: paper o RFC referenciado.
- Benchmarks reproducibles con criterio estadístico (95% CI).
- Comparativa contra estado del arte público.

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

- `pattern parsing`
- `advanced loki logql + promtail`
- `arbitragex-v2 i`

## Output Esperado

- Diff atómico (rustfmt aparte de semántica).
- Evidencia: `gh pr checks <N>`, `cargo nextest`, traces OTel, screenshots Playwright.
- ADR si la decisión cambia contrato (`docs/adr/NNNN-*.md`).


---