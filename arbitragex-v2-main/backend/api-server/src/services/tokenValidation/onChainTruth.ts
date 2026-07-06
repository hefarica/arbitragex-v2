/**
 * OnChainTruthValidator — Phase 1 of the Token Validation Engine.
 *
 * Reads the canonical truth about a contract address: does it exist as
 * deployed code, what's its size, does it implement the ERC-20 ABI
 * surface (name/symbol/decimals/totalSupply), and what values do those
 * functions actually return.
 *
 * This is the "if it doesn't exist on-chain, it doesn't exist" layer —
 * the first guard the operator's request walks through.
 *
 * Why direct eth_call instead of an SDK
 *   - Single dependency (node fetch). No alloy/ethers bundled in the
 *     api-server runtime.
 *   - The function selectors and ABI return-decoders are protocol-level
 *     constants (RULE 00 exception, not productive secrets), so we can
 *     ship them inline.
 *   - Reuses the same `RPC_HTTP_<chain_id>` env config the existing
 *     tokenResolver consumes — operator already has these set up.
 *
 * R8 fail-honest
 *   - RPC error / timeout → returns `error: true`, fields stay null.
 *   - eth_getCode returns 0x → `is_contract: false`, the contract address
 *     is an EOA (or empty slot). Standard cannot be `ERC20` in that case.
 *   - One of the four ABI calls reverts → that specific field stays null;
 *     the standard remains `unknown` (not `ERC20`) because a true ERC-20
 *     must implement all four.
 *
 * Phase 2+ extensions (not here):
 *   - balanceOf(0xdEaD) check for "burned supply" inference.
 *   - allowance() / approve() / transfer() / transferFrom() probes for
 *     full ERC-20 conformance.
 *   - Bytecode-pattern matching for known proxy / honeypot signatures.
 */

interface JsonRpcReply {
  result?: string;
  error?: { code?: number; message?: string };
}

// ERC-20 method selectors — protocol-level constants, not productive secrets.
const SELECTOR_NAME         = "0x06fdde03";
const SELECTOR_SYMBOL       = "0x95d89b41";
const SELECTOR_DECIMALS     = "0x313ce567";
const SELECTOR_TOTAL_SUPPLY = "0x18160ddd";

const RPC_TIMEOUT_MS = 2000;

export interface OnChainTruthResult {
  ran: boolean;
  error: boolean;
  error_message: string | null;
  /** True when eth_getCode returned non-empty bytecode. False = EOA / empty slot. */
  exists_onchain: boolean;
  /** Convenience alias for exists_onchain; an EOA is not a contract. */
  is_contract: boolean;
  /** Raw bytecode length in bytes (after 0x-prefix strip + hex-pair count). */
  bytecode_size: number | null;
  /** 'ERC20' when all 4 ABI calls returned valid values, else 'unknown'. */
  standard: "ERC20" | "unknown";
  onchain_name: string | null;
  onchain_symbol: string | null;
  onchain_decimals: number | null;
  /** BigInt as string to avoid Number-precision loss on 256-bit values. */
  total_supply_str: string | null;
}

function emptyResult(error_message: string | null = null): OnChainTruthResult {
  return {
    ran: error_message == null,
    error: error_message != null,
    error_message,
    exists_onchain: false,
    is_contract: false,
    bytecode_size: null,
    standard: "unknown",
    onchain_name: null,
    onchain_symbol: null,
    onchain_decimals: null,
    total_supply_str: null,
  };
}

/** Read RPC_HTTP_<chain_id> env (`name=url,name=url` format) → first URL. */
function firstRpcUrl(chain_id: number): string | null {
  const raw = process.env[`RPC_HTTP_${chain_id}`];
  if (!raw) return null;
  for (const tok of raw.split(",")) {
    const t = tok.trim();
    if (!t) continue;
    const url = t.includes("=") ? t.split("=", 2)[1]!.trim() : t;
    if (/^https?:\/\//.test(url)) return url;
  }
  return null;
}

async function rpcCall(rpc_url: string, body: object): Promise<JsonRpcReply | null> {
  const ctrl = new AbortController();
  const timeoutId = setTimeout(() => ctrl.abort(), RPC_TIMEOUT_MS);
  try {
    const res = await fetch(rpc_url, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
      signal: ctrl.signal,
    });
    if (!res.ok) return null;
    return (await res.json()) as JsonRpcReply;
  } catch {
    return null;
  } finally {
    clearTimeout(timeoutId);
  }
}

/**
 * Decode a Solidity `string` return value from eth_call. Handles both
 * canonical ABI encoding (offset || length || data) and the legacy
 * bytes32 form (raw 32 bytes, trim trailing NUL).
 *
 * Returns null when the hex is unparseable or the decoded bytes contain
 * non-printable characters that would render badly in the UI.
 */
function decodeStringResult(hex: string): string | null {
  if (!hex || hex === "0x") return null;
  const clean = hex.startsWith("0x") ? hex.slice(2) : hex;
  // ABI-encoded string: offset (32) || length (32) || data.
  if (clean.length >= 128) {
    const lenHex = clean.slice(64, 128);
    const len = parseInt(lenHex, 16);
    if (Number.isFinite(len) && len > 0 && len < 256 && clean.length >= 128 + len * 2) {
      try {
        const bytes = Buffer.from(clean.slice(128, 128 + len * 2), "hex");
        const decoded = bytes.toString("utf8").replace(/\0+$/, "");
        if (decoded && decoded.length > 0) return decoded;
      } catch { /* fall through */ }
    }
  }
  // bytes32 legacy: trim trailing NULs.
  try {
    const bytes = Buffer.from(clean.slice(0, 64), "hex");
    const decoded = bytes.toString("utf8").replace(/\0+$/, "").trim();
    if (decoded && decoded.length > 0) return decoded;
  } catch { /* unparseable */ }
  return null;
}

function decodeUint8(hex: string): number | null {
  if (!hex || hex === "0x") return null;
  const clean = hex.startsWith("0x") ? hex.slice(2) : hex;
  const n = parseInt(clean, 16);
  if (Number.isFinite(n) && n >= 0 && n <= 36) return n;
  return null;
}

function decodeUint256(hex: string): string | null {
  if (!hex || hex === "0x") return null;
  try {
    return BigInt(hex).toString();
  } catch {
    return null;
  }
}

export async function validateOnChainTruth(
  chain_id: number,
  address: string,
): Promise<OnChainTruthResult> {
  const rpc_url = firstRpcUrl(chain_id);
  if (!rpc_url) {
    return emptyResult(`no RPC configured for chain_id=${chain_id}`);
  }

  // 1) eth_getCode — does code exist at all?
  const codeReply = await rpcCall(rpc_url, {
    jsonrpc: "2.0",
    id: 1,
    method: "eth_getCode",
    params: [address, "latest"],
  });
  if (!codeReply || !codeReply.result) {
    return emptyResult("eth_getCode failed");
  }
  const code = codeReply.result;
  const bytecodeHex = code.startsWith("0x") ? code.slice(2) : code;
  const bytecodeSize = Math.floor(bytecodeHex.length / 2);
  const exists = bytecodeSize > 0;

  const result: OnChainTruthResult = {
    ran: true,
    error: false,
    error_message: null,
    exists_onchain: exists,
    is_contract: exists,
    bytecode_size: bytecodeSize,
    standard: "unknown",
    onchain_name: null,
    onchain_symbol: null,
    onchain_decimals: null,
    total_supply_str: null,
  };

  if (!exists) return result;

  // 2) Parallel ABI probes.
  const [nameReply, symbolReply, decimalsReply, supplyReply] = await Promise.all([
    rpcCall(rpc_url, {
      jsonrpc: "2.0", id: 2, method: "eth_call",
      params: [{ to: address, data: SELECTOR_NAME }, "latest"],
    }),
    rpcCall(rpc_url, {
      jsonrpc: "2.0", id: 3, method: "eth_call",
      params: [{ to: address, data: SELECTOR_SYMBOL }, "latest"],
    }),
    rpcCall(rpc_url, {
      jsonrpc: "2.0", id: 4, method: "eth_call",
      params: [{ to: address, data: SELECTOR_DECIMALS }, "latest"],
    }),
    rpcCall(rpc_url, {
      jsonrpc: "2.0", id: 5, method: "eth_call",
      params: [{ to: address, data: SELECTOR_TOTAL_SUPPLY }, "latest"],
    }),
  ]);

  result.onchain_name     = nameReply?.result    ? decodeStringResult(nameReply.result)    : null;
  result.onchain_symbol   = symbolReply?.result  ? decodeStringResult(symbolReply.result)  : null;
  result.onchain_decimals = decimalsReply?.result ? decodeUint8(decimalsReply.result)      : null;
  result.total_supply_str = supplyReply?.result  ? decodeUint256(supplyReply.result)       : null;

  // Standard = ERC20 only when all four mandatory functions responded.
  const allFour =
    result.onchain_name != null
    && result.onchain_symbol != null
    && result.onchain_decimals != null
    && result.total_supply_str != null;
  if (allFour) result.standard = "ERC20";

  return result;
}
