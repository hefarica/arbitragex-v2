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
  return mutateJson<DexToggleResult>(
    `${edgeUrl}/api/v1/dexes/${id}/active`,
    "PUT",
    { is_active: isActive },
    adminToken,
  );
}

// 2026-05-10 audit follow-up: registry CRUD for the /dex-registry page.

export interface CreateDexFactory {
  chain_id: number;
  address: string;
}

export interface CreateDexBody {
  name: string;
  protocol_type: string;
  factories: CreateDexFactory[];
}

/** POST /api/v1/dexes — creates a DEX row with per-chain factories. */
export function createDex(
  edgeUrl: string,
  body: CreateDexBody,
  adminToken: string,
): Promise<Result<DexRow>> {
  return mutateJson<DexRow>(
    `${edgeUrl}/api/v1/dexes`,
    "POST",
    body,
    adminToken,
  );
}

/** DELETE /api/v1/dexes/:id — hard delete (refuses if pools reference it). */
export function deleteDex(
  edgeUrl: string,
  id: string,
  adminToken: string,
): Promise<Result<{ id: string; deleted: boolean }>> {
  return mutateJson<{ id: string; deleted: boolean }>(
    `${edgeUrl}/api/v1/dexes/${id}`,
    "DELETE",
    null,
    adminToken,
  );
}

/**
 * Helper for JSON-body mutations. Centralises:
 *   - 5s timeout
 *   - admin token header
 *   - 404 → "endpoint not yet implemented" surfacing
 *   - server-side error messages preserved when present in the response body
 *     (PG 23505 conflict, dex_has_pools, etc.) so the UI can display the
 *     actionable reason from the API rather than a bare "HTTP 409".
 */
async function mutateJson<T>(
  url: string,
  method: "POST" | "PUT" | "DELETE",
  body: unknown,
  adminToken: string,
): Promise<Result<T>> {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), 5000);
  try {
    const res = await fetch(url, {
      method,
      headers: {
        "content-type": "application/json",
        accept: "application/json",
        "x-arbx-admin-token": adminToken,
      },
      credentials: "include",
      body: body == null ? undefined : JSON.stringify(body),
      signal: ctrl.signal,
    });
    if (res.status === 404) {
      return { ok: false, error: NOT_IMPLEMENTED };
    }
    let parsed: unknown;
    try {
      parsed = await res.json();
    } catch {
      return { ok: false, error: res.ok ? "Response is not valid JSON" : `HTTP ${res.status}` };
    }
    if (!res.ok) {
      const obj = parsed as { error?: unknown; detail?: unknown };
      const errMsg = typeof obj.error === "string" ? obj.error : `HTTP ${res.status}`;
      const detail = typeof obj.detail === "string" ? `: ${obj.detail}` : "";
      return { ok: false, error: `${errMsg}${detail}` };
    }
    return { ok: true, data: parsed as T };
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
