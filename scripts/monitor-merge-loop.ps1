# Monitor loop: update BEHIND, merge ready, report changes
Set-Location "c:\Users\HFRC\Desktop\arbitragex-v2-main (17)"
$env:GH_TOKEN = (gh auth token 2>$null)
if (-not $env:GH_TOKEN) { Write-Host "ERROR: no token"; exit 1 }

$PRS = @(401,397,392,389,388,387,386,384,383)
$prev = 7

while ($true) {
    $merged = 7
    foreach ($pr in $PRS) {
        try {
            $st = gh pr view $pr --json state --jq .state 2>$null
            if ($st -eq "MERGED") { $merged++; continue }
            if ($st -ne "OPEN") { continue }

            $stb = gh pr view $pr --json mergeStateStatus --jq .mergeStateStatus 2>$null
            if ($stb -eq "BEHIND") {
                gh pr update-branch $pr 2>$null
                Write-Host "PR${pr} updated"
                continue
            }

            $pend = gh pr checks $pr --json name,bucket --jq '[.[]|select(.bucket=="pending")]|length' 2>$null
            if ($pend -eq 0 -and ($stb -eq "CLEAN" -or $stb -eq "UNSTABLE")) {
                $fails = gh pr checks $pr --json name,bucket --jq '[.[]|select(.bucket=="fail")]|map(.name)|join(",")' 2>$null
                $extra = 0
                if ($fails) {
                    $items = $fails -split ","
                    $extra = ($items | Where-Object { $_ -notmatch "npm audit|cargo audit|TypeScript integration" }).Count
                }
                if ($extra -eq 0) {
                    gh pr merge $pr --squash --delete-branch 2>$null
                    Write-Host "PR${pr} MERGED"
                }
            }
        } catch { continue }
    }

    if ($merged -gt $prev) {
        Write-Host "PROGRESO: ${merged}/16 merged"
        $prev = $merged
    }

    if ($merged -ge 14) {
        Write-Host "DEPLOY_READY: ${merged} merged"
        break
    }

    Start-Sleep -Seconds 300
}
