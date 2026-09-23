/** Passive browser capture. Uses the repo's installed @playwright/test only.
 * No clicks, toggles, login, simulation, transactions, signer or POST from this script.
 * The loaded application may perform its usual reads; captures are not a certificate.
 */
import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
const [urlString, outputDir] = process.argv.slice(2);
if (!urlString || !outputDir) throw new Error('Usage: node probe_browser.mjs https://host/opportunities /OUTSIDE_REPO/capture-new');
const url = new URL(urlString);
if (url.protocol !== 'https:' || url.username || url.password || url.search || url.hash) throw new Error('Explicit HTTPS URL with no credentials/query/fragment required');
const out = path.resolve(outputDir);
await fs.mkdir(out, {recursive:false});
let browser;
const report = {schema:'arbx.passive-browser-capture.v1',url:url.href,started_at:new Date().toISOString(),
 scope:'passive_dom_and_public_responses',production_certified:false,post_actions:0,
 console_errors:0,websocket_frames:0,requests_failed:0,public_response_statuses:[],cards:[],instrumented_fields:0};
try {
 const {chromium} = await import('@playwright/test');
 browser = await chromium.launch({headless:true});
 const page = await browser.newPage();
 page.on('console',m=>{if(m.type()==='error')report.console_errors++;});
 page.on('requestfailed',()=>{report.requests_failed++;});
 page.on('websocket',ws=>{ws.on('framereceived',()=>{report.websocket_frames++;});});
 page.on('response',r=>{
  try { const u=new URL(r.url());
   if (u.hostname === url.hostname && /\/(opportunities|status|health)/.test(u.pathname))
    report.public_response_statuses.push({path:u.pathname,status:r.status()});
  } catch { /* Do not record credentials/opaque URLs. */ }
 });
 await page.goto(url.href,{waitUntil:'domcontentloaded',timeout:30000});
 await page.waitForSelector('[data-opp-id]',{timeout:15000}).catch(()=>{});
 report.cards = await page.locator('[data-opp-id]').evaluateAll(nodes=>nodes.slice(0,200).map(el=>({
  event_id:el.getAttribute('data-opp-id'),
  real_fields:[...el.querySelectorAll('[data-real-field]')].map(f=>({field:f.getAttribute('data-real-field'),state:f.getAttribute('data-field-state'),reason:f.getAttribute('data-field-reason'),value:f.textContent?.trim()})),
  summary_cells:[...el.querySelectorAll('[data-testid="opportunity-summary-grid"] .min-w-0')].map(f=>({label:f.children[0]?.textContent?.trim(),value:f.children[1]?.textContent?.trim()})),
  layout_present:el.getBoundingClientRect().width>0 && el.getBoundingClientRect().height>0,
 })));
 report.instrumented_fields=report.cards.reduce((n,c)=>n+c.real_fields.length,0);
 report.status=report.instrumented_fields>0?'DOM_OBSERVED_REQUIRE_RECEIPT_RECONCILIATION':'MISSING_FIELD_INSTRUMENTATION';
 await page.screenshot({path:path.join(out,'dom.png'),fullPage:true});
 report.screenshot_sha256=crypto.createHash('sha256').update(await fs.readFile(path.join(out,'dom.png'))).digest('hex');
} catch(e) {
 report.status='UNAVAILABLE';report.error_type=e?.constructor?.name ?? 'UnknownError';
 // No exception message: it may contain headers, cookies or credentials.
} finally {
 if(browser)await browser.close();
 report.completed_at=new Date().toISOString();
 await fs.writeFile(path.join(out,'capture.json'),JSON.stringify(report,null,2),{flag:'wx',mode:0o600});
 console.log(JSON.stringify({status:report.status,cards:report.cards.length,instrumented_fields:report.instrumented_fields,production_certified:false}));
 if(report.status==='UNAVAILABLE'||report.status==='MISSING_FIELD_INSTRUMENTATION')process.exitCode=2;
}
