/**
 * Typed fetchers for /api/v1/wallets endpoints.
 *
 * R8 fail-honest: every function returns a discriminated Result union.
 * If the endpoint returns 404 the caller receives ok:false with a clear
 * "endpoint not yet implemented" message — never fabricated data.
 */

export interface WalletRow {
  address: string;
  label: string;
  role: string | null;
}

export interface WalletBalance {
  chain_id: number;
  chain_name: string | null;
  token_symbol: string;
  token_address: string | null;
  balance_raw: string;
  balance_formatted: string | null;
  balance_usd: number | null;
}

export interface WalletAllowance {
  token_address: string;
  token_symbol: string | null;
  spender_address: string;
  spender_label: string | null;
  allowance_raw: string;
  allowance_formatted: string | null;
  is_unlimited: boolean;
}

export interface WalletsListResponse {
  wallets: WalletRow[];
}

export interface WalletBalancesResponse {
  address: string;
  balances: WalletBalance[];
}

export interface WalletAllowancesResponse {
  address: string;
  allowances: WalletAllowance[];
}

type Result<T> = { ok: true; data: T } | { ok: false; error: string };

const NOT_IMPLEMENTED =
  "API endpoint not yet implemented — wire backend route to populate";

async function safeFetch<T>(
  url: string,
  timeoutMs = 5000,
): Promise<Result<T>> {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), timeoutMs);
  try {
    const res = await fetch(url, {
      headers: { accept: "application/json" },
      signal: ctrl.signal,
      cache: "no-store",
    });
    if (res.status === 404) {
      return { ok: false, error: NOT_IMPLEMENTED };
    }
    if (!res.ok) {
      return { ok: false, error: `HTTP ${res.status}` };
    }
    let parsed: unknown;
    try {
      parsed = await res.json();
    } catch {
      return { ok: false, error: "Response is not valid JSON" };
    }
    return { ok: true, data: parsed as T };
  } catch (e) {
    const err = e as Error;
    return {
      ok: false,
      error: err.name === "AbortError" ? `Timeout after ${timeoutMs}ms` : err.message,
    };
  } finally {
    clearTimeout(timer);
  }
}

export function getWallets(
  edgeUrl: string,
): Promise<Result<WalletsListResponse>> {
  return safeFetch<WalletsListResponse>(`${edgeUrl}/api/v1/wallets`);
}

export function getWalletBalances(
  edgeUrl: string,
  address: string,
): Promise<Result<WalletBalancesResponse>> {
  return safeFetch<WalletBalancesResponse>(
    `${edgeUrl}/api/v1/wallets/${address}/balances`,
  );
}

export function getWalletAllowances(
  edgeUrl: string,
  address: string,
): Promise<Result<WalletAllowancesResponse>> {
  return safeFetch<WalletAllowancesResponse>(
    `${edgeUrl}/api/v1/wallets/${address}/allowances`,
  );
}
