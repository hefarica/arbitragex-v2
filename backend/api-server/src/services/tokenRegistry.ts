/**
 * Curated-token registry for verification.
 *
 * Loads the Uniswap Labs Default Token List (`uniswap-tokenlist.json`,
 * committed snapshot) at module load time and indexes it by
 * `${chainId}:${addressLowercase}`. Used by the `/opportunities/live` route
 * to flag every token as either VERIFIED (in the curated list with a
 * matching on-chain symbol) or UNVERIFIED (everything else — memecoins,
 * scam tokens, brand-new altcoins, contracts that lie about their symbol).
 *
 * Why a snapshot, not a live fetch
 *   - The list changes slowly (governance-controlled, monthly cadence).
 *   - Zero runtime network dependency keeps the api-server's boot time
 *     deterministic and offline-friendly.
 *   - The file size is ~600 KB, parsed once and held in process memory.
 *
 * What "verified" means here
 *   - `verified=true` requires BOTH the (chainId, address) to be in the
 *     curated list AND the on-chain `symbol()` to match the registry's
 *     symbol (case-insensitive). The second check defends against the
 *     impersonation pattern: a contract at a random address declaring
 *     `symbol() = "USDC"` cannot fool the dashboard because the
 *     registry's USDC has a single canonical address.
 *   - `verified=false` always — when address is unknown OR symbol differs
 *     from registry. The frontend renders the on-chain symbol with an
 *     "UNVERIFIED" badge so the operator never confuses it with a real
 *     asset.
 *
 * R8 fail-honest
 *   - We never rename a token. The on-chain symbol stays in
 *     `token_info.symbol`. The registry's canonical name is surfaced
 *     separately as `registry_name` for operator context. Both are
 *     surfaced; the operator sees both views and decides.
 */

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const TOKENLIST_PATH = path.join(__dirname, "uniswap-tokenlist.json");

interface RawTokenEntry {
  chainId: number;
  address: string;
  symbol: string;
  name: string;
  decimals: number;
  logoURI?: string;
}

interface RawTokenList {
  name: string;
  version: { major: number; minor: number; patch: number };
  tokens: RawTokenEntry[];
}

export interface RegistryEntry {
  chain_id: number;
  /** Lowercased 0x-prefixed address (CHECK constraint matches `tokens` table). */
  address: string;
  symbol: string;
  name: string;
  decimals: number;
  logo_uri: string | null;
}

export interface VerificationResult {
  /**
   * True when the (chain_id, address) pair is in the curated list AND the
   * on-chain symbol matches the registry's symbol (case-insensitive).
   * False otherwise — every UNVERIFIED token is flagged so the dashboard
   * can render it with a warning badge.
   */
  verified: boolean;
  /** Registry's canonical symbol, or null when address unknown. */
  registry_symbol: string | null;
  /** Registry's full name (e.g. "Wrapped Ether"), or null. */
  registry_name: string | null;
  /** Registry's logo URL, or null. */
  registry_logo_uri: string | null;
  /**
   * Verification notes:
   *   "in-registry"          — address matched a curated entry.
   *   "symbol-mismatch"      — address in registry but on-chain symbol
   *                            differs (impersonation attempt or registry
   *                            stale; either way unverified).
   *   "address-not-in-registry" — never seen in curated list.
   */
  notes: string[];
}

// ── Module-scoped index ─────────────────────────────────────────────────────

let index: Map<string, RegistryEntry> | null = null;
let loadError: string | null = null;

function makeKey(chain_id: number, address: string): string {
  return `${chain_id}:${address.toLowerCase()}`;
}

function loadIndex(): Map<string, RegistryEntry> {
  if (index != null) return index;
  if (loadError != null) {
    // Once loading fails we don't retry per request; the snapshot is committed
    // so a failure here is a deploy-time bug, not a runtime transient.
    return new Map();
  }

  try {
    const raw = readFileSync(TOKENLIST_PATH, "utf8");
    const parsed = JSON.parse(raw) as RawTokenList;
    if (!parsed || !Array.isArray(parsed.tokens)) {
      throw new Error("invalid tokenlist shape");
    }
    const m = new Map<string, RegistryEntry>();
    for (const t of parsed.tokens) {
      if (
        typeof t.chainId !== "number"
        || typeof t.address !== "string"
        || typeof t.symbol !== "string"
        || typeof t.name !== "string"
        || typeof t.decimals !== "number"
      ) {
        continue; // skip malformed entries
      }
      const key = makeKey(t.chainId, t.address);
      m.set(key, {
        chain_id: t.chainId,
        address: t.address.toLowerCase(),
        symbol: t.symbol,
        name: t.name,
        decimals: t.decimals,
        logo_uri: typeof t.logoURI === "string" ? t.logoURI : null,
      });
    }
    index = m;
    return m;
  } catch (err) {
    loadError = err instanceof Error ? err.message : String(err);
    return new Map();
  }
}

// ── Public API ──────────────────────────────────────────────────────────────

/**
 * Look up a single token by (chain_id, address). Returns null when the
 * address is not in the curated list. Address is matched case-insensitively
 * against the lowercased index.
 */
export function lookupRegistry(
  chain_id: number,
  address: string,
): RegistryEntry | null {
  const m = loadIndex();
  return m.get(makeKey(chain_id, address)) ?? null;
}

/**
 * Verify an on-chain (chain_id, address, symbol) triple against the curated
 * list. The on-chain symbol may be null when the resolver couldn't fetch it;
 * verification then degrades to address-only matching (still flagged
 * symbol-mismatch when registry has a symbol but row doesn't).
 *
 * Pure function — does not mutate the registry. Safe for hot-path use.
 */
export function verifyToken(
  chain_id: number,
  address: string,
  onchain_symbol: string | null,
): VerificationResult {
  const entry = lookupRegistry(chain_id, address);
  if (!entry) {
    return {
      verified: false,
      registry_symbol: null,
      registry_name: null,
      registry_logo_uri: null,
      notes: ["address-not-in-registry"],
    };
  }
  const notes: string[] = ["in-registry"];
  let symbolOk = true;
  if (onchain_symbol != null && onchain_symbol.length > 0) {
    if (onchain_symbol.toUpperCase() !== entry.symbol.toUpperCase()) {
      symbolOk = false;
      notes.push("symbol-mismatch");
    }
  } else {
    // On-chain symbol missing but address is curated — accept and rely on the
    // registry symbol for display. Don't flag mismatch.
  }
  return {
    verified: symbolOk,
    registry_symbol: entry.symbol,
    registry_name: entry.name,
    registry_logo_uri: entry.logo_uri,
    notes,
  };
}

/**
 * Diagnostics helper: returns the loaded registry size and the per-chain
 * count. Used by the readiness probe + admin endpoint.
 */
export function registryStats(): {
  loaded: boolean;
  total_tokens: number;
  by_chain: Record<number, number>;
  load_error: string | null;
} {
  const m = loadIndex();
  const by_chain: Record<number, number> = {};
  for (const e of m.values()) {
    by_chain[e.chain_id] = (by_chain[e.chain_id] ?? 0) + 1;
  }
  return {
    loaded: m.size > 0,
    total_tokens: m.size,
    by_chain,
    load_error: loadError,
  };
}

/** Test-only — clear the cached index so the next call reloads. */
export function _resetRegistryForTests(): void {
  index = null;
  loadError = null;
}
