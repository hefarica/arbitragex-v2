name: arbx-k-k014-advanced-age-sops-for-iac-secrets
description: "Post-Doc skill alineada al monorepo hefarica/arbitragex-v2 (Security & Cryptography Applied). Se activa cuando: threshold age research. Doctrina OMEGA Zero-Mocks · Fail-Closed · El Remoto Manda."
---

# K014 — Advanced age + sops for IaC Secrets

> **Dominio:** Security & Cryptography Applied
> **Nivel:** Post-Doc
> **Trigger:** threshold age research
> **Repos de referencia:** RustCrypto/* · BLST/blst · hashicorp/vault · FiloSottile/age · getsops/sops · OWASP/CheatSheetSeries · sigstore/cosign
> **Archivos del repo:** backend/api-server/src/auth/* · contracts/src/Auth/* · scripts/age-encrypt-env.sh

## Quick Start

```bash
scripts/age-encrypt-env.sh .env > .env.age && shred -u .env
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

- `threshold age research`
- `advanced age + sops for iac secrets`
- `arbitragex-v2 k`

## Output Esperado

- Diff atómico (rustfmt aparte de semántica).
- Evidencia: `gh pr checks <N>`, `cargo nextest`, traces OTel, screenshots Playwright.
- ADR si la decisión cambia contrato (`docs/adr/NNNN-*.md`).


---