// Read-only regression against the deployed public origin; no wallet or admin actions.
const { chromium } = require('playwright');
const fs = require('node:fs');
const path = require('node:path');
const ORIGIN = 'https://arbx.ape-tv.net';
const EXPECTED = process.env.TARGET_SHA;
const RUN = process.env.DEPLOY_RUN_ID;
const OUT = 'readiness-release-evidence';
const wanted = new Set(['/api/scoring/status', '/api/risk/circuit-breakers/status']);
const slots = { scoring: 'confidence-scoring-panel', breakers: 'risk-circuit-panel' };
const allowedStates = new Set(['PASS','WARN','PAUSED','KILLED','BLOCKED','NOT_AVAILABLE','UNKNOWN']);
function safePath(url) { try { return new URL(url).pathname; } catch { return 'invalid_path'; } }
(async () => {
  if (!/^[a-f0-9]{40}$/.test(EXPECTED || '') || !/^\d+$/.test(RUN || '')) throw new Error('invalid_release_identity');
  fs.mkdirSync(OUT, {recursive: true});
  const report = { started_at: new Date().toISOString(), origin: ORIGIN, expected_sha: EXPECTED,
    expected_deploy_run: RUN, scope: 'read_only_public_readiness_regression', rounds: [], blocked_writes: 0, passed: false };
  const browser = await chromium.launch({headless: true});
  const context = await browser.newContext({viewport: {width: 1440, height: 1000}, serviceWorkers: 'block'});
  try {
    await context.route('**/*', route => {
      if (!['GET','HEAD','OPTIONS'].includes(route.request().method())) {
        report.blocked_writes++; return route.abort('blockedbyclient');
      }
      return route.continue();
    });
    // Check identity before loading the expensive dashboard, without warming either target route.
    const status = await context.request.get(ORIGIN + '/api/status', {timeout: 10000, maxRedirects: 0});
    report.status_http = status.status();
    const body = await status.json();
    report.served_deploy = body.deploy ? {sha: body.deploy.sha, id: body.deploy.id, at: body.deploy.at} : null;
    if (status.status() !== 200 || body.deploy?.sha !== EXPECTED || String(body.deploy?.id) !== RUN) {
      throw new Error('public_release_identity_mismatch');
    }
    for (let n = 0; n < 2; n++) {
      // Edge scoring TTL is 30s. A second navigation after 32s exercises expiry,
      // not only the same cache hit. This is a small regression, not a load test.
      if (n) await new Promise(resolve => setTimeout(resolve, 32000));
      const round = {number: n + 1, started_at: new Date().toISOString(), requests: [], panels: {}, passed: false};
      report.rounds.push(round);
      const page = await context.newPage();
      const start = new WeakMap();
      const finished = [];
      const outstanding = new Set();
      page.on('request', r => {
        if (wanted.has(safePath(r.url()))) { start.set(r, Date.now()); outstanding.add(r); }
      });
      page.on('requestfinished', r => {
        if (!start.has(r)) return;
        finished.push((async () => {
          const response = await r.response();
          const item = {path: safePath(r.url()), ms: Date.now() - start.get(r), status: response?.status(), result: 'finished'};
          try {
            const payload = await response.json();
            if (item.path.includes('/scoring/')) {
              item.payload_valid = Number.isInteger(payload.recent_scored_count) && payload.recent_scored_count >= 0 && payload.source === 'runtime_evidence';
            } else {
              item.payload_valid = payload.summary?.total === 10 && payload.breakers?.length === 10 && allowedStates.has(payload.overall_state);
              item.overall_state = payload.overall_state;
            }
          } catch { item.payload_valid = false; }
          round.requests.push(item);
          outstanding.delete(r);
        })());
      });
      page.on('requestfailed', r => {
        if (start.has(r)) {
          round.requests.push({path: safePath(r.url()), ms: Date.now() - start.get(r), result: 'failed'});
          outstanding.delete(r);
        }
      });
      try {
        const response = await page.goto(ORIGIN + '/live-readiness', {waitUntil: 'domcontentloaded', timeout: 30000});
        round.page_http = response?.status();
        await page.locator('[data-slot="confidence-scoring-panel"]').waitFor({timeout: 20000});
        await page.waitForTimeout(22000); // also covers the risk panel's 15s poll and its full 5s deadline
        for (const [name, slot] of Object.entries(slots)) {
          const panel = page.locator(`[data-slot="${slot}"]`);
          const text = await panel.innerText({timeout: 5000});
          round.panels[name] = {error_visible: /Cannot fetch|edge timeout after/.test(text),
            content_visible: name === 'scoring' ? text.includes('Wire coverage') : /Overall:\s*(PASS|WARN|PAUSED|KILLED|BLOCKED|NOT_AVAILABLE|UNKNOWN)/i.test(text)};
          if (name === 'breakers') {
            // CSS text-transform:uppercase affects innerText, not state semantics.
            // Also require all ten actual rows, so a heading alone cannot pass.
            const renderedStates = await panel.locator('[data-state-val]').evaluateAll(
              rows => rows.map(row => row.getAttribute('data-state-val')));
            round.panels[name].rendered_breaker_count = renderedStates.length;
            round.panels[name].content_visible = round.panels[name].content_visible
              && renderedStates.length === 10 && renderedStates.every(state => allowedStates.has(state));
          }
          await panel.screenshot({path: path.join(OUT, `${name}-${n+1}.png`), timeout: 10000});
        }
        await Promise.all(finished);
        round.outstanding_at_assertion = outstanding.size;
        round.passed = outstanding.size === 0 && round.page_http === 200 && [...wanted].every(p => round.requests.some(r => r.path === p && r.status === 200 && r.payload_valid && r.ms < 5000))
          && !round.requests.some(r => r.result === 'failed' || r.status !== 200 || !r.payload_valid)
          && Object.values(round.panels).every(p => p.content_visible && !p.error_visible);
      } catch (e) { round.error_type = e.name; }
      finally {
        await page.close(); await Promise.all(finished);
        // A late abort while closing must never leave a previously computed green result.
        round.passed = round.passed && !round.requests.some(r => r.result === 'failed' || r.status !== 200 || !r.payload_valid);
      }
    }
    report.passed = report.rounds.length === 2 && report.rounds.every(r => r.passed);
  } catch (e) { report.error = ['public_release_identity_mismatch'].includes(e.message) ? e.message : e.name; }
  finally {
    report.finished_at = new Date().toISOString();
    fs.writeFileSync(path.join(OUT, 'verification.json'), JSON.stringify(report, null, 2));
    console.log(JSON.stringify(report, null, 2));
    await context.close(); await browser.close();
  }
  if (!report.passed) process.exitCode = 1;
})().catch(e => { console.error(e.name); process.exitCode = 1; });
