# KIMI GITHUB CHANNEL SETUP
## ArbitrageX v2 — Release Handoff from Antigravity

> **Purpose:** Enable Kimi to establish a secure, read/write GitHub CLI channel
> from its sandbox environment using an operator-injected PAT, without
> recreating any workflows, secrets, or infrastructure that already exists.

---

## 1. Propósito

Este documento permite a Kimi:

- Conectarse al repositorio `hefarica/arbitragex-v2` desde su sandbox.
- Observar PR #90 y sus checks de CI/CD.
- Leer logs y artifacts de workflows fallidos.
- Ejecutar merge cuando las condiciones estén cumplidas.
- Disparar deploy a producción después del merge.
- **Sin recrear** workflows, secrets ni infraestructura SSH.

---

## 2. Requisitos

| Requisito | Detalle |
|---|---|
| Sistema operativo | Linux (Ubuntu/Debian preferido) |
| GitHub PAT | Inyectado como variable segura `GITHUB_PAT` o `GH_TOKEN` |
| Permisos del PAT | `repo`, `workflow`, `read:org` (mínimo) |
| GitHub CLI | v2.x+ |
| Repositorio | `hefarica/arbitragex-v2` (privado) |
| Branch de trabajo | `omega/recovery-20260516` |
| PR objetivo | #90 |

---

## 3. Instalación de GitHub CLI

### Ubuntu/Debian

```bash
curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg \
  | sudo dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg

echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
  | sudo tee /etc/apt/sources.list.d/github-cli.list

sudo apt update
sudo apt install gh -y
```

### Verificar

```bash
gh --version
```

Debe retornar `gh version 2.x.x` o superior.

---

## 4. Autenticación segura con PAT

### Reglas de seguridad

- **NUNCA** escribir el PAT en archivos, prompts, markdown, YAML ni logs.
- **NUNCA** imprimir el PAT.
- **NUNCA** guardarlo en `.env`, `.env.local` ni artifacts.
- El PAT debe entrar **solo** como variable de entorno segura.

### Procedimiento

El operador inyecta el token en el entorno de Kimi como variable segura:

```bash
export GITHUB_PAT="TOKEN_REAL"
```

Kimi ejecuta:

```bash
printf '%s' "$GITHUB_PAT" | gh auth login --with-token
```

Alternativa si el entorno expone `GH_TOKEN` directamente:

```bash
export GH_TOKEN="TOKEN_REAL"
gh auth status
```

### Verificación

```bash
gh auth status
gh api user --jq '.login'
gh repo view hefarica/arbitragex-v2 --json nameWithOwner,viewerPermission,isPrivate
```

**Resultado esperado:**

```
✓ Logged in to github.com account hefarica
nameWithOwner: hefarica/arbitragex-v2
viewerPermission: ADMIN
isPrivate: true
```

**Si falla:**

```
GH_AUTH_NOT_AVAILABLE
```

Detenerse. No continuar sin autenticación verificada.

---

## 5. Verificación del canal GitHub

```bash
REPO="hefarica/arbitragex-v2"

# Listar workflows activos
gh workflow list --repo "$REPO"

# Ver estado del PR
gh pr view 90 --repo "$REPO" --json headRefOid,mergeStateStatus,reviewDecision

# Ver checks
gh pr checks 90 --repo "$REPO"
```

---

## 6. Uso de workflows existentes

Los siguientes workflows **ya existen** y están activos. Kimi debe usarlos, no recrearlos:

| Workflow | Archivo | Propósito |
|---|---|---|
| e2e | `e2e.yml` | Playwright smoke tests |
| Deploy to VPS | `deploy-vps.yml` | Deploy producción via SSH |
| Hardened VPS Deploy | `hardened-vps-deploy.yml` | Deploy hardened alternativo |
| Security Scans | `security.yml` | cargo audit, gitleaks, npm audit |
| Rust CI | `rust.yml` | cargo check + clippy + test |
| TypeScript CI | `typescript.yml` | tsc --noEmit |
| Frontend Build | `frontend-build.yml` | next build (production) |
| Unit Tests | `unit-tests.yml` | Rust tests + TS tests + typecheck |
| No Hardcode | `no-hardcode.yml` | Lint anti-hardcode |
| Dockerfile Audit | `dockerfile-audit.yml` | COPY coverage audit |
| Grep Gates | `omega8-m3-grep-gates.yml` | Doctrine grep gates |
| PII Gates | `omega8-pii-gates.yml` | PII wireado recursive gates |

---

## 7. Observación de PR #90

### Watch en vivo

```bash
gh pr checks 90 --repo "$REPO" --watch --interval 30
```

### Estado detallado

```bash
gh pr view 90 --repo "$REPO" --json headRefOid,mergeStateStatus,reviewDecision,statusCheckRollup
```

### HEAD remoto esperado

```
f6c42930399ed9dd533c187f7282accbdad759f9
```

Branch: `omega/recovery-20260516`
Base: `main`

---

## 8. Lectura de logs y artifacts

Si Playwright u otro check falla:

```bash
BRANCH="omega/recovery-20260516"

# Obtener el run ID del último e2e
RUN_ID=$(gh run list --repo "$REPO" --branch "$BRANCH" --workflow "e2e" \
  --limit 1 --json databaseId --jq '.[0].databaseId')

mkdir -p ./ci-artifacts

# Descargar logs fallidos
gh run view "$RUN_ID" --repo "$REPO" --log-failed > ./ci-artifacts/failed.log || true

# Descargar artifacts
gh run download "$RUN_ID" --repo "$REPO" -D ./ci-artifacts || true

# Buscar patrones de error
grep -Ei "strict mode|fetch failed|DOWN|UP|DEGRADED|NO_RPC|edge unreachable|Application error|localhost|timeout|ECONNREFUSED|failed|error" \
  ./ci-artifacts/failed.log | head -100 || true
```

---

## 9. Reglas de no reconstrucción

### PROHIBIDO para Kimi

- ❌ Recrear `e2e.yml`
- ❌ Recrear `deploy-vps.yml`
- ❌ Recrear `hardened-vps-deploy.yml`
- ❌ Recrear `security.yml`
- ❌ Recrear cualquier workflow existente
- ❌ Recrear secrets via `gh secret set`
- ❌ Tocar `VPS_SSH_KEY`
- ❌ Reinstalar public key SSH en el VPS
- ❌ Hacer deploy desde PR (solo desde main post-merge)
- ❌ Hacer merge con `--admin`
- ❌ Usar bypass de branch protection
- ❌ Imprimir, loguear o persistir tokens/secrets

### PERMITIDO para Kimi

- ✅ Leer checks: `gh pr checks`
- ✅ Leer logs: `gh run view --log-failed`
- ✅ Leer artifacts: `gh run download`
- ✅ Leer PR: `gh pr view`
- ✅ Leer workflows: `gh workflow list`
- ✅ Leer secrets (nombres, no valores): `gh secret list`
- ✅ Corregir código si un check falla (commit + push)
- ✅ Mergear si las condiciones se cumplen
- ✅ Disparar deploy post-merge

---

## 10. Merge policy

Solo mergear PR #90 si **todas** las condiciones se cumplen:

```
✅ 13/13 checks en SUCCESS
✅ 0 checks PENDING
✅ 0 checks IN_PROGRESS
✅ 0 checks FAILURE
✅ reviewDecision = APPROVED (o branch rule ajustada formalmente)
✅ No se usa --admin
✅ No se usa bypass
```

**Si mergeStateStatus = BLOCKED y reviewDecision = REVIEW_REQUIRED:**

Reportar:

```
BLOCKED_BY_REVIEW_REQUIRED
```

No usar `--admin`. Pedir al operador que:
- Apruebe el PR desde GitHub UI, o
- Ajuste temporalmente required approvals en Settings → Branches → main.

**Comando de merge autorizado:**

```bash
gh pr merge 90 --repo "$REPO" --squash
```

---

## 11. Deploy policy

### Pre-requisitos

- PR #90 mergeado a `main`.
- No hay checks rojos en `main`.

### Ejecutar deploy

```bash
gh workflow run deploy-vps.yml --repo "$REPO" --ref main
```

### Observar deploy

```bash
gh run list --repo "$REPO" --workflow deploy-vps.yml --limit 5

# Obtener RUN_ID del deploy y ver logs
RUN_ID=$(gh run list --repo "$REPO" --workflow "deploy-vps.yml" \
  --limit 1 --json databaseId --jq '.[0].databaseId')

gh run view "$RUN_ID" --repo "$REPO" --log
```

### Healthcheck esperado (ejecutado dentro del VPS por el workflow)

El workflow `deploy-vps.yml` ejecuta internamente via SSH:

```bash
curl -fsS http://127.0.0.1:8080/health    # API Server
curl -fsS http://127.0.0.1:8787/health    # Edge Worker
curl -fsS http://127.0.0.1:5173           # Frontend
```

### URL pública

```
https://<VPS_HOST>
```

**NO usar como gate autoritativo:**

```
http://<VPS_IP>/status
```

Responde 401 por Basic Auth. Es solo warning, no gate.

---

## 12. Reporte final

Después de completar el deploy, Kimi debe generar:

```
docs/omega/KIMI_DEPLOY_EXECUTION_REPORT.md
```

Contenido obligatorio:

- PR #90 estado final (merged/blocked/failed)
- Commit final en main
- Resultado de checks en main
- Resultado de deploy-vps.yml
- Health interno VPS (127.0.0.1 endpoints)
- Estado URL pública
- Bloqueos restantes
- Confirmación: no bypass, no admin merge, no secrets recreados

**NO incluir:** tokens, secrets, private keys.

---

## Secrets ya configurados (solo nombres — nunca valores)

| Secret | Última actualización |
|---|---|
| `ARBX_JWT_SECRET` | 2026-05-16T22:05:09Z |
| `ARBX_EDGE_TOKEN` | 2026-05-16T22:05:10Z |
| `ARBX_ADMIN_TOKEN` | 2026-05-16T22:05:11Z |
| `ARBX_SERVICE_TOKEN` | 2026-05-16T22:05:11Z |
| `SESSION_SECRET` | 2026-05-16T22:05:12Z |
| `COOKIE_SECRET` | 2026-05-16T22:05:13Z |
| `WEBHOOK_SECRET` | 2026-05-16T22:05:14Z |
| `DEPLOY_NONCE` | 2026-05-16T22:05:15Z |
| `INTERNAL_API_TOKEN` | 2026-05-16T22:05:16Z |
| `VPS_DEPLOY_PATH` | 2026-05-16T21:45:57Z |
| `VPS_HEALTH_URL` | 2026-05-16T21:45:59Z |
| `VPS_PUBLIC_URL` | 2026-05-16T21:45:58Z |
| `VPS_SSH_HOST` | 2026-05-16T21:45:55Z |
| `VPS_SSH_KEY` | 2026-05-14T19:57:22Z |
| `VPS_SSH_PORT` | 2026-05-16T21:45:56Z |
| `VPS_SSH_USER` | 2026-05-16T21:45:55Z |

Todos rotados con `System.Security.Cryptography.RandomNumberGenerator` (criptográficamente seguro).
`VPS_SSH_KEY` no fue tocado (preservado desde 2026-05-14).

---

## Infraestructura VPS de referencia

| Campo | Valor |
|---|---|
| VPS Host | `<VPS_IP>` |
| SSH User | `root` |
| SSH Port | `22` |
| Deploy Path | `/opt/arbitragex-v2` |
| Compose File | `docker/compose.prod.yml` |
| API Server | `127.0.0.1:8080` |
| Edge Worker | `127.0.0.1:8787` |
| Frontend | `127.0.0.1:5173` |
| Public URL | `https://<VPS_HOST>` |

---

*Handoff generado por Antigravity — 2026-05-16*
*No contiene tokens, secrets ni private keys.*
