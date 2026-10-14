import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { OpportunitySchema } from "../contracts/index.js";

/**
 * Contract gate for the ALWAYS-COMPUTE economics payload (D-001).
 *
 * The fixture is a VERBATIM Opportunity captured from the live
 * `arbx:opps:detected` Redis stream (block 26157147, chain 1, 5-hop cycle).
 * It is not hand-written: it is the exact wire bytes the Rust producer emits,
 * which is the only thing that can prove producer↔consumer isomorphism.
 *
 * Why this gate exists: the deployed `OpportunitySchema` is `.strict()` and did
 * not declare `economics`, so it rejected 100% of published messages with
 * `unrecognized_keys: ["economics"]`. Measured on the live api-server log:
 * 94.688 rejects in a 1.655 s-normalised window (57,2 msg/s), and
 * `archived` = 0 — the paper ledger received nothing. A fix without a contract
 * gate is a regression waiting for a date.
 *
 * Read through `node:fs` on purpose: `shared-ts/tsconfig.json` does not enable
 * `resolveJsonModule` and its `include` accepts TypeScript files only, so a JSON
 * import would not compile — and enabling it would emit this fixture into `dist/`,
 * the artifact the package actually publishes.
 */
const fixture = JSON.parse(
  readFileSync(new URL("./fixtures/opportunity-with-economics.json", import.meta.url), "utf8"),
) as Record<string, unknown> & { economics: Record<string, unknown> };

describe("OpportunitySchema economics contract", () => {
  it("fixture is a real producer payload (anti-vacuous-gate guard)", () => {
    expect(fixture).toHaveProperty("economics");
    expect(fixture.economics).not.toBeNull();
    expect(fixture.economics.computation_status).toBe("computed");
    expect(Array.isArray(fixture.economics.legs)).toBe(true);
    expect((fixture.economics.legs as unknown[]).length).toBeGreaterThan(0);
  });

  it("accepts a captured producer payload with economics", () => {
    expect(OpportunitySchema.safeParse(fixture).success).toBe(true);
  });

  it("remains backward-compatible when economics is absent", () => {
    const legacy = { ...fixture };
    delete (legacy as Record<string, unknown>).economics;

    expect(OpportunitySchema.safeParse(legacy).success).toBe(true);
  });

  it("accepts explicit null economics", () => {
    expect(
      OpportunitySchema.safeParse({ ...fixture, economics: null }).success,
    ).toBe(true);
  });

  it("rejects producer/consumer drift at the top level", () => {
    expect(
      OpportunitySchema.safeParse({ ...fixture, __unknown_top_level: true }).success,
    ).toBe(false);
  });

  it("rejects unknown fields inside economics", () => {
    expect(
      OpportunitySchema.safeParse({
        ...fixture,
        economics: { ...fixture.economics, __unknown_economics_field: true },
      }).success,
    ).toBe(false);
  });

  it("rejects invalid computation status", () => {
    expect(
      OpportunitySchema.safeParse({
        ...fixture,
        economics: { ...fixture.economics, computation_status: "invalid-state" },
      }).success,
    ).toBe(false);
  });

  it("rejects non-numeric wei in a typed wei field", () => {
    expect(
      OpportunitySchema.safeParse({
        ...fixture,
        economics: { ...fixture.economics, amount_out_wei: "not-a-wei-value" },
      }).success,
    ).toBe(false);
  });
});
