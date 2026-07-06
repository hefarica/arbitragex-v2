name: arbx-l-l023-advanced-strategy-decay-detection
description: "Post-Doc skill alineada al monorepo hefarica/arbitragex-v2 (Risk, Backtesting & Quant Engineering). Se activa cuando: deflated Sharpe BH-FDR. Doctrina OMEGA Zero-Mocks · Fail-Closed · El Remoto Manda."
---

# L023 — Advanced Strategy Decay Detection

> **Dominio:** Risk, Backtesting & Quant Engineering
> **Nivel:** Post-Doc
> **Trigger:** deflated Sharpe BH-FDR
> **Repos de referencia:** QuantConnect/Lean · hudson-and-thames/mlfinlab · stefan-jansen/machine-learning-for-trading · paradigmxyz/cryo
> **Archivos del repo:** tools/backtester/* · ops/scripts/risk/*

## Quick Start

```bash
cargo run --release -p backtester -- --strategy $STRATEGY --window 30d
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

- `deflated Sharpe BH-FDR`
- `advanced strategy decay detection`
- `arbitragex-v2 l`

## Output Esperado

- Diff atómico (rustfmt aparte de semántica).
- Evidencia: `gh pr checks <N>`, `cargo nextest`, traces OTel, screenshots Playwright.
- ADR si la decisión cambia contrato (`docs/adr/NNNN-*.md`).


---