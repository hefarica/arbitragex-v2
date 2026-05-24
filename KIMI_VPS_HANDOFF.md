# KIMI VPS HANDOFF — arbitragex-v2

## Repo
REPO=hefarica/arbitragex-v2
BRANCH=omega/recovery-20260516
PR=90

## Valores reales confirmados

VPS_SSH_HOST=195.201.235.70
VPS_SSH_USER=root
VPS_SSH_PORT=22
VPS_DEPLOY_PATH=/opt/arbitragex-v2
VPS_HEALTH_URL=http://195.201.235.70/status
VPS_PUBLIC_URL=http://195.201.235.70
VPS_COMPOSE_FILE=docker/compose.prod.yml

## Estado Git en VPS

VPS_GIT_REMOTE=https://github.com/hefarica/arbitragex-v2.git
VPS_CURRENT_BRANCH=main
VPS_CURRENT_COMMIT=1019447ecfc32af027de40eea668b948e0919412
VPS_APP_DIR_STATUS=OK

## Servicios detectados

Docker ps:
- arbitragex-v2-edge-1
- arbitragex-v2-frontend-1
- arbitragex-v2-api-server-1
- omega-nginx-gateway
- arbitragex-v2-relays-client-1
- arbitragex-v2-searcher-rs-1
- arbitragex-v2-recon-1
- arbitragex-v2-selector-api-1
- arbitragex-v2-token-enricher-1
- arbitragex-v2-sim-ctl-1
- arbitragex-v2-postgres-1
- arbitragex-v2-alertmanager-1
- arbitragex-v2-grafana-1
- arbitragex-v2-promtail-1
- arbitragex-v2-prometheus-1
- arbitragex-v2-redis-1
- arbitragex-v2-loki-1

Docker compose ps:
(Mismos listados, configurados vía docker/compose.prod.yml)

Puertos:
- Públicos (0.0.0.0): 80, 8443
- Internos (127.0.0.1): 3000, 5173, 8787, 8080, 5432, 6379, 3100, 9090, 9093

## Endpoints probados

Endpoint | HTTP | Resultado
---|---|---
http://localhost/status | 401 | Basic Auth Nginx
http://127.0.0.1:8787/health | 200 | OK (Edge)
http://127.0.0.1:8080/health | 200 | OK (API)
http://127.0.0.1:5173 | 200 | OK (Frontend)

## GitHub secrets

Secret | Status
---|---
VPS_SSH_KEY | UNVERIFIABLE
VPS_SSH_HOST | UNVERIFIABLE
VPS_SSH_USER | UNVERIFIABLE
VPS_SSH_PORT | UNVERIFIABLE
VPS_DEPLOY_PATH | UNVERIFIABLE
VPS_HEALTH_URL | UNVERIFIABLE
VPS_PUBLIC_URL | UNVERIFIABLE
ARBX_JWT_SECRET | UNVERIFIABLE
ARBX_EDGE_TOKEN | UNVERIFIABLE
ARBX_ADMIN_TOKEN | UNVERIFIABLE
ARBX_SERVICE_TOKEN | UNVERIFIABLE
SESSION_SECRET | UNVERIFIABLE
COOKIE_SECRET | UNVERIFIABLE
WEBHOOK_SECRET | UNVERIFIABLE
DEPLOY_NONCE | UNVERIFIABLE
INTERNAL_API_TOKEN | UNVERIFIABLE
MAINNET_RPC_URL | UNVERIFIABLE

*(Bloqueo: La CLI `gh` no está disponible en este host Windows local, por lo que no es posible auditar ni inyectar secretos directamente desde este agente. Deben auditarse externamente).*

## SSH public key

SSH_PUBLIC_KEY_INSTALL_STATUS=MISSING
SSH_PUBLIC_KEY_USER=root
SSH_PUBLIC_KEY_PATH=/root/.ssh/authorized_keys

## Bloqueos

- `gh CLI` = MISSING_REAL_VALUE. El binario no existe en la máquina local actual, lo que bloquea automatizar la carga de los secretos al repo de GitHub. El operador/Kimi debe encargarse usando la data de la sección "Valores reales confirmados".
- `MAINNET_RPC_URL` = MISSING_REAL_VALUE (dependencia de secretos).
- La public key `github-actions-deploy@arbitragex-v2` (AAAAC3NzaC1lZDI1NTE5AAAAIAGghEp3KrzknMwhxmILYU4oY0u5JAom3SSDjE8DQrH0) NO está instalada en el VPS. Comando para agregarla: `echo "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAGghEp3KrzknMwhxmILYU4oY0u5JAom3SSDjE8DQrH0 github-actions-deploy@arbitragex-v2" | ssh arbx "cat >> ~/.ssh/authorized_keys"`.

## Comandos recomendados para Kimi

gh pr checks 90 --watch --interval 30
gh workflow run deploy-vps.yml
gh run list --workflow deploy-vps.yml --limit 5
gh run view <RUN_ID> --log-failed
gh run download <RUN_ID> -D ./ci-artifacts
