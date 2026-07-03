import { describe, it, expect } from "vitest";
import express from "express";
import request from "supertest";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { mountWalletRoutes } from "./wallet.js";
import { createForkSimulator, type ForkSimDeps } from "./wallet-sim-runtime.js";

const HERE = dirname(fileURLToPath(import.meta.url));
const logger = { warn: () => {}, info: () => {} };

// Minimal fetch stub — never touches the network.
function fakeFetch(opts: { ok?: boolean; status?: number; body?: unknown; throwErr?: boolean; badJson?: boolean }): typeof fetch {
  return (async () => {
    if (opts.throwErr) throw new Error("ECONNREFUSED");
    return {
      ok: opts.ok ?? true,
      status: opts.status ?? 200,
      json: async () => {
        if (opts.badJson) throw new Error("bad json");
        return opts.body ?? {};
      },
    };
  }) as unknown as typeof fetch;
}

const CONFIGURED: ForkSimDeps = { logger, simBase: "http://sim-ctl:3003", v2Ready: true };

describe("createForkSimulator — fail-closed", () => {
  it("no SIM base configured → null", async () => {
    const sim = createForkSimulator({ logger, simBase: "", v2Ready: true });
    expect(await sim({ chain_id: 11155111 })).toBeNull();
  });

  it("simulator-v2 not ready → null", async () => {
    const sim = createForkSimulator({ logger, simBase: "http://sim-ctl:3003", v2Ready: false });
    expect(await sim({ chain_id: 11155111 })).toBeNull();
  });

  it("upstream throws (unreachable) → null", async () => {
    const sim = createForkSimulator({ ...CONFIGURED, fetchImpl: fakeFetch({ throwErr: true }) });
    expect(await sim({ chain_id: 11155111 })).toBeNull();
  });

  it("non-2xx from sim-ctl → null", async () => {
    const sim = createForkSimulator({ ...CONFIGURED, fetchImpl: fakeFetch({ ok: false, status: 501 }) });
    expect(await sim({ chain_id: 11155111 })).toBeNull();
  });

  it("unparseable body → null", async () => {
    const sim = createForkSimulator({ ...CONFIGURED, fetchImpl: fakeFetch({ badJson: true }) });
    expect(await sim({ chain_id: 11155111 })).toBeNull();
  });

  it("real gas but MISSING calldata/route/profit/risk → passed=false (incomplete), honest missing[]+reason, never echoes client hash", async () => {
    const sim = createForkSimulator({
      ...CONFIGURED,
      fetchImpl: fakeFetch({ body: { result: { passed: true, gas_estimate_wei: "21000" } } }),
    });
    const r = await sim({ chain_id: 11155111, calldataHash: `0x${"cd".repeat(32)}` });
    expect(r).not.toBeNull();
    // gas is REAL evidence; the rest is honestly absent.
    expect(r!.gasEstimate).toBe("21000");
    // Wallet-level pass requires COMPLETE evidence → false even though sim-ctl said passed:true.
    expect(r!.passed).toBe(false);
    expect(r!.missing).toEqual(["calldataHash", "routeHash", "net_profit_usd", "risk_score"]);
    expect(r!.reason).toBe("calldata_not_produced"); // first missing
    // NEVER the client's hash.
    expect(r!.calldataHash).toBe("");
    expect(r!.routeHash).toBe("");
    expect(r!.riskScore).toBe(0);
    expect(r!.netProfitUsd).toBe(0);
  });

  it("sim-ctl fail_reason wins over missing-field reason", async () => {
    const sim = createForkSimulator({
      ...CONFIGURED,
      fetchImpl: fakeFetch({ body: { result: { passed: false, fail_reason: "sim_timeout" } } }),
    });
    const r = await sim({ chain_id: 1 });
    expect(r!.passed).toBe(false);
    expect(r!.reason).toBe("sim_timeout");
    expect(r!.missing).toContain("gas_estimate");
  });
});

describe("POST /api/wallet/simulate with wired adapter — deny-by-default", () => {
  function appWith(deps: Parameters<typeof mountWalletRoutes>[1]) {
    const app = express();
    app.use(express.json());
    mountWalletRoutes(app, deps);
    return app;
  }

  it("adapter fail-closed (v2 not ready) → runtime_not_configured, allow=false", async () => {
    const app = appWith({ logger, forkSimulator: createForkSimulator({ logger, simBase: "http://x", v2Ready: false }) });
    const r = await request(app).post("/api/wallet/simulate").send({ chain_id: 11155111 });
    expect(r.body.reason).toBe("runtime_not_configured");
    expect(r.body.allow).toBe(false);
    expect(r.body.broadcast_allowed).toBe(false);
  });

  it("passing sim + readiness green + kill-switch off + calldata match → STILL allow=false (live_gate_open structural)", async () => {
    const app = appWith({
      logger,
      readiness: async () => ({ green: true }),
      killSwitch: async () => ({ off: true }),
      // adapter returns a sim whose calldataHash matches the request (simulates a future-complete runtime)
      forkSimulator: async (i) => ({
        passed: true,
        calldataHash: i.calldataHash ?? "",
        routeHash: `0x${"ab".repeat(32)}`,
        netProfitUsd: 100,
        riskScore: 90,
        gasEstimate: "21000",
      }),
    });
    const r = await request(app)
      .post("/api/wallet/simulate")
      .send({ chain_id: 11155111, calldataHash: `0x${"cd".repeat(32)}` });
    expect(r.body.simulation_passed).toBe(true);
    expect(r.body.calldata_hash_matches_sim).toBe(true);
    expect(r.body.readiness_green).toBe(true);
    expect(r.body.kill_switch_off).toBe(true);
    expect(r.body.live_gate_open).toBe(false);
    expect(r.body.allow).toBe(false);
    expect(r.body.denied).toContain("live_gate_open");
    // SAFE_POSTURE immutable.
    expect(r.body.live_enabled).toBe(false);
    expect(r.body.capital_exposed).toBe(0);
    expect(r.body.broadcast).toBe(false);
    expect(r.body.broadcast_allowed).toBe(false);
  });

  it("readiness NO-GO → deny readiness_green", async () => {
    const app = appWith({
      logger,
      readiness: async () => ({ green: false }),
      killSwitch: async () => ({ off: true }),
      forkSimulator: async (i) => ({ passed: true, calldataHash: i.calldataHash ?? "", routeHash: "", netProfitUsd: 100, riskScore: 90, gasEstimate: "1" }),
    });
    const r = await request(app).post("/api/wallet/simulate").send({ chain_id: 1, calldataHash: "0xab" });
    expect(r.body.readiness_green).toBe(false);
    expect(r.body.allow).toBe(false);
    expect(r.body.denied).toContain("readiness_green");
  });

  it("kill-switch on → deny kill_switch_off", async () => {
    const app = appWith({
      logger,
      readiness: async () => ({ green: true }),
      killSwitch: async () => ({ off: false }),
      forkSimulator: async (i) => ({ passed: true, calldataHash: i.calldataHash ?? "", routeHash: "", netProfitUsd: 100, riskScore: 90, gasEstimate: "1" }),
    });
    const r = await request(app).post("/api/wallet/simulate").send({ chain_id: 1, calldataHash: "0xab" });
    expect(r.body.kill_switch_off).toBe(false);
    expect(r.body.allow).toBe(false);
    expect(r.body.denied).toContain("kill_switch_off");
  });

  it("real adapter (partial evidence) surfaces runtime_configured + missing[] + reason, still allow=false", async () => {
    const app = appWith({
      logger,
      readiness: async () => ({ green: true }),
      killSwitch: async () => ({ off: true }),
      forkSimulator: createForkSimulator({
        logger,
        simBase: "http://sim-ctl:3003",
        v2Ready: true,
        fetchImpl: fakeFetch({ body: { result: { passed: true, gas_estimate_wei: "21000" } } }),
      }),
    });
    const r = await request(app).post("/api/wallet/simulate").send({ chain_id: 11155111, calldataHash: `0x${"cd".repeat(32)}` });
    expect(r.body.runtime_configured).toBe(true);
    expect(r.body.gas_estimate).toBe("21000"); // real evidence reached the endpoint
    expect(r.body.missing).toEqual(["calldataHash", "routeHash", "net_profit_usd", "risk_score"]);
    expect(r.body.reason).toBe("calldata_not_produced");
    expect(r.body.simulation_passed).toBe(false); // incomplete evidence
    expect(r.body.allow).toBe(false);
    expect(r.body.live_gate_open).toBe(false);
    expect(r.body.broadcast_allowed).toBe(false);
  });
});

describe("wallet-sim-runtime source — no signer/broadcast/key", () => {
  it("contains no signing/broadcast/private-key patterns", () => {
    const src = readFileSync(join(HERE, "wallet-sim-runtime.ts"), "utf8");
    expect(/privateKeyToAccount|mnemonicToAccount|signTypedData|\.sendTransaction\s*\(|\beth_sendTransaction\b|writeContract|--broadcast|--private-key/.test(src)).toBe(false);
  });
});
