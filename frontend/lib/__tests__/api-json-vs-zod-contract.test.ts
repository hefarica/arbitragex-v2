import { describe, expect, it } from "vitest";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
import { z, type ZodType } from "zod";

import * as S from "@/lib/schemas";
import {
  KpiPayloadSchema,
  ScannerHeartbeatResponseSchema,
} from "@/lib/operations-schemas";

/**
 * FE-0045 (§73 contract tier · §61) — API JSON vs Zod: every fixture here is
 * a formatted recording of what https://arbx.ape-tv.net actually served on
 * 2026-08-24 (GET read-only §77; recorder: scratchpad/f0045_record_fixtures.py,
 * manifest carries compact-wire bytes+sha256 per file). Integrity is checked
 * before parsing. Compatible recordings are parsed through the SAME
 * schema the live client validates with (api-client.ts getValidated) — a
 * failure here means schema↔wire drift: the mirror and the real payload
 * disagree, which is a defect finding, never a reason to loosen the schema.
 * The pre-go_a4 decision is the one explicit historical transition: the
 * current schema MUST reject only that missing field (added by #477).
 * Production schemas remain strict about this field; no fixture is upgraded
 * by inventing it. Current A.8/A.6 responses also require post-deploy browser
 * validation: this historical suite is not a live endpoint or latency test.
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
const ManifestSchema = z.object({
  recorded_from: z.literal("https://arbx.ape-tv.net"),
  recorded_at: z.literal("2026-08-24T20:35Z"),
  method: z.string(),
  note: z.string(),
  endpoints: z.array(z.object({
    file: z.string().regex(/^[a-z][a-z0-9_]*\.json$/),
    url: z.string().url().refine((url) => {
      const u = new URL(url);
      return u.origin === "https://arbx.ape-tv.net" && !u.username && !u.password && !u.hash;
    }),
    schema_module: z.enum(["S", "OPS"]).nullable(),
    schema: z.string().min(1).nullable(),
    bytes: z.number().int().positive(),
    sha256: z.string().regex(/^[0-9a-f]{64}$/),
  }).strict().refine((e) => (e.schema === null) === (e.schema_module === null)))
    .nonempty()
    .refine((entries) => new Set(entries.map((e) => e.file)).size === entries.length, "duplicate fixture")
    .refine((entries) => new Set(entries.map((e) => e.url)).size === entries.length, "duplicate endpoint"),
}).strict();
const MANIFEST = ManifestSchema.parse(JSON.parse(readFileSync(join(FIXTURES, "manifest.json"), "utf8")));
const SCHEMA_MODULES: Record<"S" | "OPS", Record<string, unknown>> = {
  S,
  OPS: { KpiPayloadSchema, ScannerHeartbeatResponseSchema },
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

/** Files are pretty-printed; the original manifest fingerprints compact wire JSON.
 * Whitespace is storage formatting, never permission to alter values/add fields.
 * In particular #477's injected go_a4 was not in the original captured payload.
 */
function verifyFixture(file: string, raw: string): unknown {
  const entry = MANIFEST.endpoints.find((e) => e.file === file);
  if (!entry) throw new Error(`undeclared fixture ${file}`);
  const value: unknown = JSON.parse(raw);
  const wire = Buffer.from(JSON.stringify(value), "utf8");
  if (wire.length !== entry.bytes || createHash("sha256").update(wire).digest("hex") !== entry.sha256) {
    throw new Error(`FIXTURE INTEGRITY DRIFT: ${file}; recover the original recording, never regenerate the historical hash`);
  }
  return value;
}

function loadFixture(file: string): unknown {
  return verifyFixture(file, readFileSync(join(FIXTURES, file), "utf8"));
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
        expect(SCHEMA_BY_FILE[e.file]).toBe(SCHEMA_MODULES[e.schema_module!][e.schema]);
      }
    }
  });

  it("no orphan fixtures on disk that the manifest does not declare", () => {
    const declared = new Set(MANIFEST.endpoints.map((e) => e.file));
    const onDisk = readdirSync(FIXTURES).filter((f) => f.endsWith(".json") && f !== "manifest.json");
    expect(onDisk.sort()).toEqual([...declared].sort());
    expect(Object.keys(SCHEMA_BY_FILE).sort()).toEqual(
      MANIFEST.endpoints.filter((e) => e.schema !== null).map((e) => e.file).sort(),
    );
  });

  it("all 13 historical payloads reproduce the recorded compact-wire hashes and lengths", () => {
    expect(MANIFEST.endpoints).toHaveLength(13);
    for (const e of MANIFEST.endpoints) expect(() => loadFixture(e.file)).not.toThrow();
  });

  it("detects historical field injection even when it would satisfy today's schema", () => {
    const original = loadFixture("readiness_decision.json") as Record<string, unknown>;
    expect(original).not.toHaveProperty("go_a4");
    expect(() => verifyFixture("readiness_decision.json", JSON.stringify({ ...original, go_a4: true })))
      .toThrow("FIXTURE INTEGRITY DRIFT");
  });

  it("rejects duplicate entries, traversal and inconsistent schema declarations", () => {
    const first = MANIFEST.endpoints[0]!;
    expect(ManifestSchema.safeParse({ ...MANIFEST, endpoints: [...MANIFEST.endpoints, first] }).success).toBe(false);
    for (const invalid of [
      { ...first, file: "../status.json" },
      { ...first, schema_module: null },
      { ...first, sha256: "not-a-hash" },
    ]) {
      expect(ManifestSchema.safeParse({ ...MANIFEST, endpoints: [invalid] }).success).toBe(false);
    }
  });
});

describe("FE-0045 · historical contract — current schema or explicit version transition", () => {
  for (const e of MANIFEST.endpoints) {
    if (e.schema === null) continue;
    it(`${e.file} ← ${e.url} (${e.schema_module}.${e.schema})`, () => {
      const schema = SCHEMA_BY_FILE[e.file];
      if (!schema) throw new Error(`${e.file} declarado en manifest sin map en este suite`);
      const parsed = schema.safeParse(loadFixture(e.file));
      if (e.file === "readiness_decision.json") {
        // Archived payload predates #477 (0622c9e0). Missing go_a4 is real,
        // not a false/true default and not a reason to loosen runtime validation.
        expect(parsed.success, "current decision schema must still require go_a4").toBe(false);
        if (parsed.success) throw new Error("runtime decision schema lost required go_a4");
        expect(parsed.error.issues.map((i) => ({ path: i.path, code: i.code })))
          .toEqual([{ path: ["go_a4"], code: "invalid_type" }]);
        return;
      }
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
