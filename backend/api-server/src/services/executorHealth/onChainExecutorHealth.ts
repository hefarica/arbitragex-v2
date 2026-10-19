/**
 * ExecutorHealthProbe — on-chain health of the deployed ArbitrageExecutor.
 *
 * Replaces the `WARN — probe pending` placeholder in the executor circuit
 * breaker (C-15a) with a real, reusable read-only probe.
 *
 * What it reads
 *   1. eth_getCode(EXECUTOR_1)                → does bytecode exist at all?
 *   2. paused()               (0x5c975abb)    → PausableUpgradeable state.
 *   3. getRoleAdmin(EXECUTOR_ROLE) (0x248a9ca3) → AccessControl wiring.
 *
 * Why getRoleAdmin and not owner()
 *   ArbitrageExecutor inherits AccessControlUpgradeable + PausableUpgradeable
 *   (contracts/src/ArbitrageExecutor.sol:117-121) — it is NOT Ownable, so
 *   owner() reverts on the real contract. Probing owner() would have made the
 *   positive control fail on a healthy executor and is therefore not used.
 *   getRoleAdmin(EXECUTOR_ROLE) proves the AccessControl surface is present;
 *   the returned admin role is surfaced as-is (DEFAULT_ADMIN_ROLE is the
 *   documented default, custom hierarchies are not called wrong).
 *
 * Why direct JSON-RPC instead of an SDK
 *   Same reason as services/tokenValidation/onChainTruth.ts: the selectors are
 *   protocol-level constants and node fetch is enough. No new dependency.
 *
 * R8 fail-honest mapping (consumed by routes/risk-circuit-breakers.ts)
 *   - transport/HTTP failure                → rpc_unavailable  → UNKNOWN
 *   - address with no bytecode (EOA/empty)  → no_code         → BLOCKED
 *   - bytecode but Pausable/AC surface gone → wrong_contract  → BLOCKED
 *   - paused() == true                      → paused          → PAUSED
 *   - bytecode + not paused + AC surface    → healthy         → PASS
 *   A failed probe NEVER yields `healthy`. A revert is deterministic evidence
 *   about the contract; a transport failure is not evidence of absence.
 *
 * Secrets: the configured RPC URL is never echoed — only its hostname — and
 * the executor address is available on the raw result for logs but MUST NOT
 * be copied into breaker evidence (the no-secrets regression test).
 */

export const PAUSED_SELECTOR = "0x5c975abb";
export const GET_ROLE_ADMIN_SELECTOR = "0x248a9ca3";
/** keccak256("EXECUTOR_ROLE") — same constant as scripts/post-deploy-sepolia.sh:103. */
export const EXECUTOR_ROLE_HASH =
  "0xd8aa0f3194971a2a116679f7c2090f6939c8d4e01a2a8d7e41d55e5351469e63";

const DEFAULT_TIMEOUT_MS = 2500;
const ADDRESS_RE = /^0x[0-9a-fA-F]{40}$/;

export type ExecutorProbeStatus =
  | "healthy"
  | "paused"
  | "no_code"
  | "wrong_contract"
  | "rpc_unavailable"
  | "invalid_address"
  | "not_configured";

export interface ExecutorHealthProbe {
  /** True once a JSON-RPC endpoint answered at least eth_getCode. */
  ran: boolean;
  status: ExecutorProbeStatus;
  chain_id: number;
  /** Raw address — for logs only, never for breaker evidence. */
  address: string | null;
  /** Hostname of the RPC endpoint (no path/query — API keys live there). */
  rpc_host: string | null;
  bytecode_size: number | null;
  paused: boolean | null;
  /** Admin role returned for EXECUTOR_ROLE, 32-byte hex; null when unread. */
  access_control_admin: string | null;
  latency_ms: number | null;
  error: string | null;
  checked_at: string;
}

type JsonRpcReply = {
  result?: string;
  error?: { code?: number; message?: string };
};

type RpcOutcome =
  | { kind: "ok"; result: string }
  | { kind: "rpc_error"; message: string; code: number | null }
  | { kind: "transport"; message: string };

/** First http(s) URL from `RPC_HTTP_<chain_id>` (`name=url,name=url` convention). */
export function firstRpcUrl(env: NodeJS.ProcessEnv, chainId: number): string | null {
  const raw = env[`RPC_HTTP_${chainId}`];
  if (!raw) return null;
  for (const tok of raw.split(",")) {
    const t = tok.trim();
    if (!t) continue;
    const url = t.includes("=") ? t.split("=", 2)[1]!.trim() : t;
    if (/^https?:\/\//.test(url)) return url;
  }
  return null;
}

export function rpcHost(url: string): string | null {
  try {
    return new URL(url).host || null;
  } catch {
    return null;
  }
}

function emptyProbe(
  status: ExecutorProbeStatus,
  chainId: number,
  error: string,
  extras: Partial<ExecutorHealthProbe> = {},
): ExecutorHealthProbe {
  return {
    ran: false,
    status,
    chain_id: chainId,
    address: null,
    rpc_host: null,
    bytecode_size: null,
    paused: null,
    access_control_admin: null,
    latency_ms: null,
    error,
    checked_at: new Date().toISOString(),
    ...extras,
  };
}

async function rpcCall(
  rpcUrl: string,
  body: object,
  fetchImpl: typeof fetch,
  timeoutMs: number,
): Promise<RpcOutcome> {
  const ctrl = new AbortController();
  let timeoutId: ReturnType<typeof setTimeout> | undefined;
  // Hard ceiling independent of the transport: a fetch implementation that
  // ignores AbortSignal must not stall the status endpoint or the periodic
  // evaluation loop. AbortSignal is still passed so real transports free the
  // socket promptly.
  const ceiling = new Promise<never>((_, reject) => {
    timeoutId = setTimeout(() => {
      ctrl.abort();
      reject(new Error("timeout"));
    }, timeoutMs);
  });
  try {
    const res = await Promise.race([
      fetchImpl(rpcUrl, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(body),
        signal: ctrl.signal,
      }),
      ceiling,
    ]);
    if (!res.ok) return { kind: "transport", message: `http_${res.status}` };
    let json: JsonRpcReply;
    try {
      json = (await res.json()) as JsonRpcReply;
    } catch {
      return { kind: "transport", message: "invalid_json" };
    }
    if (json.error) {
      return {
        kind: "rpc_error",
        message: (json.error.message ?? "rpc_error").slice(0, 120),
        code: json.error.code ?? null,
      };
    }
    if (typeof json.result !== "string") {
      return { kind: "transport", message: "missing_result" };
    }
    return { kind: "ok", result: json.result };
  } catch (e) {
    const err = e as Error;
    const msg = err.name === "AbortError" || err.message === "timeout" ? "timeout" : "network_error";
    return { kind: "transport", message: msg };
  } finally {
    if (timeoutId) clearTimeout(timeoutId);
  }
}

/** A revert is deterministic contract evidence; anything else is transport. */
function isRevert(outcome: RpcOutcome): boolean {
  if (outcome.kind !== "rpc_error") return false;
  return outcome.code === 3 || /revert/i.test(outcome.message);
}

function isHexData(hex: string): boolean {
  return /^0x[0-9a-fA-F]*$/.test(hex) && (hex.length - 2) % 2 === 0;
}

/** Decode a 32-byte ABI word into a bool (0 / 1, nothing else). */
function decodeBoolWord(hex: string): boolean | null {
  if (!isHexData(hex) || hex.length !== 66) return null;
  const v = BigInt(hex);
  if (v === 0n) return false;
  if (v === 1n) return true;
  return null;
}

export async function probeExecutorHealth(opts: {
  chainId: number;
  env?: NodeJS.ProcessEnv;
  fetchImpl?: typeof fetch;
  timeoutMs?: number;
}): Promise<ExecutorHealthProbe> {
  const env = opts.env ?? process.env;
  const fetchImpl = opts.fetchImpl ?? fetch;
  const timeoutMs = opts.timeoutMs ?? DEFAULT_TIMEOUT_MS;
  const chainId = opts.chainId;

  const address = (env["EXECUTOR_1"] ?? "").trim();
  if (address.length === 0) {
    return emptyProbe("not_configured", chainId, "EXECUTOR_1 missing");
  }
  if (!ADDRESS_RE.test(address)) {
    return emptyProbe("invalid_address", chainId, "EXECUTOR_1 is not a 20-byte hex address", {
      address,
    });
  }

  const rpcUrl = firstRpcUrl(env, chainId);
  if (!rpcUrl) {
    return emptyProbe("not_configured", chainId, `RPC_HTTP_${chainId} missing`, {
      address,
    });
  }

  const started = Date.now();
  const host = rpcHost(rpcUrl);
  const base = { address, rpc_host: host };

  // 1) Bytecode existence — the precondition for every other answer.
  const codeOutcome = await rpcCall(
    rpcUrl,
    { jsonrpc: "2.0", id: 1, method: "eth_getCode", params: [address, "latest"] },
    fetchImpl,
    timeoutMs,
  );
  if (codeOutcome.kind !== "ok" || !isHexData(codeOutcome.result)) {
    return emptyProbe(
      "rpc_unavailable",
      chainId,
      `eth_getCode failed: ${codeOutcome.kind === "ok" ? "invalid_result" : codeOutcome.message}`,
      { ...base, latency_ms: Date.now() - started },
    );
  }
  const bytecodeSize = Math.floor((codeOutcome.result.length - 2) / 2);
  if (bytecodeSize === 0) {
    return {
      ran: true,
      status: "no_code",
      chain_id: chainId,
      ...base,
      bytecode_size: 0,
      paused: null,
      access_control_admin: null,
      latency_ms: Date.now() - started,
      error: null,
      checked_at: new Date().toISOString(),
    };
  }

  // 2) Pausable + AccessControl surfaces, in parallel.
  const [pausedOutcome, roleOutcome] = await Promise.all([
    rpcCall(
      rpcUrl,
      {
        jsonrpc: "2.0",
        id: 2,
        method: "eth_call",
        params: [{ to: address, data: PAUSED_SELECTOR }, "latest"],
      },
      fetchImpl,
      timeoutMs,
    ),
    rpcCall(
      rpcUrl,
      {
        jsonrpc: "2.0",
        id: 3,
        method: "eth_call",
        params: [{ to: address, data: `${GET_ROLE_ADMIN_SELECTOR}${EXECUTOR_ROLE_HASH.slice(2)}` }, "latest"],
      },
      fetchImpl,
      timeoutMs,
    ),
  ]);

  const latency = Date.now() - started;

  if (pausedOutcome.kind !== "ok") {
    const reverted = isRevert(pausedOutcome);
    return {
      ran: true,
      status: reverted ? "wrong_contract" : "rpc_unavailable",
      chain_id: chainId,
      ...base,
      bytecode_size: bytecodeSize,
      paused: null,
      access_control_admin: null,
      latency_ms: latency,
      error: reverted
        ? "paused() reverted — contract does not expose PausableUpgradeable"
        : `paused() probe failed: ${pausedOutcome.kind === "transport" ? pausedOutcome.message : "rpc_error"}`,
      checked_at: new Date().toISOString(),
    };
  }

  const paused = decodeBoolWord(pausedOutcome.result);
  if (paused === null) {
    return {
      ran: true,
      status: "wrong_contract",
      chain_id: chainId,
      ...base,
      bytecode_size: bytecodeSize,
      paused: null,
      access_control_admin: null,
      latency_ms: latency,
      error: "paused() returned a non-boolean word",
      checked_at: new Date().toISOString(),
    };
  }

  // AccessControl surface: an execution revert means "not this contract"; a
  // transport failure means "we could not tell" and must not be called wrong.
  let accessControlAdmin: string | null = null;
  if (roleOutcome.kind === "ok" && isHexData(roleOutcome.result) && roleOutcome.result.length === 66) {
    accessControlAdmin = roleOutcome.result;
  } else if (roleOutcome.kind !== "ok" && !isRevert(roleOutcome)) {
    return {
      ran: true,
      status: "rpc_unavailable",
      chain_id: chainId,
      ...base,
      bytecode_size: bytecodeSize,
      paused,
      access_control_admin: null,
      latency_ms: latency,
      error: `getRoleAdmin() probe failed: ${roleOutcome.kind === "transport" ? roleOutcome.message : "rpc_error"}`,
      checked_at: new Date().toISOString(),
    };
  }

  const status: ExecutorProbeStatus =
    accessControlAdmin === null ? "wrong_contract" : paused ? "paused" : "healthy";

  return {
    ran: true,
    status,
    chain_id: chainId,
    ...base,
    bytecode_size: bytecodeSize,
    paused,
    access_control_admin: accessControlAdmin,
    latency_ms: latency,
    error:
      accessControlAdmin === null
        ? "getRoleAdmin(EXECUTOR_ROLE) reverted — contract does not expose AccessControl"
        : null,
    checked_at: new Date().toISOString(),
  };
}
