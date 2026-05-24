name: arbx-d-d025-layerzero-v2-uln-end-to-end
description: "PhD/Master skill alineada al monorepo hefarica/arbitragex-v2 (Multi-Chain, L2 & Bridges). Se activa cuando: DVN executor pair. Doctrina OMEGA Zero-Mocks · Fail-Closed · El Remoto Manda."
---

# D025 — LayerZero V2 ULN end-to-end

> **Dominio:** Multi-Chain, L2 & Bridges
> **Nivel:** PhD/Master
> **Trigger:** DVN executor pair
> **Repos de referencia:** ethereum-optimism/optimism · OffchainLabs/nitro · scroll-tech/scroll · matter-labs/zksync-era · 0xPolygonZero/plonky3 · base-org/node · LayerZero-Labs · wormhole-foundation/wormhole · across-protocol
> **Archivos del repo:** config/chains/* · contracts/cross-chain/* · services/bridges-rs/*

## Quick Start

```bash
cargo run --release -p bridges-rs -- --chain-config config/chains/all.toml
```

## Core Workflow

1. Cargar contexto del repo `hefarica/arbitragex-v2` (rama actual, último PR, estado de CI).
2. Identificar el subsistema impactado dentro de los archivos del repo listados arriba.
3. Aplicar la doctrina del nivel **PhD/Master** sin desviaciones.
4. Validar contra: `lint → typecheck → build → tests → audit → E2E → deploy`.
5. Reportar evidencia en formato Markdown forense (logs, traces, métricas, hash de commit).

## Doctrina del Nivel

- Zero mocks, zero hardcode, zero `.unwrap()` en hot-path.
- Toda función pura testeada con property tests.
- Spec antes que código (RFC interno).

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

- `DVN executor pair`
- `layerzero v2 uln end-to-end`
- `arbitragex-v2 d`

## Output Esperado

- Diff atómico (rustfmt aparte de semántica).
- Evidencia: `gh pr checks <N>`, `cargo nextest`, traces OTel, screenshots Playwright.
- ADR si la decisión cambia contrato (`docs/adr/NNNN-*.md`).


---