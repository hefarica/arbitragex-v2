/**
 * C-15a — executor on-chain probe.
 *
 * Controls (the point of the card):
 *   + deployed executor (code, paused()=false, AccessControl)  → healthy
 *   − EOA / empty slot (eth_getCode = 0x)                      → no_code (BLOCKED)
 *   − contract without Pausable/AccessControl                  → wrong_contract
 *   − paused()=true                                            → paused (PAUSED)
 *   − RPC down/HTTP 500/timeout                                → rpc_unavailable (UNKNOWN)
 *   − malformed / missing config                               → invalid_address / not_configured
 * No failure path may return `healthy`.
 */
import { describe, it, expect } from "vitest";
import {
  probeExecutorHealth,
  EXECUTOR_ROLE_HASH,
  GET_ROLE_ADMIN_SELECTOR,
  PAUSED_SELECTOR,
} from "./onChainExecutorHealth.js";

const ADDR = "0x1111111111111111111111111111111111111111";
const ENV = {
  EXECUTOR_1: ADDR,
  RPC_HTTP_1: "https://eth.example.com/v2/SECRET-API-KEY-9999",
} as NodeJS.ProcessEnv;

const word = (n: number): string => `0x${n.toString(16).padStart(64, "0")}`;
const CODE = "0x6001600155"; // 5 bytes

type Reply = { result?: string; error?: { code?: number; message?: string } };

function mockFetch(handlers: {
  getCode?: Reply | "http500" | "throw" | "hang";
  paused?: Reply | "throw";
  role?: Reply | "throw";
}): typeof fetch {
  return (async (_url: string, init: RequestInit) => {
    const body = JSON.parse(String(init.body)) as {
      method: string;
      params?: Array<{ data?: string }>;
    };
    const data = body.params?.[0]?.data ?? "";
    let h: Reply | "http500" | "throw" | "hang" | undefined;
    if (body.method === "eth_getCode") h = handlers.getCode;
    else if (data.startsWith(PAUSED_SELECTOR)) h = handlers.paused;
    else if (data.startsWith(GET_ROLE_ADMIN_SELECTOR)) h = handlers.role;
    if (h === "hang") {
      await new Promise<void>(() => { /* never resolves */ });
      throw new Error("unreachable");
    }
    if (h === "throw") throw new Error("boom");
    if (h === "http500") return { ok: false, status: 500 } as Response;
    if (!h) throw new Error(`unexpected rpc call: ${body.method} ${data}`);
    return { ok: true, status: 200, json: async () => h } as Response;
  }) as unknown as typeof fetch;
}

const probe = (handlers: Parameters<typeof mockFetch>[0], timeoutMs = 500) =>
  probeExecutorHealth({ chainId: 1, env: ENV, fetchImpl: mockFetch(handlers), timeoutMs });

describe("probeExecutorHealth — positive control", () => {
  it("deployed, unpaused, AccessControl wired → healthy", async () => {
    const r = await probe({
      getCode: { result: CODE },
      paused: { result: word(0) },
      role: { result: word(0) },
    });
    expect(r.status).toBe("healthy");
    expect(r.ran).toBe(true);
    expect(r.bytecode_size).toBe(5);
    expect(r.paused).toBe(false);
    expect(r.access_control_admin).toBe(word(0));
    expect(r.error).toBe(null);
    expect(r.rpc_host).toBe("eth.example.com");
    // The URL path carries the API key — it must never leave the module.
    expect(JSON.stringify(r)).not.toContain("SECRET-API-KEY");
  });

  it("asks exactly getCode + paused() + getRoleAdmin(EXECUTOR_ROLE)", async () => {
    const seen: string[] = [];
    const impl = (async (_url: string, init: RequestInit) => {
      const body = JSON.parse(String(init.body)) as { method: string; params?: Array<{ data?: string }> };
      seen.push(`${body.method}:${body.params?.[0]?.data ?? ""}`);
      const data = body.params?.[0]?.data ?? "";
      const reply =
        body.method === "eth_getCode"
          ? { result: CODE }
          : data.startsWith(PAUSED_SELECTOR)
            ? { result: word(0) }
            : { result: word(0) };
      return { ok: true, status: 200, json: async () => reply } as Response;
    }) as unknown as typeof fetch;
    await probeExecutorHealth({ chainId: 1, env: ENV, fetchImpl: impl, timeoutMs: 500 });
    expect(seen[0]).toBe(`eth_getCode:${ADDR}`);
    expect(seen).toContain(`eth_call:${PAUSED_SELECTOR}`);
    expect(seen).toContain(`eth_call:${GET_ROLE_ADMIN_SELECTOR}${EXECUTOR_ROLE_HASH.slice(2)}`);
    expect(seen).toHaveLength(3);
  });
});

describe("probeExecutorHealth — negative controls", () => {
  it("EOA / empty slot (0x) → no_code", async () => {
    const r = await probe({ getCode: { result: "0x" } });
    expect(r.status).toBe("no_code");
    expect(r.bytecode_size).toBe(0);
    expect(r.paused).toBe(null);
  });

  it("paused()=true → paused", async () => {
    const r = await probe({
      getCode: { result: CODE },
      paused: { result: word(1) },
      role: { result: word(0) },
    });
    expect(r.status).toBe("paused");
    expect(r.paused).toBe(true);
  });

  it("paused() reverted (not a Pausable contract) → wrong_contract", async () => {
    const r = await probe({
      getCode: { result: CODE },
      paused: { error: { code: 3, message: "execution reverted" } },
      role: { result: word(0) },
    });
    expect(r.status).toBe("wrong_contract");
    expect(r.error).toContain("Pausable");
  });

  it("getRoleAdmin reverted (no AccessControl) → wrong_contract", async () => {
    const r = await probe({
      getCode: { result: CODE },
      paused: { result: word(0) },
      role: { error: { code: 3, message: "execution reverted" } },
    });
    expect(r.status).toBe("wrong_contract");
    expect(r.access_control_admin).toBe(null);
  });

  it("paused() returns a non-boolean word → wrong_contract", async () => {
    const r = await probe({
      getCode: { result: CODE },
      paused: { result: word(2) },
      role: { result: word(0) },
    });
    expect(r.status).toBe("wrong_contract");
  });

  it("RPC HTTP 500 → rpc_unavailable", async () => {
    const r = await probe({ getCode: "http500" });
    expect(r.status).toBe("rpc_unavailable");
    expect(r.ran).toBe(false);
  });

  it("RPC network error → rpc_unavailable", async () => {
    const r = await probe({ getCode: "throw" });
    expect(r.status).toBe("rpc_unavailable");
    expect(r.error).toContain("eth_getCode");
  });

  it("RPC timeout → rpc_unavailable", async () => {
    const r = await probe({ getCode: "hang" }, 25);
    expect(r.status).toBe("rpc_unavailable");
    expect(r.error).toContain("timeout");
  });

  it("paused() probe transport failure → rpc_unavailable (not wrong_contract)", async () => {
    const r = await probe({
      getCode: { result: CODE },
      paused: "throw",
      role: { result: word(0) },
    });
    expect(r.status).toBe("rpc_unavailable");
  });

  it("missing EXECUTOR_1 → not_configured", async () => {
    const r = await probeExecutorHealth({ chainId: 1, env: {} as NodeJS.ProcessEnv, fetchImpl: mockFetch({}) });
    expect(r.status).toBe("not_configured");
    expect(r.error).toContain("EXECUTOR_1");
  });

  it("malformed EXECUTOR_1 → invalid_address", async () => {
    const r = await probeExecutorHealth({
      chainId: 1,
      env: { EXECUTOR_1: "0xdeadbeef", RPC_HTTP_1: "https://eth.example.com" } as NodeJS.ProcessEnv,
      fetchImpl: mockFetch({}),
    });
    expect(r.status).toBe("invalid_address");
  });

  it("missing RPC for the chain → not_configured", async () => {
    const r = await probeExecutorHealth({
      chainId: 1,
      env: { EXECUTOR_1: ADDR } as NodeJS.ProcessEnv,
      fetchImpl: mockFetch({}),
    });
    expect(r.status).toBe("not_configured");
    expect(r.error).toContain("RPC_HTTP_1");
  });
});
