# Runbook — SSH CI/CD (sin GitHub Actions)

Protocolo completo: [`docs/ops/SSH_CICD_PROTOCOL.md`](../docs/ops/SSH_CICD_PROTOCOL.md)  
Driver: [`scripts/vps/ssh-cicd-driver.sh`](../scripts/vps/ssh-cicd-driver.sh)

## 30 segundos

```bash
# 1) Código ya en GitHub
git push origin HEAD
SHA=$(git rev-parse HEAD)

# 2) Siempre dry-run primero
bash scripts/vps/ssh-cicd-driver.sh \
  --sha "$SHA" \
  --change-type relays-sim-selector \
  --dry-run

# 3) Leer repo-vps-audits/SSH-CICD-*/DRY_RUN_REPORT.md
# 4) Apply solo si aceptas el plan
bash scripts/vps/ssh-cicd-driver.sh \
  --sha "$SHA" --confirm-sha "$SHA" \
  --change-type relays-sim-selector \
  --apply --hot-path-ok --strict

# 5) Si se fue a la mierda
bash scripts/vps/ssh-cicd-driver.sh --rollback
```

## Reglas de oro

1. Dry-run por defecto. Sin `--confirm-sha` no hay apply.
2. Un SHA de 40 hex, no “main”.
3. Lock en VPS — si está ocupado, no borres a ciegas.
4. Nunca `compose down`, nunca live, nunca pisar `.env` con el del laptop.
5. Todo queda en `repo-vps-audits/SSH-CICD-<UTC>/`.
