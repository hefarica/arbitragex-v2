# Sidebar Crisis Prevention Skill

## Description

Detects and prevents the "stale Docker :5173" crisis where the browser preview serves an outdated build from a Linux-side Docker container instead of the current Windows clone. Provides exact commands to kill stale processes, start a clean dev server, and verify the sidebar renders correctly.

## Trigger

Run this skill when:
- Sidebar changes don't appear in browser preview
- Port 5173 is in use but edits don't reflect
- Preview shows old labels (OBSERVE/CONTROL/SETUP instead of Pipeline/Risk & Control)
- CSS changes (globals.css, liquid-glass, aurora-drift) don't apply
- `npm run dev` fails with EADDRINUSE

## Detection: Is :5173 Stale?

### Step 1: Identify what's on port 5173

```powershell
# PowerShell - Check what's listening on :5173
Get-NetTCPConnection -LocalPort 5173 -ErrorAction SilentlyContinue | 
    Select-Object LocalPort, OwningProcess, @{Name="PID";Expression={$_.OwningProcess}}
```

**Stale if:**
- OwningProcess is `wslrelay.exe` or `com.docker.backend.exe`
- OwningProcess is NOT `node.exe`

### Step 2: Verify content freshness

```powershell
# Fetch the CSS bundle and check for key rules
curl -s http://localhost:5173 | Select-String -Pattern "arbx-aurora-drift|liquid-glass"
```

**Stale if:**
- No match for `arbx-aurora-drift` (should be in globals.css)
- No match for `liquid-glass` (should be in tailwind config)
- Sidebar shows old labels (OBSERVE/CONTROL/SETUP)

### Step 3: Check for Docker/WSL processes

```powershell
# Check for Docker backend processes
Get-Process | Where-Object { $_.ProcessName -match "docker|wsl" } | 
    Select-Object ProcessName, Id, Path
```

## Kill Stale Processes

### Option A: Kill by Port (Recommended)

```powershell
# Kill whatever is on port 5173
$conn = Get-NetTCPConnection -LocalPort 5173 -ErrorAction SilentlyContinue
if ($conn) {
    $pid = $conn.OwningProcess
    Write-Host "Killing PID $pid on port 5173"
    Stop-Process -Id $pid -Force
} else {
    Write-Host "Port 5173 is free"
}
```

### Option B: Kill Docker/WSL specifically

```powershell
# Kill Docker Desktop backend
Get-Process | Where-Object { $_.ProcessName -match "com.docker.backend" } | Stop-Process -Force

# Kill WSL relay
Get-Process | Where-Object { $_.ProcessName -match "wslrelay" } | Stop-Process -Force
```

### Option C: Nuclear - Kill all Node + Docker + WSL

```powershell
# Kill all node processes
Get-Process | Where-Object { $_.ProcessName -eq "node" } | Stop-Process -Force

# Kill Docker
Get-Process | Where-Object { $_.ProcessName -match "docker" } | Stop-Process -Force

# Kill WSL
wsl --shutdown
```

## Start Clean Dev Server

### Step 1: Navigate to correct directory

```powershell
cd "C:\Users\HFRC\Desktop\arbitragex-v2-main (17)\frontend"
```

### Step 2: Verify node_modules exists

```powershell
if (-Not (Test-Path "node_modules")) {
    Write-Host "ERROR: node_modules missing. Run: npm install"
    exit 1
}
```

### Step 3: Start dev server on port 3000 (NOT 5173)

```powershell
# CRITICAL: Use port 3000, NOT 5173 (which conflicts with stale Docker)
npx next dev -p 3000
```

**Why port 3000?**
- `npm run dev` hardcodes `-p 5173` in package.json
- Port 5173 is occupied by stale Docker/WSL
- Port 3000 is the standard Next.js dev port

### Step 4: Verify server started

```powershell
# In another terminal - check if :3000 responds
Start-Sleep -Seconds 12  # Wait for boot (~11s typical)
curl -s -o /dev/null -w "%{http_code}" http://localhost:3000
# Should return: 200
```

## Verification Checklist

### Check 1: Sidebar Renders

```powershell
# Fetch the page and check for sidebar component
curl -s http://localhost:3000 | Select-String -Pattern "AppSidebar|sidebar"
```
**Pass:** Contains sidebar references
**Fail:** No sidebar found (hydration error or component not mounted)

### Check 2: Correct Navigation Labels

```powershell
# Check for new labels (NOT old OBSERVE/CONTROL/SETUP)
curl -s http://localhost:3000 | Select-String -Pattern "Pipeline|Risk & Control|Configuration"
```
**Pass:** Shows "Pipeline", "Risk & Control", "Configuration"
**Fail:** Shows old "OBSERVE", "CONTROL", "SETUP" (stale build)

### Check 3: CSS Applied

```powershell
# Find CSS bundle URL and check for key rules
$html = curl -s http://localhost:3000
$cssUrl = ($html | Select-String -Pattern '/_next/static/css/[^"]+').Matches[0].Value
curl -s "http://localhost:3000$cssUrl" | Select-String -Pattern "arbx-aurora-drift|liquid-glass"
```
**Pass:** CSS contains custom keyframes and glassmorphism
**Fail:** CSS is missing custom rules (stale build)

### Check 4: No Hydration Errors

```powershell
# Check browser console for hydration errors
# (Manual: Open DevTools > Console, look for red errors)
```
**Pass:** No "Text content does not match" or "Hydration failed" errors
**Fail:** Hydration mismatch (usually from Date.now() or window in SSR)

### Check 5: Interactive Elements Work

```powershell
# Test via Playwright or manual:
# 1. Sidebar collapse/expand button works
# 2. Navigation links route correctly
# 3. Paper mode indicator shows "paper-mode"
```

## Quick Diagnostic Script

Save this as `diagnose-sidebar.ps1`:

```powershell
# Sidebar Crisis Diagnostic
Write-Host "=== SIDEBAR CRISIS DIAGNOSTIC ===" -ForegroundColor Cyan

# Check port 5173
Write-Host "`n[1] Checking port 5173..." -ForegroundColor Yellow
$conn5173 = Get-NetTCPConnection -LocalPort 5173 -ErrorAction SilentlyContinue
if ($conn5173) {
    $proc = Get-Process -Id $conn5173.OwningProcess -ErrorAction SilentlyContinue
    Write-Host "  Port 5173 in use by: $($proc.ProcessName) (PID: $($conn5173.OwningProcess))" -ForegroundColor Red
    if ($proc.ProcessName -match "docker|wsl") {
        Write-Host "  STALE DOCKER DETECTED! Run kill commands." -ForegroundColor Red
    }
} else {
    Write-Host "  Port 5173 is free" -ForegroundColor Green
}

# Check port 3000
Write-Host "`n[2] Checking port 3000..." -ForegroundColor Yellow
$conn3000 = Get-NetTCPConnection -LocalPort 3000 -ErrorAction SilentlyContinue
if ($conn3000) {
    $proc = Get-Process -Id $conn3000.OwningProcess -ErrorAction SilentlyContinue
    Write-Host "  Port 3000 in use by: $($proc.ProcessName) (PID: $($conn3000.OwningProcess))" -ForegroundColor Green
} else {
    Write-Host "  Port 3000 is free (no dev server running)" -ForegroundColor Red
}

# Test content freshness
Write-Host "`n[3] Testing content freshness..." -ForegroundColor Yellow
if ($conn3000) {
    $html = curl -s http://localhost:3000
    if ($html -match "Pipeline") {
        Write-Host "  Sidebar shows NEW labels (Pipeline/Risk & Control)" -ForegroundColor Green
    } elseif ($html -match "OBSERVE") {
        Write-Host "  Sidebar shows OLD labels (OBSERVE/CONTROL/SETUP) - STALE!" -ForegroundColor Red
    } else {
        Write-Host "  Cannot determine sidebar state" -ForegroundColor Yellow
    }
} else {
    Write-Host "  No server on :3000 to test" -ForegroundColor Yellow
}

Write-Host "`n=== END DIAGNOSTIC ===" -ForegroundColor Cyan
```

## Recovery Protocol

### If stale Docker detected:

1. **Kill stale processes:**
   ```powershell
   Get-NetTCPConnection -LocalPort 5173 | Stop-Process -Id {$_.OwningProcess} -Force
   wsl --shutdown
   ```

2. **Start fresh dev server:**
   ```powershell
   cd "C:\Users\HFRC\Desktop\arbitragex-v2-main (17)\frontend"
   npx next dev -p 3000
   ```

3. **Verify in browser:**
   - Open `http://localhost:3000` (NOT :5173)
   - Check sidebar shows "Pipeline", "Risk & Control", "Configuration"
   - Check DevTools Console for no hydration errors

### If hydration errors occur:

1. Check `app-sidebar.tsx` for non-deterministic SSR:
   - `Date.now()` → Move to `useEffect`
   - `window`/`document` → Move to `useEffect`
   - `Math.random()` → Move to `useEffect`

2. Check `layout.tsx` for similar issues in `SiteHeader`, `SiteFooter`

3. Use `suppressHydrationWarning` only on individual `<span>`, never on containers

## Prevention

### Always use port 3000

```powershell
# Create alias in PowerShell profile
function arbx-dev {
    cd "C:\Users\HFRC\Desktop\arbitragex-v2-main (17)\frontend"
    npx next dev -p 3000
}
```

### Verify before editing

Always run diagnostic before starting work:
```powershell
./diagnose-sidebar.ps1
```

### Never trust :5173

- :5173 = Docker/WSL stale build (edits don't apply)
- :3000 = Live Windows dev server (edits apply immediately)

## References

- Memory: `arbx-dev-server-5173-stale-docker.md`
- File: `frontend/components/app-sidebar.tsx`
- File: `frontend/app/globals.css`
