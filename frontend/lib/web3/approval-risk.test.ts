import { describe, it, expect } from "vitest";
import { analyzeApproval, isUnlimitedApproval, type ApprovalRequest } from "./approval-risk";

const SPENDER = "0x5000000000000000000000000000000000000abc";
const TOKEN = "0xaaaa000000000000000000000000000000000001";

function req(over: Partial<ApprovalRequest> = {}): ApprovalRequest {
  return { token: TOKEN, spender: SPENDER, chainId: 1, amountWei: "1000", kind: "exact", nowSec: 1000, ...over };
}

describe("Approval Risk Analyzer — block-by-default for dangerous approvals", () => {
  it("exact-amount approval is low risk and not blocked", () => {
    const r = analyzeApproval(req({ kind: "exact" }));
    expect(r.level).toBe("low");
    expect(r.blocked).toBe(false);
    expect(r.display.amount).toBe("1000");
  });

  it("Permit2 SignatureTransfer is preferred (low, not blocked)", () => {
    const r = analyzeApproval(req({ kind: "permit2_signature" }));
    expect(r.level).toBe("low");
    expect(r.blocked).toBe(false);
  });

  it("unlimited approval is CRITICAL and BLOCKED", () => {
    const r = analyzeApproval(req({ kind: "unlimited" }));
    expect(r.level).toBe("critical");
    expect(r.blocked).toBe(true);
    expect(r.revokeSuggested).toBe(true);
    expect(r.display.amount).toBe("UNLIMITED");
  });

  it("MaxUint-class exact amount is treated as unlimited and BLOCKED", () => {
    const r = analyzeApproval(req({ kind: "exact", amountWei: (2n ** 256n - 1n).toString() }));
    expect(r.blocked).toBe(true);
    expect(r.level).toBe("critical");
    expect(isUnlimitedApproval(req({ kind: "exact", amountWei: (2n ** 256n - 1n).toString() }))).toBe(true);
  });

  it("setApprovalForAll is CRITICAL and BLOCKED", () => {
    const r = analyzeApproval(req({ kind: "set_approval_for_all" }));
    expect(r.level).toBe("critical");
    expect(r.blocked).toBe(true);
    expect(r.display.amount).toBe("ALL (setApprovalForAll)");
  });

  it("Permit2 unlimited allowance is BLOCKED", () => {
    expect(analyzeApproval(req({ kind: "permit2_allowance_unlimited" })).blocked).toBe(true);
  });

  it("Permit2 bounded allowance without expiry is high risk (revoke suggested)", () => {
    const r = analyzeApproval(req({ kind: "permit2_allowance_bounded", expiry: undefined }));
    expect(r.level).toBe("high");
    expect(r.revokeSuggested).toBe(true);
  });

  it("an invalid spender is CRITICAL and BLOCKED regardless of kind", () => {
    const r = analyzeApproval(req({ kind: "exact", spender: "0xnope" }));
    expect(r.blocked).toBe(true);
    expect(r.level).toBe("critical");
  });
});
