/**
 * C-15a — executor on-chain probe.
 *
 * Controls (the point of the card):
 *   + deployed executor, unpaused, FLE wired and armed, operator armed → healthy
 *   − EOA / empty slot (eth_getCode = 0x)                      → no_code (BLOCKED)
 *   − contract without Pausable/AccessControl                  → wrong_contract
 *   − paused()=true                                            → paused (PAUSED)
 *   − FLE holds no EXECUTOR_ROLE                               → not_armed (BLOCKED)
 *   − operator holds no EXECUTOR_ROLE                          → operator_not_armed (BLOCKED)
 *   − FLE points at another AE                                 → wiring_mismatch (BLOCKED)
 *   − FLE address / operator identity unknown                  → identity_not_configured (UNKNOWN)
 *   − RPC down/HTTP 500/timeout                                → rpc_unavailable (UNKNOWN)
 *   − malformed / missing config                               → invalid_address / not_configured
 * No failure path may return `healthy`.
 */
import { describe, it, expect } from "vitest";
import {
  probeExecutorHealth,
  ARBITRAGE_EXECUTOR_SELECTOR,
  EXECUTOR_ROLE_HASH,
  HAS_ROLE_SELECTOR,
  PAUSED_SELECTOR,
} from "./onChainExecutorHealth.js";

const AE = "0x1111111111111111111111111111111111111111";
const FLE = "0x2222222222222222222222222222222222222222";
const OP = "0x3333333333333333333333333333333333333333";
const ENV = {
  EXECUTOR_1: AE,
  FLASHLOAN_EXECUTOR_1: FLE,
  SIGNER_ADDRESS: OP,
  RPC_HTTP_1: "https://eth.example.com/v2/SECRET-API-KEY-9999",
} as NodeJS.ProcessEnv;

const word = (n: number): string => `0x${n.toString(16).padStart(64, "0")}`;
const addrWord = (a: string): string => `0x${a.slice(2).toLowerCase().padStart(64, "0")}`;
const CODE = "0x6001600155"; // 5 bytes

type Reply = { result?: string; error?: { code?: number; message?: string } };
type Handler = Reply | "http500" | "throw" | "hang";

function mockFetch(handlers: {
  getCode?: Handler;
  paused?: Handler;
  fleRole?: Handler;
  operatorRole?: Handler;
  wiring?: Handler;
}): typeof fetch {
  return (async (_url: string, init: RequestInit) => {
    const body = JSON.parse(String(init.body)) as {
      method: string;
      params?: Array<string | { to?: string; data?: string }>;
    };
    const first = body.params?.[0];
    const to = typeof first === "string" ? undefined : first?.to;
    const data = typeof first === "string" ? "" : (first?.data ?? "");
    let h: Handler | undefined;
    if (body.method === "eth_getCode") h = handlers.getCode;
    else if (data.startsWith(PAUSED_SELECTOR)) h = handlers.paused;
    else if (data.startsWith(HAS_ROLE_SELECTOR)) {
      const grantee = `0x${data.slice(-40).toLowerCase()}`;
      h =
        grantee === FLE.toLowerCase()
          ? handlers.fleRole
          : grantee === OP.toLowerCase()
            ? handlers.operatorRole
            : undefined;
    } else if (data.startsWith(ARBITRAGE_EXECUTOR_SELECTOR)) h = handlers.wiring;
    if (h === "hang") {
      await new Promise<void>(() => {
        /* never resolves */
      });
      throw new Error("unreachable");
    }
    if (h === "throw") throw new Error("boom");
    if (h === "http500") return { ok: false, status: 500 } as Response;
    if (!h) throw new Error(`unexpected rpc call: ${body.method} ${String(to)} ${data}`);
    return { ok: true, status: 200, json: async () => h } as Response;
  }) as unknown as typeof fetch;
}

const healthy = {
  getCode: { result: CODE },
  paused: { result: word(0) },
  fleRole: { result: word(1) },
  operatorRole: { result: word(1) },
  wiring: { result: addrWord(AE) },
};

const probe = (handlers: Parameters<typeof mockFetch>[0], over: NodeJS.ProcessEnv = ENV, chainId = 1, timeoutMs = 500) =>
  probeExecutorHealth({ chainId, env: over, fetchImpl: mockFetch(handlers), timeoutMs });

describe("probeExecutorHealth — positive control", () => {
  it("deployed, unpaused, FLE wired + armed, operator armed → healthy", async () => {
    const r = await probe(healthy);
    expect(r.status).toBe("healthy");
    expect(r.ran).toBe(true);
    expect(r.bytecode_size).toBe(5);
    expect(r.paused).toBe(false);
    expect(r.fle_address).toBe(FLE);
    expect(r.fle_has_executor_role).toBe(true);
    expect(r.operator_address).toBe(OP);
    expect(r.operator_has_executor_role).toBe(true);
    expect(r.fle_reported_executor).toBe(AE);
    expect(r.error).toBe(null);
    expect(r.rpc_host).toBe("eth.example.com");
    // The URL path carries the API key — it must never leave the module.
    expect(JSON.stringify(r)).not.toContain("SECRET-API-KEY");
  });

  it("accepts EXECUTION_SIGNER_ADDRESS as the operator alias", async () => {
    const env = { ...ENV } as NodeJS.ProcessEnv;
    delete env["SIGNER_ADDRESS"];
    env["EXECUTION_SIGNER_ADDRESS"] = OP;
    const r = await probe(healthy, env);
    expect(r.status).toBe("healthy");
    expect(r.operator_address).toBe(OP);
  });

  it("asks exactly getCode + paused + hasRole(FLE) + arbitrageExecutor + hasRole(operator)", async () => {
    const seen: string[] = [];
    const impl = (async (_url: string, init: RequestInit) => {
      const body = JSON.parse(String(init.body)) as {
        method: string;
        params?: Array<string | { to?: string; data?: string }>;
      };
      const first = body.params?.[0];
      const to = typeof first === "string" ? first : (first?.to ?? "");
      const data = typeof first === "string" ? "" : (first?.data ?? "");
      seen.push(`${body.method}:${to}:${data}`);
      const reply =
        body.method === "eth_getCode"
          ? { result: CODE }
          : data.startsWith(PAUSED_SELECTOR)
            ? { result: word(0) }
            : data.startsWith(HAS_ROLE_SELECTOR)
              ? { result: word(1) }
              : { result: addrWord(AE) };
      return { ok: true, status: 200, json: async () => reply } as Response;
    }) as unknown as typeof fetch;
    await probeExecutorHealth({ chainId: 1, env: ENV, fetchImpl: impl, timeoutMs: 500 });
    expect(seen[0]).toBe(`eth_getCode:${AE}:`);
    expect(seen).toContain(`eth_call:${AE}:${PAUSED_SELECTOR}`);
    expect(seen).toContain(
      `eth_call:${AE}:${HAS_ROLE_SELECTOR}${EXECUTOR_ROLE_HASH.slice(2)}${addrWord(FLE).slice(2)}`,
    );
    expect(seen).toContain(`eth_call:${FLE}:${ARBITRAGE_EXECUTOR_SELECTOR}`);
    expect(seen).toContain(
      `eth_call:${AE}:${HAS_ROLE_SELECTOR}${EXECUTOR_ROLE_HASH.slice(2)}${addrWord(OP).slice(2)}`,
    );
    expect(seen).toHaveLength(5);
  });

  it("reads chain-scoped env (EXECUTOR_<id> / FLASHLOAN_EXECUTOR_<id>)", async () => {
    const env = {
      EXECUTOR_8453: AE,
      FLASHLOAN_EXECUTOR_8453: FLE,
      SIGNER_ADDRESS: OP,
      RPC_HTTP_8453: "https://base.example.com",
    } as NodeJS.ProcessEnv;
    const r = await probe(healthy, env, 8453);
    expect(r.status).toBe("healthy");
    expect(r.chain_id).toBe(8453);
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
    const r = await probe({ ...healthy, paused: { result: word(1) } });
    expect(r.status).toBe("paused");
    expect(r.paused).toBe(true);
  });

  it("paused() reverted (not a Pausable contract) → wrong_contract", async () => {
    const r = await probe({ ...healthy, paused: { error: { code: 3, message: "execution reverted" } } });
    expect(r.status).toBe("wrong_contract");
    expect(r.error).toContain("Pausable");
  });

  it("hasRole reverted (no AccessControl) → wrong_contract", async () => {
    const r = await probe({ ...healthy, fleRole: { error: { code: 3, message: "execution reverted" } } });
    expect(r.status).toBe("wrong_contract");
    expect(r.fle_has_executor_role).toBe(null);
  });

  it("FLE holds no EXECUTOR_ROLE → not_armed (never healthy)", async () => {
    const r = await probe({ ...healthy, fleRole: { result: word(0) } });
    expect(r.status).toBe("not_armed");
    expect(r.fle_has_executor_role).toBe(false);
    expect(r.error).toContain("flash_executor_to_arbitrage_executor");
  });

  it("operator holds no EXECUTOR_ROLE → operator_not_armed", async () => {
    const r = await probe({ ...healthy, operatorRole: { result: word(0) } });
    expect(r.status).toBe("operator_not_armed");
    expect(r.operator_has_executor_role).toBe(false);
    expect(r.error).toContain("caller_to_flash_executor");
  });

  it("FLE points at another AE → wiring_mismatch", async () => {
    const r = await probe({
      ...healthy,
      wiring: { result: addrWord("0x4444444444444444444444444444444444444444") },
    });
    expect(r.status).toBe("wiring_mismatch");
    expect(r.fle_reported_executor).toBe("0x4444444444444444444444444444444444444444");
  });

  it("FLE does not expose arbitrageExecutor() → wiring_mismatch (not wrong_contract)", async () => {
    const r = await probe({ ...healthy, wiring: { error: { code: 3, message: "execution reverted" } } });
    expect(r.status).toBe("wiring_mismatch");
  });

  it("missing FLASHLOAN_EXECUTOR_<id> → identity_not_configured", async () => {
    const env = { ...ENV } as NodeJS.ProcessEnv;
    delete env["FLASHLOAN_EXECUTOR_1"];
    const r = await probe({ getCode: { result: CODE }, paused: { result: word(0) } }, env);
    expect(r.status).toBe("identity_not_configured");
    expect(r.error).toContain("FLASHLOAN_EXECUTOR_1");
  });

  it("no operator identity configured → identity_not_configured (never PASS)", async () => {
    const env = { ...ENV } as NodeJS.ProcessEnv;
    delete env["SIGNER_ADDRESS"];
    const r = await probe({ ...healthy, operatorRole: undefined }, env);
    expect(r.status).toBe("identity_not_configured");
    expect(r.fle_has_executor_role).toBe(true);
    expect(r.operator_address).toBe(null);
  });

  it("SIM_SIGNER_ADDRESS alone is NOT the operator identity", async () => {
    const env = {
      EXECUTOR_1: AE,
      FLASHLOAN_EXECUTOR_1: FLE,
      SIM_SIGNER_ADDRESS: OP,
      RPC_HTTP_1: "https://eth.example.com",
    } as NodeJS.ProcessEnv;
    const r = await probe({ ...healthy, operatorRole: undefined }, env);
    expect(r.status).toBe("identity_not_configured");
    expect(r.operator_address).toBe(null);
  });

  it("paused() returns a non-boolean word → wrong_contract", async () => {
    const r = await probe({ ...healthy, paused: { result: word(2) } });
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
    const r = await probe({ getCode: "hang" }, ENV, 1, 25);
    expect(r.status).toBe("rpc_unavailable");
    expect(r.error).toContain("timeout");
  });

  it("paused() probe transport failure → rpc_unavailable (not wrong_contract)", async () => {
    const r = await probe({ ...healthy, paused: "throw" });
    expect(r.status).toBe("rpc_unavailable");
  });

  it("missing EXECUTOR_1 → not_configured", async () => {
    const r = await probeExecutorHealth({
      chainId: 1,
      env: { FLASHLOAN_EXECUTOR_1: FLE, SIGNER_ADDRESS: OP } as NodeJS.ProcessEnv,
      fetchImpl: mockFetch({}),
    });
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

  it("zero EXECUTOR_1 → invalid_address (no zero-address probe)", async () => {
    const r = await probeExecutorHealth({
      chainId: 1,
      env: {
        EXECUTOR_1: "0x0000000000000000000000000000000000000000",
        RPC_HTTP_1: "https://eth.example.com",
      } as NodeJS.ProcessEnv,
      fetchImpl: mockFetch({}),
    });
    expect(r.status).toBe("invalid_address");
  });

  it("missing RPC for the chain → not_configured", async () => {
    const r = await probeExecutorHealth({
      chainId: 1,
      env: { EXECUTOR_1: AE, FLASHLOAN_EXECUTOR_1: FLE, SIGNER_ADDRESS: OP } as NodeJS.ProcessEnv,
      fetchImpl: mockFetch({}),
    });
    expect(r.status).toBe("not_configured");
    expect(r.error).toContain("RPC_HTTP_1");
  });
});
