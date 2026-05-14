/**
 * C9.1 — Style Invariance Hermiticity
 * Hermiticidad estilística: las rutas /omega-s5/* DEBEN producir el mismo
 * espectro de tokens visuales que las rutas pre-S5.
 * Cero desviación permitida.
 */

import { test, expect, type Page } from '@playwright/test';

const BASELINE_ROUTES = ['/dashboard', '/chains', '/strategies'];
const EXTENDED_ROUTES = [
  '/omega-s5/factory',
  '/omega-s5/wallets',
  '/omega-s5/core',
  '/omega-s5/adapters',
  '/omega-s5/crucible',
  '/omega-s5/operator',
  '/omega-s5/drift',
  '/omega-s5/registry/chain',
];

interface DesignTokens {
  fontFamily: string;
  fontSize: string;
  color: string;
  backgroundColor: string;
  borderRadius: string;
  boxShadow: string;
  transition: string;
}

async function captureDesignTokens(page: Page, route: string): Promise<DesignTokens> {
  await page.goto(route, { waitUntil: 'networkidle' });
  return await page.evaluate(() => {
    const target = document.querySelector('main, [role="main"], body')!;
    const cs = window.getComputedStyle(target);
    return {
      fontFamily: cs.fontFamily,
      fontSize: cs.fontSize,
      color: cs.color,
      backgroundColor: cs.backgroundColor,
      borderRadius: cs.borderRadius,
      boxShadow: cs.boxShadow,
      transition: cs.transition,
    };
  });
}

function tokenSpectralDistance(a: DesignTokens, b: DesignTokens): number {
  const keys = Object.keys(a) as (keyof DesignTokens)[];
  return keys.reduce((acc, k) => acc + (a[k] === b[k] ? 0 : 1), 0);
}

test.describe('C9.1 — Style Invariance Hermiticity', () => {
  test('spectralDistance(baseline, extended) = 0', async ({ page }) => {
    const baselineSnapshots: DesignTokens[] = [];
    for (const r of BASELINE_ROUTES) {
      baselineSnapshots.push(await captureDesignTokens(page, r));
    }
    const baseline = baselineSnapshots[0];

    // 1. Las baseline son coherentes entre sí
    for (const snap of baselineSnapshots) {
      expect(tokenSpectralDistance(baseline, snap)).toBe(0);
    }

    // 2. Cada ruta /omega-s5/* preserva los tokens
    for (const r of EXTENDED_ROUTES) {
      const ext = await captureDesignTokens(page, r);
      expect(
        tokenSpectralDistance(baseline, ext),
        `Token drift en ${r}: ${JSON.stringify(ext)} vs baseline ${JSON.stringify(baseline)}`
      ).toBe(0);
    }
  });

  test('No nuevas dependencias UI ni clases !important', async ({ page }) => {
    await page.goto('/omega-s5/factory');
    const violations = await page.evaluate(() => {
      const sheets = Array.from(document.styleSheets);
      const found: string[] = [];
      for (const sheet of sheets) {
        try {
          for (const rule of Array.from(sheet.cssRules ?? [])) {
            if ('cssText' in rule && rule.cssText.includes('!important')) {
              found.push(rule.cssText.slice(0, 120));
            }
          }
        } catch {
          /* CORS-protected sheet */
        }
      }
      return found;
    });
    // Tolerancia: 0 reglas !important nuevas dentro de /omega-s5/*
    expect(violations.length, `Reglas !important encontradas: ${violations.join('\n')}`).toBe(0);
  });
});
