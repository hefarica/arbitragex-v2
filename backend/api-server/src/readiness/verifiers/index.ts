import type pg from "pg";
import type { ReadinessItem, ReadinessReport } from "../types.js";
import { pending } from "./pending.js";
import { verifyVDB1 } from "./v-db-1.js";
import { verifyVAT1 } from "./v-at-1.js";
import { verifyVNH1 } from "./v-nh-1.js";
import { verifyPR1CSP } from "./pr-1-csp.js";
import { verifyPR2Audit } from "./pr-2-audit.js";
import { verifyMonitoring } from "./monitoring.js";
import { verifyRunbook } from "./runbook.js";

/**
 * Run all 16 verifiers in parallel. Real verifiers do live work; sentinel
 * "pending" items return immediately. The summary reflects reality with
 * no false greens.
 *
 * When a doctrine PR ships (e.g., PR-3 RPC failover), REPLACE its pending()
 * call with `await verifyGRPC1(...)` and add the verifier file.
 */
export async function verifyAll(deps: {
  pool: pg.Pool | null;
  now?: () => Date;
}): Promise<ReadinessReport> {
  const now = deps.now ?? (() => new Date());

  const [vNH1, vDB1, vAT1, pr1, pr2, monitoring, runbook] = await Promise.all([
    verifyVNH1({ now }),
    verifyVDB1({ now }),
    verifyVAT1({ now }),
    verifyPR1CSP({ now }),
    verifyPR2Audit({ pool: deps.pool, now }),
    verifyMonitoring({ now }),
    verifyRunbook({ now }),
  ]);

  // Sentinels for unimplemented features. Each replaces with a real verifier
  // when its underlying PR ships.
  const sentinels: ReadinessItem[] = [
    pending({ id: "G-RPC-1", group: "risk_doctrines", label: "RPC failover ≥3 providers + health scoring", doctrine: "arbx-rpc-failover-discipline", reason: "PR-3 not started", now }),
    pending({ id: "G-NET-1", group: "risk_doctrines", label: "Net-profit gate (gross−gas−slippage−relay−p_fail)", doctrine: "arbx-net-profit-gate", reason: "PR-4 not started", now }),
    pending({ id: "G-SIM-1", group: "risk_doctrines", label: "Simulation mandatory (revm or eth_call+stateOverride)", doctrine: "arbx-simulation-mandatory", reason: "PR-5 not started", now }),
    pending({ id: "G-PEC-1", group: "risk_doctrines", label: "Pre-execute checklist (7 gates in series)", doctrine: "arbx-pre-execute-checklist", reason: "PR-6 not started", now }),
    pending({ id: "G-RIS-1", group: "risk_doctrines", label: "Risk limits + auto-kill on drawdown", doctrine: "arbx-risk-limits-enforcement", reason: "feature not implemented", now }),
    pending({ id: "G-TOK-1", group: "tokens_strategies", label: "Token safety screen (honeypot, tax, blacklist)", doctrine: "arbx-token-safety-screen", reason: "feature not implemented", now }),
    pending({ id: "G-PAP-1", group: "tokens_strategies", label: "Paper trading ≥7 days continuous + report", doctrine: "arbx-paper-trade-first", reason: "shadow execution pipeline not running", now }),
    pending({ id: "G-FL-1", group: "contracts", label: "Flash loan callbacks: 7 discipline rules + foundry forktest", doctrine: "arbx-flash-loan-discipline", reason: "no contracts/ directory yet", now }),
    pending({ id: "G-MEV-1", group: "operations", label: "MEV ethics evidence: no-sandwich/no-frontrun documented", doctrine: "arbx-mev-ethics-gate", reason: "policy doc not written", now }),
    pending({ id: "ALERTS", group: "operations", label: "Alert rules wired to PagerDuty/Telegram", reason: "rules exist but no real alerts have fired through", now }),
  ];

  const items = [vNH1, vDB1, vAT1, pr1, pr2, monitoring, runbook, ...sentinels];

  const summary = items.reduce(
    (acc, it) => {
      acc[it.status] += 1;
      acc.total += 1;
      return acc;
    },
    { green: 0, yellow: 0, red: 0, pending: 0, total: 0 },
  );

  return {
    items,
    summary,
    flip_blocked: summary.green !== summary.total,
    generated_at: now().toISOString(),
  };
}
