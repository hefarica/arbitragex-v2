import { describe, it, expect } from "vitest";
import { evaluatePolicy, policyVerdict, POLICY_GATE_NAMES, type PolicyContext } from "./policy";
import { ARBX_INTENT_VERSION, type TransactionIntent } from "./intent";

const NOW = 1_800_000_000;
const CALLDATA_HASH = `0x${"cd".repeat(32)}` as const;

function intent(over: Partial<TransactionIntent> = {}): TransactionIntent {
  return {
    version: ARBX_INTENT_VERSION,
    chainId: 11155111,
    wallet: "0x1111111111111111111111111111111111111111",
    executor: "0x2222222222222222222222222222222222222222",
    routeHash: `0x${"ab".repeat(32)}`,
    calldataHash: CALLDATA_HASH,
    simulationId: "sim-1",
    tokenIn: "0xAAAa000000000000000000000000000000000001",
    tokenOut: "0xbbbb000000000000000000000000000000000002",
    amountIn: "1000",
    maxGasWei: "1000",
    maxSlippageBps: 50,
    minProfitUsd: "10",
    deadline: NOW + 600,
    nonce: "1",
    policyId: "p1",
    riskScore: 90,
    ...over,
  };
}

// A context under which EVERY gate passes (allow=true).
function passingCtx(over: Partial<PolicyContext> = {}): PolicyContext {
  return {
    chainAllowlist: [11155111],
    tokenAllowlist: ["0xaaaa000000000000000000000000000000000001", "0xbbbb000000000000000000000000000000000002"],
    routerAllowlist: ["0xr0000000000000000000000000000000000000ab".toLowerCase()],
    dexAllowlist: ["uniswap_v2"],
    spenderAllowlist: ["0x5000000000000000000000000000000000000abc"],
    selectorAllowlist: ["0x38ed1739"],
    router: "0xr0000000000000000000000000000000000000ab",
    dex: "uniswap_v2",
    spender: "0x5000000000000000000000000000000000000ABC",
    selector: "0x38ED1739",
    amountCapWei: "10000",
    approvalAmountWei: "1000",
    approvalCapWei: "10000",
    approvalExpiry: NOW + 3600,
    simulationPassed: true,
    simulationCalldataHash: CALLDATA_HASH,
    usedNonces: [],
    netProfitUsd: 10,
    riskScoreMin: 60,
    gasCapWei: "10000",
    nowSec: NOW,
    readinessGreen: true,
    killSwitchOff: true,
    liveGateOpen: true,
    ...over,
  };
}

describe("Policy Engine — 19 gates, deny-by-default", () => {
  it("exposes exactly the 19 canonical gate names", () => {
    expect(POLICY_GATE_NAMES.length).toBe(19);
  });

  it("all gates pass => allow (SIGN_ALLOWED)", () => {
    const r = evaluatePolicy(intent(), passingCtx());
    expect(r.allow).toBe(true);
    expect(r.gates.length).toBe(19);
    expect(policyVerdict(r)).toBe("SIGN_ALLOWED");
  });

  const denyCases: Array<[string, Partial<PolicyContext>, Partial<TransactionIntent>, string]> = [
    ["unknown router", { routerAllowlist: ["0xother"] }, {}, "router_allowed"],
    ["unknown spender", { spenderAllowlist: ["0xother"] }, {}, "spender_allowed"],
    ["unknown selector", { selectorAllowlist: ["0xdeadbeef"] }, {}, "selector_allowed"],
    ["unlimited approval", { approvalAmountWei: (2n ** 255n).toString() }, {}, "approval_within_cap"],
    ["approval no expiry", { approvalExpiry: 0 }, {}, "approval_has_expiry"],
    ["expired deadline", {}, { deadline: NOW - 1 }, "deadline_valid"],
    ["calldata mismatch", {}, { calldataHash: `0x${"00".repeat(32)}` as `0x${string}` }, "calldata_hash_matches_sim"],
    ["netProfit <= 0", { netProfitUsd: 0 }, {}, "net_profit_usd_positive"],
    ["riskScore below min", { riskScoreMin: 95 }, {}, "risk_score_minimum"],
    ["sim not passed", { simulationPassed: false }, {}, "simulation_passed"],
    ["readiness NO_GO", { readinessGreen: false }, {}, "readiness_green"],
    ["kill switch on", { killSwitchOff: false }, {}, "kill_switch_off"],
    ["live gate closed", { liveGateOpen: false }, {}, "live_gate_open"],
    ["chain not allowed", { chainAllowlist: [1] }, {}, "chain_allowed"],
    ["token not allowed", { tokenAllowlist: [] }, {}, "token_allowed"],
    ["amount over cap", { amountCapWei: "1" }, {}, "amount_within_cap"],
    ["gas over cap", { gasCapWei: "1" }, {}, "gas_within_cap"],
    ["nonce reused", { usedNonces: ["1"] }, {}, "nonce_unused"],
  ];

  for (const [label, ctxOver, intentOver, gate] of denyCases) {
    it(`blocks: ${label} (${gate})`, () => {
      const r = evaluatePolicy(intent(intentOver), passingCtx(ctxOver));
      expect(r.allow).toBe(false);
      expect(r.denied).toContain(gate);
      expect(policyVerdict(r)).toBe("DENIED");
    });
  }

  it("fails closed on unparseable amounts (does not throw)", () => {
    const r = evaluatePolicy(intent({ amountIn: "not-a-number" }), passingCtx());
    expect(r.allow).toBe(false);
    expect(r.denied).toContain("amount_within_cap");
  });
});
