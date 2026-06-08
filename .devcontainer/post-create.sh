#!/usr/bin/env bash
set -euo pipefail

cd /workspace

printf '\n[devcontainer] Instalando dependencias npm del workspace...\n'
npm install --workspaces --include-workspace-root

printf '\n[devcontainer] Verificando paquete frontend...\n'
npm run -w @arbx/frontend typecheck -- --pretty false || true

cat <<'MSG'

[devcontainer] Entorno listo.

Para desarrollo visual:
  npm run -w @arbx/frontend dev:container

URL principal:
  http://localhost:5173/executions

Si deseas conectar el frontend local contra staging, abre en Windows una terminal PowerShell separada y ejecuta:
  .\scripts\devcontainer-tunnel-staging.ps1

MSG
