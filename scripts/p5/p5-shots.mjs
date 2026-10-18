import { chromium } from "playwright";
const b = await chromium.launch();
const ctx = await b.newContext({ viewport: { width: 390, height: 844 } });
const p = await ctx.newPage();
for (const [route, name] of [["/live-readiness","liver"],["/quant","quant"],["/omega-s5/crucible","crucible"],["/wallet","wallet"]]) {
  await p.goto("https://arbx.ape-tv.net"+route, { waitUntil: "load", timeout: 45000 });
  await p.waitForTimeout(2000);
  await p.screenshot({ path: `/tmp/shot-390${name}.png`, fullPage: false });
  const ov = await p.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  console.log(route, "overflow=", ov);
}
await b.close();
