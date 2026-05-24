name: arbx-m-m009-applied-branch-protection-bypass-hotfix-playbook-in-arbitragex-v2
description: "Staff Engineer skill alineada al monorepo hefarica/arbitragex-v2 (Doctrine, Process & Release Engineering). Se activa cuando: one-button revert. Doctrina OMEGA Zero-Mocks · Fail-Closed · El Remoto Manda."
---

# M009 — Applied Branch Protection Bypass Hotfix Playbook in arbitragex-v2

> **Dominio:** Doctrine, Process & Release Engineering
> **Nivel:** Staff Engineer
> **Trigger:** one-button revert
> **Repos de referencia:** conventional-commits/conventionalcommits.org · semantic-release · diataxis.fr · adr-tools
> **Archivos del repo:** .github/PULL_REQUEST_TEMPLATE.md · docs/adr/* · docs/runbooks/*

## Quick Start

```bash
git rebase -i origin/main && pre-commit run --all-files
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

- `one-button revert`
- `applied branch protection bypass hotfix playbook in arbitragex-v2`
- `arbitragex-v2 m`

## Output Esperado

- Diff atómico (rustfmt aparte de semántica).
- Evidencia: `gh pr checks <N>`, `cargo nextest`, traces OTel, screenshots Playwright.
- ADR si la decisión cambia contrato (`docs/adr/NNNN-*.md`).


---