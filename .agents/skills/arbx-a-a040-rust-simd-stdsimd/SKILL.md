name: arbx-a-a040-rust-simd-stdsimd
description: "PhD/Master skill alineada al monorepo hefarica/arbitragex-v2 (Rust Systems & Searcher Engine). Se activa cuando: AVX2/AVX-512 lane ops. Doctrina OMEGA Zero-Mocks · Fail-Closed · El Remoto Manda."
---

# A040 — Rust SIMD (std::simd)

> **Dominio:** Rust Systems & Searcher Engine
> **Nivel:** PhD/Master
> **Trigger:** AVX2/AVX-512 lane ops
> **Repos de referencia:** paradigmxyz/reth · tokio-rs/{tokio,axum} · launchbadge/sqlx · alloy-rs/alloy · foundry-rs/foundry · paritytech/jsonrpsee · bluealloy/revm
> **Archivos del repo:** services/searcher-rs/* · services/sim-ctl/* · services/executor-rs/* · services/mempool-listener-rs/* · backend/api-server/src/routes/status.ts

## Quick Start

```bash
cargo build --release --workspace
cargo nextest run --workspace --no-fail-fast
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

- `AVX2/AVX-512 lane ops`
- `rust simd (std::simd)`
- `arbitragex-v2 a`

## Output Esperado

- Diff atómico (rustfmt aparte de semántica).
- Evidencia: `gh pr checks <N>`, `cargo nextest`, traces OTel, screenshots Playwright.
- ADR si la decisión cambia contrato (`docs/adr/NNNN-*.md`).


---