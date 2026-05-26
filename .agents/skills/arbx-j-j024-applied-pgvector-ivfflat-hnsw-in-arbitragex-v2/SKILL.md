name: arbx-j-j024-applied-pgvector-ivfflat-hnsw-in-arbitragex-v2
description: "Staff Engineer skill alineada al monorepo hefarica/arbitragex-v2 (Database & Storage (Postgres/Redis/Timescale)). Se activa cuando: opportunity dedup embedding. Doctrina OMEGA Zero-Mocks · Fail-Closed · El Remoto Manda."
---

# J024 — Applied pgvector ivfflat / HNSW in arbitragex-v2

> **Dominio:** Database & Storage (Postgres/Redis/Timescale)
> **Nivel:** Staff Engineer
> **Trigger:** opportunity dedup embedding
> **Repos de referencia:** postgres/postgres · redis/redis · timescale/timescaledb · pgvector/pgvector · supabase/supabase · pgbackrest/pgbackrest
> **Archivos del repo:** db/migrations/* · backend/api-server/src/db/* · ops/timescale/*

## Quick Start

```bash
psql $DATABASE_URL -f db/migrations/034_reconcile_tokens.sql
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

- `opportunity dedup embedding`
- `applied pgvector ivfflat / hnsw in arbitragex-v2`
- `arbitragex-v2 j`

## Output Esperado

- Diff atómico (rustfmt aparte de semántica).
- Evidencia: `gh pr checks <N>`, `cargo nextest`, traces OTel, screenshots Playwright.
- ADR si la decisión cambia contrato (`docs/adr/NNNN-*.md`).


---