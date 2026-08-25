// EMIT-04 — universeFingerprint contract tests (pure function, no DB/Redis).
//
// The fingerprint is the version-bump DECIDER (PUT /admin/trading-config) and
// doubles as config_hash_before/after in the token_universe runtime_ack wire,
// where RuntimeAckBroadcastSchema requires exactly /^[0-9a-f]{64}$/ (or null
// for "no prior row"). These tests pin both roles.
import { describe, expect, it } from "vitest";
import { tokenUniverseVersionKey, universeFingerprint } from "./trading-config.js";

const HEX64 = /^[0-9a-f]{64}$/;

describe("universeFingerprint (EMIT-04)", () => {
  it("null only for absent prior config — every real array is hex64", () => {
    expect(universeFingerprint(null)).toBeNull();
    expect(universeFingerprint(undefined)).toBeNull();
    expect(universeFingerprint([])).toMatch(HEX64);
    expect(universeFingerprint(["WETH"])).toMatch(HEX64);
  });

  it("set-semantics: order, case, whitespace and duplicates do not change it", () => {
    const a = universeFingerprint(["WETH", "USDC", "1INCH"]);
    const b = universeFingerprint(["1INCH", " usdc ", "WETH", "WETH"]);
    expect(b).toBe(a);
    // TW-002 form: addresses fold to lowercase identity like symbols fold up.
    expect(universeFingerprint(["0xABC0000000000000000000000000000000000001"])).toBe(
      universeFingerprint(["0xabc0000000000000000000000000000000000001"]),
    );
  });

  it("an effective change produces a different fingerprint (the bump trigger)", () => {
    const before = universeFingerprint(["WETH", "USDC"]);
    const afterAdd = universeFingerprint(["WETH", "USDC", "DAI"]);
    const afterRemove = universeFingerprint(["WETH"]);
    expect(afterAdd).not.toBe(before);
    expect(afterRemove).not.toBe(before);
    // permissive → permissive (empty stays empty) is NOT a change: a first
    // config saved with an empty allowlist must not churn the version counter.
    expect(universeFingerprint([])).toBe(universeFingerprint(["  ", ""]));
  });

  it("version key is chain-scoped under the token_universe namespace", () => {
    expect(tokenUniverseVersionKey(1)).toBe("arbx:token_universe:version:1");
    expect(tokenUniverseVersionKey(11155111)).toBe(
      "arbx:token_universe:version:11155111",
    );
  });
});
