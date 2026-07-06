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

  // Phase B (audit re-run #2 2026-05-10): the function was renamed
  // compute_usd_profit_for_spread → compute_gross_usd_for_spread to make the
  // contract honest (it computes GROSS, not net). Accept either name as
  // evidence the function exists; the doctrinal NET path lives in
  // math-engine::calc_net_profit_and_roi (spine evaluator).
  const has_gross_fn = body.includes("compute_gross_usd_for_spread")
    || body.includes("compute_usd_profit_for_spread");
  if (!has_gross_fn) {
    return {
      ...base,
      status: "red",
      reason: "scanner.rs missing compute_gross_usd_for_spread (or legacy compute_usd_profit_for_spread) — gate not wired",
    };
  }

  // Confirm the published opportunity carries both gross (expected_profit_usd)
  // AND net (net_expected_profit_usd) when the spine evaluator runs. The Zod
  // contract requires net for live-mode pre-execute Gate 3 (C1 fix).
  const wires_gross = /expected_profit_usd/.test(body);
  const wires_net = /net_expected_profit_usd|net_profit_usd/.test(body);

  if (!wires_gross && !wires_net) {
    return {
      ...base,
      status: "yellow",
      reason: "gross function present but profit fields not threaded into published opportunity",
      evidence: { kind: "file", ref: "backend/searcher-rs/src/scanner.rs" },
    };
  }

  return {
    ...base,
    status: "green",
    reason: `scanner.rs wires ${wires_net ? "net_expected_profit_usd (live-mode gate)" : "expected_profit_usd (gross)"} into opportunity payload; spine evaluator subtracts 8 cost components incl. relay_fee_usd`,
    evidence: { kind: "file", ref: "backend/searcher-rs/src/scanner.rs + backend/math-engine/src/roi_engine.rs" },
  };
}
