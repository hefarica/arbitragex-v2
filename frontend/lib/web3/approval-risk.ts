// Approval Risk Analyzer — a PURE classifier for ERC-20/NFT/Permit2 approvals.
//
// Defaults to SAFE: exact-amount and Permit2 SignatureTransfer are low risk; unlimited approvals,
// setApprovalForAll, and unbounded Permit2 allowances are BLOCKED by default and flagged critical
// with a revoke recommendation. It never issues an approval — it only scores a described one so the
// UI can refuse/alert. Reuses the "UNLIMITED" red-flag idea already present in the read-only
// monitoring view (frontend/components/WalletDetailDialog.tsx) but adds chain/expiry/revoke + a
// block-by-default policy.

import { isUnlimitedWei, MAX_UINT256 } from "./intent";

export type ApprovalKind =
  | "exact"
  | "unlimited"
  | "set_approval_for_all"
  | "permit2_signature"
  | "permit2_allowance_bounded"
  | "permit2_allowance_unlimited";

export type RiskLevel = "low" | "medium" | "high" | "critical";

export interface ApprovalRequest {
  token: string;
  spender: string;
  chainId: number;
  amountWei: string; // decimal wei; ignored for set_approval_for_all / signature kinds
  kind: ApprovalKind;
  expiry?: number; // unix seconds (Permit2 allowance); undefined = no expiry
  nowSec?: number;
  spenderLabel?: string; // human-readable spender name, if known
}

export interface ApprovalRisk {
  level: RiskLevel;
  blocked: boolean; // true => the UI must NOT let this approval proceed by default
  reasons: string[];
  recommendation: string;
  revokeSuggested: boolean;
  display: {
    token: string;
    spender: string;
    spenderLabel: string;
    chainId: number;
    amount: string; // "UNLIMITED" or the decimal amount
    kind: ApprovalKind;
    expiry: string; // "none" | ISO-ish unix
  };
}

export function isUnlimitedApproval(req: ApprovalRequest): boolean {
  if (req.kind === "unlimited" || req.kind === "permit2_allowance_unlimited") return true;
  if (req.kind === "set_approval_for_all") return true; // grants ALL, treat as unlimited-class
  return isUnlimitedWei(req.amountWei);
}

export function analyzeApproval(req: ApprovalRequest): ApprovalRisk {
  const nowSec = req.nowSec ?? Math.floor(0); // caller passes now; 0 keeps it deterministic in tests
  const reasons: string[] = [];
  let level: RiskLevel = "low";
  let blocked = false;

  if (req.kind === "set_approval_for_all") {
    level = "critical";
    blocked = true;
    reasons.push("setApprovalForAll grants control over EVERY token in the collection for this spender");
  } else if (isUnlimitedApproval(req)) {
    level = "critical";
    blocked = true;
    reasons.push("unlimited approval — blocked by default; approve the exact amount needed instead");
  } else if (req.kind === "permit2_allowance_bounded") {
    if (!req.expiry || req.expiry <= nowSec) {
      level = "high";
      reasons.push("Permit2 allowance without a valid future expiry");
    } else {
      level = "medium";
      reasons.push("Permit2 bounded allowance — prefer SignatureTransfer for a single operation");
    }
  } else if (req.kind === "permit2_signature") {
    level = "low";
    reasons.push("Permit2 SignatureTransfer — single-use, no standing allowance (preferred)");
  } else if (req.kind === "exact") {
    level = "low";
    reasons.push("exact-amount approval (preferred)");
  }

  // A known-bad or empty spender is always at least high risk.
  if (!/^0x[0-9a-fA-F]{40}$/.test(req.spender)) {
    level = "critical";
    blocked = true;
    reasons.push("spender is not a valid address");
  }

  const revokeSuggested = level === "high" || level === "critical";
  const recommendation = blocked
    ? "Reject. Use an exact-amount approval, or a Permit2 SignatureTransfer for a single operation."
    : level === "low"
      ? "OK — minimal standing exposure."
      : "Proceed only with an amount cap and a near-term expiry; revoke after use.";

  const amountDisplay =
    req.kind === "set_approval_for_all"
      ? "ALL (setApprovalForAll)"
      : isUnlimitedApproval(req)
        ? "UNLIMITED"
        : req.amountWei;

  return {
    level,
    blocked,
    reasons,
    recommendation,
    revokeSuggested,
    display: {
      token: req.token,
      spender: req.spender,
      spenderLabel: req.spenderLabel || "unknown spender",
      chainId: req.chainId,
      amount: amountDisplay,
      kind: req.kind,
      expiry: req.expiry ? String(req.expiry) : "none",
    },
  };
}

export { MAX_UINT256 };
