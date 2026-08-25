import { describe, expect, it } from "vitest";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import type { ZodType } from "zod";

import * as S from "@/lib/schemas";
import {
  KpiPayloadSchema,
  ScannerHeartbeatResponseSchema,
} from "@/lib/operations-schemas";

/**
 * FE-0045 (§73 contract tier · §61) — API JSON vs Zod: every fixture here is
 * a VERBATIM recording of what https://arbx.ape-tv.net actually served on
 * 2026-08-24 (GET read-only §77; recorder: scratchpad/f0045_record_fixtures.py,
 * manifest carries url+sha256 per file). Each one is parsed through the SAME
 * schema the live client validates with (api-client.ts getValidated) — a
 * failure here means schema↔wire drift: the mirror and the real payload
 * disagree, which is a defect finding, never a reason to loosen the schema.
 *
 * Scope honesty (R8): prod runs the DEPLOYED wave. The 6 apex FE.* endpoints
 * (pairs, strategies/detectors catalog, quote/anchor, route-discovery/tick,
 * canonical-knobs) are 404 pre-deploy — no real payload exists to record, so
 * they are OUT of this suite until F-007 (§82) re-records post-deploy.
 * paper/history is consumed WITHOUT any Zod schema (raw fetchJson<T> in
 * app/paper/history/page.tsx) — recorded here to document the payload, the
 * missing mirror is the finding (level-(b) gap, not fabricated now).
 */

// lib/__tests__ -> fixtures/prod_20260824
const FIXTURES = join(dirname(fileURLToPath(import.meta.url)), "fixtures", "prod_20260824");
const MANIFEST = JSON.parse(readFileSync(join(FIXTURES, "manifest.json"), "utf8")) as {
  recorded_from: string;
  recorded_at: string;
  endpoints: { file: string; url: string; schema_module: "S" | "OPS" | null; schema: string | null }[];
};

/** Endpoint → the exact schema api-client.ts validates it with. */
const SCHEMA_BY_FILE: Record<string, ZodType> = {
  "status.json": S.StatusResponseSchema,
  "opportunities_live.json": S.OpportunitiesLiveSchema,
  "risk_alerts.json": S.RiskAlertsResponseSchema,
  "recon_summary.json": S.ReconSummarySchema,
  "config_current.json": S.AppConfigViewSchema,
  "readiness.json": S.ReadinessReportSchema,
  "readiness_blockers.json": S.ReadinessBlockersResponseSchema,
  "readiness_decision.json": S.ReadinessDecisionResponseSchema,
  "readiness_steps.json": S.ReadinessStepsResponseSchema,
  "scanner_heartbeat.json": ScannerHeartbeatResponseSchema,
  "operations_kpi.json": KpiPayloadSchema,
  "paper_mode_state.json": S.PaperModeStateSchema,
  // paper_history.json: SIN schema — consumido crudo (gap documentado abajo).
};

function loadFixture(file: string): unknown {
  const raw = readFileSync(join(FIXTURES, file), "utf8");
  return JSON.parse(raw);
}

/** Zod issues surfaced path|code|message — a drift finding must be legible. */
function issuesOf(result: { success: boolean; error?: { issues: { path: (string | number)[]; code: string; message: string }[] } }): string {
  return result.error!.issues
    .slice(0, 12)
    .map((i) => `${i.path.join(".") || "(root)"} | ${i.code} | ${i.message}`)
    .join("\n");
}

describe("FE-0045 · fixtures — anti-stale structure (manifest ↔ disk ↔ this suite)", () => {
  it("every manifest endpoint's fixture exists on disk and the map here covers it identically", () => {
    expect(MANIFEST.recorded_from).toBe("https://arbx.ape-tv.net");
    expect(MANIFEST.recorded_at).toBe("2026-08-24T20:35Z");
    for (const e of MANIFEST.endpoints) {
      expect(existsSync(join(FIXTURES, e.file)), `missing fixture ${e.file}`).toBe(true);
      if (e.schema === null) {
        expect(SCHEMA_BY_FILE[e.file], `${e.file} claims no schema in manifest`).toBeUndefined();
      } else {
        expect(SCHEMA_BY_FILE[e.file], `${e.file} manifest schema ${e.schema_module}.${e.schema} not mapped here`).toBeDefined();
      }
    }
  });

  it("no orphan fixtures on disk that the manifest does not declare", () => {
    const declared = new Set(MANIFEST.endpoints.map((e) => e.file));
    const onDisk = readdirSync(FIXTURES).filter((f) => f.endsWith(".json") && f !== "manifest.json");
    expect(onDisk.sort()).toEqual([...declared].sort());
  });
});

describe("FE-0045 · contract — prod JSON parses through the live client's schema", () => {
  for (const e of MANIFEST.endpoints) {
    if (e.schema === null) continue;
    it(`${e.file} ← ${e.url} (${e.schema_module}.${e.schema})`, () => {
      const schema = SCHEMA_BY_FILE[e.file];
      if (!schema) throw new Error(`${e.file} declarado en manifest sin map en este suite`);
      const parsed = schema.safeParse(loadFixture(e.file));
      if (!parsed.success) {
        throw new Error(
          `SCHEMA↔WIRE DRIFT on ${e.url}:\n${issuesOf(parsed)}\n` +
            `(fixture grabado ${MANIFEST.recorded_at}; si el campo proviene de la onda sin desplegar, clasificar pre-deploy con ID de task)`,
        );
      }
      expect(parsed.success).toBe(true);
    });
  }
});

describe("FE-0045 · finding — paper/history consumed WITHOUT a Zod mirror", () => {
  it("fixture exists (payload real documentado) pero no hay schema — gap nivel-(b), no se fabrica aquí", () => {
    const entry = MANIFEST.endpoints.find((e) => e.file === "paper_history.json");
    expect(entry).toBeDefined();
    expect(entry!.schema).toBeNull();
    // El payload ES JSON válido del wire real; su consumo sin validación es
    // el hallazgo (app/paper/history/page.tsx fetchJson<T> crudo). Crear el
    // mirror sería tarea de emisión propia, no parte de FE-0045.
    expect(() => loadFixture("paper_history.json")).not.toThrow();
  });
});
