name: arbx-g-g004-react-19-usetransitionuseoptimistic
description: "PhD/Master skill alineada al monorepo hefarica/arbitragex-v2 (Frontend Doctrinal (Next.js + Playwright)). Se activa cuando: concurrent rendering. Doctrina OMEGA Zero-Mocks · Fail-Closed · El Remoto Manda."
---

# G004 — React 19 useTransition/useOptimistic

> **Dominio:** Frontend Doctrinal (Next.js + Playwright)
> **Nivel:** PhD/Master
> **Trigger:** concurrent rendering
> **Repos de referencia:** vercel/next.js · facebook/react · microsoft/playwright · TanStack/query · shadcn-ui/ui · radix-ui/primitives · wevm/viem · wagmi-dev/wagmi
> **Archivos del repo:** frontend/app/* · frontend/components/* · frontend/tests/e2e/* · packages/schemas/*

## Quick Start

```bash
ARBX_BUILD_FOR_LOCAL_E2E=1 ARBX_ASSUME_NO_RPC=1 pnpm playwright test --reporter=list,html
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

- `concurrent rendering`
- `react 19 usetransition/useoptimistic`
- `arbitragex-v2 g`

## Output Esperado

- Diff atómico (rustfmt aparte de semántica).
- Evidencia: `gh pr checks <N>`, `cargo nextest`, traces OTel, screenshots Playwright.
- ADR si la decisión cambia contrato (`docs/adr/NNNN-*.md`).


---