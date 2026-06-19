param([string]$Mode='safediff',[string]$VpsHost='arbx',[string]$EnvPath='/opt/arbitragex-v2/.env')
# Pure-PowerShell deploy runner using Windows NATIVE OpenSSH (runs headless/hidden;
# MSYS/Git bash does NOT). ONE ssh connection: the 3 small files are shipped inline
# (base64) and decoded on the VPS, then the remote runner executes. No local bash,
# no scp round-trips. bash runs only on the Linux side.
$ErrorActionPreference='Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$frag = Join-Path $here 'arbx_env_fragment.env'
$ups  = Join-Path $here 'arbx_env_upsert.sh'
$rem  = Join-Path $here 'arbx_remote.sh'

function Find-Bin($n){
  $w = Join-Path $env:WINDIR "System32\OpenSSH\$n"; if(Test-Path $w){ return $w }   # native OpenSSH first
  $c = (Get-Command $n -ErrorAction SilentlyContinue).Source; if($c){ return $c }
  $g = "C:\Program Files\Git\usr\bin\$n"; if(Test-Path $g){ return $g }
  throw "$n not found (install Windows OpenSSH client)"
}
$ssh = Find-Bin 'ssh.exe'
foreach($f in @($frag,$ups,$rem)){ if(-not (Test-Path $f)){ Write-Output "missing required file: $f"; exit 3 } }

function B64($p){ [Convert]::ToBase64String([IO.File]::ReadAllBytes($p)) }
$bf = B64 $frag; $bu = B64 $ups; $br = B64 $rem

# Single connection: decode the 3 files to /tmp, then run the remote runner. base64 is
# pure [A-Za-z0-9+/=] so it is safe inside single quotes; only ';' separators (no &&).
$remote = "printf %s '$bf' | base64 -d > /tmp/arbx_env_fragment.env; " +
          "printf %s '$bu' | base64 -d > /tmp/arbx_env_upsert.sh; " +
          "printf %s '$br' | base64 -d > /tmp/arbx_remote.sh; " +
          "bash /tmp/arbx_remote.sh '$Mode' '$EnvPath'"

Write-Output ("ssh (1 connection) mode={0} host={1}" -f $Mode, $VpsHost)
# native ssh.exe relays the remote's stderr (e.g. a benign `docker compose` "GITHUB_TOKEN not set"
# warning). In PS 5.1 under -ErrorAction Stop each stderr line becomes a terminating
# NativeCommandError -> FALSE failure even when the remote bash exited 0. Judge success ONLY by the
# remote exit code; relay stdout+stderr as plain text so real errors still appear in the panel log.
$ErrorActionPreference = 'Continue'
$sshOut = & $ssh -o BatchMode=yes -o ConnectTimeout=20 $VpsHost $remote 2>&1
$code = $LASTEXITCODE
foreach($l in $sshOut){ Write-Output ([string]$l) }
exit $code
