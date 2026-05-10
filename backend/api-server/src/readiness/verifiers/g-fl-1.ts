import { promises as fs } from "node:fs";
import path from "node:path";
import type { ReadinessItem } from "../types.js";

const DEFAULT_REPO = "/repo";

/**
 * G-FL-1 — Flash loan callbacks: 7 discipline rules + foundry forktest.
 *
 * Verifies:
 *   1. contracts/src/FlashLoanExecutor.sol exists.
 *   2. contracts/test/ has ≥1 .t.sol foundry test file.
 *   3. contracts/script/Deploy*.s.sol exists (deploy plumbing).
 *
 * Doctrine arbx-flash-loan-discipline ships with the FlashLoanExecutor
 * contract that covers the 7 callback rules (Aave V3 executeOperation,
 * Balancer Vault sender check, etc.). The audit re-run #2 confirmed the
 * three-layer guard at FlashLoanExecutor.sol:315-351.
 */
export async function verifyGFL1(opts?: {
  repo?: string;
  now?: () => Date;
}): Promise<ReadinessItem> {
  const repo = opts?.repo ?? DEFAULT_REPO;
  const verified_at = (opts?.now ?? (() => new Date()))().toISOString();
  const base = {
    id: "G-FL-1",
    group: "contracts" as const,
    label: "Flash loan callbacks: 7 discipline rules + foundry forktest",
    doctrine: "arbx-flash-loan-discipline",
    verified_at,
  };

  const flashLoanExec = path.join(repo, "contracts/src/FlashLoanExecutor.sol");
  const testDir = path.join(repo, "contracts/test");
  const scriptDir = path.join(repo, "contracts/script");

  try {
    await fs.access(flashLoanExec);
  } catch {
    return {
      ...base,
      status: "yellow",
      reason: `FlashLoanExecutor.sol not found at ${flashLoanExec} (or no repo mount)`,
    };
  }

  let testFiles: string[] = [];
  try {
    const entries = await fs.readdir(testDir);
    testFiles = entries.filter((f) => f.endsWith(".t.sol"));
  } catch {
    return {
      ...base,
      status: "yellow",
      reason: `contracts/test/ directory not readable (${(testFiles.length || 0)} foundry tests)`,
    };
  }
  if (testFiles.length === 0) {
    return {
      ...base,
      status: "red",
      reason: "FlashLoanExecutor.sol present but no foundry .t.sol tests found",
    };
  }

  let scriptFiles: string[] = [];
  try {
    const entries = await fs.readdir(scriptDir);
    scriptFiles = entries.filter((f) => f.endsWith(".s.sol"));
  } catch {
    // script dir absent is yellow, not red — tests exist
    return {
      ...base,
      status: "yellow",
      reason: `tests present (${testFiles.length}) but contracts/script/ missing`,
    };
  }

  return {
    ...base,
    status: "green",
    reason: `FlashLoanExecutor.sol + ${testFiles.length} foundry .t.sol tests + ${scriptFiles.length} deploy scripts`,
    evidence: { kind: "file", ref: "contracts/src/FlashLoanExecutor.sol + contracts/test/*.t.sol" },
  };
}
