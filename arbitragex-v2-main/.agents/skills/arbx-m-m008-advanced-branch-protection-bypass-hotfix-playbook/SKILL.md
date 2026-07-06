name: arbx-m-m008-advanced-branch-protection-bypass-hotfix-playbook
description: "Post-Doc skill alineada al monorepo hefarica/arbitragex-v2 (Doctrine, Process & Release Engineering). Se activa cuando: after-action audit. Doctrina OMEGA Zero-Mocks · Fail-Closed · El Remoto Manda."
---

# M008 — Advanced Branch Protection Bypass Hotfix Playbook

> **Dominio:** Doctrine, Process & Release Engineering
> **Nivel:** Post-Doc
> **Trigger:** after-action audit
> **Repos de referencia:** conventional-commits/conventionalcommits.org · semantic-release · diataxis.fr · adr-tools
> **Archivos del repo:** .github/PULL_REQUEST_TEMPLATE.md · docs/adr/* · docs/runbooks/*

## Quick Start

```bash
git rebase -i origin/main && pre-commit run --all-files
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

- `after-action audit`
- `advanced branch protection bypass hotfix playbook`
- `arbitragex-v2 m`

## Output Esperado

- Diff atómico (rustfmt aparte de semántica).
- Evidencia: `gh pr checks <N>`, `cargo nextest`, traces OTel, screenshots Playwright.
- ADR si la decisión cambia contrato (`docs/adr/NNNN-*.md`).


---