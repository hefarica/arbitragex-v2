<#
ArbitrageX v2 — túnel local para Frontend en Dev Container.

Este script no contiene secretos. Requiere que tu Windows tenga un alias SSH llamado
`arbx` en ~/.ssh/config o que pases -SshTarget root@IP.

Ejemplos:
  .\scripts\devcontainer-tunnel-staging.ps1
  .\scripts\devcontainer-tunnel-staging.ps1 -SshTarget arbx
  .\scripts\devcontainer-tunnel-staging.ps1 -SshTarget root@<VPS_IP>
#>
param(
  [string]$SshTarget = "arbx"
)

$ErrorActionPreference = "Stop"

Write-Host "Abriendo túneles hacia staging mediante SSH target: $SshTarget" -ForegroundColor Cyan
Write-Host "Mantén esta ventana abierta mientras uses el Frontend local." -ForegroundColor Yellow
Write-Host "Edge API local: http://localhost:8787" -ForegroundColor Green
Write-Host "WS local:       http://localhost:3000" -ForegroundColor Green
Write-Host "Frontend:      http://localhost:5173/executions" -ForegroundColor Green

ssh -N `
  -L 8787:127.0.0.1:8787 `
  -L 3000:127.0.0.1:3000 `
  $SshTarget
