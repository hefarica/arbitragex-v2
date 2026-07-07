name: arbx-c-c015-applied-jit-liquidity-uni-v3-in-arbitragex-v2
description: "Staff Engineer skill alineada al monorepo hefarica/arbitragex-v2 (MEV, Mempool & Bundle Submission). Se activa cuando: JIT with revert protection. Doctrina OMEGA Zero-Mocks · Fail-Closed · El Remoto Manda."
---

# C015 — Applied JIT Liquidity (Uni V3) in arbitragex-v2

> **Dominio:** MEV, Mempool & Bundle Submission
> **Nivel:** Staff Engineer
> **Trigger:** JIT with revert protection
> **Repos de referencia:** flashbots/{mev-boost,builder,mev-share,suave-geth} · paradigmxyz/{mev-rs,artemis} · jito-foundation/jito-relayer · cowprotocol/services · 1inch/fusion-sdk
> **Archivos del repo:** services/searcher-rs/strategies/* · services/mempool-listener-rs/* · services/executor-rs/bundle/*

## Quick Start

```bash
cargo run --release -p searcher-rs -- --rpc-url $RPC_URL_MAINNET
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

- `JIT with revert protection`
- `applied jit liquidity (uni v3) in arbitragex-v2`
- `arbitragex-v2 c`

## Output Esperado

- Diff atómico (rustfmt aparte de semántica).
- Evidencia: `gh pr checks <N>`, `cargo nextest`, traces OTel, screenshots Playwright.
- ADR si la decisión cambia contrato (`docs/adr/NNNN-*.md`).


---