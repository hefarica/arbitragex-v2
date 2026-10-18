import { chromium } from "playwright";
import fs from "node:fs";

const BASE = process.env.P5_BASE || "https://arbx.ape-tv.net";
const OUT = process.env.P5_OUT || "/tmp/p5-live-scan.json";
const ALL_VIEWPORTS = [
  { name: "1440", width: 1440, height: 900 },
  { name: "1280", width: 1280, height: 800 },
  { name: "1024", width: 1024, height: 768 },
  { name: "390", width: 390, height: 844 },
];
// Additive (PR-E): allow narrowing the sweep and pointing at the route list, so
// the same scanner serves the live probe and the local G-NO-OVERFLOW-390-01 gate.
const VIEWPORTS = process.env.P5_VIEWPORTS
  ? ALL_VIEWPORTS.filter((v) => process.env.P5_VIEWPORTS.split(",").includes(v.name))
  : ALL_VIEWPORTS;
const ROUTES_FILE = process.env.P5_ROUTES || "/tmp/routes.json";
const routes = JSON.parse(fs.readFileSync(ROUTES_FILE, "utf8"));

function stripTags(html) {
  return html
    .replace(/<script[\s\S]*?<\/script>/gi, " ")
    .replace(/<style[\s\S]*?<\/style>/gi, " ")
    .replace(/<[^>]+>/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

const results = [];
const browser = await chromium.launch();

for (const vp of VIEWPORTS) {
  const ctx = await browser.newContext({
    viewport: { width: vp.width, height: vp.height },
    ignoreHTTPSErrors: false,
  });
  const page = await ctx.newPage();
  for (const route of routes) {
    const rec = { viewport: vp.name, route, url: BASE + route };
    const consoleErrors = [];
    const pageErrors = [];
    const failed = [];
    const onConsole = (m) => {
      if (m.type() === "error") consoleErrors.push(m.text().slice(0, 300));
    };
    const onPageError = (e) => pageErrors.push(String(e).slice(0, 300));
    const onFailed = (r) => failed.push(`${r.method()} ${r.url().slice(0, 160)} :: ${r.failure()?.errorText}`);
    page.on("console", onConsole);
    page.on("pageerror", onPageError);
    page.on("requestfailed", onFailed);
    try {
      const resp = await page.goto(BASE + route, { waitUntil: "domcontentloaded", timeout: 25000 });
      rec.status = resp ? resp.status() : null;
      await page.waitForTimeout(900);
      rec.title = (await page.title()).slice(0, 120);
      rec.body_text_len = (await page.evaluate(() => document.body.innerText.length)).toString();
      rec.body_text_len = Number(rec.body_text_len);
      rec.root_children = await page.evaluate(() => document.getElementById("__next")?.children.length ?? document.body.children.length);
      rec.scroll_overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
      rec.headings = await page.evaluate(() => Array.from(document.querySelectorAll("h1,h2,h3")).slice(0, 4).map((h) => h.innerText.trim().slice(0, 60)));
      rec.buttons = await page.evaluate(() => Array.from(document.querySelectorAll("button,[role=button],a[href]")).length);

      // SSR first-render check: raw HTML from the server, before any JS runs.
      const raw = await page.request.get(BASE + route, { timeout: 25000 });
      const html = await raw.text();
      rec.ssr_status = raw.status();
      rec.ssr_html_bytes = html.length;
      rec.ssr_text_len = stripTags(html).length;
      rec.ssr_has_viewport_meta = /<meta[^>]+name="viewport"/i.test(html);
      rec.ssr_marker = /self\.__next_f|__NEXT_DATA__/i.test(html) ? "next-flight" : "none";
    } catch (e) {
      rec.error = String(e).slice(0, 300);
    }
    rec.console_errors = consoleErrors;
    rec.page_errors = pageErrors;
    rec.failed_requests = failed;
    page.off("console", onConsole);
    page.off("pageerror", onPageError);
    page.off("requestfailed", onFailed);
    results.push(rec);
    process.stderr.write(`${vp.name} ${route} status=${rec.status} text=${rec.body_text_len} errs=${consoleErrors.length + pageErrors.length}\n`);
  }
  await ctx.close();
}
await browser.close();

fs.writeFileSync(OUT, JSON.stringify({ base: BASE, generated_at: new Date().toISOString(), viewports: VIEWPORTS.map((v) => v.name), routes: routes.length, results }, null, 1));
console.log("wrote", OUT, results.length, "rows");
