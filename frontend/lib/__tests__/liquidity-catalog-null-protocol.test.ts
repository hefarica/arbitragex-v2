import { describe, expect, it } from "vitest";
import { readCatalog, type CatalogLevel } from "../../app/dex-registry/liquidity-catalog-model";

const dexId = "00000000-0000-4000-8000-000000000001";
function example(level: CatalogLevel, protocol: unknown) {
  const scope = { level, chainId: "1", dexId: null, search: "" };
  const item = { id: dexId, chain_id: "1", label: "Unclassified inventory", active: true,
    protocol_type: protocol, factory_count: "1", pool_count: "1",
    pool_address: "0x0000000000000000000000000000000000000001",
    dex_id: dexId, dex_name: "Inventory only", dex_active: true,
    factory_address: "0x0000000000000000000000000000000000000002",
    fee_tier: null, token0_address: null, token0_symbol: null,
    token1_address: null, token1_symbol: null };
  return { scope, wire: { schema_version: 1, source: "postgresql-registry", level,
    observed_at: "2026-09-14T00:00:00.000Z", execution_verified: false,
    counts_include_inactive: true, limit: 25, count: 1, next_after: null,
    scope: { chain_id: "1", dex_id: null, q: "" }, items: [item] } };
}

describe("catalog protocol matches nullable database contract", () => {
  for (const level of ["dexes", "pools"] as const) {
    it(`${level} preserves a recorded null without asserting execution`, () => {
      const { scope, wire } = example(level, null);
      const parsed = readCatalog(wire, scope);
      expect(parsed.items).toHaveLength(1);
      expect(parsed.items[0]?.protocol_type).toBeNull();
      expect(wire.execution_verified).toBe(false);
    });
    it.each([undefined, 12, false, {}, "", "   ", "\t\n"])(`${level} rejects malformed protocol %j`, (protocol) => {
      const { scope, wire } = example(level, protocol);
      expect(() => readCatalog(wire, scope)).toThrow();
    });
    it(`${level} still preserves a known protocol`, () => {
      const { scope, wire } = example(level, "UNISWAP_V2");
      expect(readCatalog(wire, scope).items[0]?.protocol_type).toBe("UNISWAP_V2");
    });
  }
});
