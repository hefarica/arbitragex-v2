name: arbx-e-e009-applied-stableswap-newton-raphson-in-arbitragex-v2
description: "Staff Engineer skill alineada al monorepo hefarica/arbitragex-v2 (AMM Math & Quantitative DeFi). Se activa cuando: Curve adapter routing. Doctrina OMEGA Zero-Mocks · Fail-Closed · El Remoto Manda."
---

# E009 — Applied StableSwap Newton-Raphson in arbitragex-v2

> **Dominio:** AMM Math & Quantitative DeFi
> **Nivel:** Staff Engineer
> **Trigger:** Curve adapter routing
> **Repos de referencia:** Uniswap/v3-core · curvefi/curve · balancer-labs/balancer-v2-monorepo · bluealloy/revm · oxc-project/clarabel-rs
> **Archivos del repo:** services/quoter-rs/* · services/sim-ctl/* · contracts/src/Math/*

## Quick Start

```bash
cargo bench -p quoter-rs --bench amm_math
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

- `Curve adapter routing`
- `applied stableswap newton-raphson in arbitragex-v2`
- `arbitragex-v2 e`

## Output Esperado

- Diff atómico (rustfmt aparte de semántica).
- Evidencia: `gh pr checks <N>`, `cargo nextest`, traces OTel, screenshots Playwright.
- ADR si la decisión cambia contrato (`docs/adr/NNNN-*.md`).


---