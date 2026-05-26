name: arbx-d-d011-advanced-scroll-zkevm-equivalence
description: "Post-Doc skill alineada al monorepo hefarica/arbitragex-v2 (Multi-Chain, L2 & Bridges). Se activa cuando: blob fee market. Doctrina OMEGA Zero-Mocks · Fail-Closed · El Remoto Manda."
---

# D011 — Advanced Scroll zkEVM Equivalence

> **Dominio:** Multi-Chain, L2 & Bridges
> **Nivel:** Post-Doc
> **Trigger:** blob fee market
> **Repos de referencia:** ethereum-optimism/optimism · OffchainLabs/nitro · scroll-tech/scroll · matter-labs/zksync-era · 0xPolygonZero/plonky3 · base-org/node · LayerZero-Labs · wormhole-foundation/wormhole · across-protocol
> **Archivos del repo:** config/chains/* · contracts/cross-chain/* · services/bridges-rs/*

## Quick Start

```bash
cargo run --release -p bridges-rs -- --chain-config config/chains/all.toml
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

- `blob fee market`
- `advanced scroll zkevm equivalence`
- `arbitragex-v2 d`

## Output Esperado

- Diff atómico (rustfmt aparte de semántica).
- Evidencia: `gh pr checks <N>`, `cargo nextest`, traces OTel, screenshots Playwright.
- ADR si la decisión cambia contrato (`docs/adr/NNNN-*.md`).


---