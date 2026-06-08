/**
 * readiness-steps tests — pure-function assertions (no Express, no PG, no file IO).
 *
 * Coverage:
 *   - evalTopology: PASS only with ≥1 WSS + ≥1 RPC; empty/partial → PENDING; read error → FAIL.
 *   - evalCredentials: PASS on signer OR key; redacts to "present"|null; relay alone is not enough.
 *   - evalMarkets: PASS needs chains+dexes+pools; DB error → FAIL.
 *   - evalEngines: PASS on ≥1 engine; live_enabled ALWAYS false.
 *   - statusFor + assembleSteps: cascade (step N BLOCKED until N-1 PASSES).
 *   - buildResponse: completed_steps / N/4 / all_ready / live_enabled false.
 *   - REGRESSION: no raw secret ever appears in the serialized response.
 */
import { describe, it, expect, beforeEach, afterEach } from "vitest";
import {
  __forTesting,
  type StepInputs,
  type TopologyStep,
  type CredentialsStep,
  type MarketsStep,
  type EnginesStep,
} from "./readiness-steps.js";
import type { TopologySnapshot } from "./topology-vault.js";

const { evalTopology, evalCredentials, evalMarkets, evalEngines, statusFor, assembleSteps, buildResponse, anyEnvPresent } =
  __forTesting;

const SAVED_ENV = { ...process.env };
beforeEach(() => {
  process.env = { ...SAVED_ENV };
});
afterEach(() => {
  process.env = { ...SAVED_ENV };
});

function snapshot(ws: Array<{ scheme: string; host: string }>, http: Array<{ scheme: string; host: string }>): TopologySnapshot {
  return {
    scope: "staging",
    chain_id: 1,
    mempool_mode: "auto",
    rpc_http_1: http.map((p, i) => ({
      name: `h${i}`,
      url_masked: `${p.scheme}://...${p.host}/****abcd`,
      scheme: p.scheme as TopologySnapshot["rpc_http_1"][number]["scheme"],
      host: p.host,
      provider_kind: "standard",
    })),
    rpc_ws_1: ws.map((p, i) => ({
      name: `w${i}`,
      url_masked: `${p.scheme}://...${p.host}/****wxyz`,
      scheme: p.scheme as TopologySnapshot["rpc_ws_1"][number]["scheme"],
      host: p.host,
      provider_kind: "standard",
    })),
    checksum: "deadbeef",
    version_id: 42,
    updated_at: "2026-06-08T00:00:00.000Z",
  };
}

describe("evalTopology", () => {
  it("PASS with ≥1 active WSS + ≥1 active RPC", () => {
    const t = evalTopology({ ok: true, error: null, snapshot: snapshot([{ scheme: "wss", host: "a.io" }], [{ scheme: "https", host: "b.io" }]) });
    expect(t.ready).toBe(true);
    expect(t.fail).toBe(false);
    expect(t.wss_active_count).toBe(1);
    expect(t.rpc_active_count).toBe(1);
    expect(t.snapshot_non_empty).toBe(true);
  });

  it("PENDING (not ready) when vault empty", () => {
    const t = evalTopology({ ok: true, error: null, snapshot: null });
    expect(t.ready).toBe(false);
    expect(t.fail).toBe(false);
    expect(t.snapshot_non_empty).toBe(false);
    expect(t.reason).toContain("topology_vault_empty");
  });

  it("not ready when RPC present but no WSS", () => {
    const t = evalTopology({ ok: true, error: null, snapshot: snapshot([], [{ scheme: "https", host: "b.io" }]) });
    expect(t.ready).toBe(false);
    expect(t.wss_active_count).toBe(0);
    expect(t.rpc_active_count).toBe(1);
    expect(t.snapshot_non_empty).toBe(true);
  });

  it("FAIL on hard read error", () => {
    const t = evalTopology({ ok: false, error: "EACCES", snapshot: null });
    expect(t.fail).toBe(true);
    expect(t.ready).toBe(false);
    expect(t.reason).toContain("topology_vault_read_error");
  });
});

describe("evalCredentials (redaction)", () => {
  it("PASS on signer present; emits 'present' not the value", () => {
    const c = evalCredentials({ signerPresent: true, privateKeyPresent: false, privateRelayPresent: false });
    expect(c.ready).toBe(true);
    expect(c.signer).toBe("present");
    expect(c.private_key).toBe(null);
    expect(c.redacted).toBe(true);
  });

  it("PASS on private key present", () => {
    const c = evalCredentials({ signerPresent: false, privateKeyPresent: true, privateRelayPresent: false });
    expect(c.ready).toBe(true);
    expect(c.private_key).toBe("present");
  });

  it("NOT ready on relay alone (venue, not authority)", () => {
    const c = evalCredentials({ signerPresent: false, privateKeyPresent: false, privateRelayPresent: true });
    expect(c.ready).toBe(false);
    expect(c.private_relay).toBe("present");
    expect(c.signer).toBe(null);
  });

  it("all null when nothing present", () => {
    const c = evalCredentials({ signerPresent: false, privateKeyPresent: false, privateRelayPresent: false });
    expect(c.ready).toBe(false);
    expect(c.signer).toBe(null);
    expect(c.private_key).toBe(null);
    expect(c.private_relay).toBe(null);
  });
});

describe("evalMarkets", () => {
  it("PASS needs chains+dexes+pools ≥1", () => {
    const m = evalMarkets({ chains: 1, dexes: 2, pools: 3, tokens: 4, dbError: false });
    expect(m.ready).toBe(true);
    expect(m.fail).toBe(false);
  });

  it("NOT ready when pools=0", () => {
    const m = evalMarkets({ chains: 1, dexes: 1, pools: 0, tokens: 0, dbError: false });
    expect(m.ready).toBe(false);
    expect(m.reason).toContain("market_topology_incomplete");
  });

  it("FAIL on db error", () => {
    const m = evalMarkets({ chains: null, dexes: null, pools: null, tokens: null, dbError: true });
    expect(m.fail).toBe(true);
    expect(m.ready).toBe(false);
  });
});

describe("evalEngines", () => {
  it("PASS on ≥1 engine; live_enabled ALWAYS false", () => {
    const e = evalEngines({ enginesActive: ["dex_arb"], paperMode: true, shadowMode: true, dbError: false });
    expect(e.ready).toBe(true);
    expect(e.live_enabled).toBe(false);
    expect(e.paper_ready).toBe(true);
    expect(e.shadow_ready).toBe(true);
  });

  it("paper_ready false when paper mode off, but never live", () => {
    const e = evalEngines({ enginesActive: ["dex_arb"], paperMode: false, shadowMode: false, dbError: false });
    expect(e.live_enabled).toBe(false);
    expect(e.paper_ready).toBe(false);
  });

  it("NOT ready when no engines", () => {
    const e = evalEngines({ enginesActive: [], paperMode: true, shadowMode: true, dbError: false });
    expect(e.ready).toBe(false);
    expect(e.reason).toContain("resolution_engines_off");
  });
});

describe("statusFor (cascade primitive)", () => {
  it("BLOCKED when previous step did not pass", () => {
    expect(statusFor(true, false, false)).toBe("BLOCKED");
  });
  it("FAIL takes precedence over ready when reachable", () => {
    expect(statusFor(true, true, true)).toBe("FAIL");
  });
  it("PASS when ready + reachable + no fail", () => {
    expect(statusFor(true, false, true)).toBe("PASS");
  });
  it("PENDING when reachable but not ready", () => {
    expect(statusFor(false, false, true)).toBe("PENDING");
  });
});

function inputs(over: Partial<StepInputs> = {}): StepInputs {
  return {
    topology: { ok: true, error: null, snapshot: null },
    credentials: { signerPresent: false, privateKeyPresent: false, privateRelayPresent: false },
    markets: { chains: 0, dexes: 0, pools: 0, tokens: 0, dbError: false },
    engines: { enginesActive: [], paperMode: true, shadowMode: false, dbError: false },
    ...over,
  };
}

describe("assembleSteps (full cascade)", () => {
  it("0/4: nothing ready → step1 PENDING, steps 2-4 BLOCKED", () => {
    const steps = assembleSteps(inputs());
    expect(steps.map((s) => s.status)).toEqual(["PENDING", "BLOCKED", "BLOCKED", "BLOCKED"]);
    expect(steps.every((s) => s.ready === false)).toBe(true);
  });

  it("1/4: topology PASS unlocks credentials to PENDING; markets/engines still BLOCKED", () => {
    const steps = assembleSteps(
      inputs({ topology: { ok: true, error: null, snapshot: snapshot([{ scheme: "wss", host: "a.io" }], [{ scheme: "https", host: "b.io" }]) } }),
    );
    expect(steps.map((s) => s.status)).toEqual(["PASS", "PENDING", "BLOCKED", "BLOCKED"]);
  });

  it("4/4 path: all real evidence → all PASS, all_ready, live still off", () => {
    const steps = assembleSteps(
      inputs({
        topology: { ok: true, error: null, snapshot: snapshot([{ scheme: "wss", host: "a.io" }], [{ scheme: "https", host: "b.io" }]) },
        credentials: { signerPresent: true, privateKeyPresent: false, privateRelayPresent: false },
        markets: { chains: 1, dexes: 1, pools: 1, tokens: 1, dbError: false },
        engines: { enginesActive: ["dex_arb"], paperMode: true, shadowMode: true, dbError: false },
      }),
    );
    expect(steps.map((s) => s.status)).toEqual(["PASS", "PASS", "PASS", "PASS"]);
    const engines = steps[3] as EnginesStep;
    expect(engines.live_enabled).toBe(false);
    const resp = buildResponse(steps, new Date("2026-06-08T00:00:00Z"));
    expect(resp.completed_steps).toBe(4);
    expect(resp.readiness_score).toBe("4/4");
    expect(resp.all_ready).toBe(true);
    expect(resp.live_enabled).toBe(false);
    expect(resp.capital_exposure_usd).toBe(0);
  });

  it("a downstream FAIL stays hidden as BLOCKED until its prerequisite passes", () => {
    // markets db is down, but credentials hasn't passed → step 3 must be BLOCKED, not FAIL.
    const steps = assembleSteps(
      inputs({
        topology: { ok: true, error: null, snapshot: snapshot([{ scheme: "wss", host: "a.io" }], [{ scheme: "https", host: "b.io" }]) },
        markets: { chains: null, dexes: null, pools: null, tokens: null, dbError: true },
      }),
    );
    expect(steps[2]!.status).toBe("BLOCKED");
  });
});

describe("buildResponse counts", () => {
  it("readiness_score reflects PASS count", () => {
    const steps = assembleSteps(
      inputs({ topology: { ok: true, error: null, snapshot: snapshot([{ scheme: "wss", host: "a.io" }], [{ scheme: "https", host: "b.io" }]) } }),
    );
    const resp = buildResponse(steps, new Date("2026-06-08T00:00:00Z"));
    expect(resp.completed_steps).toBe(1);
    expect(resp.readiness_score).toBe("1/4");
    expect(resp.all_ready).toBe(false);
  });
});

describe("REGRESSION: no secret leakage", () => {
  it("response never embeds raw signer/key/url even when env vars are set", () => {
    process.env["SIM_SIGNER_ADDRESS"] = "0xSIGNERADDRESS00000000000000000000000000";
    process.env["PRIVATE_KEY"] = "0xPRIVATEKEYSECRET1234567890abcdefSECRET";
    process.env["PRIVATE_RELAY_URL"] = "https://relay.example.com/SECRET-RELAY-PATH";
    const creds = {
      signerPresent: anyEnvPresent(__forTesting.SIGNER_ENV_VARS),
      privateKeyPresent: anyEnvPresent(__forTesting.PRIVATE_KEY_ENV_VARS),
      privateRelayPresent: anyEnvPresent(__forTesting.PRIVATE_RELAY_ENV_VARS),
    };
    const steps = assembleSteps(inputs({ credentials: creds, topology: { ok: true, error: null, snapshot: snapshot([{ scheme: "wss", host: "a.io" }], [{ scheme: "https", host: "b.io" }]) } }));
    const serialized = JSON.stringify(buildResponse(steps, new Date()));
    expect(serialized).not.toContain("SIGNERADDRESS");
    expect(serialized).not.toContain("PRIVATEKEYSECRET");
    expect(serialized).not.toContain("SECRET-RELAY-PATH");
    // but presence IS reported
    const cstep = steps[1] as CredentialsStep;
    expect(cstep.signer).toBe("present");
    expect(cstep.private_key).toBe("present");
    expect(cstep.private_relay).toBe("present");
  });

  it("topology evidence carries only masked urls (no raw key path)", () => {
    const steps = assembleSteps(
      inputs({ topology: { ok: true, error: null, snapshot: snapshot([{ scheme: "wss", host: "a.io" }], [{ scheme: "https", host: "b.io" }]) } }),
    );
    const tstep = steps[0] as TopologyStep;
    const serialized = JSON.stringify(tstep.evidence);
    expect(serialized).toContain("****");
    expect(serialized).not.toContain("/v2/");
  });
});
