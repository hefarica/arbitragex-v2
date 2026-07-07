name: arbx-c-c005-advanced-tx-decoding-selector-matching
description: "Post-Doc skill alineada al monorepo hefarica/arbitragex-v2 (MEV, Mempool & Bundle Submission). Se activa cuando: proxy detection ABI. Doctrina OMEGA Zero-Mocks · Fail-Closed · El Remoto Manda."
---

# C005 — Advanced Tx Decoding & Selector Matching

> **Dominio:** MEV, Mempool & Bundle Submission
> **Nivel:** Post-Doc
> **Trigger:** proxy detection ABI
> **Repos de referencia:** flashbots/{mev-boost,builder,mev-share,suave-geth} · paradigmxyz/{mev-rs,artemis} · jito-foundation/jito-relayer · cowprotocol/services · 1inch/fusion-sdk
> **Archivos del repo:** services/searcher-rs/strategies/* · services/mempool-listener-rs/* · services/executor-rs/bundle/*

## Quick Start

```bash
cargo run --release -p searcher-rs -- --rpc-url $RPC_URL_MAINNET
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

- `proxy detection ABI`
- `advanced tx decoding & selector matching`
- `arbitragex-v2 c`

## Output Esperado

- Diff atómico (rustfmt aparte de semántica).
- Evidencia: `gh pr checks <N>`, `cargo nextest`, traces OTel, screenshots Playwright.
- ADR si la decisión cambia contrato (`docs/adr/NNNN-*.md`).


---