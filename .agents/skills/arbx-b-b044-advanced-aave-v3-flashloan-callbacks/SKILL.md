name: arbx-b-b044-advanced-aave-v3-flashloan-callbacks
description: "Post-Doc skill alineada al monorepo hefarica/arbitragex-v2 (Smart Contracts, Solidity & Auditing). Se activa cuando: cross-chain Flash research. Doctrina OMEGA Zero-Mocks · Fail-Closed · El Remoto Manda."
---

# B044 — Advanced Aave V3 Flashloan Callbacks

> **Dominio:** Smart Contracts, Solidity & Auditing
> **Nivel:** Post-Doc
> **Trigger:** cross-chain Flash research
> **Repos de referencia:** OpenZeppelin/openzeppelin-contracts · Vectorized/solady · transmissions11/solmate · foundry-rs/foundry · a16z/halmos · crytic/{slither,echidna} · Certora/CVL · Uniswap/{v3-core,v4-core} · curvefi/curve · balancer-labs/balancer-v2-monorepo · aave/aave-v3-core · pyth-network · smartcontractkit/chainlink
> **Archivos del repo:** contracts/src/* · contracts/test/* · contracts/foundry.toml · contracts/script/*

## Quick Start

```bash
forge build --sizes && forge test -vvv && forge coverage --report lcov
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

- `cross-chain Flash research`
- `advanced aave v3 flashloan callbacks`
- `arbitragex-v2 b`

## Output Esperado

- Diff atómico (rustfmt aparte de semántica).
- Evidencia: `gh pr checks <N>`, `cargo nextest`, traces OTel, screenshots Playwright.
- ADR si la decisión cambia contrato (`docs/adr/NNNN-*.md`).


---