/**
 * ExecutorHealthProbe — on-chain health of the deployed ArbitrageExecutor.
 *
 * Replaces the `WARN — probe pending` placeholder in the executor circuit
 * breaker (C-15a) with a real, reusable read-only probe.
 *
 * What it reads
 *   1. eth_getCode(EXECUTOR_<chain>)            → does bytecode exist at all?
 *   2. paused()                 (0x5c975abb)    → PausableUpgradeable state.
 *   3. hasRole(EXECUTOR_ROLE, FLE) (0x91d14854) → is the FlashLoanExecutor proxy
 *      actually armed on the ArbitrageExecutor? This is the gate that decides
 *      whether `onlyExecutor` lets the live path through.
 *   4. arbitrageExecutor()      (0xb7525248) on the FLE → does the configured
 *      FLE point at THIS ArbitrageExecutor? A role granted on the wrong AE is
 *      worth nothing.
 *   5. hasRole(EXECUTOR_ROLE, operator) → second documented grantee. The
 *      operator EOA has no live env carrying its address, so when neither
 *      SIGNER_ADDRESS nor EXECUTION_SIGNER_ADDRESS is set this answer is
 *      unknowable and the probe caps at `identity_not_configured`.
 *
 * Why hasRole and not getRoleAdmin or owner()
 *   `getRoleAdmin(EXECUTOR_ROLE)` answers "who may grant the role", not "who
 *   holds it" — it made the probe PASS on an executor that was not armed.
 *   The contract gates execution with `hasRole(EXECUTOR_ROLE, msg.sender)`
 *   (contracts/src/ArbitrageExecutor.sol:212-214; `onlyExecutor` at :260 and
 *   :313), so that is what is probed. `owner()` is not used at all:
 *   ArbitrageExecutor inherits AccessControlUpgradeable + PausableUpgradeable
 *   (:117-121) and is NOT Ownable, so owner() reverts on the real contract.
 *
 * Why direct JSON-RPC instead of an SDK
 *   Same reason as services/tokenValidation/onChainTruth.ts: the selectors are
 *   protocol-level constants and node fetch is enough. No new dependency.
 *
 * R8 fail-honest mapping (consumed by routes/risk-circuit-breakers.ts)
 *   - transport/HTTP failure                   → rpc_unavailable         → UNKNOWN
 *   - address with no bytecode (EOA/empty)     → no_code                 → BLOCKED
 *   - bytecode but Pausable/AC surface gone    → wrong_contract          → BLOCKED
 *   - paused() == true                         → paused                  → PAUSED
 *   - FLE holds no EXECUTOR_ROLE               → not_armed               → BLOCKED
 *   - operator holds no EXECUTOR_ROLE          → operator_not_armed      → BLOCKED
 *   - FLE points at another AE                 → wiring_mismatch         → BLOCKED
 *   - FLE address or operator address unknown  → identity_not_configured → UNKNOWN
 *   - bytecode + not paused + both roles held  → healthy                 → PASS
 *   A failed probe NEVER yields `healthy`. A revert is deterministic evidence
 *   about the contract; a transport failure is not evidence of absence, and an
 *   unidentified grantee is not a role holder.
 *
 * Secrets: the configured RPC URL is never echoed — only its hostname — and
 * the executor address is available on the raw result for logs but MUST NOT
 * be copied into breaker evidence (the no-secrets regression test).
 */

export const PAUSED_SELECTOR = "0x5c975abb";
/** hasRole(bytes32,address) — the gate `onlyExecutor` applies to msg.sender. */
export const HAS_ROLE_SELECTOR = "0x91d14854";
/** arbitrageExecutor() — the FLE's pointer back to the ArbitrageExecutor. */
export const ARBITRAGE_EXECUTOR_SELECTOR = "0xb7525248";
/** keccak256("EXECUTOR_ROLE") — same constant as scripts/post-deploy-sepolia.sh:103. */
export const EXECUTOR_ROLE_HASH =
  "0xd8aa0f3194971a2a116679f7c2090f6939c8d4e01a2a8d7e41d55e5351469e63";

const DEFAULT_TIMEOUT_MS = 2500;
const ADDRESS_RE = /^0x[0-9a-fA-F]{40}$/;
const ZERO_ADDRESS = "0x0000000000000000000000000000000000000000";

export type ExecutorProbeStatus =
  | "healthy"
  | "paused"
  | "no_code"
  | "wrong_contract"
  | "rpc_unavailable"
  | "invalid_address"
  | "not_configured"
  | "identity_not_configured"
  | "not_armed"
  | "operator_not_armed"
  | "wiring_mismatch";

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
  /** Configured FlashLoanExecutor proxy (FLASHLOAN_EXECUTOR_<chain>). */
  fle_address: string | null;
  /** hasRole(EXECUTOR_ROLE, fle_address) on the AE; null when unread. */
  fle_has_executor_role: boolean | null;
  /** Configured operator EOA (SIGNER_ADDRESS | EXECUTION_SIGNER_ADDRESS). */
  operator_address: string | null;
  /** hasRole(EXECUTOR_ROLE, operator_address) on the AE; null when unknown. */
  operator_has_executor_role: boolean | null;
  /** What the FLE's arbitrageExecutor() returned; null when unread. */
  fle_reported_executor: string | null;
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
    fle_address: null,
    fle_has_executor_role: null,
    operator_address: null,
    operator_has_executor_role: null,
    fle_reported_executor: null,
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

/** Decode a 32-byte ABI word into a 20-byte address (zero → null). */
function decodeAddressWord(hex: string): string | null {
  if (!isHexData(hex) || hex.length !== 66) return null;
  const addr = `0x${hex.slice(26).toLowerCase()}`;
  return addr === ZERO_ADDRESS ? null : addr;
}

/** Left-pad an address to a 32-byte ABI word (without the 0x prefix). */
function addressWord(address: string): string {
  return address.slice(2).toLowerCase().padStart(64, "0");
}

/** A configured address, or null when unset/malformed/zero (fail-closed). */
function readAddress(value: string | undefined): string | null {
  const raw = (value ?? "").trim();
  if (!ADDRESS_RE.test(raw)) return null;
  const addr = raw.toLowerCase();
  return addr === ZERO_ADDRESS ? null : addr;
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

  const aeKey = `EXECUTOR_${chainId}`;
  const address = (env[aeKey] ?? "").trim();
  if (address.length === 0) {
    return emptyProbe("not_configured", chainId, `${aeKey} missing`);
  }
  if (!ADDRESS_RE.test(address) || address.toLowerCase() === ZERO_ADDRESS) {
    return emptyProbe("invalid_address", chainId, `${aeKey} is not a 20-byte non-zero hex address`, {
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
      fle_address: null,
      fle_has_executor_role: null,
      operator_address: null,
      operator_has_executor_role: null,
      fle_reported_executor: null,
      latency_ms: Date.now() - started,
      error: null,
      checked_at: new Date().toISOString(),
    };
  }

  // 2) Pausable surface — a revert means this is not the executor contract.
  const pausedOutcome = await rpcCall(
    rpcUrl,
    {
      jsonrpc: "2.0",
      id: 2,
      method: "eth_call",
      params: [{ to: address, data: PAUSED_SELECTOR }, "latest"],
    },
    fetchImpl,
    timeoutMs,
  );
  const latencyAfterPaused = Date.now() - started;
  if (pausedOutcome.kind !== "ok") {
    const reverted = isRevert(pausedOutcome);
    return {
      ran: true,
      status: reverted ? "wrong_contract" : "rpc_unavailable",
      chain_id: chainId,
      ...base,
      bytecode_size: bytecodeSize,
      paused: null,
      fle_address: null,
      fle_has_executor_role: null,
      operator_address: null,
      operator_has_executor_role: null,
      fle_reported_executor: null,
      latency_ms: latencyAfterPaused,
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
      fle_address: null,
      fle_has_executor_role: null,
      operator_address: null,
      operator_has_executor_role: null,
      fle_reported_executor: null,
      latency_ms: latencyAfterPaused,
      error: "paused() returned a non-boolean word",
      checked_at: new Date().toISOString(),
    };
  }

  // 3) Identities. The FLE proxy is the caller `onlyExecutor` sees on the live
  // path; the operator EOA is the second documented grantee. A missing FLE
  // leaves the gate unanswerable: UNKNOWN, never PASS.
  const fleKey = `FLASHLOAN_EXECUTOR_${chainId}`;
  const fle = readAddress(env[fleKey]);
  const operatorRaw = (env["SIGNER_ADDRESS"] ?? "").trim().length
    ? (env["SIGNER_ADDRESS"] ?? "").trim()
    : (env["EXECUTION_SIGNER_ADDRESS"] ?? "").trim();
  const operatorKey = (env["SIGNER_ADDRESS"] ?? "").trim().length ? "SIGNER_ADDRESS" : "EXECUTION_SIGNER_ADDRESS";
  const operator = readAddress(operatorRaw);

  const identityFields = {
    paused,
    fle_address: fle,
    fle_has_executor_role: null as boolean | null,
    operator_address: operator,
    operator_has_executor_role: null as boolean | null,
    fle_reported_executor: null as string | null,
  };

  if (fle === null) {
    const reason = (env[fleKey] ?? "").trim().length
      ? `${fleKey} is not a 20-byte non-zero hex address`
      : `${fleKey} missing`;
    return {
      ran: true,
      status: "identity_not_configured",
      chain_id: chainId,
      ...base,
      ...identityFields,
      bytecode_size: bytecodeSize,
      latency_ms: Date.now() - started,
      error: `${reason} — cannot prove the executor is armed without the FLE identity`,
      checked_at: new Date().toISOString(),
    };
  }

  const roleCall = (grantee: string): Promise<RpcOutcome> =>
    rpcCall(
      rpcUrl,
      {
        jsonrpc: "2.0",
        id: 3,
        method: "eth_call",
        params: [
          {
            to: address,
            data: `${HAS_ROLE_SELECTOR}${EXECUTOR_ROLE_HASH.slice(2)}${addressWord(grantee)}`,
          },
          "latest",
        ],
      },
      fetchImpl,
      timeoutMs,
    );

  // 4) Roles + wiring, in parallel.
  const [fleRoleOutcome, wiringOutcome, operatorRoleOutcome] = await Promise.all([
    roleCall(fle),
    rpcCall(
      rpcUrl,
      {
        jsonrpc: "2.0",
        id: 4,
        method: "eth_call",
        params: [{ to: fle, data: ARBITRAGE_EXECUTOR_SELECTOR }, "latest"],
      },
      fetchImpl,
      timeoutMs,
    ),
    operator ? roleCall(operator) : Promise.resolve<RpcOutcome | null>(null),
  ]);
  const latency = Date.now() - started;

  if (fleRoleOutcome.kind !== "ok") {
    const reverted = isRevert(fleRoleOutcome);
    return {
      ran: true,
      status: reverted ? "wrong_contract" : "rpc_unavailable",
      chain_id: chainId,
      ...base,
      ...identityFields,
      bytecode_size: bytecodeSize,
      latency_ms: latency,
      error: reverted
        ? "hasRole(EXECUTOR_ROLE, FLE) reverted — contract does not expose AccessControl"
        : `hasRole(EXECUTOR_ROLE, FLE) probe failed: ${fleRoleOutcome.kind === "transport" ? fleRoleOutcome.message : "rpc_error"}`,
      checked_at: new Date().toISOString(),
    };
  }
  const fleHasRole = decodeBoolWord(fleRoleOutcome.result);
  if (fleHasRole === null) {
    return {
      ran: true,
      status: "wrong_contract",
      chain_id: chainId,
      ...base,
      ...identityFields,
      bytecode_size: bytecodeSize,
      latency_ms: latency,
      error: "hasRole(EXECUTOR_ROLE, FLE) returned a non-boolean word",
      checked_at: new Date().toISOString(),
    };
  }

  // Wiring: the FLE must point back at THIS ArbitrageExecutor. A role granted
  // on an AE the FLE does not call is worth nothing.
  if (wiringOutcome.kind !== "ok") {
    const reverted = isRevert(wiringOutcome);
    return {
      ran: true,
      status: reverted ? "wiring_mismatch" : "rpc_unavailable",
      chain_id: chainId,
      ...base,
      ...identityFields,
      fle_has_executor_role: fleHasRole,
      bytecode_size: bytecodeSize,
      latency_ms: latency,
      error: reverted
        ? `arbitrageExecutor() reverted on ${fleKey} — not a FlashLoanExecutor proxy`
        : `arbitrageExecutor() probe failed: ${wiringOutcome.kind === "transport" ? wiringOutcome.message : "rpc_error"}`,
      checked_at: new Date().toISOString(),
    };
  }
  const fleReported = decodeAddressWord(wiringOutcome.result);
  const wiringOk = fleReported !== null && fleReported === address.toLowerCase();

  // Operator role is the second requirement; without a configured identity the
  // honest answer is "unknown", never PASS.
  let operatorHasRole: boolean | null = null;
  if (operator && operatorRoleOutcome) {
    if (operatorRoleOutcome.kind !== "ok") {
      return {
        ran: true,
        status: "rpc_unavailable",
        chain_id: chainId,
        ...base,
        ...identityFields,
        fle_has_executor_role: fleHasRole,
        fle_reported_executor: fleReported,
        bytecode_size: bytecodeSize,
        latency_ms: latency,
        error: `hasRole(EXECUTOR_ROLE, operator) probe failed: ${operatorRoleOutcome.kind === "transport" ? operatorRoleOutcome.message : "rpc_error"}`,
        checked_at: new Date().toISOString(),
      };
    }
    operatorHasRole = decodeBoolWord(operatorRoleOutcome.result);
    if (operatorHasRole === null) {
      return {
        ran: true,
        status: "rpc_unavailable",
        chain_id: chainId,
        ...base,
        ...identityFields,
        fle_has_executor_role: fleHasRole,
        fle_reported_executor: fleReported,
        bytecode_size: bytecodeSize,
        latency_ms: latency,
        error: "hasRole(EXECUTOR_ROLE, operator) returned a non-boolean word",
        checked_at: new Date().toISOString(),
      };
    }
  }

  const status: ExecutorProbeStatus = paused
    ? "paused"
    : !wiringOk
      ? "wiring_mismatch"
      : !fleHasRole
        ? "not_armed"
        : operator === null
          ? "identity_not_configured"
          : operatorHasRole === false
            ? "operator_not_armed"
            : "healthy";

  const error =
    status === "wiring_mismatch"
      ? `executor_wiring_mismatch: ${fleKey}.arbitrageExecutor()=${
          fleReported ?? "0x0"
        } != ${aeKey}=${address.toLowerCase()}`
      : status === "not_armed"
        ? `missing_onchain_role:flash_executor_to_arbitrage_executor:contract=${address.toLowerCase()}:grantee=${fle}`
        : status === "identity_not_configured"
          ? `${operatorKey} missing — the operator grant cannot be proven`
          : status === "operator_not_armed"
            ? `missing_onchain_role:caller_to_flash_executor:contract=${address.toLowerCase()}:grantee=${operator}`
            : null;

  return {
    ran: true,
    status,
    chain_id: chainId,
    ...base,
    ...identityFields,
    fle_has_executor_role: fleHasRole,
    operator_has_executor_role: operatorHasRole,
    fle_reported_executor: fleReported,
    bytecode_size: bytecodeSize,
    latency_ms: latency,
    error,
    checked_at: new Date().toISOString(),
  };
}
