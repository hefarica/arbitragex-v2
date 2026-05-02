import { promises as fs } from "node:fs";
import path from "node:path";
import type { ReadinessItem } from "../types.js";

const DEFAULT_DIR = "/repo/database/migrations";
// Matches PASSWORD '<literal>' but NOT psql variable substitution PASSWORD :'var'.
const PASSWORD_LITERAL_RE = /\bPASSWORD\s+'[^']+'/i;

/**
 * V-DB-1 — DB role passwords must come from psql variables, never inline literals.
 *
 * Verification: regex over /repo/database/migrations/*.sql.
 * - Match found anywhere → red (literal password leaked).
 * - No matches → green.
 * - Directory missing (no repo mount) → yellow with explicit reason.
 */
export async function verifyVDB1(opts?: {
  dir?: string;
  now?: () => Date;
}): Promise<ReadinessItem> {
  const dir = opts?.dir ?? DEFAULT_DIR;
  const verified_at = (opts?.now ?? (() => new Date()))().toISOString();
  const base = {
    id: "V-DB-1",
    group: "security_compliance" as const,
    label: "DB role passwords via psql vars (no SQL literals)",
    doctrine: "arbx-no-hardcode-doctrine",
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

  const sqlFiles = entries.filter((f) => f.endsWith(".sql"));
  for (const f of sqlFiles) {
    const content = await fs.readFile(path.join(dir, f), "utf8");
    if (PASSWORD_LITERAL_RE.test(content)) {
      return {
        ...base,
        status: "red",
        reason: `password literal found in ${f}`,
        evidence: { kind: "file", ref: `database/migrations/${f}` },
      };
    }
  }

  return {
    ...base,
    status: "green",
    reason: `${sqlFiles.length} migrations scanned, 0 password literals`,
    evidence: { kind: "shell", ref: "regex /\\bPASSWORD\\s+'[^']+'/i" },
  };
}
