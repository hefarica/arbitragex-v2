import { promises as fs } from "node:fs";
import path from "node:path";
import type { ReadinessItem } from "../types.js";

const DEFAULT_REPO = "/repo";
const REQUIRED_TERMS = ["sandwich", "frontrun", "flashbots"];

/**
 * G-MEV-1 — MEV ethics: no-sandwich/no-frontrun documented.
 *
 * Doctrine arbx-mev-ethics-gate. The repo carries a public, signed policy
 * stating which strategies the platform refuses to run (sandwich attacks,
 * frontrunning of victim transactions, generalized backrunning of retail).
 * Without the doc, the gate cannot be flipped — operators have no
 * referenceable contract for what is and isn't allowed.
 *
 * Verification:
 *   1. docs/policies/mev-ethics.md exists.
 *   2. Body mentions sandwich, frontrun, and the chosen private mempool
 *      vendor (flashbots / mev-blocker / titan).
 *   3. Body has at least one machine-readable bullet (markdown - or *).
 */
export async function verifyGMEV1(opts?: {
  repo?: string;
  now?: () => Date;
}): Promise<ReadinessItem> {
  const repo = opts?.repo ?? DEFAULT_REPO;
  const verified_at = (opts?.now ?? (() => new Date()))().toISOString();
  const base = {
    id: "G-MEV-1",
    group: "operations" as const,
    label: "MEV ethics evidence: no-sandwich/no-frontrun documented",
    doctrine: "arbx-mev-ethics-gate",
    verified_at,
  };

  const docPath = path.join(repo, "docs/policies/mev-ethics.md");
  let body: string;
  try {
    body = await fs.readFile(docPath, "utf8");
  } catch {
    return {
      ...base,
      status: "red",
      reason: `policy doc missing at ${docPath}`,
    };
  }

  const lower = body.toLowerCase();
  const missing = REQUIRED_TERMS.filter((t) => !lower.includes(t));
  if (missing.length > 0) {
    return {
      ...base,
      status: "yellow",
      reason: `doc present but missing required terms: ${missing.join(", ")}`,
      evidence: { kind: "file", ref: "docs/policies/mev-ethics.md" },
    };
  }
  if (!/^[\s]*[-*]\s/m.test(body)) {
    return {
      ...base,
      status: "yellow",
      reason: "doc present and covers required terms but has no bullet list",
      evidence: { kind: "file", ref: "docs/policies/mev-ethics.md" },
    };
  }

  return {
    ...base,
    status: "green",
    reason: "policy doc names sandwich + frontrun + private-mempool vendor with structured bullets",
    evidence: { kind: "file", ref: "docs/policies/mev-ethics.md" },
  };
}
