param(
  [string]$Fragment,
  [string]$VpsHost = "arbx",                       # SSH host alias in ~/.ssh/config (arbx | arbx-git)
  [string]$EnvPath = "/opt/arbitragex-v2/.env",    # remote .env path (verified present, 127 lines)
  [switch]$Apply                                   # default = DRY-RUN (diff only, no change)
)
# ArbitrageX .env deploy orchestrator (Windows side).
# All remote bash control-flow lives in arbx_remote.sh — PowerShell only scp's the
# 3 files and makes ONE simple `ssh bash remote.sh` call (no &&/|| in PS: WinPS 5.1
# rejects them even inside strings). Transport uses the operator's own ~/.ssh/config
# host alias — NO credentials are stored here. paper_mode is never touched.
$ErrorActionPreference = "Stop"
$here   = Split-Path -Parent $MyInvocation.MyCommand.Path
if (-not $Fragment) { $Fragment = Join-Path $here "arbx_env_fragment.env" }
$upsert = Join-Path $here "arbx_env_upsert.sh"
$remote = Join-Path $here "arbx_remote.sh"

function Find-Bin($n) {
  $c = (Get-Command $n -ErrorAction SilentlyContinue).Source
  if ($c) { return $c }
  $g = "C:\Program Files\Git\usr\bin\$n"
  if (Test-Path $g) { return $g }
  throw "$n not found (install OpenSSH or Git for Windows)"
}
$ssh = Find-Bin "ssh.exe"; $scp = Find-Bin "scp.exe"
foreach ($f in @($Fragment, $upsert, $remote)) { if (-not (Test-Path $f)) { throw "missing required file: $f (run ExportEnvFragment macro first?)" } }

$mode = if ($Apply) { 'apply' } else { 'dryrun' }
Write-Host "================ ArbitrageX .env deploy ================" -ForegroundColor Cyan
Write-Host ("host={0}  envPath={1}  mode={2}" -f $VpsHost, $EnvPath, $(if ($Apply) { 'APPLY (write + restart)' } else { 'DRY-RUN (diff only)' }))

& $scp $Fragment "${VpsHost}:/tmp/arbx_env_fragment.env"
& $scp $upsert   "${VpsHost}:/tmp/arbx_env_upsert.sh"
& $scp $remote   "${VpsHost}:/tmp/arbx_remote.sh"
& $ssh $VpsHost "bash /tmp/arbx_remote.sh $mode '$EnvPath'"

if ($mode -eq 'dryrun') {
  Write-Host "`nDRY-RUN only - nothing changed on the VPS. Re-run with -Apply to write + restart." -ForegroundColor Yellow
} else {
  Write-Host "`nApplied. Confirm 'scanner.subscribed chain_id=1' above. paper_mode unchanged." -ForegroundColor Green
}
