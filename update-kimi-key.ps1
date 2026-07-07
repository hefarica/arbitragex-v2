# Script para actualizar la API key de Kimi en settings.json
# Uso: .update-kimi-key.ps1 -ApiKey "sk-tu-key-real-aqui"

param(
    [Parameter(Mandatory=$true)]
    [string]$ApiKey
)

$settingsPath = "$env:USERPROFILE\.claude\settings.json"

# Leer el archivo
$json = Get-Content $settingsPath -Raw | ConvertFrom-Json

# Actualizar la key
$json.env.ANTHROPIC_AUTH_TOKEN = $ApiKey

# Guardar el archivo
$json | ConvertTo-Json -Depth 10 | Set-Content $settingsPath

Write-Host "✅ API key actualizada correctamente en $settingsPath" -ForegroundColor Green
Write-Host ""
Write-Host "Para probar:" -ForegroundColor Cyan
Write-Host "  claude --help"
Write-Host "  cd 'C:\Users\HFRC\Desktop\arbitragex-v2-main (17)'"
Write-Host "  claude"
