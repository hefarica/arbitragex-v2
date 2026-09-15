import { test, expect } from '@playwright/test';

// This laboratory regression never targets a public domain or a real pool.
test.beforeEach(() => {
  const target = new URL(process.env.E2E_BASE_URL ?? 'http://localhost:3000');
  expect(['localhost', '127.0.0.1', '[::1]']).toContain(target.hostname);
});
const row = { id: '1', chain_id: '1', label: 'Recovery fixture chain', active: true,
  registered: true, dex_count: '0', factory_count: '0', pool_count: '0' };
function firstPage() {
  return { schema_version: 1, source: 'postgresql-registry', level: 'chains',
    observed_at: '2026-09-14T00:00:00.000Z', execution_verified: false, counts_include_inactive: true,
    scope: { chain_id: null, dex_id: null, q: '' }, count: 1, limit: 25, items: [row], next_after: '1' };
}
for (const failure of ['http-503', 'invalid-payload'] as const) {
  test(`previous page remains reachable after ${failure} without stale rows`, async ({ page }) => {
    const cursors: (string | null)[] = [];
    await page.route('**/api/v1/pools?*', async route => {
      const url = new URL(route.request().url());
      if (url.searchParams.get('view') !== 'liquidity_catalog') { await route.continue(); return; }
      expect(route.request().method()).toBe('GET');
      const after = url.searchParams.get('after'); cursors.push(after);
      if (after !== null) {
        await route.fulfill(failure === 'http-503'
          ? { status: 503, json: { error: 'catalog_unavailable' } }
          : { status: 200, json: { count: 0, items: [] } });
      } else { await route.fulfill({ json: firstPage() }); }
    });
    await page.goto('/dex-registry');
    const panel = page.getByTestId('liquidity-catalog');
    await expect(panel.getByText(row.label, { exact: true })).toBeVisible();
    await panel.getByRole('button', { name: 'Siguiente', exact: true }).click();
    await expect(panel.getByRole('alert')).toBeVisible();
    await expect(panel.getByText(row.label, { exact: true })).toHaveCount(0);
    await expect(panel.getByText('Página 2', { exact: true })).toBeVisible();
    await expect(panel.getByRole('button', { name: 'Siguiente', exact: true })).toBeDisabled();
    const previous = panel.getByRole('button', { name: 'Anterior', exact: true });
    await expect(previous).toBeEnabled(); await previous.click();
    await expect(panel.getByText(row.label, { exact: true })).toBeVisible();
    await expect(panel.getByRole('alert')).toHaveCount(0);
    await expect(previous).toBeDisabled();
    expect(cursors).toEqual([null, '1', null]);
  });
}
