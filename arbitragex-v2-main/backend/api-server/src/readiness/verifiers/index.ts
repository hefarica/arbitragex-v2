import type pg from "pg";
import type { ReadinessReport } from "../types.js";
import { verifyVDB1 } from "./v-db-1.js";
import { verifyVAT1 } from "./v-at-1.js";
import { verifyVNH1 } from "./v-nh-1.js";
import { verifyPR1CSP } from "./pr-1-csp.js";
import { verifyPR2Audit } from "./pr-2-audit.js";
import { verifyMonitoring } from "./monitoring.js";
import { verifyRunbook } from "./runbook.js";
import { verifyGRPC1 } from "./g-rpc-1.js";
import { verifyGNET1 } from "./g-net-1.js";
import { verifyGSIM1 } from "./g-sim-1.js";
import { verifyGPEC1 } from "./g-pec-1.js";
import { verifyGRIS1 } from "./g-ris-1.js";
import { verifyGTOK1 } from "./g-tok-1.js";
import { verifyGPAP1 } from "./g-pap-1.js";
import { verifyGFL1 } from "./g-fl-1.js";
import { verifyGMEV1 } from "./g-mev-1.js";
import { verifyAlerts } from "./alerts.js";

/**
 * Run all 17 verifiers in parallel. Each does live work — there are no
 * sentinel "pending" items left after the audit re-run #2 (2026-05-10).
 *
 * Honesty contract: a verifier returns:
 *   - green  → the doctrine is fully satisfied; evidence attached.
 *   - yellow → partially satisfied; specific reason in the response.
 *   - red    → fully unsatisfied; specific reason in the response.
 *
 * Time-based gates (G-PAP-1) return yellow with countdown until the
 * threshold is met; they are NEVER green-faked.
 */
export async function verifyAll(deps: {
  pool: pg.Pool | null;
  now?: () => Date;
}): Promise<ReadinessReport> {
  const now = deps.now ?? (() => new Date());

  const items = await Promise.all([
    verifyVNH1({ now }),
    verifyVDB1({ now }),
    verifyVAT1({ now }),
    verifyPR1CSP({ now }),
    verifyPR2Audit({ pool: deps.pool, now }),
    verifyMonitoring({ now }),
    verifyRunbook({ now }),
    verifyGRPC1({ now }),
    verifyGNET1({ now }),
    verifyGSIM1({ now }),
    verifyGPEC1({ now }),
    verifyGRIS1({ pool: deps.pool, now }),
    verifyGTOK1({ pool: deps.pool, now }),
    verifyGPAP1({ pool: deps.pool, now }),
    verifyGFL1({ now }),
    verifyGMEV1({ now }),
    verifyAlerts({ now }),
  ]);

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
