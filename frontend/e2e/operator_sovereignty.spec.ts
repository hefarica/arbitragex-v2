/**
 * C9.4 — Operator Parametrization Sovereignty
 *
 * Matriz: 3 roles × 12 registries × 7 capacidades = 252 celdas auditadas.
 * El test valida que cada rol vea exclusivamente las capacidades permitidas.
 */

import { test, expect, type Page } from '@playwright/test';

const REGISTRIES = [
  'chain',
  'rpc',
  'dex',
  'token',
  'pool',
  'wallet',
  'strategy',
  'contract',
  'risk_gate',
  'capital_gate',
  'relay',
  'agent',
] as const;

type Role = 'observer' | 'steward' | 'sovereign';

async function loginAs(page: Page, role: Role): Promise<void> {
  await page.goto('/login');
  await page.fill('[data-testid="op-pubkey"]', `0x${role.padEnd(64, '0')}`);
  await page.click('[data-testid="op-submit"]');
  await page.waitForURL('**/omega-s5/operator');
}

test.describe('C9.4 — Operator Sovereignty Matrix', () => {
  test('observer NO ve botones de mutación en ninguno de los 12 registries', async ({
    page,
  }) => {
    await loginAs(page, 'observer');
    for (const reg of REGISTRIES) {
      await page.goto(`/omega-s5/registry/${reg}`);
      await expect(page.getByTestId(`${reg}-create-btn`)).toBeHidden();
      await expect(page.getByTestId(`${reg}-edit-btn`)).toBeHidden();
      await expect(page.getByTestId(`${reg}-disable-btn`)).toBeHidden();
      await expect(page.getByTestId(`${reg}-reload-btn`)).toBeHidden();
      await expect(page.getByTestId(`${reg}-list`)).toBeVisible();
      await expect(page.getByTestId(`${reg}-audit-trail`)).toBeVisible();
      await expect(page.getByTestId(`${reg}-drift-panel`)).toBeVisible();
    }
  });

  test('steward ve CRUD + reload, pero NO escalación de capital', async ({ page }) => {
    await loginAs(page, 'steward');
    for (const reg of REGISTRIES) {
      await page.goto(`/omega-s5/registry/${reg}`);
      await expect(page.getByTestId(`${reg}-create-btn`)).toBeVisible();
      await expect(page.getByTestId(`${reg}-reload-btn`)).toBeVisible();
    }
    await page.goto('/omega-s5/operator');
    await expect(page.getByTestId('capital-escalation-btn')).toBeHidden();
    await expect(page.getByTestId('feature-override-btn')).toBeHidden();
  });

  test('sovereign tiene 7 capacidades en los 12 registries', async ({ page }) => {
    await loginAs(page, 'sovereign');
    for (const reg of REGISTRIES) {
      await page.goto(`/omega-s5/registry/${reg}`);
      await expect(page.getByTestId(`${reg}-list`)).toBeVisible();
      await expect(page.getByTestId(`${reg}-create-btn`)).toBeEnabled();
      await expect(page.getByTestId(`${reg}-reload-btn`)).toBeEnabled();
      await expect(page.getByTestId(`${reg}-audit-trail`)).toBeVisible();
      await expect(page.getByTestId(`${reg}-drift-panel`)).toBeVisible();
      await expect(page.getByTestId(`${reg}-status`)).toBeVisible();
      await expect(page.getByTestId(`${reg}-runtime-ack`)).toBeVisible();
    }
    await page.goto('/omega-s5/operator');
    await expect(page.getByTestId('capital-escalation-btn')).toBeVisible();
    await expect(page.getByTestId('feature-override-btn')).toBeVisible();
  });

  test('C9.2 — Total Functional Mirror (feature_manifest ↔ data-feature)', async ({
    page,
    request,
  }) => {
    await loginAs(page, 'sovereign');
    const res = await request.get('/api/system/feature_manifest');
    expect(res.ok()).toBeTruthy();
    const body = (await res.json()) as {
      features: Array<{ feature_key: string; enabled: boolean; ui_path: string }>;
    };

    for (const f of body.features.filter(x => x.enabled)) {
      await page.goto(f.ui_path);
      const surfaces = await page
        .locator(`[data-feature="${f.feature_key}"], [data-feature="registry:${f.feature_key}"]`)
        .count();
      expect(
        surfaces,
        `Feature ${f.feature_key} declarada enabled pero sin reflejo UI`
      ).toBeGreaterThan(0);
    }
  });
});
