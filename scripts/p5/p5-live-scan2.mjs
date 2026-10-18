import { chromium } from "playwright";
import fs from "node:fs";

const BASE = "https://arbx.ape-tv.net";
const routes = JSON.parse(fs.readFileSync("/tmp/routes.json", "utf8"));
const out = [];
const browser = await chromium.launch();

for (const route of routes) {
  // Fresh context per route: no cross-route navigation, so no aborted
  // in-flight RSC prefetch from a previous page (that artefact polluted scan 1).
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  const page = await ctx.newPage();
  const bad = [];
  const consoleErrors = [];
  const pageErrors = [];
  page.on("response", (r) => {
    const s = r.status();
    if (s >= 400) bad.push(`${s} ${r.request().method()} ${r.url().replace(BASE, "").slice(0, 130)}`);
  });
  page.on("console", (m) => {
    if (m.type() === "error") consoleErrors.push(m.text().slice(0, 200));
  });
  page.on("pageerror", (e) => pageErrors.push(String(e).slice(0, 200)));
  const rec = { route };
  try {
    const resp = await page.goto(BASE + route, { waitUntil: "load", timeout: 45000 });
    rec.status = resp?.status() ?? null;
    await page.waitForTimeout(2500);
    rec.body_text_len = await page.evaluate(() => document.body.innerText.length);
    rec.headings = await page.evaluate(() =>
      Array.from(document.querySelectorAll("h1,h2,h3")).slice(0, 3).map((h) => h.innerText.trim().slice(0, 50))
    );
    rec.interactive = await page.evaluate(() => document.querySelectorAll("button,[role=button],a[href],input,select").length);
  } catch (e) {
    rec.error = String(e).slice(0, 160);
  }
  rec.http_errors = [...new Set(bad)];
  rec.console_errors = [...new Set(consoleErrors)];
  rec.page_errors = [...new Set(pageErrors)];
  out.push(rec);
  process.stderr.write(`${route} status=${rec.status} bad=${rec.http_errors.length} console=${rec.console_errors.length} text=${rec.body_text_len}\n`);
  await ctx.close();
}
await browser.close();
fs.writeFileSync("/tmp/p5-live-scan2.json", JSON.stringify({ base: BASE, generated_at: new Date().toISOString(), routes: routes.length, results: out }, null, 1));
console.log("wrote /tmp/p5-live-scan2.json", out.length);
