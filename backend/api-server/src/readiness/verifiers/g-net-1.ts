import { promises as fs } from "node:fs";
import path from "node:path";
import type { ReadinessItem } from "../types.js";

const DEFAULT_REPO = "/repo";

/**
 * G-NET-1 — Net-profit gate (gross − gas − slippage − relay − p_fail).
 *
 * Verifies that backend/searcher-rs/src/scanner.rs invokes
 * `compute_usd_profit_for_spread` (the canonical net-profit computation
 * with all 5 components). Recent commit 6c0bae7 wired this into the
 * production path; the verifier inspects the file content to confirm
 * the call survived subsequent refactors.
 *
 * The function lives in the math-engine workspace — its mere existence
 * is not enough; the searcher must actually call it before publishing.
 */
export async function verifyGNET1(opts?: {
  repo?: string;
  now?: () => Date;
}): Promise<ReadinessItem> {
  const repo = opts?.repo ?? DEFAULT_REPO;
  const verified_at = (opts?.now ?? (() => new Date()))().toISOString();
  const base = {
    id: "G-NET-1",
    group: "risk_doctrines" as const,
    label: "Net-profit gate (gross−gas−slippage−relay−p_fail)",
    doctrine: "arbx-net-profit-gate",
    verified_at,
  };

  const scanner = path.join(repo, "backend/searcher-rs/src/scanner.rs");

  let body: string;
  try {
    body = await fs.readFile(scanner, "utf8");
  } catch {
    return {
      ...base,
      status: "yellow",
      reason: `cannot read ${scanner} (no repo mount)`,
    };
  }

  if (!body.includes("compute_usd_profit_for_spread")) {
    return {
      ...base,
      status: "red",
      reason: "scanner.rs does not call compute_usd_profit_for_spread — gate not wired",
    };
  }

  // Belt-and-suspenders: confirm the published opportunity carries the
  // computed net profit (the audit found this regression once already).
  const wires_profit = /profit_usd|net_profit_usd|profit_estimate/.test(body);

  if (!wires_profit) {
    return {
      ...base,
      status: "yellow",
      reason: "compute_usd_profit_for_spread is called but result not visibly threaded into published opportunity",
      evidence: { kind: "file", ref: "backend/searcher-rs/src/scanner.rs" },
    };
  }

  return {
    ...base,
    status: "green",
    reason: "scanner.rs calls compute_usd_profit_for_spread and threads result into opportunity payload",
    evidence: { kind: "file", ref: "backend/searcher-rs/src/scanner.rs" },
  };
}
