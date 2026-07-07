name: arbx-f-f032-advanced-bullmq-nats-jetstream-redis-streams
description: "Post-Doc skill alineada al monorepo hefarica/arbitragex-v2 (Backend Services (Node + Rust)). Se activa cuando: exactly-once research. Doctrina OMEGA Zero-Mocks · Fail-Closed · El Remoto Manda."
---

# F032 — Advanced BullMQ / NATS JetStream / Redis Streams

> **Dominio:** Backend Services (Node + Rust)
> **Nivel:** Post-Doc
> **Trigger:** exactly-once research
> **Repos de referencia:** fastify/fastify · colinhacks/zod · drizzle-team/drizzle-orm · trpc/trpc · pinojs/pino · taskforcesh/bullmq · nats-io/nats-server
> **Archivos del repo:** backend/api-server/* · backend/admin-rpc/* · packages/schemas/*

## Quick Start

```bash
pnpm --filter api-server run dev:strict
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

- `exactly-once research`
- `advanced bullmq / nats jetstream / redis streams`
- `arbitragex-v2 f`

## Output Esperado

- Diff atómico (rustfmt aparte de semántica).
- Evidencia: `gh pr checks <N>`, `cargo nextest`, traces OTel, screenshots Playwright.
- ADR si la decisión cambia contrato (`docs/adr/NNNN-*.md`).


---