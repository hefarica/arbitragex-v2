/**
 * RunFullSyncCycle FASE 2 — manifest contract tests.
 *
 * Guards the operator-critical invariants of the macro's manifest parser
 * (scripts/run_full_sync_cycle.mjs — imported dynamically so tsc never
 * statically resolves a non-workspace .mjs; vitest loads it at runtime):
 *   - CSV order is preserved VERBATIM — for rpc_* the order IS the rotation
 *     priority (first entry = titular). The parser must never sort/dedup.
 *   - Validation errors name the item index + field, and NEVER echo the
 *     secret value (redacted-logger discipline, REGLA 02).
 *   - Cap 200 items; scope shape; secret length bounds.
 */
import { describe, it, expect } from "vitest";

interface ParsedManifest {
  ok: boolean;
  items?: Array<{ provider: string; scope: string; secret_value?: string | null; metadata?: Record<string, unknown> }>;
  errors?: string[];
}
interface CsvEntry {
  position: number;
  name: string;
}

async function loadMacro(): Promise<{
  parseCredentialManifest: (raw: unknown) => ParsedManifest;
  csvEntries: (secretValue: unknown) => CsvEntry[];
}> {
  // Relative path from shared-ts/src/__tests__ → repo scripts/ (three levels
  // up: __tests__ → src → shared-ts → root). Variable dynamic import: runtime
  // resolution only, invisible to tsc.
  const spec = "../../../scripts/run_full_sync_cycle.mjs";
  return (await import(spec)) as unknown as {
    parseCredentialManifest: (raw: unknown) => ParsedManifest;
    csvEntries: (secretValue: unknown) => CsvEntry[];
  };
}

describe("parseCredentialManifest — orden = prioridad de rotación", () => {
  it("preserves rpc CSV order VERBATIM (titular first, no sort/dedup)", async () => {
    const { parseCredentialManifest, csvEntries } = await loadMacro();
    const csv = "zeta=https://z.example,alpha=https://a.example,zeta=https://z2.example";
    const r = parseCredentialManifest({
      items: [{ provider: "rpc_http", scope: "chain:1", secret_value: csv }],
    });
    expect(r.ok).toBe(true);
    const entries = csvEntries(r.items![0]!.secret_value);
    expect(entries.map((e) => e.name)).toEqual(["zeta", "alpha", "zeta"]);
    expect(entries[0]!.position).toBe(1); // titular = position 1, always
  });

  it("bare URLs (no name=) get positional names without reordering", async () => {
    const { csvEntries } = await loadMacro();
    const entries = csvEntries("https://a.example,drpc=https://d.example");
    expect(entries.map((e) => e.name)).toEqual(["(bare #1)", "drpc"]);
  });
});

describe("parseCredentialManifest — validación fail-honest", () => {
  it("rejects empty/oversized batches with a clear cap error", async () => {
    const { parseCredentialManifest } = await loadMacro();
    expect(parseCredentialManifest({ items: [] }).ok).toBe(false);
    const big = { items: Array.from({ length: 201 }, () => ({ provider: "x", scope: "global" })) };
    expect(parseCredentialManifest(big).errors![0]).toContain("cap is 200");
  });

  it("rejects malformed scopes/secrets naming the index+field", async () => {
    const { parseCredentialManifest } = await loadMacro();
    const r = parseCredentialManifest({
      items: [
        { provider: "coingecko_pro", scope: "wrong-scope", secret_value: "CG-x" },
        { provider: "coingecko_pro", scope: "global", secret_value: "x".repeat(2049) },
      ],
    });
    expect(r.ok).toBe(false);
    expect(r.errors!.some((e) => e.startsWith("items[0].scope"))).toBe(true);
    expect(r.errors!.some((e) => e.startsWith("items[1].secret_value"))).toBe(true);
  });

  it("errors NEVER echo the secret value", async () => {
    const { parseCredentialManifest } = await loadMacro();
    const secret = "SUPERSECRETVALUE";
    const r = parseCredentialManifest({
      items: [{ provider: "coingecko_pro", scope: "global", secret_value: secret }],
    });
    expect(r.ok).toBe(true); // shape-valid — but assert on an INVALID shape too:
    const bad = parseCredentialManifest({
      items: [{ provider: "coingecko_pro", scope: "global", secret_value: 12345 }],
    });
    expect(bad.ok).toBe(false);
    expect(JSON.stringify(bad.errors)).not.toContain("12345");
  });

  it("metadata defaults to {} and secret absence is allowed (metadata-only refresh)", async () => {
    const { parseCredentialManifest } = await loadMacro();
    const r = parseCredentialManifest({
      items: [{ provider: "coingecko_pro", scope: "global", metadata: { tier: "pro" } }],
    });
    expect(r.ok).toBe(true);
    expect(r.items![0]!.metadata).toEqual({ tier: "pro" });
    expect(r.items![0]!.secret_value).toBeUndefined();
    const bare = parseCredentialManifest({
      items: [{ provider: "coingecko_pro", scope: "global" }],
    });
    expect(bare.items![0]!.metadata).toEqual({});
  });
});
