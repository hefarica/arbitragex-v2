name: arbx-k-k001-secp256k1-ecdsa-schnorrbip-340
description: "PhD/Master skill alineada al monorepo hefarica/arbitragex-v2 (Security & Cryptography Applied). Se activa cuando: EIP-2 low-s. Doctrina OMEGA Zero-Mocks · Fail-Closed · El Remoto Manda."
---

# K001 — secp256k1 ECDSA + Schnorr/BIP-340

> **Dominio:** Security & Cryptography Applied
> **Nivel:** PhD/Master
> **Trigger:** EIP-2 low-s
> **Repos de referencia:** RustCrypto/* · BLST/blst · hashicorp/vault · FiloSottile/age · getsops/sops · OWASP/CheatSheetSeries · sigstore/cosign
> **Archivos del repo:** backend/api-server/src/auth/* · contracts/src/Auth/* · scripts/age-encrypt-env.sh

## Quick Start

```bash
scripts/age-encrypt-env.sh .env > .env.age && shred -u .env
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

- `EIP-2 low-s`
- `secp256k1 ecdsa + schnorr/bip-340`
- `arbitragex-v2 k`

## Output Esperado

- Diff atómico (rustfmt aparte de semántica).
- Evidencia: `gh pr checks <N>`, `cargo nextest`, traces OTel, screenshots Playwright.
- ADR si la decisión cambia contrato (`docs/adr/NNNN-*.md`).


---