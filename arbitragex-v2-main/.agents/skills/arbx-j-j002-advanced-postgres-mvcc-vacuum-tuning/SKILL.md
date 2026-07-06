name: arbx-j-j002-advanced-postgres-mvcc-vacuum-tuning
description: "Post-Doc skill alineada al monorepo hefarica/arbitragex-v2 (Database & Storage (Postgres/Redis/Timescale)). Se activa cuando: logical replication. Doctrina OMEGA Zero-Mocks · Fail-Closed · El Remoto Manda."
---

# J002 — Advanced Postgres MVCC & Vacuum Tuning

> **Dominio:** Database & Storage (Postgres/Redis/Timescale)
> **Nivel:** Post-Doc
> **Trigger:** logical replication
> **Repos de referencia:** postgres/postgres · redis/redis · timescale/timescaledb · pgvector/pgvector · supabase/supabase · pgbackrest/pgbackrest
> **Archivos del repo:** db/migrations/* · backend/api-server/src/db/* · ops/timescale/*

## Quick Start

```bash
psql $DATABASE_URL -f db/migrations/034_reconcile_tokens.sql
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

- `logical replication`
- `advanced postgres mvcc & vacuum tuning`
- `arbitragex-v2 j`

## Output Esperado

- Diff atómico (rustfmt aparte de semántica).
- Evidencia: `gh pr checks <N>`, `cargo nextest`, traces OTel, screenshots Playwright.
- ADR si la decisión cambia contrato (`docs/adr/NNNN-*.md`).


---