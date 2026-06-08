<#
  run-claude-with-mcp.ps1 — load the generated .env.mcp into THIS session's
  environment, then launch Claude Code so the ${VAR} placeholders in .mcp.json
  resolve. Secrets stay ephemeral (this process only); nothing is persisted.

  Usage:
    powershell -File scripts/mcp/run-claude-with-mcp.ps1            # verify: claude mcp list
    powershell -File scripts/mcp/run-claude-with-mcp.ps1 -Code      # launch VS Code (extension inherits env)
    powershell -File scripts/mcp/run-claude-with-mcp.ps1 -- <args>  # run: claude <args>

  Prereq: run the generator first ->  python scripts/mcp/gen-env-mcp.py
#>
[CmdletBinding()]
param(
    [switch]$Code,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$ClaudeArgs
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$envFile  = Join-Path $repoRoot '.env.mcp'

if (-not (Test-Path $envFile)) {
    Write-Error "Missing $envFile. Run:  python scripts/mcp/gen-env-mcp.py"
}

$loaded = 0
foreach ($line in Get-Content $envFile) {
    $t = $line.Trim()
    if ($t -eq '' -or $t.StartsWith('#')) { continue }
    $kv = $t -split '=', 2
    if ($kv.Count -ne 2) { continue }
    $key = $kv[0].Trim()
    $val = $kv[1]            # literal; no interpolation, no trimming of URL chars
    Set-Item -Path ("Env:" + $key) -Value $val
    $loaded++
    Write-Host ("  loaded {0} (len={1})" -f $key, $val.Length)
}
Write-Host ("Loaded {0} var(s) from .env.mcp into this session." -f $loaded) -ForegroundColor Green

# Ensure the local Foundry binaries (forge/cast/anvil) are reachable by the
# foundry MCP server, which shells out to them.
$foundryBin = Join-Path $env:USERPROFILE '.foundry\bin'
if (Test-Path $foundryBin) {
    if (($env:PATH -split ';') -notcontains $foundryBin) { $env:PATH = "$foundryBin;$env:PATH" }
    Write-Host "  foundry bin on PATH: $foundryBin" -ForegroundColor Green
}

if ($Code) {
    Write-Host "Launching VS Code (extension inherits this env)..." -ForegroundColor Cyan
    & code $repoRoot
    return
}

$argv = if ($ClaudeArgs -and $ClaudeArgs.Count -gt 0) { $ClaudeArgs } else { @('mcp', 'list') }
Write-Host ("Running: claude {0}" -f ($argv -join ' ')) -ForegroundColor Cyan
& claude @argv
