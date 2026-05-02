import { promises as fs } from "node:fs";
import { exec } from "node:child_process";
import { promisify } from "node:util";
import type { ReadinessItem } from "../types.js";

const execp = promisify(exec);

const DEFAULT_SCRIPT = "/repo/automation/tools/lint-no-hardcode.sh";
const DEFAULT_CWD = "/repo";

/**
 * V-NH-1 — No hardcoded productive data (RPC URLs, contract addresses, keys).
 *
 * Verification: shell out to lint-no-hardcode.sh. Exit 0 → green; non-zero → red.
 * Timeout 10s. If script not present (no repo mount) → yellow.
 */
export async function verifyVNH1(opts?: {
  script?: string;
  cwd?: string;
  timeoutMs?: number;
  now?: () => Date;
}): Promise<ReadinessItem> {
  const script = opts?.script ?? DEFAULT_SCRIPT;
  const cwd = opts?.cwd ?? DEFAULT_CWD;
  const timeout = opts?.timeoutMs ?? 10_000;
  const verified_at = (opts?.now ?? (() => new Date()))().toISOString();
  const base = {
    id: "V-NH-1",
    group: "security_compliance" as const,
    label: "No-hardcode doctrine: zero productive literals in repo",
    doctrine: "arbx-no-hardcode-doctrine",
    verified_at,
  };

  try {
    await fs.access(script);
  } catch {
    return {
      ...base,
      status: "yellow",
      reason: `lint script not available at ${script} (repo mount missing)`,
    };
  }

  try {
    await execp(`bash ${script}`, { cwd, timeout, maxBuffer: 1024 * 1024 });
    return {
      ...base,
      status: "green",
      reason: "lint-no-hardcode.sh exited 0",
      evidence: { kind: "shell", ref: "automation/tools/lint-no-hardcode.sh" },
    };
  } catch (e) {
    const msg = (e as Error).message.split("\n")[0]?.slice(0, 200) ?? "unknown";
    return {
      ...base,
      status: "red",
      reason: `lint-no-hardcode.sh failed: ${msg}`,
      evidence: { kind: "shell", ref: "automation/tools/lint-no-hardcode.sh" },
    };
  }
}
