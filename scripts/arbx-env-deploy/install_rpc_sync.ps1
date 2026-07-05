# install_rpc_sync.ps1 - installs the RPC Providers -> Chain Builder auto-sync.
#
# Adds: (1) SyncRpcCatalog.bas + ArbxBundleShipper.bas (with Public ChainIdFor),
# (2) a Worksheet_Change event on the "RPC Providers" sheet that calls
# SyncRpcToChainBuilder on every col A-D edit (idempotent, non-destructive),
# (3) seeds the curated public RPC catalog + syncs the new chains.
#
# Idempotent: safe to re-run after editing the .bas files. Backs up first.
# ASCII-only (PS 5.1 reads .ps1 as ANSI without BOM - runbook gotcha #6).
#
# Usage: powershell -Sta -NoProfile -ExecutionPolicy Bypass -File install_rpc_sync.ps1

$ErrorActionPreference = 'Stop'

$WbPath  = 'C:\Users\HFRC\Downloads\ArbitrageX_Unified_Config.xlsm'
$Dir     = $PSScriptRoot
$BasPath = Join-Path $Dir 'ArbxBundleShipper.bas'
$SyncPath = Join-Path $Dir 'SyncRpcCatalog.bas'

if (-not (Test-Path $WbPath))   { Write-Error "Workbook not found: $WbPath"; exit 1 }
if (-not (Test-Path $BasPath))  { Write-Error "Missing: $BasPath"; exit 1 }
if (-not (Test-Path $SyncPath)) { Write-Error "Missing: $SyncPath"; exit 1 }

$proc = Get-Process -Name EXCEL -ErrorAction SilentlyContinue
if ($proc) { Write-Error "Excel is OPEN (PID $($proc.Id)). Close it first."; exit 1 }

# --- 1. backup -------------------------------------------------------------
$ts = Get-Date -Format 'yyyyMMdd_HHmmss'
$BakPath = $WbPath + ".bak_$ts"
Copy-Item $WbPath $BakPath -Force
Write-Host "backup -> $BakPath"

# --- 2. trust VBA project object model -------------------------------------
$reg = 'HKCU:\Software\Microsoft\Office\16.0\Excel\Security'
if (-not (Test-Path $reg)) { New-Item -Path $reg -Force | Out-Null }
Set-ItemProperty -Path $reg -Name 'AccessVBOM' -Value 1 -Type DWord

# --- 3. the Worksheet_Change event body to inject into the RPC Providers sheet
$eventCode = @"
Option Explicit

' Auto-sync RPC Providers -> Chain Builder. Injected by install_rpc_sync.ps1.
' Idempotent + non-destructive: only appends new chains, never touches existing.
Private Sub Worksheet_Change(ByVal Target As Range)
    On Error GoTo reenable
    If Target.Row < 2 Then Exit Sub
    If Intersect(Target, Me.Range("A:D")) Is Nothing Then Exit Sub
    Application.EnableEvents = False
    Application.Calculation = xlCalculationManual
    SyncRpcToChainBuilder
reenable:
    Application.Calculation = xlCalculationAutomatic
    Application.EnableEvents = True
End Sub
"@

# --- 4. COM safe-mode ------------------------------------------------------
$missing = [System.Reflection.Missing]::Value
$excel = New-Object -ComObject Excel.Application
$excel.Visible = $false
$excel.DisplayAlerts = $false
$excel.EnableEvents = $false
$excel.ScreenUpdating = $false

$wb = $null
try {
    $wb = $excel.Workbooks.Open($WbPath)
    $vbProj = $wb.VBProject

    # 4a. (Re)import ArbxBundleShipper (ChainIdFor is now Public so SyncRpcCatalog can call it).
    foreach ($nm in @('ArbxBundleShipper', 'SyncRpcCatalog')) {
        try { $ex = $vbProj.VBComponents.Item($nm); if ($ex) { $vbProj.VBComponents.Remove($ex) } } catch {}
    }
    $vbProj.VBComponents.Import($BasPath)
    Write-Host "imported ArbxBundleShipper.bas (ChainIdFor Public)"
    $vbProj.VBComponents.Import($SyncPath)
    Write-Host "imported SyncRpcCatalog.bas"

    # 4b. Inject Worksheet_Change into the "RPC Providers" sheet module (idempotent).
    $rpcComp = $null
    foreach ($comp in $vbProj.VBComponents) {
        if ($comp.Type -eq 100) {
            try { if ($comp.Properties.Item('Name').Value -eq 'RPC Providers') { $rpcComp = $comp; break } } catch {}
        }
    }
    if ($null -eq $rpcComp) {
        Write-Error "Could not find the 'RPC Providers' worksheet component"
    } else {
        $cm = $rpcComp.CodeModule
        $lineCount = $cm.CountOfLines
        $existing = if ($lineCount -gt 0) { $cm.Lines(1, $lineCount) } else { "" }
        if ($existing -match 'Sub Worksheet_Change') {
            Write-Host "Worksheet_Change already on RPC Providers (skipped)"
        } else {
            $cm.AddFromString($eventCode)
            Write-Host "Worksheet_Change injected into RPC Providers"
        }
    }

    # 4c. Seed the curated RPC catalog + sync chains into Chain Builder.
    #     Headless (no MsgBox) - SeedRpcProviders is idempotent.
    $excel.Run('SeedRpcProviders')
    Write-Host "SeedRpcProviders + SyncRpcToChainBuilder executed"

    $wb.Save()
    Write-Host "saved $WbPath"
} finally {
    if ($wb) { try { $wb.Close($false) } catch {} }
    try { $excel.Quit() } catch {}
    [System.Runtime.InteropServices.Marshal]::ReleaseComObject($excel) | Out-Null
    [GC]::Collect()
    [GC]::WaitForPendingFinalizers()
}

Write-Host ''
Write-Host 'DONE. Open the workbook:'
Write-Host '  - "RPC Providers" now has the curated catalog (45 public endpoints, idempotent).'
Write-Host '  - "Chain Builder" now has the new chains (Sepolia/Amoy/Avalanche/Gnosis/Linea/Scroll/Blast/...).'
Write-Host '  - Adding a row to "RPC Providers" auto-appends the chain to "Chain Builder" (Worksheet_Change).'
Write-Host '  - Mark col B in Chain Builder to activate the chains you want, then run ShipBundle.'
