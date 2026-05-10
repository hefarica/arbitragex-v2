/**
 * Typed fetchers for /api/v1/dexes endpoints.
 *
 * R8 fail-honest: 404 surfaces as "endpoint not yet implemented", never
 * as fabricated data. PUT /api/v1/dexes/:id/active is not retried (mutation).
 */

export interface DexRow {
  id: string;
  name: string;
  protocol_type: string;
  chain_ids: number[];
  volume_24h_usd: number | null;
  tvl_usd: number | null;
  is_active: boolean;
  router_address: string | null;
  factory_address: string | null;
  fee_bps: number | null;
  created_at: string | null;
}

export interface DexListResponse {
  dexes: DexRow[];
}

export interface DexToggleResult {
  id: string;
  is_active: boolean;
  updated_at: string;
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

export function getDexes(
  edgeUrl: string,
  chainId?: number,
): Promise<Result<DexListResponse>> {
  const url = chainId != null
    ? `${edgeUrl}/api/v1/dexes?chain_id=${chainId}`
    : `${edgeUrl}/api/v1/dexes`;
  return safeFetch<DexListResponse>(url);
}

export async function toggleDexActive(
  edgeUrl: string,
  id: string,
  isActive: boolean,
  adminToken: string,
): Promise<Result<DexToggleResult>> {
  const url = `${edgeUrl}/api/v1/dexes/${id}/active`;
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), 5000);
  try {
    const res = await fetch(url, {
      method: "PUT",
      headers: {
        "content-type": "application/json",
        accept: "application/json",
        "x-arbx-admin-token": adminToken,
      },
      credentials: "include",
      body: JSON.stringify({ is_active: isActive }),
      signal: ctrl.signal,
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
    return { ok: true, data: parsed as DexToggleResult };
  } catch (e) {
    const err = e as Error;
    return {
      ok: false,
      error: err.name === "AbortError" ? "Timeout after 5000ms" : err.message,
    };
  } finally {
    clearTimeout(timer);
  }
}
