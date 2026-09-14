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
  blocked_writes: 0, failures: [], javascript_error_names: [], fresh_reads: [] };
(async () => {
  fs.mkdirSync(out, { recursive: true });
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ viewport: { width: 1440, height: 1000 } });
  const pending = [], starts = new WeakMap();
  let firstScoring = null;
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
          if (p === '/api/scoring/status' && parsed.success) {
            const d = parsed.data;
            row.scoring_version = d.scoring_version;
            row.generated_at = d.generated_at;
            row.count = d.recent_scored_count;
            row.count_exact = d.recent_scored_count_exact;
            row.count_limit = d.scoring_count_limit;
            if (!firstScoring) firstScoring = d;
            if (d.scoring_version !== '0.3.0-bounded-evidence') report.failures.push('scoring_version');
          }
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
    await Promise.all(pending);
    if (!firstScoring) report.failures.push('scoring_payload_missing');
    if (firstScoring && firstScoring.recent_scored_count_exact === false) {
      const panel = page.locator('[data-slot="confidence-scoring-panel"]');
      report.lower_bound_note_visible = await panel.locator('[data-slot="scoring-count-bound"]').isVisible();
      report.lower_bound_value_visible = (await panel.innerText()).includes(`≥ ${firstScoring.recent_scored_count}`);
      if (!report.lower_bound_note_visible || !report.lower_bound_value_visible) report.failures.push('lower_bound_mislabeled');
    }
    // Wait past the 30s Edge cache TTL, then request and validate complete bodies.
    // Require a newer scoring generated_at, not just a repeated cached success.
    await page.waitForTimeout(35000);
    for (const [p, schema] of routes) {
      const row = { path: p, phase: 'after_cache_ttl' };
      const start = performance.now();
      try {
        const r = await context.request.get(origin + p, { timeout: 5000, maxRedirects: 0 });
        const raw = await r.body();
        row.ms = Math.round(performance.now() - start);
        row.status = r.status();
        row.bytes = raw.length;
        row.sha256 = createHash('sha256').update(raw).digest('hex');
        const parsed = schema.safeParse(JSON.parse(raw.toString('utf8')));
        row.schema_valid = parsed.success;
        if (!parsed.success) row.issues = parsed.error.issues.map(i => ({path: i.path, code: i.code}));
        if (p === '/api/scoring/status' && parsed.success) {
          row.scoring_version = parsed.data.scoring_version;
          row.generated_at = parsed.data.generated_at;
          row.new_generation = !!firstScoring && Date.parse(parsed.data.generated_at) > Date.parse(firstScoring.generated_at);
          if (!row.new_generation || row.scoring_version !== '0.3.0-bounded-evidence') report.failures.push('fresh_scoring_not_verified');
        }
        if (row.status !== 200 || !row.schema_valid || row.ms >= 5000) report.failures.push(`fresh_read:${p}`);
      } catch (e) {
        row.error = e.name;
        row.ms = Math.round(performance.now() - start);
        report.failures.push(`fresh_read:${p}`);
      }
      report.fresh_reads.push(row);
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
