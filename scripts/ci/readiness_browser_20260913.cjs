// Public, unauthenticated, read-only browser reproduction. Never toggle LIVE.
const { chromium } = require('playwright');
const fs = require('node:fs');
const path = require('node:path');
const ORIGIN = 'https://arbx.ape-tv.net';
const out = 'readiness-evidence';
const target = /\/api\/(?:v1\/)?(?:scoring\/status|risk\/circuit-breakers\/status)$/;
const clean = value => { try { const u = new URL(value); return u.origin + u.pathname; } catch { return 'invalid_url'; } };
(async () => {
  fs.mkdirSync(out, {recursive: true});
  const browser = await chromium.launch({headless: true});
  const context = await browser.newContext({viewport: {width: 1440, height: 1000}});
  const report = {utc: new Date().toISOString(), origin: ORIGIN, browser: 'chromium',
    scope: 'unauthenticated_read_only', requests: [], panels: [], blocked_writes: 0};
  try {
    await context.route('**/*', route => {
      if (!['GET', 'HEAD', 'OPTIONS'].includes(route.request().method())) {
        report.blocked_writes++; return route.abort('blockedbyclient');
      }
      return route.continue();
    });
    const page = await context.newPage();
    const starts = new WeakMap();
    page.on('request', r => { if (target.test(new URL(r.url()).pathname)) starts.set(r, Date.now()); });
    page.on('requestfinished', r => { if (starts.has(r)) report.requests.push({
      path: clean(r.url()), status: r.response().then(v => v?.status()), ms: Date.now()-starts.get(r), result: 'finished'}); });
    page.on('requestfailed', r => { if (starts.has(r)) report.requests.push({path: clean(r.url()),
      ms: Date.now()-starts.get(r), result: 'failed', error: r.failure()?.errorText?.replace(/https?:\/\/\S+/g, '[url]')}); });
    const errors = {};
    page.on('pageerror', e => { errors[e.name] = (errors[e.name] || 0) + 1; });
    try {
      const response = await page.goto(ORIGIN + '/live-readiness', {waitUntil: 'domcontentloaded', timeout: 30000});
      report.page_http = response?.status();
      report.final_url = clean(page.url());
      await page.getByText('Confidence scoring (A.8)', {exact: false}).first().waitFor({timeout: 20000});
      await page.waitForTimeout(16000);
      for (const [label, name] of [['Confidence scoring (A.8)', 'scoring'], ['Risk circuit breakers (A.6)', 'breakers']]) {
        const heading = page.getByText(label, {exact: false}).first();
        if (await heading.count()) {
          const panel = heading.locator('xpath=../..');
          const text = await panel.innerText();
          report.panels.push({name, timeout_visible: text.includes('edge timeout after'),
            fetch_error_visible: /Cannot fetch/.test(text), text_length: text.length});
          await panel.screenshot({path: path.join(out, name + '.png'), timeout: 10000});
        }
      }
    } catch (e) { report.page_error = e.name; }
    report.javascript_errors = errors;
    for (const route of ['/api/scoring/status', '/api/risk/circuit-breakers/status']) {
      const started = Date.now();
      try {
        const r = await context.request.get(ORIGIN + route, {timeout: 10000, maxRedirects: 0});
        report.requests.push({path: route, method: 'APIRequestContext.GET', status: r.status(), ms: Date.now()-started});
      } catch (e) { report.requests.push({path: route, method: 'APIRequestContext.GET', error: e.name, ms: Date.now()-started}); }
    }
    for (const r of report.requests) if (r.status && typeof r.status.then === 'function') r.status = await r.status;
    fs.writeFileSync(path.join(out,'browser.json'), JSON.stringify(report,null,2));
    console.log(JSON.stringify(report,null,2));
  } finally { await context.close(); await browser.close(); }
})().catch(e => { console.error(e.name); process.exitCode = 1; });
