name: arbx-a-a039-applied-rust-trait-objects-vs-generics-in-arbitragex-v2
description: "Staff Engineer skill alineada al monorepo hefarica/arbitragex-v2 (Rust Systems & Searcher Engine). Se activa cuando: multi-chain Provider trait hierarchy. Doctrina OMEGA Zero-Mocks · Fail-Closed · El Remoto Manda."
---

# A039 — Applied Rust Trait Objects vs Generics in arbitragex-v2

> **Dominio:** Rust Systems & Searcher Engine
> **Nivel:** Staff Engineer
> **Trigger:** multi-chain Provider trait hierarchy
> **Repos de referencia:** paradigmxyz/reth · tokio-rs/{tokio,axum} · launchbadge/sqlx · alloy-rs/alloy · foundry-rs/foundry · paritytech/jsonrpsee · bluealloy/revm
> **Archivos del repo:** services/searcher-rs/* · services/sim-ctl/* · services/executor-rs/* · services/mempool-listener-rs/* · backend/api-server/src/routes/status.ts

## Quick Start

```bash
cargo build --release --workspace
cargo nextest run --workspace --no-fail-fast
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

- `multi-chain Provider trait hierarchy`
- `applied rust trait objects vs generics in arbitragex-v2`
- `arbitragex-v2 a`

## Output Esperado

- Diff atómico (rustfmt aparte de semántica).
- Evidencia: `gh pr checks <N>`, `cargo nextest`, traces OTel, screenshots Playwright.
- ADR si la decisión cambia contrato (`docs/adr/NNNN-*.md`).


---