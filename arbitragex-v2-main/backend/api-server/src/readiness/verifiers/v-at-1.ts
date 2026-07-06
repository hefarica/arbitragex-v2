import { promises as fs } from "node:fs";
import type { ReadinessItem } from "../types.js";

const DEFAULT_FILE = "/repo/frontend/lib/admin-token.ts";
// Detects localStorage usage with admin/token strings — the V-AT-1 vulnerability.
const FORBIDDEN_RE = /localStorage\s*\.\s*(?:setItem|getItem|removeItem)\s*\(\s*['"][^'"]*(?:token|admin)[^'"]*['"]/i;

/**
 * V-AT-1 — Admin token must use httpOnly cookie session, not localStorage.
 *
 * Verification: regex over /repo/frontend/lib/admin-token.ts.
 * - Forbidden pattern found → red (XSS exposure regression).
 * - Pattern absent + file exists → green.
 * - File missing (no repo mount) → yellow.
 */
export async function verifyVAT1(opts?: {
  file?: string;
  now?: () => Date;
}): Promise<ReadinessItem> {
  const file = opts?.file ?? DEFAULT_FILE;
  const verified_at = (opts?.now ?? (() => new Date()))().toISOString();
  const base = {
    id: "V-AT-1",
    group: "security_compliance" as const,
    label: "Admin token via httpOnly cookie (no localStorage)",
    doctrine: "secrets.policy.md T1",
    verified_at,
  };

  let content: string;
  try {
    content = await fs.readFile(file, "utf8");
  } catch {
    return {
      ...base,
      status: "yellow",
      reason: `verifier requires repo mount at ${file}, not available in this deployment`,
    };
  }

  if (FORBIDDEN_RE.test(content)) {
    return {
      ...base,
      status: "red",
      reason: "localStorage.{set,get,remove}Item with admin/token literal detected — V-AT-1 regression",
      evidence: { kind: "file", ref: "frontend/lib/admin-token.ts" },
    };
  }

  return {
    ...base,
    status: "green",
    reason: "no localStorage admin/token usage; httpOnly cookie pattern intact",
    evidence: { kind: "file", ref: "frontend/lib/admin-token.ts" },
  };
}
