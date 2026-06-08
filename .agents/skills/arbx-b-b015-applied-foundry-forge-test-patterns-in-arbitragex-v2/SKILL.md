name: arbx-b-b015-applied-foundry-forge-test-patterns-in-arbitragex-v2
description: "Staff Engineer skill alineada al monorepo hefarica/arbitragex-v2 (Smart Contracts, Solidity & Auditing). Se activa cuando: mainnet vs Arbitrum fork tests. Doctrina OMEGA Zero-Mocks · Fail-Closed · El Remoto Manda."
---

# B015 — Applied Foundry Forge Test Patterns in arbitragex-v2

> **Dominio:** Smart Contracts, Solidity & Auditing
> **Nivel:** Staff Engineer
> **Trigger:** mainnet vs Arbitrum fork tests
> **Repos de referencia:** OpenZeppelin/openzeppelin-contracts · Vectorized/solady · transmissions11/solmate · foundry-rs/foundry · a16z/halmos · crytic/{slither,echidna} · Certora/CVL · Uniswap/{v3-core,v4-core} · curvefi/curve · balancer-labs/balancer-v2-monorepo · aave/aave-v3-core · pyth-network · smartcontractkit/chainlink
> **Archivos del repo:** contracts/src/* · contracts/test/* · contracts/foundry.toml · contracts/script/*

## Quick Start

```bash
forge build --sizes && forge test -vvv && forge coverage --report lcov
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

- `mainnet vs Arbitrum fork tests`
- `applied foundry forge test patterns in arbitragex-v2`
- `arbitragex-v2 b`

## Output Esperado

- Diff atómico (rustfmt aparte de semántica).
- Evidencia: `gh pr checks <N>`, `cargo nextest`, traces OTel, screenshots Playwright.
- ADR si la decisión cambia contrato (`docs/adr/NNNN-*.md`).


---