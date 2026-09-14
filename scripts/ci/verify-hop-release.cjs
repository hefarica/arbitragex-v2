'use strict';
const {chromium} = require('playwright');
const fs = require('node:fs');
const path = require('node:path');
const Module = require('node:module');
const ts = require('typescript');
const assert = require('node:assert/strict');
const target = process.env.TARGET_SHA;
const deploy = process.env.DEPLOY_RUN_ID;
const full = process.env.VERIFY_OPPORTUNITIES === 'true';
assert.match(target || '', /^[0-9a-f]{40}$/);
assert.match(deploy || '', /^[0-9]+$/);
const source = path.resolve('target-source/frontend/lib/schemas.ts');
const moduleForSchema = new Module(source, module);
moduleForSchema.filename = source;
moduleForSchema.paths = Module._nodeModulePaths(path.dirname(source));
moduleForSchema._compile(ts.transpileModule(fs.readFileSync(source, 'utf8'), {
  compilerOptions: {target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS},
}).outputText, source);
const S = moduleForSchema.exports;
const out = 'release-verification';
fs.mkdirSync(out, {recursive: true});
const report = {
  started_at: new Date().toISOString(), expected_sha: target, deploy_run: deploy,
  scope: full ? 'readiness_and_opportunity_evidence' : 'readiness_bounded_evidence',
  blocked_posts: 0, requests: [], rounds: [], passed: false,
};
let browser;
(async () => {
  try {
    browser = await chromium.launch({headless: true});
    const context = await browser.newContext({viewport: {width: 1440, height: 1050}, serviceWorkers: 'block'});
    await context.route('**/*', r => ['GET', 'HEAD', 'OPTIONS'].includes(r.request().method())
      ? r.continue() : (report.blocked_posts++, r.abort('blockedbyclient')));
    const page = await context.newPage();
    const navigation = await page.goto('https://arbx.ape-tv.net/live-readiness', {
      waitUntil: 'domcontentloaded', timeout: 30000,
    });
    assert.equal(navigation.status(), 200);
    async function get(url, schema) {
      const result = await page.evaluate(async url => {
        const controller = new AbortController();
        const timer = setTimeout(() => controller.abort(), 5000);
        const start = performance.now();
        try {
          const response = await fetch(url, {signal: controller.signal, credentials: 'same-origin', cache: 'no-store'});
          const body = await response.json();
          return {http: response.status, ms: Math.round(performance.now() - start), body};
        } catch (e) {
          return {ms: Math.round(performance.now() - start), error: e.name};
        } finally {clearTimeout(timer);}
      }, url);
      report.requests.push({path: url, http: result.http, ms: result.ms, error: result.error});
      assert.equal(result.http, 200, `GET failed ${url}: ${result.error || result.http}`);
      assert.ok(result.ms < 5000, `latency exceeded ${url}`);
      const checked = S[schema].safeParse(result.body);
      assert.ok(checked.success, `schema failed ${schema}`);
      return result.body;
    }
    async function identity(label) {
      const status = await get('/api/status', 'StatusResponseSchema');
      assert.equal(status.deploy?.sha, target, 'served SHA differs from target');
      assert.equal(String(status.deploy?.id), deploy, 'served deployment ID differs');
      report[label] = {at: new Date().toISOString(), deploy: status.deploy};
    }
    await identity('identity_before');
    for (let round = 1; round <= 3; round++) {
      if (round > 1) await page.waitForTimeout(32000);
      const scoring = await get('/api/scoring/status', 'ScoringStatusResponseSchema');
      const risk = await get('/api/risk/circuit-breakers/status', 'CircuitBreakersStatusResponseSchema');
      assert.equal(typeof scoring.recent_scored_count_exact, 'boolean');
      assert.ok(Number.isInteger(scoring.scoring_count_limit));
      if (!scoring.recent_scored_count_exact) assert.equal(scoring.recent_scored_count, scoring.scoring_count_limit);
      assert.equal(risk.breakers.length, 10);
      assert.equal(risk.summary.total, 10);
      if (full && risk.overall_state !== 'PASS') assert.ok(!risk.next_action.includes('All real breakers PASS'));
      await page.waitForTimeout(1500);
      const scorePanel = page.locator('[data-slot="confidence-scoring-panel"]');
      const riskPanel = page.locator('[data-slot="risk-circuit-panel"]');
      const scoreText = await scorePanel.innerText({timeout: 8000});
      const riskText = await riskPanel.innerText({timeout: 8000});
      assert.ok(!scoreText.includes('Cannot fetch scoring status'));
      assert.ok(!riskText.includes('Cannot fetch circuit breakers'));
      assert.match(scoreText, /wire coverage/i);
      assert.match(riskText, /overall:/i);
      if (!scoring.recent_scored_count_exact) assert.ok(scoreText.includes('≥'), 'limited count must be visibly identified');
      assert.equal(await riskPanel.locator('[data-state-val]').count(), 10);
      await scorePanel.screenshot({path: `${out}/scoring-${round}.png`});
      await riskPanel.screenshot({path: `${out}/risk-${round}.png`});
      report.rounds.push({round, at: new Date().toISOString(), scoring_count: scoring.recent_scored_count,
        scoring_count_exact: scoring.recent_scored_count_exact, scoring_limit: scoring.scoring_count_limit,
        risk_state: risk.overall_state, risk_summary: risk.summary, next_action: risk.next_action, panels_rendered: true});
    }
    if (full) {
      const payloads = new Map();
      const pending = [];
      page.on('response', r => {
        if (new URL(r.url()).pathname !== '/api/opportunities/live') return;
        pending.push(r.json().then(data => {
          if (r.status() === 200 && S.OpportunitiesLiveSchema.safeParse(data).success) {
            for (const item of data.items) payloads.set(item.id, item);
          }
        }).catch(() => {}));
      });
      const exchange = await page.goto('https://arbx.ape-tv.net/opportunities/exchange', {
        waitUntil: 'domcontentloaded', timeout: 30000,
      });
      assert.equal(exchange.status(), 200);
      await page.waitForTimeout(10000);
      const latest = await get('/api/opportunities/live?limit=200', 'OpportunitiesLiveSchema');
      for (const item of latest.items) payloads.set(item.id, item);
      await Promise.race([Promise.allSettled(pending), new Promise(r => setTimeout(r, 6000))]);
      const cards = await page.locator('[data-opp-id]').evaluateAll(elements => elements.map(el => ({
        id: el.getAttribute('data-opp-id'), badge: el.querySelector('.dapp-badge')?.innerText || '', text: el.innerText,
      })));
      const byHops = {}; let checkedCards = 0; let noSimulation = 0;
      for (const item of payloads.values()) {
        const rm = item.route_metadata;
        const h = rm?.dex_adapters?.length;
        if (Number.isInteger(h) && rm.pool_addresses?.length === h && rm.token_addresses?.length === h + 1) {
          byHops[h] = (byHops[h] || 0) + 1;
        } else {byHops.unknown = (byHops.unknown || 0) + 1;}
        if (item.simulated_at == null) {
          for (const key of ['simulated_net_profit_usd', 'simulated_amount_in_usd', 'simulated_roi_pct', 'simulated_cost_breakdown']) {
            assert.ok(item[key] == null, `${key} fabricated without simulation for ${item.id}`);
          }
          noSimulation++;
        }
      }
      for (const card of cards) {
        const item = payloads.get(card.id); if (!item) continue;
        checkedCards++;
        assert.ok(!/\bLIVE\b/.test(card.badge), 'freshness badge claims LIVE');
        if (item.rejection_reason || ['rejected', 'failed'].includes(item.status)) {
          assert.match(card.badge, /Rechazada|Fallida/i);
          assert.ok(!/Evaluada/i.test(card.badge));
        }
        assert.ok(!card.text.includes('EXECUTE (PAPER SHADOW)'));
      }
      assert.ok(checkedCards > 0, 'no rendered opportunity IDs correlated with real API payloads');
      report.opportunities = {unique_ids: payloads.size, by_hops: byHops, cards_correlated: checkedCards,
        no_simulation_checked: noSimulation, coverage_2_5: Object.fromEntries([2,3,4,5].map(h => [h, byHops[h] ? 'observed_not_execution_certified' : 'not_observed']))};
      await page.screenshot({path: `${out}/opportunities.png`, fullPage: true});
      fs.writeFileSync(`${out}/opportunities.json`, JSON.stringify([...payloads.values()], null, 2));
    }
    await identity('identity_after');
    report.passed = true;
    await context.close();
  } catch (e) {
    report.error = {name: e.name, message: String(e.message).slice(0, 300)};
    process.exitCode = 1;
  } finally {
    if (browser) await browser.close();
    report.finished_at = new Date().toISOString();
    fs.writeFileSync(`${out}/verification.json`, JSON.stringify(report, null, 2));
    console.log(JSON.stringify(report, null, 2));
  }
})();
