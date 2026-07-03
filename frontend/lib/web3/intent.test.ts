import { describe, it, expect } from "vitest";
import {
  ARBX_INTENT_VERSION,
  buildIntentTypedData,
  legibleIntentPreview,
  validateIntent,
  hashCalldata,
  isUnlimitedWei,
  UNLIMITED_THRESHOLD,
  type TransactionIntent,
} from "./intent";

const NOW = 1_800_000_000;

function goodIntent(over: Partial<TransactionIntent> = {}): TransactionIntent {
  return {
    version: ARBX_INTENT_VERSION,
    chainId: 11155111,
    wallet: "0x1111111111111111111111111111111111111111",
    executor: "0x2222222222222222222222222222222222222222",
    routeHash: `0x${"ab".repeat(32)}`,
    calldataHash: `0x${"cd".repeat(32)}`,
    simulationId: "sim-123",
    tokenIn: "0x3333333333333333333333333333333333333333",
    tokenOut: "0x4444444444444444444444444444444444444444",
    amountIn: "1000000000000000000",
    maxGasWei: "500000000000000",
    maxSlippageBps: 50,
    minProfitUsd: "12.5",
    deadline: NOW + 600,
    nonce: "7",
    policyId: "policy-abc",
    riskScore: 80,
    ...over,
  };
}

describe("TransactionIntent EIP-712", () => {
  it("builds a legible typed-data payload with the named primaryType (never opaque calldata)", () => {
    const td = buildIntentTypedData(goodIntent());
    expect(td.primaryType).toBe("TransactionIntent");
    expect(td.types.TransactionIntent.length).toBe(17);
    expect(td.domain.chainId).toBe(11155111);
    expect(td.message.version).toBe("ARBX_INTENT_V1");
  });

  it("preview renders every field as a human-readable row incl. the binding calldata hash", () => {
    const rows = legibleIntentPreview(goodIntent());
    const labels = rows.map((r) => r.label);
    expect(labels).toContain("Calldata hash (bound to sim)");
    expect(labels).toContain("Token in");
    expect(labels).toContain("Risk score");
    expect(rows.length).toBeGreaterThanOrEqual(16);
  });

  it("validateIntent accepts a well-formed intent", () => {
    expect(validateIntent(goodIntent(), NOW)).toEqual([]);
  });

  it("validateIntent rejects a missing calldataHash (cannot bind to simulation)", () => {
    const errs = validateIntent(goodIntent({ calldataHash: "0x00" as `0x${string}` }), NOW);
    expect(errs.some((e) => e.includes("calldataHash"))).toBe(true);
  });

  it("validateIntent rejects an expired deadline", () => {
    const errs = validateIntent(goodIntent({ deadline: NOW - 1 }), NOW);
    expect(errs.some((e) => e.includes("deadline"))).toBe(true);
  });

  it("hashCalldata is deterministic keccak256", () => {
    expect(hashCalldata("0x1234")).toBe(hashCalldata("0x1234"));
    expect(hashCalldata("0x1234")).not.toBe(hashCalldata("0x5678"));
  });

  it("isUnlimitedWei flags MaxUint-class amounts", () => {
    expect(isUnlimitedWei(UNLIMITED_THRESHOLD.toString())).toBe(true);
    expect(isUnlimitedWei((2n ** 256n - 1n).toString())).toBe(true);
    expect(isUnlimitedWei("1000000000000000000")).toBe(false);
  });
});
