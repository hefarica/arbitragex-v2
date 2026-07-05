# install_bundle_shipper.ps1 - installs the Encrypted Bundle Shipper into the
# operator's .xlsm: imports the ArbxBundleShipper VBA module + adds the
# "Bundle Shipper" sheet (instructions + public-key fingerprint + button).
#
# Idempotent: safe to re-run after editing the .bas. Backs up the workbook first.
# ASCII-only (PS 5.1 reads .ps1 as ANSI without BOM - runbook gotcha #6).
#
# Usage: powershell -Sta -NoProfile -ExecutionPolicy Bypass -File install_bundle_shipper.ps1

$ErrorActionPreference = 'Stop'

$WbPath    = 'C:\Users\HFRC\Downloads\ArbitrageX_Unified_Config.xlsm'
$BasPath   = Join-Path $PSScriptRoot 'ArbxBundleShipper.bas'
$PubKeyPath = 'C:\Users\HFRC\Downloads\arbx_bundle_public.pem'
$SheetName = 'Bundle Shipper'

# --- 0. preflight ----------------------------------------------------------
if (-not (Test-Path $WbPath))     { Write-Error "Workbook not found: $WbPath"; exit 1 }
if (-not (Test-Path $BasPath))    { Write-Error "VBA module not found: $BasPath"; exit 1 }

# Excel must be CLOSED (COM exclusive - MK_E_UNAVAILABLE if open elsewhere).
$proc = Get-Process -Name EXCEL -ErrorAction SilentlyContinue
if ($proc) { Write-Error "Excel is OPEN (PID $($proc.Id)). Close it first."; exit 1 }

# --- 1. backup -------------------------------------------------------------
$ts = Get-Date -Format 'yyyyMMdd_HHmmss'
$BakPath = $WbPath + ".bak_$ts"
Copy-Item $WbPath $BakPath -Force
Write-Host "backup -> $BakPath"

# --- 2. trust access to the VBA project object model (HKCU, no admin) ------
$reg = 'HKCU:\Software\Microsoft\Office\16.0\Excel\Security'
if (-not (Test-Path $reg)) { New-Item -Path $reg -Force | Out-Null }
Set-ItemProperty -Path $reg -Name 'AccessVBOM' -Value 1 -Type DWord
Write-Host "AccessVBOM = 1"

# --- 3. public-key fingerprint (sha256, hex) -------------------------------
$fingerprint = '<public key not found>'
if (Test-Path $PubKeyPath) {
    $bytes = [System.IO.File]::ReadAllBytes($PubKeyPath)
    $sha = [System.Security.Cryptography.SHA256]::Create()
    $hash = $sha.ComputeHash($bytes)
    $fingerprint = ($hash | ForEach-Object { $_.ToString('x2') }) -join ''
    Write-Host "public key sha256: $($fingerprint.Substring(0,16))..."
}

# --- 4. COM safe-mode open -------------------------------------------------
$missing = [System.Reflection.Missing]::Value
$excel = New-Object -ComObject Excel.Application
$excel.Visible = $false
$excel.DisplayAlerts = $false
$excel.EnableEvents = $false
$excel.ScreenUpdating = $false

$wb = $null
$ws = $null
try {
    $wb = $excel.Workbooks.Open($WbPath)

    # --- 4a. import the VBA module (remove existing first = idempotent) ----
    $vbProj = $wb.VBProject
    try {
        $existing = $vbProj.VBComponents.Item('ArbxBundleShipper')
        if ($existing) {
            $vbProj.VBComponents.Remove($existing)
            Write-Host "removed existing ArbxBundleShipper module"
        }
    } catch { }  # not present yet - fine
    $vbProj.VBComponents.Import($BasPath)
    Write-Host "imported ArbxBundleShipper.bas"

    # --- 4b. add the sheet at the end (remove existing first = idempotent) -
    foreach ($s in $wb.Worksheets) {
        if ($s.Name -eq $SheetName) {
            $s.Delete()
            Write-Host "removed existing '$SheetName' sheet"
            break
        }
    }
    $lastIdx = $wb.Worksheets.Count
    # Worksheets.Add(Before, After, Count, Type) - we want AFTER the last sheet.
    if ($lastIdx -ge 1) {
        $after = $wb.Worksheets.Item($lastIdx)
        $ws = $wb.Worksheets.Add($missing, $after)
    } else {
        $ws = $wb.Worksheets.Add()
    }
    $ws.Name = $SheetName

    # --- 4c. populate the sheet (ASCII only - PS 5.1 ANSI gotcha) ---------
    $ws.Cells.Item(1, 1) = 'Encrypted Bundle Shipper'
    $ws.Cells.Item(1, 1).Font.Size = 14
    $ws.Cells.Item(1, 1).Font.Bold = $true

    $ws.Cells.Item(3, 1) = 'Public key sha256:'
    $ws.Cells.Item(3, 1).Font.Bold = $true
    $ws.Cells.Item(3, 2) = $fingerprint
    $ws.Cells.Item(3, 2).Font.Name = 'Consolas'
    $ws.Cells.Item(3, 2).Font.Size = 9

    $ws.Cells.Item(5, 1) = 'How it works:'
    $ws.Cells.Item(5, 1).Font.Bold = $true
    $ws.Cells.Item(6, 1) = '1. Click the button -> ShipBundle reads the 4 sheets, encrypts, writes the .enc'
    $ws.Cells.Item(7, 1) = '2. SSH upload now (Ruta 1)? or leave the .enc for browser upload (Ruta 2)'
    $ws.Cells.Item(8, 1) = '3. paper_mode / DEPLOYER_* / MULTISIG / MAINNET_RPC are NEVER shipped'
    $ws.Cells.Item(9, 1) = '4. The .enc lands on the VPS; trigger the importer from the /rpcs panel or via SSH'
    $ws.Cells.Item(10, 1) = '5. Crypto: RSA-OAEP-4096 + AES-256-GCM. VBA reads sheets, Python encrypts (audited lib)'
    $ws.Cells.Item(11, 1) = '6. Private key stays ONLY on the VPS. If this .xlsm leaks, no data leaks.'

    $ws.Cells.Item(13, 1) = 'Reqs: pythonw on PATH; pip install openpyxl cryptography;'
    $ws.Cells.Item(13, 1).Font.Italic = $true
    $ws.Cells.Item(14, 1) = '      public key at .\arbx-env-deploy\arbx_bundle_public.pem'
    $ws.Cells.Item(14, 1).Font.Italic = $true

    $ws.Cells.Item(16, 1) = 'Encrypted bundle output:'
    $ws.Cells.Item(16, 1).Font.Bold = $true
    $ws.Cells.Item(16, 2) = (Join-Path (Split-Path $WbPath) 'arbx-env-deploy\arbx_config_bundle.json.enc')

    # Column A width
    $ws.Columns.Item(1).ColumnWidth = 28
    $ws.Columns.Item(2).ColumnWidth = 80

    # --- 4d. add the button, wire to ShipBundle ---------------------------
    # xlButtonControl = 0. AddFormControl(Format, Left, Top, Width, Height) in points.
    # For form-control buttons the caption is on OLEFormat.Object (Button), not
    # TextFrame.Characters - the latter fails via PowerShell COM late-binding.
    $btn = $ws.Shapes.AddFormControl(0, 10, 250, 240, 42)
    $btn.Name = 'btnShipBundle'
    $btnObj = $btn.OLEFormat.Object
    $btnObj.Caption = 'Ship Bundle (.enc)'
    $btnObj.Font.Size = 12
    $btnObj.Font.Bold = $true
    $btn.OnAction = 'ShipBundle'
    Write-Host "button wired to ShipBundle"

    # --- 4e. save (preserve .xlsm + VBA) ----------------------------------
    $wb.Save()
    Write-Host "saved $WbPath (xlsm + VBA preserved)"
} finally {
    if ($wb) { try { $wb.Close($false) } catch {} }
    try { $excel.Quit() } catch {}
    if ($ws)    { [System.Runtime.InteropServices.Marshal]::ReleaseComObject($ws) | Out-Null }
    if ($wb)    { [System.Runtime.InteropServices.Marshal]::ReleaseComObject($wb) | Out-Null }
    [System.Runtime.InteropServices.Marshal]::ReleaseComObject($excel) | Out-Null
    [GC]::Collect()
    [GC]::WaitForPendingFinalizers()
}

Write-Host ''
Write-Host 'DONE. Open the workbook -> the "Bundle Shipper" sheet has the button.'
Write-Host 'Click it to run ShipBundle (generates + uploads the .enc).'
