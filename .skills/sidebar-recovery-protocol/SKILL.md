# Sidebar Recovery Protocol

## Description
Emergency recovery skill for broken sidebar state in Next.js frontend. Detects hydration mismatches, stale builds, and process conflicts. Restores from last known good commit and verifies with Playwright.

## Triggers
- Sidebar not rendering or showing stale data
- Hydration mismatch errors in console
- `EADDRINUSE` errors on port 5173
- Blank page after navigation
- `TypeError: Cannot read properties of undefined` in sidebar components

---

## Phase 1: Detect Broken State

### 1.1 Check for Stale Docker Container
```powershell
# Check if port 5173 is occupied by Docker (stale build)
Get-NetTCPConnection -LocalPort 5173 -ErrorAction SilentlyContinue | Select-Object LocalPort, OwningProcess, @{Name="ProcessName";Expression={(Get-Process -Id $_.OwningProcess -ErrorAction SilentlyContinue).Name}}
```

**If `com.docker.backend` or `wslrelay` owns port 5173:**
- The port is serving a STALE Docker build from an older Linux clone
- Editing the current frontend will NOT reach the browser
- Must kill Docker or use alternate port

### 1.2 Check for Hydration Mismatch
Look for these errors in browser console:
- `Warning: Text content did not match`
- `Hydration failed because the initial UI does not match`
- `Error: There was an error while hydrating`

### 1.3 Check Git Status
```powershell
cd "C:\Users\HFRC\Desktop\arbitragex-v2-main (17)"
git status
git log --oneline -5
```

---

## Phase 2: Emergency Process Cleanup

### 2.1 Kill All Node/Next.js Processes
```powershell
# Find and kill all node processes
Get-Process node -ErrorAction SilentlyContinue | Stop-Process -Force
Get-Process next -ErrorAction SilentlyContinue | Stop-Process -Force

# Verify port is free
Get-NetTCPConnection -LocalPort 5173 -ErrorAction SilentlyContinue
# Should return nothing if port is free
```

### 2.2 Kill Stale Docker Container (if applicable)
```powershell
# List running containers
docker ps

# Stop any stale frontend containers
docker stop $(docker ps -q --filter "name=frontend")
docker rm $(docker ps -aq --filter "name=frontend")
```

### 2.3 Clear Next.js Cache
```powershell
cd "C:\Users\HFRC\Desktop\arbitragex-v2-main (17)\frontend"
Remove-Item -Recurse -Force .next -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force node_modules\.cache -ErrorAction SilentlyContinue
```

---

## Phase 3: Restore from Last Known Good

### 3.1 Identify Last Known Good Commit
```powershell
cd "C:\Users\HFRC\Desktop\arbitragex-v2-main (17)"

# View recent commits
git log --oneline -10

# Check if main branch has the fix
git log github/main --oneline -5

# IMPORTANT: Use github/main NOT origin/main
# origin points to stale VPS mirror
git fetch github main
```

### 3.2 Hard Reset to Known Good (Destructive - discards local changes)
```powershell
cd "C:\Users\HFRC\Desktop\arbitragex-v2-main (17)"

# Reset to github main (canonical source)
git reset --hard github/main

# Or reset to specific known good commit
git reset --hard <commit-hash>
```

### 3.3 Clean Untracked Files (Optional)
```powershell
cd "C:\Users\HFRC\Desktop\arbitragex-v2-main (17)"

# Preview what will be deleted
git clean -n -d

# Actually delete untracked files
git clean -f -d
```

---

## Phase 4: Fresh Dependency Install

### 4.1 Clean Install Node Modules
```powershell
cd "C:\Users\HFRC\Desktop\arbitragex-v2-main (17)\frontend"

# Remove node_modules completely
Remove-Item -Recurse -Force node_modules -ErrorAction SilentlyContinue
Remove-Item package-lock.json -ErrorAction SilentlyContinue

# Fresh install
npm install
```

### 4.2 Verify TypeScript Types
```powershell
cd "C:\Users\HFRC\Desktop\arbitragex-v2-main (17)\frontend"
npx tsc --noEmit
```

### 4.3 Run Vitest (if applicable)
```powershell
cd "C:\Users\HFRC\Desktop\arbitragex-v2-main (17)\frontend"
npx vitest run
```

---

## Phase 5: Start Dev Server Properly

### 5.1 Option A: Use Alternate Port (Recommended if 5173 is blocked)
```powershell
cd "C:\Users\HFRC\Desktop\arbitragex-v2-main (17)\frontend"

# Use port 3000 instead of 5173
npx next dev -p 3000
```

### 5.2 Option B: Force Port 5173 (after cleanup)
```powershell
cd "C:\Users\HFRC\Desktop\arbitragex-v2-main (17)\frontend"

# Ensure port is free first
Get-NetTCPConnection -LocalPort 5173 -ErrorAction SilentlyContinue
# Should return nothing

# Start on default port (package.json script uses -p 5173)
npm run dev
```

### 5.3 Wait for Ready Signal
Watch for these messages in terminal:
```
ready - started server on 0.0.0.0:3000, url: http://localhost:3000
ready - started server on 0.0.0.0:5173, url: http://localhost:5173
```

---

## Phase 6: Playwright Verification

### 6.1 Create Verification Script
Save as `C:\Users\HFRC\Desktop\arbitragex-v2-main (17)\tests\e2e\sidebar-recovery.spec.ts`:

```typescript
import { test, expect } from '@playwright/test';

const BASE_URL = process.env.TEST_BASE_URL || 'http://localhost:3000';

test.describe('Sidebar Recovery Verification', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto(BASE_URL);
    // Wait for hydration
    await page.waitForLoadState('networkidle');
  });

  test('sidebar renders without hydration errors', async ({ page }) => {
    // Check for hydration errors in console
    const consoleErrors: string[] = [];
    page.on('console', msg => {
      if (msg.type() === 'error') {
        consoleErrors.push(msg.text());
      }
    });
    page.on('pageerror', error => {
      consoleErrors.push(error.message);
    });

    // Navigate to trigger any lazy-loaded sidebar components
    await page.goto(`${BASE_URL}/opportunities`);
    await page.waitForLoadState('networkidle');

    // Verify no hydration errors
    const hydrationErrors = consoleErrors.filter(e => 
      e.includes('hydrat') || 
      e.includes('did not match') ||
      e.includes('Text content')
    );
    expect(hydrationErrors).toHaveLength(0);
  });

  test('sidebar navigation works', async ({ page }) => {
    // Check sidebar is visible
    const sidebar = page.locator('[data-testid="sidebar"], aside, [class*="sidebar"]').first();
    await expect(sidebar).toBeVisible();

    // Test navigation items exist
    const navItems = page.locator('nav a, [role="navigation"] a, .sidebar a');
    const count = await navItems.count();
    expect(count).toBeGreaterThan(0);
  });

  test('no 404 errors on page load', async ({ page }) => {
    const failedRequests: string[] = [];
    page.on('response', response => {
      if (response.status() >= 400) {
        failedRequests.push(`${response.status()}: ${response.url()}`);
      }
    });

    await page.goto(BASE_URL);
    await page.waitForLoadState('networkidle');

    // Filter out expected API calls that may 404 in dev
    const unexpectedErrors = failedRequests.filter(url => 
      !url.includes('/api/') && 
      !url.includes('favicon')
    );
    expect(unexpectedErrors).toHaveLength(0);
  });
});
```

### 6.2 Run Verification
```powershell
cd "C:\Users\HFRC\Desktop\arbitragex-v2-main (17)\tests\e2e"

# Install Playwright if needed
npm install
npx playwright install chromium

# Run verification
$env:TEST_BASE_URL="http://localhost:3000"
npx playwright test sidebar-recovery.spec.ts --project=chromium
```

### 6.3 Manual Verification Checklist
- [ ] Page loads without console errors
- [ ] Sidebar is visible on left side
- [ ] Navigation links are clickable
- [ ] Active state shows correctly
- [ ] No "undefined" or "null" text visible
- [ ] No flashing or layout shift

---

## Phase 7: Prevention (Post-Recovery)

### 7.1 Update package.json Scripts
Ensure `package.json` has a non-conflicting port option:
```json
{
  "scripts": {
    "dev": "next dev -p 5173",
    "dev:alt": "next dev -p 3000",
    "dev:clean": "rimraf .next && next dev -p 5173"
  }
}
```

### 7.2 Add Pre-Dev Check Script
Create `scripts/pre-dev-check.ps1`:
```powershell
$port = 5173
$connection = Get-NetTCPConnection -LocalPort $port -ErrorAction SilentlyContinue
if ($connection) {
    $process = Get-Process -Id $connection.OwningProcess -ErrorAction SilentlyContinue
    Write-Host "WARNING: Port $port is occupied by $($process.Name) (PID: $($process.Id))"
    Write-Host "Run: Get-Process $($process.Name) | Stop-Process -Force"
    exit 1
}
Write-Host "Port $port is free - safe to start dev server"
exit 0
```

### 7.3 Git Hooks (Optional)
Prevent commits with known-bad patterns:
```bash
# .git/hooks/pre-commit
#!/bin/sh
if git diff --cached --name-only | grep -q "sidebar"; then
    echo "Sidebar changes detected - running type check..."
    cd frontend && npx tsc --noEmit || exit 1
fi
```

---

## Quick Reference: Recovery Commands

### Full Reset (Nuclear Option)
```powershell
cd "C:\Users\HFRC\Desktop\arbitragex-v2-main (17)"
Get-Process node -ErrorAction SilentlyContinue | Stop-Process -Force
git fetch github main
git reset --hard github/main
cd frontend
Remove-Item -Recurse -Force node_modules,.next -ErrorAction SilentlyContinue
npm install
npx next dev -p 3000
```

### Soft Reset (Keep Changes)
```powershell
cd "C:\Users\HFRC\Desktop\arbitragex-v2-main (17)"
Get-Process node -ErrorAction SilentlyContinue | Stop-Process -Force
Remove-Item -Recurse -Force frontend/.next -ErrorAction SilentlyContinue
cd frontend
npx next dev -p 3000
```

### Port Conflict Resolution
```powershell
# Find what's using port 5173
Get-NetTCPConnection -LocalPort 5173 | Select-Object LocalPort, OwningProcess, @{Name="ProcessName";Expression={(Get-Process -Id $_.OwningProcess).Name}}

# Kill specific process by PID
Stop-Process -Id <PID> -Force
```

---

## Common Error Patterns

| Error | Cause | Solution |
|-------|-------|----------|
| `EADDRINUSE: port 5173` | Stale Docker or node process | Kill process, use alt port |
| `Text content did not match` | Hydration mismatch | Check for `Date.now()`, `Math.random()` in render |
| `Cannot read properties of undefined` | Missing data in sidebar | Check API response, add null checks |
| Blank white page | Build cache corruption | Clear `.next` folder, restart |
| Sidebar flashes then disappears | Client/server mismatch | Move non-deterministic code to useEffect |

---

## Related Skills
- `sidebar-crisis-prevention` - Prevent sidebar issues before they occur
- `01-hydration-forensics-expert` - Deep hydration mismatch analysis
- `arbx-frontend-vitest-jsx-runtime-gotcha` - JSX runtime issues
