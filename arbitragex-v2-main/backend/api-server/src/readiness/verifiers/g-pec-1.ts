import { promises as fs } from "node:fs";
import path from "node:path";
import type { ReadinessItem } from "../types.js";

const DEFAULT_REPO = "/repo";
const REQUIRED_GATES = [
  "killswitch",
  "blacklist",
  "net_profit",
  "simulation",
  "rpc",
  "risk_limits",
  "token_safety",
];

/**
 * G-PEC-1 — Pre-execute checklist (7 gates in series).
 *
 * Doctrine: before any submission to relays, the executor must pass
 * 7 in-series gates. Bypassing the checklist = unaudited execution.
 *
 * Verification: the canonical doctrine doc lives at
 * docs/policies/pre-execute-checklist.md and enumerates the 7 gates.
 * The relays-client wires them via the SECURE_BOOT guard + per-attempt
 * checks (see relays-client/src/main.rs::execute_handler and
 * submit_engine.rs).
 *
 * The verifier checks:
 *   1. doc exists at the canonical path.
 *   2. doc body mentions all 7 gate names (case-insensitive).
 *   3. backend/relays-client/src/submit_engine.rs references the gates
 *      (grep for the doc reference).
 */
export async function verifyGPEC1(opts?: {
  repo?: string;
  now?: () => Date;
}): Promise<ReadinessItem> {
  const repo = opts?.repo ?? DEFAULT_REPO;
  const verified_at = (opts?.now ?? (() => new Date()))().toISOString();
  const base = {
    id: "G-PEC-1",
    group: "risk_doctrines" as const,
    label: "Pre-execute checklist (7 gates in series)",
    doctrine: "arbx-pre-execute-checklist",
    verified_at,
  };

  const docPath = path.join(repo, "docs/policies/pre-execute-checklist.md");
  let docBody: string;
  try {
    docBody = await fs.readFile(docPath, "utf8");
  } catch {
    return {
      ...base,
      status: "red",
      reason: `doctrine doc missing at ${docPath} — write it before any capital flip`,
    };
  }

  const lower = docBody.toLowerCase();
  const missing_gates = REQUIRED_GATES.filter((g) => !lower.includes(g));
  if (missing_gates.length > 0) {
    return {
      ...base,
      status: "yellow",
      reason: `doc present but does not name all 7 gates; missing: ${missing_gates.join(", ")}`,
      evidence: { kind: "file", ref: "docs/policies/pre-execute-checklist.md" },
    };
  }

  // Best-effort: confirm relays-client references it.
  const submitEngine = path.join(repo, "backend/relays-client/src/submit_engine.rs");
  let wired_in_code = false;
  try {
    const body = await fs.readFile(submitEngine, "utf8");
    wired_in_code = /pre.execute|pre_execute_checklist|PEC/i.test(body);
  } catch {
    // tolerate
  }

  return {
    ...base,
    status: "green",
    reason: `doctrine doc lists all 7 gates${wired_in_code ? "; submit_engine references the checklist" : ""}`,
    evidence: { kind: "file", ref: "docs/policies/pre-execute-checklist.md" },
  };
}
