name: arbx-h-h021-applied-trivy-container-scan-sbom-in-arbitragex-v2
description: "Staff Engineer skill alineada al monorepo hefarica/arbitragex-v2 (CI/CD Doctrinal & DevSecOps). Se activa cuando: deploy-vps pre-step gate. Doctrina OMEGA Zero-Mocks · Fail-Closed · El Remoto Manda."
---

# H021 — Applied Trivy Container Scan + SBOM in arbitragex-v2

> **Dominio:** CI/CD Doctrinal & DevSecOps
> **Nivel:** Staff Engineer
> **Trigger:** deploy-vps pre-step gate
> **Repos de referencia:** actions/runner · sigstore/cosign · slsa-framework/slsa · in-toto/in-toto · gitleaks/gitleaks · aquasecurity/trivy · github/codeql · chainguard-dev/apko
> **Archivos del repo:** .github/workflows/* · docker/* · scripts/omega-secrets-bootstrap.sh

## Quick Start

```bash
gh workflow run e2e.yml --ref $(git rev-parse --abbrev-ref HEAD) && gh run watch
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

- `deploy-vps pre-step gate`
- `applied trivy container scan + sbom in arbitragex-v2`
- `arbitragex-v2 h`

## Output Esperado

- Diff atómico (rustfmt aparte de semántica).
- Evidencia: `gh pr checks <N>`, `cargo nextest`, traces OTel, screenshots Playwright.
- ADR si la decisión cambia contrato (`docs/adr/NNNN-*.md`).


---