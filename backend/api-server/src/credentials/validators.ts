/**
 * Provider-specific credential validators.
 *
 * Each function executes a single, cheap, live test against the external
 * service and returns a structured result. R8 fail-honest: a validator MUST
 * return `status: "invalid"` on any error, never silently swallow.
 *
 * Timeouts:
 *   - HTTP probes: 6s default (3s connect + 3s body).
 *   - WS probes: 4s connect to close.
 *
 * Every probe is read-only on the remote side (no writes, no orders).
 */

import { ethers } from "ethers";
import { createHmac } from "node:crypto";
import WebSocket from "ws";
import type {
  CredentialProvider,
  CredentialTestResult,
} from "@arbx/shared";
import { getExchangeEndpoints } from "../config/exchange-endpoints";

const DEFAULT_TIMEOUT_MS = 6000;
const WS_TIMEOUT_MS = 4000;

/**
 * D. dirección EVM de protocolo — WETH canonical Ethereum mainnet address.
 * Used exclusively as a probe token for Alchemy Prices API validation.
 * Immutable: Wrapped Ether contract is non-upgradeable and the address
 * has not changed since deployment. Source: etherscan.io/token/0xc02a...
 * Long-term: move to shared token catalog (H. deuda — tracked separately).
 */
const WETH_MAINNET_ADDR = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";

type Validator = (
  scope: string,
  secret: string,
  metadata: Record<string, unknown>,
) => Promise<CredentialTestResult>;

function parseScopeChainId(scope: string): number | null {
  const m = /^chain:(\d+)$/.exec(scope);
  return m ? parseInt(m[1]!, 10) : null;
}

function nowIso(): string {
  return new Date().toISOString();
}

function ok(message: string, details?: Record<string, unknown>): CredentialTestResult {
  return { status: "valid", message, details, tested_at: nowIso() };
}

function fail(message: string, details?: Record<string, unknown>): CredentialTestResult {
  return { status: "invalid", message, details, tested_at: nowIso() };
}

async function fetchWithTimeout(
  url: string,
  init: RequestInit & { timeoutMs?: number } = {},
): Promise<Response> {
  const { timeoutMs = DEFAULT_TIMEOUT_MS, ...rest } = init;
  const ctrl = new AbortController();
  const t = setTimeout(() => ctrl.abort(), timeoutMs);
  try {
    return await fetch(url, { ...rest, signal: ctrl.signal });
  } finally {
    clearTimeout(t);
  }
}

// --- RPC HTTP ---------------------------------------------------------------
const validateRpcHttp: Validator = async (scope, secret) => {
  const expectedChain = parseScopeChainId(scope);
  if (expectedChain === null) return fail("rpc_http requires scope=chain:<id>");

  const tokens = secret.split(",").map((t) => t.trim()).filter(Boolean);
  if (tokens.length === 0) return fail("no providers in CSV");

  const results: Array<{ name: string; url: string; ok: boolean; detail: string }> = [];
  for (const tok of tokens) {
    const [name, url] = tok.includes("=") ? tok.split("=", 2) : [`provider-${results.length + 1}`, tok];
    if (!url || !/^https?:\/\//.test(url)) {
      results.push({ name: name!, url: url ?? "", ok: false, detail: "invalid url" });
      continue;
    }
    try {
      const r = await fetchWithTimeout(url, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ jsonrpc: "2.0", method: "eth_chainId", params: [], id: 1 }),
      });
      if (!r.ok) {
        results.push({ name: name!, url, ok: false, detail: `http ${r.status}` });
        continue;
      }
      const j = (await r.json()) as { result?: string; error?: { message?: string } };
      if (j.error) {
        results.push({ name: name!, url, ok: false, detail: j.error.message ?? "rpc error" });
        continue;
      }
      const observed = j.result ? parseInt(j.result, 16) : NaN;
      if (observed !== expectedChain) {
        results.push({ name: name!, url, ok: false, detail: `chain mismatch: ${observed}` });
        continue;
      }
      results.push({ name: name!, url, ok: true, detail: `chain ${observed}` });
    } catch (e) {
      results.push({ name: name!, url, ok: false, detail: (e as Error).message.slice(0, 100) });
    }
  }

  const alive = results.filter((r) => r.ok).length;
  if (alive === 0) return fail(`0/${results.length} providers alive`, { providers: results });
  return ok(`${alive}/${results.length} providers responding chain=${expectedChain}`, { providers: results });
};

// --- RPC WS -----------------------------------------------------------------
const validateRpcWs: Validator = async (scope, secret) => {
  const expectedChain = parseScopeChainId(scope);
  if (expectedChain === null) return fail("rpc_ws requires scope=chain:<id>");

  const tokens = secret.split(",").map((t) => t.trim()).filter(Boolean);
  if (tokens.length === 0) return fail("no providers in CSV");

  const results: Array<{ name: string; url: string; ok: boolean; detail: string }> = [];
  for (const tok of tokens) {
    const [name, url] = tok.includes("=") ? tok.split("=", 2) : [`provider-${results.length + 1}`, tok];
    if (!url || !/^wss?:\/\//.test(url)) {
      results.push({ name: name!, url: url ?? "", ok: false, detail: "invalid wss url" });
      continue;
    }
    const detail = await new Promise<{ ok: boolean; detail: string }>((resolve) => {
      let settled = false;
      const ws = new WebSocket(url);
      const t = setTimeout(() => {
        if (settled) return;
        settled = true;
        try { ws.terminate(); } catch { /* ignore */ }
        resolve({ ok: false, detail: "connect timeout" });
      }, WS_TIMEOUT_MS);
      ws.on("open", () => {
        if (settled) return;
        settled = true;
        clearTimeout(t);
        try { ws.close(); } catch { /* ignore */ }
        resolve({ ok: true, detail: "ws connected" });
      });
      ws.on("error", (err) => {
        if (settled) return;
        settled = true;
        clearTimeout(t);
        resolve({ ok: false, detail: err.message.slice(0, 100) });
      });
    });
    results.push({ name: name!, url, ok: detail.ok, detail: detail.detail });
  }

  const alive = results.filter((r) => r.ok).length;
  if (alive === 0) return fail(`0/${results.length} ws providers alive`, { providers: results });
  return ok(`${alive}/${results.length} ws providers reachable`, { providers: results });
};

// --- Coingecko Demo ---------------------------------------------------------
const validateCoingeckoDemo: Validator = async () => {
  try {
    // B. URL operativa — resolved from ENDPOINT_COINGECKO_FREE env var.
    const r = await fetchWithTimeout(`${getExchangeEndpoints().coingeckoFree}/api/v3/ping`);
    if (!r.ok) return fail(`http ${r.status}`);
    const j = (await r.json()) as { gecko_says?: string };
    if (j.gecko_says) return ok("coingecko free-tier reachable", { gecko_says: j.gecko_says });
    return fail("unexpected /ping body");
  } catch (e) {
    return fail((e as Error).message.slice(0, 200));
  }
};

// --- Coingecko Pro ----------------------------------------------------------
const validateCoingeckoPro: Validator = async (_scope, secret) => {
  try {
    // B. URL operativa — resolved from ENDPOINT_COINGECKO_PRO env var.
    const r = await fetchWithTimeout(
      `${getExchangeEndpoints().coingeckoPro}/api/v3/ping`,
      { headers: { "x-cg-pro-api-key": secret } },
    );
    if (!r.ok) return fail(`http ${r.status}`);
    const j = (await r.json()) as { gecko_says?: string };
    if (j.gecko_says) return ok("coingecko pro reachable with key");
    return fail("unexpected /ping body");
  } catch (e) {
    return fail((e as Error).message.slice(0, 200));
  }
};

// --- Alchemy Prices ---------------------------------------------------------
const validateAlchemyPrices: Validator = async (_scope, secret) => {
  const key = secret.includes("/v2/")
    ? secret.split("/v2/")[1]!.split("/")[0]!
    : secret;
  try {
    // B. URL operativa — resolved from ENDPOINT_ALCHEMY_PRICES env var.
    // D. EVM address: WETH canonical mainnet address used as probe token; immutable.
    const r = await fetchWithTimeout(
      `${getExchangeEndpoints().alchemyPrices}/prices/v1/${key}/tokens/by-address`,
      {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          addresses: [{ network: "eth-mainnet", address: WETH_MAINNET_ADDR }],
        }),
      },
    );
    if (r.status === 429) return fail("quota exceeded (429)");
    if (!r.ok) return fail(`http ${r.status}`);
    const j = (await r.json()) as { data?: Array<{ prices?: Array<{ value?: string }> }> };
    const price = j.data?.[0]?.prices?.[0]?.value;
    if (!price) return fail("no WETH price in response");
    return ok(`alchemy prices WETH=$${price}`);
  } catch (e) {
    return fail((e as Error).message.slice(0, 200));
  }
};

// --- Flashbots signer -------------------------------------------------------
const validateFlashbotsSigner: Validator = async (_scope, secret) => {
  try {
    const clean = secret.startsWith("0x") ? secret : `0x${secret}`;
    if (!/^0x[0-9a-fA-F]{64}$/.test(clean)) {
      return fail("not a 32-byte hex (expected 0x + 64 hex chars)");
    }
    const wallet = new ethers.Wallet(clean);
    return ok(`signer address ${wallet.address}`, { address: wallet.address });
  } catch (e) {
    return fail((e as Error).message.slice(0, 200));
  }
};

// --- BloxRoute --------------------------------------------------------------
const validateBloxroute: Validator = async (_scope, secret) => {
  try {
    // B. URL operativa — resolved from ENDPOINT_BLOXROUTE env var.
    const r = await fetchWithTimeout(`${getExchangeEndpoints().bloxroute}/ping`, {
      headers: { authorization: secret },
    });
    if (r.status === 401 || r.status === 403) return fail(`auth rejected (${r.status})`);
    if (r.status >= 200 && r.status < 500) return ok(`bloxroute responded ${r.status}`);
    return fail(`http ${r.status}`);
  } catch (e) {
    return fail((e as Error).message.slice(0, 200));
  }
};

// --- Titan Builder ----------------------------------------------------------
const validateTitan: Validator = async (_scope, secret, metadata) => {
  const url = typeof metadata["url"] === "string" ? (metadata["url"] as string) : "";
  if (!url) return fail("metadata.url is required");
  try {
    const r = await fetchWithTimeout(url, {
      method: "POST",
      headers: { "content-type": "application/json", authorization: secret },
      body: JSON.stringify({ jsonrpc: "2.0", method: "eth_blockNumber", params: [], id: 1 }),
    });
    if (r.status === 401 || r.status === 403) return fail(`auth rejected (${r.status})`);
    if (!r.ok) return fail(`http ${r.status}`);
    const j = (await r.json()) as { result?: string; error?: unknown };
    if (j.error) return fail("rpc error", { error: j.error });
    return ok(`titan responded with block ${j.result}`);
  } catch (e) {
    return fail((e as Error).message.slice(0, 200));
  }
};

// --- Binance ----------------------------------------------------------------
const validateBinance: Validator = async (_scope, secret, metadata) => {
  const apiKey = typeof metadata["api_key"] === "string" ? (metadata["api_key"] as string) : "";
  if (!apiKey) return fail("metadata.api_key is required");
  try {
    const ts = Date.now();
    const query = `timestamp=${ts}&recvWindow=5000`;
    const sig = createHmac("sha256", secret).update(query).digest("hex");
    // B. URL operativa — resolved from ENDPOINT_BINANCE env var.
    const r = await fetchWithTimeout(`${getExchangeEndpoints().binance}/api/v3/account?${query}&signature=${sig}`, {
      headers: { "x-mbx-apikey": apiKey },
    });
    if (r.status === 401 || r.status === 403) return fail(`auth rejected (${r.status})`);
    if (!r.ok) {
      const body = await r.text();
      return fail(`http ${r.status}: ${body.slice(0, 100)}`);
    }
    const j = (await r.json()) as { canTrade?: boolean };
    if (j.canTrade !== undefined) return ok(`binance account read OK (canTrade=${j.canTrade})`);
    return fail("unexpected account response");
  } catch (e) {
    return fail((e as Error).message.slice(0, 200));
  }
};

// --- OKX --------------------------------------------------------------------
const validateOkx: Validator = async (_scope, secret, metadata) => {
  const apiKey = typeof metadata["api_key"] === "string" ? (metadata["api_key"] as string) : "";
  const passphrase = typeof metadata["passphrase"] === "string" ? (metadata["passphrase"] as string) : "";
  if (!apiKey || !passphrase) return fail("metadata.api_key + metadata.passphrase required");
  try {
    const ts = new Date().toISOString();
    const path = "/api/v5/account/balance";
    const presign = ts + "GET" + path;
    const sig = createHmac("sha256", secret).update(presign).digest("base64");
    // B. URL operativa — resolved from ENDPOINT_OKX env var.
    const r = await fetchWithTimeout(`${getExchangeEndpoints().okx}${path}`, {
      headers: {
        "OK-ACCESS-KEY": apiKey,
        "OK-ACCESS-SIGN": sig,
        "OK-ACCESS-TIMESTAMP": ts,
        "OK-ACCESS-PASSPHRASE": passphrase,
      },
    });
    if (r.status === 401) return fail("auth rejected (401)");
    const j = (await r.json()) as { code?: string; msg?: string };
    if (j.code === "0") return ok("okx account read OK");
    return fail(`code ${j.code}: ${j.msg ?? ""}`);
  } catch (e) {
    return fail((e as Error).message.slice(0, 200));
  }
};

// --- Bybit ------------------------------------------------------------------
const validateBybit: Validator = async (_scope, secret, metadata) => {
  const apiKey = typeof metadata["api_key"] === "string" ? (metadata["api_key"] as string) : "";
  if (!apiKey) return fail("metadata.api_key required");
  try {
    const ts = Date.now().toString();
    const recvWindow = "5000";
    const queryString = "accountType=UNIFIED";
    const presign = ts + apiKey + recvWindow + queryString;
    const sig = createHmac("sha256", secret).update(presign).digest("hex");
    // B. URL operativa — resolved from ENDPOINT_BYBIT env var.
    const r = await fetchWithTimeout(
      `${getExchangeEndpoints().bybit}/v5/account/wallet-balance?${queryString}`,
      {
        headers: {
          "X-BAPI-API-KEY": apiKey,
          "X-BAPI-SIGN": sig,
          "X-BAPI-TIMESTAMP": ts,
          "X-BAPI-RECV-WINDOW": recvWindow,
        },
      },
    );
    if (!r.ok) return fail(`http ${r.status}`);
    const j = (await r.json()) as { retCode?: number; retMsg?: string };
    if (j.retCode === 0) return ok("bybit account read OK");
    return fail(`retCode ${j.retCode}: ${j.retMsg ?? ""}`);
  } catch (e) {
    return fail((e as Error).message.slice(0, 200));
  }
};

// --- GitHub token -----------------------------------------------------------
const validateGithubToken: Validator = async (_scope, secret) => {
  try {
    const r = await fetchWithTimeout("https://api.github.com/user", {
      headers: { authorization: `token ${secret}`, accept: "application/vnd.github+json" },
    });
    if (r.status === 401) return fail("token rejected (401)");
    if (!r.ok) return fail(`http ${r.status}`);
    const j = (await r.json()) as { login?: string };
    if (j.login) return ok(`github user @${j.login}`);
    return fail("unexpected /user body");
  } catch (e) {
    return fail((e as Error).message.slice(0, 200));
  }
};

// --- Internal tokens (shape + entropy heuristic) ---------------------------
const PLACEHOLDERS = new Set([
  "change-me", "changeme", "default", "admin", "demo", "test123",
  "your-token-here", "todo", "tbd", "hfrc2026",
]);

function validateInternalToken(secret: string): CredentialTestResult {
  if (secret.length < 32) return fail("token too short (need >=32 chars)");
  if (PLACEHOLDERS.has(secret.toLowerCase())) return fail("placeholder token rejected");
  const distinct = new Set(secret).size;
  if (distinct < 12) return fail(`low entropy (${distinct} distinct chars; need >=12)`);
  return ok(`token shape OK (${secret.length} chars, ${distinct} distinct)`);
}

const validateAdminToken: Validator = async (_s, secret) => validateInternalToken(secret);
const validateEdgeToken: Validator = async (_s, secret) => validateInternalToken(secret);

// --- Dispatcher -------------------------------------------------------------
const VALIDATORS: Record<CredentialProvider, Validator> = {
  rpc_http: validateRpcHttp,
  rpc_ws: validateRpcWs,
  coingecko_demo: validateCoingeckoDemo,
  coingecko_pro: validateCoingeckoPro,
  alchemy_prices: validateAlchemyPrices,
  flashbots_signer: validateFlashbotsSigner,
  bloxroute: validateBloxroute,
  titan: validateTitan,
  binance: validateBinance,
  okx: validateOkx,
  bybit: validateBybit,
  github_token: validateGithubToken,
  admin_token: validateAdminToken,
  edge_token: validateEdgeToken,
};

export async function runValidator(
  provider: CredentialProvider,
  scope: string,
  secret: string,
  metadata: Record<string, unknown>,
): Promise<CredentialTestResult> {
  const fn = VALIDATORS[provider];
  if (!fn) return fail(`unknown provider: ${provider}`);
  try {
    return await fn(scope, secret, metadata);
  } catch (e) {
    return fail(`validator threw: ${(e as Error).message.slice(0, 200)}`);
  }
}
