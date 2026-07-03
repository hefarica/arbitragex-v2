/**
 * wallet-sim-runtime — READ-ONLY fork-sim adapter for POST /api/wallet/simulate.
 *
 * It implements the `WalletRoutesDeps.forkSimulator` DI seam already declared in wallet.ts. It proxies
 * a simulation request to sim-ctl (the real simulator service, same host as opportunity-simulate.ts) and
 * maps the result onto the SimResult contract. It NEVER signs, broadcasts, or holds a key — it only asks
 * a simulation service to simulate. CI stays keyless; capital stays 0.
 *
 * FAIL-CLOSED (returns null → the endpoint reports reason="runtime_not_configured"):
 *   - no SIM base URL configured, OR
 *   - simulator-v2 is not declared ready (ARBX_SIMULATOR_V2_READY !== "true"), OR
 *   - sim-ctl is unreachable / times out / returns non-2xx / an unparseable body.
 *
 * HONEST MAPPING: fields sim-ctl does not actually produce stay DENY-triggering — we NEVER echo the
 * client's calldataHash as the simulation's (that would falsely satisfy calldata_hash_matches_sim).
 * The simulation's own calldataHash/routeHash/riskScore/netProfitUsd are taken from the sim result if
 * present, else default to empty/0 so the policy gates deny. allow stays false regardless (the endpoint
 * forces live_gate_open:false).
 */

import type { SimulateInput, SimResult } from "./wallet.js";

const DEFAULT_SIM_BASE = process.env["SIM_URL"] ?? process.env["SIM_CTL_INTERNAL_URL"] ?? "";
const DEFAULT_V2_READY = process.env["ARBX_SIMULATOR_V2_READY"] === "true";
const TIMEOUT_MS = 15_000;

export interface ForkSimDeps {
  logger: { warn: (obj: object, msg?: string) => void };
  // Overridable for tests / explicit wiring. Defaults read the environment.
  simBase?: string;
  v2Ready?: boolean;
  fetchImpl?: typeof fetch;
}

type SimCtlResult = {
  passed?: unknown;
  gas_estimate_wei?: unknown;
  simulated_profit_usd?: unknown;
  calldata_hash?: unknown;
  route_hash?: unknown;
  risk_score?: unknown;
};

function asNumberString(v: unknown): string {
  if (typeof v === "string" && /^\d+$/.test(v)) return v;
  if (typeof v === "number" && Number.isFinite(v)) return String(Math.trunc(v));
  return "0";
}

/** Build the read-only forkSimulator dependency. Fail-closed until a ready runtime is reachable. */
export function createForkSimulator(deps: ForkSimDeps): (input: SimulateInput) => Promise<SimResult | null> {
  const simBase = deps.simBase ?? DEFAULT_SIM_BASE;
  const v2Ready = deps.v2Ready ?? DEFAULT_V2_READY;
  const doFetch = deps.fetchImpl ?? fetch;

  return async (input: SimulateInput): Promise<SimResult | null> => {
    // Fail-closed: the runtime must be explicitly configured AND declared ready. Today simulator-v2 is a
    // stub (readiness g-sim-1), so this returns null and the endpoint stays runtime_not_configured.
    if (!simBase || !v2Ready) return null;

    const ctrl = new AbortController();
    const timer = setTimeout(() => ctrl.abort(), TIMEOUT_MS);
    try {
      const upstream = await doFetch(`${simBase}/simulate`, {
        method: "POST",
        headers: { "content-type": "application/json", accept: "application/json" },
        // Read-only simulation request. No key, no signer, no broadcast field.
        body: JSON.stringify({
          chain_id: input.chain_id ?? null,
          to: input.to ?? null,
          value: input.value ?? null,
          data: input.data ?? null,
          route_hash: input.routeHash ?? null,
          calldata_hash: input.calldataHash ?? null,
          mode: "read_only",
        }),
        signal: ctrl.signal,
      });

      if (!upstream.ok) return null; // non-2xx → fail-closed (honest refusal)

      let body: unknown;
      try {
        body = await upstream.json();
      } catch {
        return null; // unparseable → fail-closed
      }
      const r: SimCtlResult | null =
        body && typeof body === "object"
          ? ((body as { result?: unknown }).result as SimCtlResult) ?? (body as SimCtlResult)
          : null;
      if (!r || typeof r !== "object") return null;

      // Map only what the simulator truly produces. Missing fields default to deny-triggering values.
      return {
        passed: r.passed === true,
        // The simulation's OWN calldata hash — NOT the client's. Empty today → mismatch → deny.
        calldataHash: typeof r.calldata_hash === "string" ? r.calldata_hash : "",
        routeHash: typeof r.route_hash === "string" ? r.route_hash : "",
        netProfitUsd: typeof r.simulated_profit_usd === "number" ? r.simulated_profit_usd : 0,
        riskScore: typeof r.risk_score === "number" ? r.risk_score : 0,
        gasEstimate: asNumberString(r.gas_estimate_wei),
      };
    } catch (e) {
      deps.logger.warn({ event: "wallet.forksim.upstream_failed", err: (e as Error).message });
      return null; // unreachable / timeout → fail-closed
    } finally {
      clearTimeout(timer);
    }
  };
}
