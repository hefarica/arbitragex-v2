import { describe, it, expect } from "vitest";
import { readFileSync, readdirSync, existsSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

/**
 * FE-0058 (§63) — Omega S5 introspection reads the SAME SSOT: no second
 * configuration. The S5 surfaces (adapters/core/crucible/drift/factory/
 * operator/registry/wallets) must obtain data EXCLUSIVELY through the shared
 * hooks/snapshot-loaders (single endpoint chain: page → lib/hooks → edge →
 * api-server → PG), never through a page-local fetch, a page-local Zod schema,
 * or a page-local copy of a registry list.
 *
 * Scan-style audit (same pattern as lib/web3/security.test.ts and the FE-0041
 * write-path SSOT pin): reads the omega-s5 source tree and the hook modules,
 * fails on the §63 smells. Zero mocks — this tests SOURCE, not fixtures.
 */

// frontend/app/omega-s5/__tests__ -> frontend/
const FRONTEND = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
const S5_ROOT = join(FRONTEND, "app", "omega-s5");

function collectTs(dir: string, acc: string[] = []): string[] {
  if (!existsSync(dir)) return acc;
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) collectTs(p, acc);
    else if (/\.(ts|tsx)$/.test(name) && !/\.test\.(ts|tsx)$/.test(name)) acc.push(p);
  }
  return acc;
}

const s5Files = collectTs(S5_ROOT).map((path) => ({
  path,
  rel: path.slice(S5_ROOT.length + 1).replace(/\\/g, "/"),
  src: readFileSync(path, "utf8"),
}));

const read = (rel: string) => readFileSync(join(FRONTEND, rel), "utf8");

describe("omega-s5 §63 — introspection over the SAME SSOT (no second configuration)", () => {
  it("S5 surfaces contain NO direct fetch() — all data flows via shared hooks/snapshot loaders", () => {
    const offenders = s5Files.filter(
      (f) => !f.rel.includes("snapshot.ts") && /\bfetch\(\s/.test(f.src),
    );
    // snapshot.ts is the ONLY sanctioned data loader (SSR, shared admin
    // endpoints); every page/component must go through hooks or its result.
    expect(
      offenders.map((f) => f.rel),
      "page-local fetch = unshared data path (§63 second configuration)",
    ).toEqual([]);
  });

  it("S5 tree defines NO local Zod schemas — wire contracts live once in lib/", () => {
    const offenders = s5Files.filter((f) => /z\s*\.\s*object\s*\(/.test(f.src));
    expect(offenders.map((f) => f.rel), "page-local Zod = second schema").toEqual([]);
  });

  it("the shared hooks hit exactly the canonical endpoints (pin against drift)", () => {
    const pins: Array<[hook: string, endpoint: string]> = [
      ["lib/hooks/useContracts.ts", "/api/contracts"],
      ["lib/hooks/useFeatureManifest.ts", "/api/system/feature_manifest"],
      ["lib/hooks/useCrucibleStatus.ts", "/api/crucible/status"],
      ["lib/hooks/useCapitalGates.ts", "/api/capital-gates"],
    ];
    for (const [hook, endpoint] of pins) {
      const src = read(hook);
      expect(src, `${hook} must request ${endpoint}`).toContain(endpoint);
      // and must build it on the SHARED base — never an absolute second origin
      expect(src, `${hook} must use getApiBaseUrl()`).toContain("getApiBaseUrl()");
    }
  });

  it("drift introspection consumes the SAME useOmniDrift source as the coherence strip (FE-0040)", () => {
    const driftPage = s5Files.find((f) => f.rel === "drift/page.tsx");
    expect(driftPage, "drift/page.tsx exists").toBeTruthy();
    expect(driftPage!.src).toContain('from "@/lib/drift/useOmniDrift"');
  });

  it("view filters (KINDS/ROLES) only contain members of the SHARED contract_kind union", () => {
    const unionSrc = read("lib/registries/types-omni.ts");
    const unionBlock = unionSrc.match(/export type ContractKind\s*=\s*([\s\S]*?);/);
    expect(unionBlock, "ContractKind union found in types-omni.ts").toBeTruthy();
    const members = new Set(
      [...unionBlock![1]!.matchAll(/"([a-z0-9_]+)"/g)].map((m) => m[1]),
    );
    expect(members.size).toBeGreaterThan(0);

    for (const rel of ["adapters/page.tsx", "wallets/page.tsx"]) {
      const page = s5Files.find((f) => f.rel === rel);
      expect(page, `${rel} exists`).toBeTruthy();
      // Extract the page's OWN filter array (KINDS/ROLES), not any literal.
      const filterBlock = page!.src.match(/(?:KINDS|ROLES)[^=]*=\s*\[([\s\S]*?)\]/);
      expect(filterBlock, `${rel} declares its KINDS/ROLES filter array`).toBeTruthy();
      const literals = [...filterBlock![1]!.matchAll(/"([a-z0-9_]+)"/g)].map((m) => m[1]);
      // every filter literal must be a real member of the shared union — a
      // literal outside the union would be a typo'd SECOND registry entry.
      expect(literals.length, `${rel} has filter literals`).toBeGreaterThan(0);
      for (const l of literals) {
        expect(members.has(l), `${rel}: "${l}" is not in the shared ContractKind union`).toBe(true);
      }
      // and the filter must be TYPED against the shared union, not a local one
      expect(page!.src).toContain('ContractEntity["contract_kind"]');
    }
  });

  it("registry snapshot loader uses the SHARED admin schema — no S5-local contract", () => {
    const snap = s5Files.find((f) => f.rel === "registry/[entity]/snapshot.ts");
    expect(snap, "registry/[entity]/snapshot.ts exists").toBeTruthy();
    expect(snap!.src).toContain("AdminChainsListSchema");
    expect(snap!.src).toMatch(/from ["']@\/lib\/schemas["']/);
    // RegistryKey comes from the shared operator registry map, not a local list
    expect(snap!.src).toContain("REGISTRY_KEYS");
    expect(snap!.src).toMatch(/from ["']@\/lib\/operator\/types["']/);
  });
});
