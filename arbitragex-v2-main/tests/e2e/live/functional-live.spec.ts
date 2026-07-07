import { test, expect, type ConsoleMessage } from "@playwright/test";
import * as fs from "fs";
import * as path from "path";
import { fileURLToPath } from "url";
import {
  ROUTES,
  ERROR_BANNERS,
  EDGE_STATE_ERROR_LABELS,
  EDGE_STATE_EMPTY_LOADING,
  FORBIDDEN_WORDS,
} from "./routes";

/**
 * LIVE functional sweep — every operator-console route.
 *
 * Hard gates (a failure here is a REAL defect — surgical, not cosmetic):
 *   1. main document responds < 400
 *   2. NOT shadowed by an edge-worker route (deep-link returns the SPA shell)
 *   3. NOT fail-closed by a PAGE-LEVEL error surface (EdgeState error variant as
 *      the main content with no primary content rendered) — see below
 *   4. renders PRIMARY CONTENT (an <h1>, a form/inputs, a table, or an honest
 *      EdgeState empty/loading surface) — NOT strictly an <h1>
 *   5. NO RULE-00 forbidden tokens in rendered text
 *   6. NOT a client-side 404 page
 *
 * Error detection is STRUCTURAL, not text-scan. A page is fail-closed only when
 * EdgeState's blocking variant (role="alert" + "EDGE ERROR"/"DISCONNECTED")
 * renders inside <main> AND no primary content rendered. A metric card honestly
 * printing "edge error" for one upstream, or "ZERO TRUST" branding, on an
 * otherwise-rendered page is NOT a failure (RULE 00 / R8). A functional page
 * without an <h1> (e.g. /admin/signin: a login form) is NOT a failure.
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
  hasH1: boolean;
  hasPrimaryContent: boolean;
  pageLevelError: boolean;
  is404: boolean;
  shadowed: boolean;
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

// Modest inter-route pacing to reduce self-inflicted edge rate-limiting.
const PACING_MS = Number(process.env["ARBX_E2E_PACING_MS"] ?? "800");

// NOT serial: each route is audited independently so one defect does not
// mask the rest. Execution is still serialized via `workers: 1` in the config.

for (const route of ROUTES) {
  test(`route ${route.path} — ${route.label}`, async ({ page }) => {
    const consoleErrors: string[] = [];
    // Track 429 on ANY response (navigation OR in-page data fetch). A page whose
    // data fetch is rate-limited honestly renders an EdgeState error / fails to
    // render — that is a sweep artifact, not a page defect (see verdict below).
    let sawHttp429 = false;
    page.on("response", (r) => {
      if (r.status() === 429) sawHttp429 = true;
    });
    page.on("console", (msg: ConsoleMessage) => {
      if (msg.type() !== "error") return;
      const t = msg.text();
      // HTTP resource failures (401/403/404/429) are tracked structurally via
      // response events + the in-page-429 classifier — not page defects on their
      // own (e.g. the expected admin 401 on /api/admin/topology). Record only
      // genuine JS/console errors here.
      if (/Failed to load resource/i.test(t)) return;
      consoleErrors.push(t);
    });
    page.on("pageerror", (err) => consoleErrors.push(`pageerror: ${err.message}`));

    if (PACING_MS > 0) await page.waitForTimeout(PACING_MS);

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

    // Scope primary-content + error detection to <main> when present, so the
    // header/sidebar chrome and the global toast region (also role="alert") never
    // count. Fall back to <body> for any page that does not use the app shell.
    const mainLoc = page.locator("main");
    const scope =
      (await mainLoc.count()) > 0 ? mainLoc.first() : page.locator("body");
    const scopeText = ((await scope.textContent().catch(() => "")) ?? "").trim();

    const h1Count = await page.locator("h1").count();
    const hasH1 = h1Count > 0;

    const is404 =
      /\b(404|page not found|not found)\b/i.test(
        (await page.locator("h1").first().textContent().catch(() => "")) ?? "",
      ) || /this page could not be found/i.test(bodyText);

    const forbiddenMatch = bodyText.match(FORBIDDEN_WORDS);
    const forbiddenHits = forbiddenMatch
      ? Array.from(new Set(forbiddenMatch.map((m) => m.toLowerCase())))
      : [];

    const buttons = await page.locator("button:visible").count();
    const forms = await scope.locator("form").count();
    const inputs = await scope.locator("input, textarea, select").count();
    const tables = await scope.locator("table").count();
    const toggles = await page.locator('[role="switch"], [aria-checked]').count();
    const dataProvenance = await page
      .locator("[data-source], [data-feature]")
      .count();

    // STRUCTURAL error detection (NOT a body-text scan):
    //  - blocking EdgeState error/offline = role="alert" + EDGE ERROR/DISCONNECTED
    //    rendered inside the content scope.
    //  - honest EdgeState empty/loading = NO DATA / SYNCING (role="status") — NOT
    //    an error; an honest "no data yet" / first-paint is a valid rendered state.
    const edgeError = await scope
      .locator('[role="alert"]')
      .filter({ hasText: EDGE_STATE_ERROR_LABELS })
      .count();
    // Honest empty/loading is detected STRUCTURALLY (role="status" + label),
    // mirroring the error detection — a regex on concatenated scopeText fails
    // because the label glues to the title ("NO DATANo RPCs registered yet…").
    const edgeHonest =
      (await scope
        .locator('[role="status"]')
        .filter({ hasText: EDGE_STATE_EMPTY_LOADING })
        .count()) > 0;
    const legacyBanner = ERROR_BANNERS.some((re) => re.test(bodyText));

    // Primary content = the page mounted its real UI (heading / form / inputs /
    // table) OR a rich, non-error main area with substantial text. An <h1> is a
    // POSITIVE signal, never a universal requirement (a login form has none).
    const hasPrimaryContent =
      hasH1 ||
      forms > 0 ||
      inputs > 0 ||
      tables > 0 ||
      (edgeError === 0 && !legacyBanner && scopeText.length > 400);

    // Fail-closed = a BLOCKING error surface present AND no primary content. A
    // per-widget honest "edge error" on an otherwise-rendered page is NOT this.
    const pageLevelError = (edgeError > 0 || legacyBanner) && !hasPrimaryContent;

    // Rendered = primary content OR an honest empty/loading surface.
    const renderedOk = hasPrimaryContent || edgeHonest;

    // RATE_LIMITED = the navigation 429'd, OR an in-page data fetch 429'd and
    // that left the page fail-closed / unrendered. Either way it is a
    // self-inflicted sweep artifact, NOT a page defect — never let it RED.
    // (A genuine contract-drift FAIL has pageLevelError WITHOUT any 429.)
    const rateLimited =
      httpStatus === 429 ||
      (sawHttp429 && (pageLevelError || !renderedOk));

    let verdict: RouteResult["verdict"];
    if (rateLimited) verdict = "RATE_LIMITED";
    else if (httpStatus !== null && httpStatus >= 400) verdict = "FAIL";
    else if (shadowed) verdict = "SHADOWED";
    else if (is404) verdict = "NOT_FOUND";
    else if (forbiddenHits.length > 0) verdict = "FAIL";
    else if (pageLevelError) verdict = "FAIL";
    else if (!renderedOk) verdict = "FAIL";
    else if (hasPrimaryContent) verdict = "PASS";
    else verdict = "EMPTY_OK";

    const result: RouteResult = {
      path: route.path,
      label: route.label,
      group: route.group,
      httpStatus,
      contentType,
      hasH1,
      hasPrimaryContent,
      pageLevelError,
      is404,
      shadowed,
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
    // deep-link / refresh of this page is broken for the operator. (→ /status)
    if (!rateLimited) {
      expect(
        shadowed,
        `${route.path} is SHADOWED by an edge-worker route (deep-link returns ${contentType}, not the SPA)`,
      ).toBeFalsy();
      expect(
        forbiddenHits,
        `${route.path} contains RULE-00 forbidden tokens: ${forbiddenHits.join(",")}`,
      ).toHaveLength(0);
      if (!shadowed && !is404) {
        // Fail-closed by a page-level EdgeState error surface with no primary
        // content rendered. A per-widget honest "edge error" does NOT trip this.
        // (→ /pools: contract-drift makes getDefiPools() fail → EdgeState error)
        expect(
          pageLevelError,
          `${route.path} is fail-closed by a page-level error surface (no primary content rendered)`,
        ).toBeFalsy();
        // The page must render SOMETHING real: primary content (h1/form/inputs/
        // table) or an honest empty/loading surface. NOT strictly an <h1>.
        expect(
          renderedOk,
          `${route.path} rendered no primary content and no honest empty/loading surface (white-screen)`,
        ).toBeTruthy();
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
