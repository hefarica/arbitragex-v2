#!/usr/bin/env python3
"""
Script de verificacion del sidebar usando Playwright.
Navega a localhost:5173, toma screenshots y verifica la visibilidad del sidebar.
"""

import asyncio
import os
import sys
from pathlib import Path

from playwright.async_api import async_playwright

# Rutas de salida
BASE_DIR = Path("C:/Users/HFRC/Desktop/arbitragex-v2-main (17)")
SCREENSHOTS_DIR = BASE_DIR / "docs" / "crisis" / "verification-screenshots"
FULL_PAGE_PATH = SCREENSHOTS_DIR / "full-page.png"
SIDEBAR_FOCUSED_PATH = SCREENSHOTS_DIR / "sidebar-focused.png"

# Asegurar que el directorio existe
SCREENSHOTS_DIR.mkdir(parents=True, exist_ok=True)


async def verify_sidebar():
    """Verifica el sidebar en localhost:5173"""

    console_errors = []
    mime_errors = []

    async with async_playwright() as p:
        # Lanzar navegador
        browser = await p.chromium.launch(headless=True)
        context = await browser.new_context(
            viewport={"width": 1920, "height": 1080}
        )

        # Capturar errores de consola
        page = await context.new_page()

        page.on("console", lambda msg: console_errors.append({
            "type": msg.type,
            "text": msg.text,
            "location": msg.location
        }))

        page.on("pageerror", lambda err: console_errors.append({
            "type": "pageerror",
            "text": str(err),
            "location": {}
        }))

        page.on("response", lambda response: (
            mime_errors.append({
                "url": response.url,
                "status": response.status,
                "content_type": response.headers.get("content-type", "unknown")
            })
            if response.status == 200 and "javascript" in response.headers.get("content-type", "").lower()
            and not response.url.endswith(".js") and not response.url.endswith(".mjs")
            else None
        ))

        print("Navegando a http://localhost:3000...")

        try:
            # Navegar a la pagina con timeout extendido
            response = await page.goto(
                "http://localhost:3000",
                wait_until="networkidle",
                timeout=60000
            )

            if response:
                print(f"  Status: {response.status}")
                print(f"  URL: {response.url}")
            else:
                print("ERROR: No se pudo cargar la pagina")
                return False

        except Exception as e:
            print(f"ERROR al navegar: {e}")
            return False

        # Esperar un momento para que todo cargue
        await page.wait_for_timeout(3000)

        # Verificar si el sidebar existe
        print("\nVerificando sidebar...")

        # Multiples selectores posibles para el sidebar
        sidebar_selectors = [
            "[data-sidebar]",
            "aside",
            "nav[aria-label]",
            ".sidebar",
            "#sidebar",
            '[class*="sidebar"]',
            "[data-slot='sidebar']",
        ]

        sidebar_found = False
        sidebar_selector = None

        for selector in sidebar_selectors:
            try:
                element = await page.query_selector(selector)
                if element:
                    is_visible = await element.is_visible()
                    if is_visible:
                        sidebar_found = True
                        sidebar_selector = selector
                        print(f"  [OK] Sidebar encontrado con selector: {selector}")
                        break
            except Exception:
                continue

        if not sidebar_found:
            print("  [FAIL] No se encontro sidebar visible")
            # Tomar screenshot de todas formas para diagnostico

        # Tomar screenshot de pagina completa
        print("\nTomando screenshot de pagina completa...")
        await page.screenshot(path=str(FULL_PAGE_PATH), full_page=True)
        print(f"  [OK] Guardado: {FULL_PAGE_PATH}")

        # Tomar screenshot enfocado en el sidebar si existe
        if sidebar_found and sidebar_selector:
            print("\nTomando screenshot del sidebar...")
            try:
                sidebar = await page.query_selector(sidebar_selector)
                if sidebar:
                    await sidebar.screenshot(path=str(SIDEBAR_FOCUSED_PATH))
                    print(f"  [OK] Guardado: {SIDEBAR_FOCUSED_PATH}")
            except Exception as e:
                print(f"  [FAIL] Error al capturar sidebar: {e}")

        # Verificar errores de MIME type
        print("\nVerificando errores de MIME type...")

        js_mime_errors = [
            err for err in console_errors
            if "mime" in err.get("text", "").lower()
            or "javascript" in err.get("text", "").lower()
            or "module" in err.get("text", "").lower()
        ]

        if js_mime_errors:
            print("  [FAIL] Se encontraron errores de MIME:")
            for err in js_mime_errors[:5]:  # Mostrar maximo 5
                print(f"    - {err['type']}: {err['text'][:100]}")
        else:
            print("  [OK] No hay errores de MIME type")

        # Reportar otros errores de consola
        print("\nErrores de consola (excluyendo logs informativos):")
        serious_errors = [
            e for e in console_errors
            if e["type"] in ["error", "pageerror"]
        ]

        if serious_errors:
            print(f"  [FAIL] Se encontraron {len(serious_errors)} errores:")
            for err in serious_errors[:5]:
                print(f"    [{err['type']}] {err['text'][:120]}")
        else:
            print("  [OK] No hay errores graves en consola")

        # Resumen final
        print("\n" + "="*60)
        print("RESUMEN DE VERIFICACION:")
        print("="*60)

        if sidebar_found:
            print("  [OK] Sidebar: VISIBLE")
        else:
            print("  [FAIL] Sidebar: NO ENCONTRADO")

        if not js_mime_errors and not serious_errors:
            print("  [OK] Errores: NINGUNO")
        else:
            print(f"  [FAIL] Errores: {len(js_mime_errors) + len(serious_errors)} detectados")

        print("\n  Screenshots guardados en:")
        print(f"    - {FULL_PAGE_PATH}")
        if sidebar_found:
            print(f"    - {SIDEBAR_FOCUSED_PATH}")

        await browser.close()

        return sidebar_found and not js_mime_errors and len(serious_errors) == 0


if __name__ == "__main__":
    try:
        success = asyncio.run(verify_sidebar())
        sys.exit(0 if success else 1)
    except KeyboardInterrupt:
        print("\nInterrumpido por usuario")
        sys.exit(130)
    except Exception as e:
        print(f"\nError fatal: {e}")
        sys.exit(1)
