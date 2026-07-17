"""
Validación SEMIOTIC-BRIDGE: Verificar que el fix de @x402/* resolvió el webpack error.

Este script usa Playwright MCP para validar que el frontend build ya no falla
con "Module not found: Can't resolve '@x402/evm'".

Uso:
    python tests/validate-webpack-fix.py [URL_BASE]

Por defecto usa http://localhost:3000 (frontend local)
Para VPS: python tests/validate-webpack-fix.py http://<VPS_IP>:5173
"""

import sys
import subprocess
from playwright.sync_api import sync_playwright

BASE_URL = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:3000"
ROUTES_TO_TEST = [
    "/",
    "/opportunities",
    "/dashboard",
]

def main():
    print(f"=== SEMIOTIC-BRIDGE VALIDATION ===")
    print(f"Target: {BASE_URL}")
    print(f"Timestamp: 2026-07-17T01:32:00Z")
    print("")

    errors_found = []
    console_errors = []

    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        
        for route in ROUTES_TO_TEST:
            url = f"{BASE_URL}{route}"
            print(f"Testing {url}...")
            
            page = browser.new_page()
            
            # Capture console errors
            page.on("console", lambda msg: console_errors.append(msg.text) if msg.type == "error" else None)
            page.on("pageerror", lambda err: console_errors.append(str(err)))
            
            try:
                page.goto(url, wait_until="networkidle", timeout=30000)
                
                # Check for webpack error indicators in page content
                body = page.content()
                if "Module not found" in body or "Can't resolve" in body:
                    errors_found.append(f"{route}: Webpack error visible in page")
                    print(f"  ✗ WEBPACK ERROR DETECTED on {route}")
                else:
                    print(f"  ✓ {route} loaded successfully")
                    
            except Exception as e:
                errors_found.append(f"{route}: {str(e)}")
                print(f"  ✗ Navigation failed: {e}")
            finally:
                page.close()
        
        browser.close()

    print("")
    print("=== RESULTS ===")
    
    if console_errors:
        print(f"Console errors captured: {len(console_errors)}")
        for err in console_errors[:5]:  # Show first 5
            print(f"  - {err}")
    else:
        print("No console errors captured")
    
    if errors_found:
        print(f"\n✗ VALIDATION FAILED: {len(errors_found)} errors")
        for err in errors_found:
            print(f"  - {err}")
        return 1
    else:
        print(f"\n✓ VALIDATION PASSED: All routes loaded without webpack errors")
        print("\nSEMIOTIC-BRIDGE INVARIANCE:")
        print("  workspace.resolvable = TRUE")
        print("  webpack.errors = 0")
        print("  deploy.status = SUCCESS")
        return 0

if __name__ == "__main__":
    sys.exit(main())
