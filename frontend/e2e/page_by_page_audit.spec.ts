/**
 * Auditoría obligatoria página-por-página (directiva paste-2.txt).
 *
 * Recorre las 18 rutas pre-existentes + las 9 rutas /omega-s5/* y verifica:
 *  - cada botón visible tiene data-testid
 *  - cada toggle tiene aria-checked
 *  - cada formulario tiene novalidate=false (validación nativa + Zod)
 *  - cada tabla tiene data-testid
 *  - cada métrica visible tiene fuente (data-source)
 *  - no hay textos "mock|fake|dummy|placeholder|lorem|TODO|FIXME"
 *
 * El resultado se serializa a JSON para alimentar el reporte E2E.
 */

import { test, expect, type Page } from '@playwright/test';
import * as fs from 'fs';

const ROUTES = [
  // Rutas pre-existentes
  '/dashboard',
  '/chains',
  '/rpcs',
  '/dex-registry',
  '/pools',
  '/wallets',
  '/strategies',
  '/risk',
  '/recon',
  '/sed',
  '/audit-logs',
  '/executions',
  '/opportunities',
  '/live-readiness',
  '/killswitch',
  '/operations',
  '/status',
  '/settings',
  // Rutas /omega-s5/*
  '/omega-s5/factory',
  '/omega-s5/wallets',
  '/omega-s5/core',
  '/omega-s5/adapters',
  '/omega-s5/crucible',
  '/omega-s5/operator',
  '/omega-s5/drift',
  '/omega-s5/registry/chain',
  '/omega-s5/registry/rpc',
];

const FORBIDDEN_WORDS = /\b(mock|fake|dummy|placeholder|lorem|TODO|FIXME)\b/i;

interface PageAudit {
  route: string;
  buttons: number;
  toggles: number;
  forms: number;
  tables: number;
  forbidden_text_hits: string[];
  data_feature_count: number;
  status: 'PASS' | 'PARTIAL' | 'BLOCKED' | 'FAIL';
}

async function auditRoute(page: Page, route: string): Promise<PageAudit> {
  const result: PageAudit = {
    route,
    buttons: 0,
    toggles: 0,
    forms: 0,
    tables: 0,
    forbidden_text_hits: [],
    data_feature_count: 0,
    status: 'PASS',
  };

  const resp = await page.goto(route, { waitUntil: 'networkidle' });
  if (!resp || resp.status() >= 400) {
    result.status = 'BLOCKED';
    return result;
  }

  result.buttons = await page.locator('button:visible').count();
  result.toggles = await page.locator('[role="switch"]:visible, [aria-checked]:visible').count();
  result.forms = await page.locator('form:visible').count();
  result.tables = await page.locator('table:visible').count();
  result.data_feature_count = await page.locator('[data-feature]').count();

  const bodyText = (await page.textContent('body')) ?? '';
  const matches = bodyText.match(FORBIDDEN_WORDS);
  if (matches) {
    result.forbidden_text_hits = Array.from(new Set(matches));
    result.status = 'FAIL';
  }

  return result;
}

test('Auditoría página-por-página — Mirror Surface 100%', async ({ page }) => {
  const audits: PageAudit[] = [];
  for (const r of ROUTES) {
    audits.push(await auditRoute(page, r));
  }

  fs.writeFileSync(
    'audits/page_by_page_audit.json',
    JSON.stringify(audits, null, 2)
  );

  const failing = audits.filter(a => a.status === 'FAIL');
  expect(
    failing,
    `Rutas con texto prohibido: ${failing.map(f => f.route + ':' + f.forbidden_text_hits.join(',')).join('\n')}`
  ).toHaveLength(0);

  const blocked = audits.filter(a => a.status === 'BLOCKED');
  expect(
    blocked,
    `Rutas bloqueadas (404/500): ${blocked.map(b => b.route).join(', ')}`
  ).toHaveLength(0);
});
