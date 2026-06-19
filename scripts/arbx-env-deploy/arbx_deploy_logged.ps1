param([string]$Mode,[string]$VpsHost,[string]$EnvPath,[string]$LogPath,[string]$Runner)
# Runs the deploy runner, captures ALL output to $LogPath, and appends a completion
# marker the panel polls for. Keeps the panel's done-detection independent of any
# process handle (robust). Runs as a hidden child PowerShell (no bash, no console).
$ec = 0
try {
  & $Runner -Mode $Mode -VpsHost $VpsHost -EnvPath $EnvPath *>&1 | Out-File -LiteralPath $LogPath -Encoding utf8
  $ec = $LASTEXITCODE
  if($null -eq $ec){ $ec = 0 }
} catch {
  ($_ | Out-String) | Out-File -LiteralPath $LogPath -Encoding utf8
  $ec = 99
}
Add-Content -LiteralPath $LogPath ("__ARBX_EXIT__=" + $ec)
