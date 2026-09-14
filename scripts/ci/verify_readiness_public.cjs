// Public GET-only E2E of A.8/A.6. No wallet, sign-off, credential or LIVE writes.
const { chromium } = require('playwright');
const fs = require('node:fs');
const path = require('node:path');
const { createHash } = require('node:crypto');
const S = require(path.resolve(process.env.READINESS_SCHEMA_BUNDLE));
const origin = 'https://arbx.ape-tv.net';
const expectedSha = process.env.EXPECTED_DEPLOY_SHA;
if (!/^[0-9a-f]{40}$/.test(expectedSha || '')) throw new Error('expected SHA required');
const out = 'readiness-verification';
const routes = new Map([
  ['/api/scoring/status', S.ScoringStatusResponseSchema],
  ['/api/risk/circuit-breakers/status', S.CircuitBreakersStatusResponseSchema],
]);
const clean = url => { const u = new URL(url); return u.origin + u.pathname; };
const report = { utc: new Date().toISOString(), expected_sha: expectedSha,
  source_sha: process.env.GITHUB_SHA || null, origin, requests: [], panels: [],
  blocked_writes: 0, failures: [], javascript_error_names: [] };
(async () => {
  fs.mkdirSync(out, { recursive: true });
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ viewport: { width: 1440, height: 1000 } });
  const pending = [], starts = new WeakMap();
  try {
    await context.route('**/*', route => {
      if (!['GET', 'HEAD', 'OPTIONS'].includes(route.request().method())) {
        report.blocked_writes++;
        return route.abort('blockedbyclient');
      }
      return route.continue();
    });
    const page = await context.newPage();
    const trackedPath = request => {
      const u = new URL(request.url());
      return u.origin === origin && routes.has(u.pathname) ? u.pathname : null;
    };
    page.on('pageerror', e => report.javascript_error_names.push(e.name));
    page.on('request', request => {
      if (trackedPath(request)) starts.set(request, performance.now());
    });
    page.on('requestfailed', request => {
      const p = trackedPath(request);
      if (!p || !starts.has(request)) return;
      report.requests.push({ path: p, result: 'failed', ms: Math.round(performance.now() - starts.get(request)) });
    });
    page.on('requestfinished', request => {
      const p = trackedPath(request);
      if (!p || !starts.has(request)) return;
      const ms = Math.round(performance.now() - starts.get(request));
      pending.push((async () => {
        const row = { path: p, result: 'finished', ms };
        try {
          const response = await request.response();
          row.status = response.status();
          const raw = await response.body();
          row.bytes = raw.length;
          row.sha256 = createHash('sha256').update(raw).digest('hex');
          const parsed = routes.get(p).safeParse(JSON.parse(raw.toString('utf8')));
          row.schema_valid = parsed.success;
          if (!parsed.success) row.issues = parsed.error.issues.map(i => ({path: i.path, code: i.code}));
        } catch (e) { row.decode_error = e.name; }
        report.requests.push(row);
      })());
    });
    const response = await page.goto(origin + '/live-readiness', { waitUntil: 'domcontentloaded', timeout: 30000 });
    report.page_http = response?.status();
    report.final_url = clean(page.url());
    if (report.page_http !== 200 || report.final_url !== origin + '/live-readiness') report.failures.push('public_page_identity');
    await page.locator('[data-slot="confidence-scoring-panel"]').waitFor({ timeout: 20000 });
    // Covers the original 3 x 5s retry chain and at least one A.6 polling interval.
    await page.waitForTimeout(17000);
    for (const [slot, readyText] of [
      ['confidence-scoring-panel', 'Wire coverage'],
      ['risk-circuit-panel', 'global_kill_switch'],
    ]) {
      const panel = page.locator(`[data-slot="${slot}"]`);
      const text = await panel.innerText();
      const row = { slot, fetch_error_visible: /Cannot fetch|edge timeout after/.test(text),
        loading_visible: /Loading scoring wire status|Loading circuit breaker status/.test(text) };
      if (slot === 'confidence-scoring-panel') row.data_visible = text.includes(readyText);
      else row.data_visible = (await panel.locator('[data-slot="accordion-item"]').count()) > 0;
      report.panels.push(row);
      if (row.fetch_error_visible || row.loading_visible || !row.data_visible) report.failures.push(slot);
      await panel.screenshot({ path: path.join(out, `${slot}.png`), timeout: 10000 });
    }
    // Version check is independent of a successful build or checkout.
    try {
      const status = await context.request.get(origin + '/api/status', { timeout: 10000, maxRedirects: 0 });
      const parsed = S.StatusResponseSchema.safeParse(await status.json());
      report.declared_deploy_sha = parsed.success ? parsed.data.deploy?.sha || null : null;
      report.declared_deploy_id = parsed.success ? parsed.data.deploy?.id || null : null;
      if (status.status() !== 200 || report.declared_deploy_sha !== expectedSha) report.failures.push('served_api_sha_not_verified');
    } catch (e) { report.failures.push('served_api_sha_unavailable'); }
    await Promise.all(pending);
    for (const p of routes.keys()) {
      const rows = report.requests.filter(r => r.path === p);
      if (!rows.length || rows.some(r => r.result !== 'finished' || r.status !== 200 || !r.schema_valid || r.ms >= 5000)) {
        report.failures.push(`endpoint_or_5000ms_budget:${p}`);
      }
    }
  } catch (e) {
    report.failures.push(`browser:${e.name}`);
  } finally {
    await context.close();
    await Promise.allSettled(pending);
    await browser.close();
    report.completed_at = new Date().toISOString();
    report.success = report.failures.length === 0;
    fs.writeFileSync(path.join(out, 'result.json'), JSON.stringify(report, null, 2));
    console.log(JSON.stringify(report, null, 2));
    process.exitCode = report.success ? 0 : 1;
  }
})().catch(e => { console.error(e.name); process.exitCode = 1; });
