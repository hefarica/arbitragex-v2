/**
 * OMEGA-8 / M5 Capa 4 — Frontier Client (Fail-Honest, Zod-parsed).
 *
 * Centralises every "fetch backend → Zod parse → discriminated union" flow so
 * that pages never silently coerce a backend error into a usable shape. Each
 * call returns `FrontierResult<T>` (`lib/schemas.ts`) and the caller pattern-
 * matches on `kind`.
 *
 * Doctrine:
 *   - R8 Fail-Honest: caída ≠ tabla vacía. `unavailable`/`endpoint_not_implemented`
 *     are first-class shapes.
 *   - Zero-Mocks: we never fabricate `items: []` to make a UI render.
 *   - No `NEXT_PUBLIC_*` secrets: cookies travel with `credentials: "include"`
 *     when running in the browser; SSR uses `INTERNAL_EDGE_URL` Docker DNS.
 */

import type { z } from "zod";
import type { FrontierResult } from "@/lib/schemas";
import { getApiBaseUrl } from "@/lib/api-client";

export function resolveEdgeBase(): string {
  // SSR Docker DNS preferred; browser falls back to the public edge URL via
  // getApiBaseUrl (which carries its own R2 build-time localhost guard).
  const internal = process.env.INTERNAL_EDGE_URL;
  if (internal && internal.trim().length > 0) return internal.replace(/\/$/, "");
  return getApiBaseUrl();
}

export interface GetValidatedOptions extends RequestInit {
  /** When true, send cookies on cross-origin fetches (browser only). */
  withCredentials?: boolean;
  /** Override base URL — caller-controlled (e.g. preflight tests). */
  base?: string;
}

/**
 * Fetch + Zod-parse a JSON frontier. Returns a `FrontierResult` so the page
 * can render the correct fail-honest state instead of empty rows.
 *
 * Status mapping:
 *   200 OK + valid JSON + schema parse OK → `{kind:"ok",data}`
 *   200 OK + invalid schema               → `{kind:"invalid_response"}`
 *   401 / 403                              → `{kind:"auth_required"}`
 *   404                                    → `{kind:"endpoint_not_implemented"}`
 *   network / 5xx / parse-throw            → `{kind:"unavailable"}`
 */
export async function getValidated<T extends z.ZodTypeAny>(
  path: string,
  schema: T,
  opts: GetValidatedOptions = {},
): Promise<FrontierResult<z.infer<T>>> {
  const { withCredentials, base, ...init } = opts;
  const baseUrl = base ?? resolveEdgeBase();
  if (!baseUrl) {
    return { kind: "unavailable", detail: "no edge base URL configured" };
  }
  let res: Response;
  try {
    res = await fetch(`${baseUrl}${path}`, {
      cache: "no-store",
      headers: { accept: "application/json", ...(init.headers ?? {}) },
      credentials: withCredentials ? "include" : (init.credentials ?? "same-origin"),
      ...init,
    });
  } catch (e) {
    return { kind: "unavailable", detail: (e as Error).message };
  }
  if (res.status === 401 || res.status === 403) {
    return { kind: "auth_required", status: res.status };
  }
  if (res.status === 404) {
    return { kind: "endpoint_not_implemented", detail: `HTTP 404 ${path}` };
  }
  if (!res.ok) {
    return { kind: "unavailable", detail: `HTTP ${res.status}` };
  }
  let raw: unknown;
  try {
    raw = await res.json();
  } catch (e) {
    return { kind: "invalid_response", detail: `JSON parse failed: ${(e as Error).message}` };
  }
  const parsed = schema.safeParse(raw);
  if (!parsed.success) {
    const issues = parsed.error.issues.slice(0, 3).map((i) => `${i.path.join(".")}: ${i.message}`).join("; ");
    return { kind: "invalid_response", detail: `schema drift: ${issues}` };
  }
  return { kind: "ok", data: parsed.data };
}
