name: arbx-c-c055-frontrunning-detection-defensive
description: "PhD/Master skill alineada al monorepo hefarica/arbitragex-v2 (MEV, Mempool & Bundle Submission). Se activa cuando: victim simulation. Doctrina OMEGA Zero-Mocks · Fail-Closed · El Remoto Manda."
---

# C055 — Frontrunning Detection (Defensive)

> **Dominio:** MEV, Mempool & Bundle Submission
> **Nivel:** PhD/Master
> **Trigger:** victim simulation
> **Repos de referencia:** flashbots/{mev-boost,builder,mev-share,suave-geth} · paradigmxyz/{mev-rs,artemis} · jito-foundation/jito-relayer · cowprotocol/services · 1inch/fusion-sdk
> **Archivos del repo:** services/searcher-rs/strategies/* · services/mempool-listener-rs/* · services/executor-rs/bundle/*

## Quick Start

```bash
cargo run --release -p searcher-rs -- --rpc-url $RPC_URL_MAINNET
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

- `victim simulation`
- `frontrunning detection (defensive)`
- `arbitragex-v2 c`

## Output Esperado

- Diff atómico (rustfmt aparte de semántica).
- Evidencia: `gh pr checks <N>`, `cargo nextest`, traces OTel, screenshots Playwright.
- ADR si la decisión cambia contrato (`docs/adr/NNNN-*.md`).


---