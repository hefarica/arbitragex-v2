import { promises as fs } from "node:fs";
import type { ReadinessItem } from "../types.js";

const DEFAULT_DIR = "/repo/docs/runbooks";
const MIN_RUNBOOKS = 3; // killswitch-activated, relay-degraded, rotate-secrets at minimum

/**
 * Runbook coverage — operator runbooks present in /repo/docs/runbooks/.
 *
 * - >=MIN_RUNBOOKS .md files → green.
 * - 1..MIN_RUNBOOKS-1 → yellow (partial coverage).
 * - 0 → red.
 * - Directory missing (no repo mount) → yellow.
 */
export async function verifyRunbook(opts?: {
  dir?: string;
  minCount?: number;
  now?: () => Date;
}): Promise<ReadinessItem> {
  const dir = opts?.dir ?? DEFAULT_DIR;
  const minCount = opts?.minCount ?? MIN_RUNBOOKS;
  const verified_at = (opts?.now ?? (() => new Date()))().toISOString();
  const base = {
    id: "RUNBOOK",
    group: "operations" as const,
    label: `Operator runbooks (≥${minCount} .md files in docs/runbooks/)`,
    verified_at,
  };

  let entries: string[];
  try {
    entries = await fs.readdir(dir);
  } catch {
    return {
      ...base,
      status: "yellow",
      reason: `verifier requires repo mount at ${dir}, not available in this deployment`,
    };
  }

  const md = entries.filter((f) => f.endsWith(".md"));
  if (md.length === 0) {
    return { ...base, status: "red", reason: "no runbook .md files present" };
  }
  if (md.length < minCount) {
    return {
      ...base,
      status: "yellow",
      reason: `${md.length} runbook file(s); minimum ${minCount} expected`,
      evidence: { kind: "file", ref: `docs/runbooks/ (${md.join(", ")})` },
    };
  }
  return {
    ...base,
    status: "green",
    reason: `${md.length} runbook files present`,
    evidence: { kind: "file", ref: `docs/runbooks/` },
  };
}
