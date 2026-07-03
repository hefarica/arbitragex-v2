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

  it("passing sim with MISSING fields maps to deny-triggering values (never echoes client calldataHash)", async () => {
    const sim = createForkSimulator({
      ...CONFIGURED,
      fetchImpl: fakeFetch({ body: { result: { passed: true, gas_estimate_wei: "21000" } } }),
    });
    const r = await sim({ chain_id: 11155111, calldataHash: `0x${"cd".repeat(32)}` });
    expect(r).not.toBeNull();
    expect(r!.passed).toBe(true);
    expect(r!.gasEstimate).toBe("21000");
    // sim-ctl produced NO calldata/route/risk/profit → the sim's own values are empty/0 (deny-triggering).
    expect(r!.calldataHash).toBe(""); // NOT the client's hash
    expect(r!.routeHash).toBe("");
    expect(r!.riskScore).toBe(0);
    expect(r!.netProfitUsd).toBe(0);
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
});

describe("wallet-sim-runtime source — no signer/broadcast/key", () => {
  it("contains no signing/broadcast/private-key patterns", () => {
    const src = readFileSync(join(HERE, "wallet-sim-runtime.ts"), "utf8");
    expect(/privateKeyToAccount|mnemonicToAccount|signTypedData|\.sendTransaction\s*\(|\beth_sendTransaction\b|writeContract|--broadcast|--private-key/.test(src)).toBe(false);
  });
});
