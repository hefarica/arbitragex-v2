name: arbx-e-e023-advanced-pre-trade-simulation-revm-fork
description: "Post-Doc skill alineada al monorepo hefarica/arbitragex-v2 (AMM Math & Quantitative DeFi). Se activa cuando: custom Inspector taint. Doctrina OMEGA Zero-Mocks · Fail-Closed · El Remoto Manda."
---

# E023 — Advanced Pre-Trade Simulation (revm fork)

> **Dominio:** AMM Math & Quantitative DeFi
> **Nivel:** Post-Doc
> **Trigger:** custom Inspector taint
> **Repos de referencia:** Uniswap/v3-core · curvefi/curve · balancer-labs/balancer-v2-monorepo · bluealloy/revm · oxc-project/clarabel-rs
> **Archivos del repo:** services/quoter-rs/* · services/sim-ctl/* · contracts/src/Math/*

## Quick Start

```bash
cargo bench -p quoter-rs --bench amm_math
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

- `custom Inspector taint`
- `advanced pre-trade simulation (revm fork)`
- `arbitragex-v2 e`

## Output Esperado

- Diff atómico (rustfmt aparte de semántica).
- Evidencia: `gh pr checks <N>`, `cargo nextest`, traces OTel, screenshots Playwright.
- ADR si la decisión cambia contrato (`docs/adr/NNNN-*.md`).


---