/**
 * Live readiness checklist — types and contract.
 *
 * The 16 items represent the full doctrinal gate between paper_mode=true and
 * any future flip to live execution. Status is computed dynamically (option C
 * from spec): real verifiers for shipped systems, sentinel "pending" for
 * unimplemented features. No false greens.
 */

export type ReadinessStatus = "green" | "yellow" | "red" | "pending";

export type ReadinessGroup =
  | "security_compliance"
  | "audit_trail"
  | "risk_doctrines"
  | "tokens_strategies"
  | "contracts"
  | "operations";

export type ReadinessEvidence = {
  kind: "commit" | "file" | "endpoint" | "db_query" | "shell" | "config";
  ref: string;
};

export type ReadinessItem = {
  id: string;                    // V-NH-1, G-RPC-1, PR-2, etc.
  group: ReadinessGroup;
  label: string;
  status: ReadinessStatus;
  reason: string;                // human-readable, always set
  evidence?: ReadinessEvidence;
  doctrine?: string;             // arbx-* skill name when relevant
  verified_at: string;           // ISO timestamp
};

export type ReadinessReport = {
  items: ReadinessItem[];
  summary: {
    green: number;
    yellow: number;
    red: number;
    pending: number;
    total: number;
  };
  flip_blocked: boolean;         // true if any item not green
  generated_at: string;
};
