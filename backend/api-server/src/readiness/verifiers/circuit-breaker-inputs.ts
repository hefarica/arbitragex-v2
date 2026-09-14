/** Readiness inputs actually consumed by the circuit-breaker evaluator.
 * This is NOT a full readiness report or permission to enable LIVE. Full
 * go/no-go validation continues to call verifyAll (all 19 gates).
 */
import type pg from "pg";
import type { ReadinessReport } from "../types.js";
import { createSingleFlight } from "../single-flight.js";
import { verifyGRPC1 } from "./g-rpc-1.js";
import { verifyGSIM1 } from "./g-sim-1.js";
import { verifyGTOK1 } from "./g-tok-1.js";

type BreakerInputs = Pick<ReadinessReport, "items">;
const flight = createSingleFlight<pg.Pool | null, BreakerInputs>();
export function verifyCircuitBreakerInputs(deps: { pool: pg.Pool | null }): Promise<BreakerInputs> {
  return flight(deps.pool, async () => ({
    items: await Promise.all([
      verifyGRPC1(),
      verifyGSIM1({ pool: deps.pool }),
      verifyGTOK1({ pool: deps.pool }),
    ]),
  }));
}
