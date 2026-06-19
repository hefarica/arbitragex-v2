param([string]$VpsHost='arbx',[string]$OutFile)
# Pull the REQUESTED-credentials manifest from the VPS (TSV: KEY<TAB>VALUE<TAB>NOTES).
# Runs arbx_cred_manifest.sh on the VPS (union of .env.example + .env with status), captures
# its output to a LOCAL file the macro imports into ".env Production". Native OpenSSH (headless).
# Secrets travel VPS -> local file -> Excel and the macro shreds the file after. Nothing printed.
$ErrorActionPreference='Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
if(-not $OutFile){ $OutFile = Join-Path $here '.arbx_manifest.tsv' }
$manifest = Join-Path $here 'arbx_cred_manifest.sh'
function Find-Bin($n){
  $w = Join-Path $env:WINDIR "System32\OpenSSH\$n"; if(Test-Path $w){ return $w }
  $c = (Get-Command $n -ErrorAction SilentlyContinue).Source; if($c){ return $c }
  $g = "C:\Program Files\Git\usr\bin\$n"; if(Test-Path $g){ return $g }
  throw "$n not found (install Windows OpenSSH client)"
}
$ssh = Find-Bin 'ssh.exe'; $scp = Find-Bin 'scp.exe'
if(-not (Test-Path $manifest)){ Write-Output "missing manifest script: $manifest"; exit 3 }

& $scp -q -o BatchMode=yes -o ConnectTimeout=20 $manifest "${VpsHost}:/tmp/arbx_cred_manifest.sh"
if($LASTEXITCODE -ne 0){ Write-Output "scp manifest failed ($LASTEXITCODE)"; exit 4 }
# judge by exit code, not stderr: a benign remote warning on stderr must not abort the pull
# (PS 5.1 turns native stderr into a terminating error under -ErrorAction Stop). stderr is dropped
# so it never contaminates the TSV ($tsv must be pure stdout = the manifest).
$ErrorActionPreference = 'Continue'
$tsv = & $ssh -o BatchMode=yes -o ConnectTimeout=25 $VpsHost "bash /tmp/arbx_cred_manifest.sh; rm -f /tmp/arbx_cred_manifest.sh" 2>$null
if($LASTEXITCODE -ne 0){ Write-Output "manifest run failed ($LASTEXITCODE)"; exit 5 }
[IO.File]::WriteAllText($OutFile, (($tsv -join "`n") + "`n"), (New-Object System.Text.UTF8Encoding $false))
$n = ($tsv | Where-Object { $_ -match "`t" }).Count
Write-Output ("manifest OK rows={0} -> {1}" -f $n, $OutFile)
exit 0
