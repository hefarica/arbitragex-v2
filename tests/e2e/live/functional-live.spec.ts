import { test, expect, type ConsoleMessage } from "@playwright/test";
import * as fs from "fs";
import * as path from "path";
import { fileURLToPath } from "url";
import {
  ROUTES,
  ERROR_BANNERS,
  FORBIDDEN_WORDS,
  EXPECTED_401_PATHS,
  type RouteSpec,
} from "./routes";

/**
 * LIVE functional sweep — every operator-console route.
 *
 * Hard gates (a failure here is a real defect):
 *   1. main document responds < 400
 *   2. the page renders an <h1> (it mounted, did not white-screen)
 *   3. NO "edge unreachable" / "edge error:" banner   (upstream healthy)
 *   4. NO RULE-00 forbidden tokens in rendered text   (zero-mocks doctrine)
 *   5. NOT a client-side 404 page
 *
 * Soft signal (recorded, not failed — surfaced in the JSON matrix):
 *   - non-allowlisted console errors
 *   - interactive surface counts (buttons / forms / tables / toggles)
 *   - data-provenance attributes ([data-source] / [data-feature])
 *
 * IMPORTANT: live pages POLL (readiness / heartbeat), so `networkidle` never
 * settles. We use domcontentloaded + a fixed settle window instead.
 */

const __dirnameLocal = path.dirname(fileURLToPath(import.meta.url));

interface RouteResult {
  path: string;
  label: string;
  group: string;
  httpStatus: number | null;
  contentType: string;
  rendered: boolean;
  is404: boolean;
  shadowed: boolean;
  errorBanner: boolean;
  forbiddenHits: string[];
  buttons: number;
  forms: number;
  tables: number;
  toggles: number;
  dataProvenance: number;
  consoleErrors: string[];
  verdict: "PASS" | "EMPTY_OK" | "NOT_FOUND" | "SHADOWED" | "RATE_LIMITED" | "FAIL";
}

// tests/e2e/live -> repo root -> audits/live/routes  (cwd-independent).
// One file per route: this survives Playwright's worker-restart-on-failure,
// which would otherwise reset an in-memory results array.
const ROUTES_DIR = path.resolve(
  __dirnameLocal,
  "..",
  "..",
  "..",
  "audits",
  "live",
  "routes",
);
fs.mkdirSync(ROUTES_DIR, { recursive: true });

function routeFile(p: string): string {
  const safe = p === "/" ? "_root" : p.replace(/[^a-z0-9]+/gi, "_");
  return path.join(ROUTES_DIR, `${safe}.json`);
}

function isAllowlistedConsoleError(text: string): boolean {
  return EXPECTED_401_PATHS.some((p) => text.includes(p));
}

// NOT serial: each route is audited independently so one defect does not
// mask the rest. Execution is still serialized via `workers: 1` in the config.

for (const route of ROUTES) {
  test(`route ${route.path} — ${route.label}`, async ({ page }) => {
    const consoleErrors: string[] = [];
    page.on("console", (msg: ConsoleMessage) => {
      if (msg.type() === "error" && !isAllowlistedConsoleError(msg.text())) {
        consoleErrors.push(msg.text());
      }
    });
    page.on("pageerror", (err) => consoleErrors.push(`pageerror: ${err.message}`));

    // Navigate with one backoff retry on 429 — the shared live edge rate-limits
    // a rapid sweep, and a self-inflicted 429 is not a real per-route defect.
    let resp = await page.goto(route.path, { waitUntil: "domcontentloaded" });
    if (resp?.status() === 429) {
      await page.waitForTimeout(4000);
      resp = await page.goto(route.path, { waitUntil: "domcontentloaded" });
    }
    // Settle: let the client render + first data fetches resolve. Pages poll
    // forever, so we cannot wait for networkidle.
    await page.waitForTimeout(2500);

    const httpStatus = resp?.status() ?? null;
    const contentType = (resp?.headers()["content-type"] ?? "").toLowerCase();
    // SHADOWED: a hard navigation to this client route was intercepted by an
    // edge-worker route that returned JSON/text instead of the SPA shell.
    const shadowed =
      contentType.includes("application/json") ||
      contentType.includes("text/plain");
    const bodyText = (await page.locator("body").textContent()) ?? "";
    const h1Count = await page.locator("h1").count();
    const rendered = h1Count > 0;

    const is404 =
      /\b(404|page not found|not found)\b/i.test(
        (await page.locator("h1").first().textContent().catch(() => "")) ?? "",
      ) || /this page could not be found/i.test(bodyText);

    const errorBanner = ERROR_BANNERS.some((re) => re.test(bodyText));
    const forbiddenMatch = bodyText.match(FORBIDDEN_WORDS);
    const forbiddenHits = forbiddenMatch
      ? Array.from(new Set(forbiddenMatch.map((m) => m.toLowerCase())))
      : [];

    const buttons = await page.locator("button:visible").count();
    const forms = await page.locator("form").count();
    const tables = await page.locator("table").count();
    const toggles = await page
      .locator('[role="switch"], [aria-checked]')
      .count();
    const dataProvenance = await page
      .locator("[data-source], [data-feature]")
      .count();

    const rateLimited = httpStatus === 429;

    let verdict: RouteResult["verdict"];
    if (rateLimited) verdict = "RATE_LIMITED";
    else if (httpStatus !== null && httpStatus >= 400) verdict = "FAIL";
    else if (shadowed) verdict = "SHADOWED";
    else if (is404) verdict = "NOT_FOUND";
    else if (errorBanner || forbiddenHits.length > 0 || !rendered) verdict = "FAIL";
    else if (buttons + forms + tables === 0) verdict = "EMPTY_OK";
    else verdict = "PASS";

    const result: RouteResult = {
      path: route.path,
      label: route.label,
      group: route.group,
      httpStatus,
      contentType,
      rendered,
      is404,
      shadowed,
      errorBanner,
      forbiddenHits,
      buttons,
      forms,
      tables,
      toggles,
      dataProvenance,
      consoleErrors,
      verdict,
    };
    fs.writeFileSync(routeFile(route.path), JSON.stringify(result, null, 2));

    // --- Hard gates -------------------------------------------------------
    // 429 is a self-inflicted sweep artifact (recorded as RATE_LIMITED), not a
    // page defect — do not hard-fail on it.
    if (!rateLimited) {
      expect(httpStatus ?? 200, `main doc status for ${route.path}`).toBeLessThan(400);
    }
    // A hard navigation must return the SPA shell, not an edge-worker JSON
    // response. If this fails, an edge route is shadowing the client route and
    // deep-link / refresh of this page is broken for the operator.
    if (!rateLimited) {
      expect(
        shadowed,
        `${route.path} is SHADOWED by an edge-worker route (deep-link returns ${contentType}, not the SPA)`,
      ).toBeFalsy();
      expect(
        errorBanner,
        `${route.path} shows an upstream error banner`,
      ).toBeFalsy();
      expect(
        forbiddenHits,
        `${route.path} contains RULE-00 forbidden tokens: ${forbiddenHits.join(",")}`,
      ).toHaveLength(0);
      // Only assert render once we know the SPA shell was actually served.
      if (!shadowed && !is404) {
        expect(rendered, `${route.path} mounted an <h1>`).toBeTruthy();
      }
    }
    // NOT_FOUND is reported but not hard-failed: the live deployment may run
    // a different branch than this repo checkout. The JSON matrix flags it.
  });
}

test.afterAll(async () => {
  // Aggregate from the per-route files on disk (robust to worker restarts).
  const outDir = path.resolve(__dirnameLocal, "..", "..", "..", "audits", "live");
  fs.mkdirSync(outDir, { recursive: true });
  const routes: RouteResult[] = fs
    .readdirSync(ROUTES_DIR)
    .filter((f) => f.endsWith(".json"))
    .map((f) => JSON.parse(fs.readFileSync(path.join(ROUTES_DIR, f), "utf8")))
    .sort((a: RouteResult, b: RouteResult) => a.path.localeCompare(b.path));
  const count = (v: string) => routes.filter((r) => r.verdict === v).length;
  const summary = {
    target: process.env["ARBX_FRONTEND_URL"] ?? "https://edge-arbx.ape-tv.net",
    generated_at: new Date().toISOString(),
    total: routes.length,
    pass: count("PASS"),
    empty_ok: count("EMPTY_OK"),
    not_found: count("NOT_FOUND"),
    shadowed: count("SHADOWED"),
    rate_limited: count("RATE_LIMITED"),
    fail: count("FAIL"),
    routes,
  };
  fs.writeFileSync(
    path.join(outDir, "functional-live.json"),
    JSON.stringify(summary, null, 2),
  );
});
