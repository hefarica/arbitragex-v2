/**
 * @vitest-environment node
 *
 * OPERATOR-IDENTITY (2026-09-02, workbook 20260902_152349Z): the registry
 * pages mount several <OperatorGate> per page and every mount used to fire
 * its own /api/operator/me — 2 guaranteed-401 requests per page for an
 * anonymous visitor (12 of the audit's 13 actionable failures). These tests
 * pin the contract of resolveOperator(), the single resolution path the
 * hook uses:
 *
 *   - anonymous  → blocked OPERATOR_UNAUTHENTICATED, ZERO fetches
 *   - signed-in  → N concurrent resolutions = ONE fetch (module dedupe)
 *   - 401/403    → honest blocked reason from the body
 *   - non-OK 5xx → unavailable HTTP_<code>
 *   - network    → unavailable with the error message
 *   - refresh    → invalidateOperatorShared() forces a fresh request
 *
 * The backend L8 gate itself is NOT under test here and stays untouched.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  invalidateOperatorShared,
  resolveOperator,
} from "./useOperator";

const mockFetch = vi.fn();

function jsonResponse(status: number, body: unknown): Response {
  return {
    status,
    ok: status >= 200 && status < 300,
    json: async () => body,
  } as unknown as Response;
}

beforeEach(() => {
  // Same shim pattern as admin-token.test.ts: node env + fake cookie store.
  // @ts-expect-error injecting document.cookie for the cookie-reading functions.
  globalThis.document = { cookie: "" };
  globalThis.fetch = mockFetch;
  mockFetch.mockReset();
  invalidateOperatorShared();
});

afterEach(() => {
  // @ts-expect-error tearing down our shim.
  delete globalThis.document;
});

describe("resolveOperator — OPERATOR-IDENTITY session gate + dedupe", () => {
  it("anonymous visitor → blocked OPERATOR_UNAUTHENTICATED with ZERO requests", async () => {
    const state = await resolveOperator();
    expect(state).toEqual({ status: "blocked", reason: "OPERATOR_UNAUTHENTICATED" });
    expect(mockFetch).not.toHaveBeenCalled();
  });

  it("expired session cookie → same honest blocked, still zero requests", async () => {
    const past = Date.now() - 1000;
    // @ts-expect-error setting cookie
    globalThis.document = { cookie: `arbx_admin_session_ttl=${past}` };
    const state = await resolveOperator();
    expect(state).toEqual({ status: "blocked", reason: "OPERATOR_UNAUTHENTICATED" });
    expect(mockFetch).not.toHaveBeenCalled();
  });

  it("signed-in: N concurrent resolutions share ONE request (page had 2+ gates)", async () => {
    const future = Date.now() + 3600_000;
    // @ts-expect-error setting cookie
    globalThis.document = { cookie: `arbx_admin_session_ttl=${future}` };
    mockFetch.mockResolvedValueOnce(
      jsonResponse(401, { status: "BLOCKED", reason: "OPERATOR_MISSING_IDENTITY", layer: "L8_AUTHZ" }),
    );
    const [a, b, c] = await Promise.all([resolveOperator(), resolveOperator(), resolveOperator()]);
    expect(mockFetch).toHaveBeenCalledTimes(1);
    // All callers observe the same honest live state.
    expect(a).toEqual({ status: "blocked", reason: "OPERATOR_MISSING_IDENTITY" });
    expect(b).toEqual(a);
    expect(c).toEqual(a);
  });

  it("ready payload passes through untouched (RULE 00 verbatim)", async () => {
    const future = Date.now() + 3600_000;
    // @ts-expect-error setting cookie
    globalThis.document = { cookie: `arbx_admin_session_ttl=${future}` };
    const me = { operator: { role: "steward" }, gates: {}, feature_manifest: [] };
    mockFetch.mockResolvedValueOnce(jsonResponse(200, me));
    const state = await resolveOperator();
    expect(state).toEqual({ status: "ready", data: me });
  });

  it("503 → unavailable HTTP_503 (honest, never fabricated ready)", async () => {
    const future = Date.now() + 3600_000;
    // @ts-expect-error setting cookie
    globalThis.document = { cookie: `arbx_admin_session_ttl=${future}` };
    mockFetch.mockResolvedValueOnce(jsonResponse(503, { ok: false }));
    const state = await resolveOperator();
    expect(state).toEqual({ status: "unavailable", error: "HTTP_503" });
  });

  it("network failure → unavailable with the error message", async () => {
    const future = Date.now() + 3600_000;
    // @ts-expect-error setting cookie
    globalThis.document = { cookie: `arbx_admin_session_ttl=${future}` };
    mockFetch.mockRejectedValueOnce(new TypeError("Failed to fetch"));
    const state = await resolveOperator();
    expect(state).toEqual({ status: "unavailable", error: "Failed to fetch" });
  });

  it("sign-out then sign-in: invalidate → fresh request; anon gate resets in between", async () => {
    const future = Date.now() + 3600_000;
    // @ts-expect-error setting cookie
    globalThis.document = { cookie: `arbx_admin_session_ttl=${future}` };
    mockFetch.mockResolvedValueOnce(jsonResponse(401, { reason: "OPERATOR_MISSING_IDENTITY" }));
    await resolveOperator();
    expect(mockFetch).toHaveBeenCalledTimes(1);

    // sign-out: anonymous resolution makes no request and clears the share
    // @ts-expect-error setting cookie
    globalThis.document = { cookie: "" };
    await resolveOperator();
    expect(mockFetch).toHaveBeenCalledTimes(1);

    // sign-in again: invalidate + resolve → exactly one new request
    // @ts-expect-error setting cookie
    globalThis.document = { cookie: `arbx_admin_session_ttl=${future}` };
    mockFetch.mockResolvedValueOnce(jsonResponse(401, { reason: "OPERATOR_MISSING_IDENTITY" }));
    const state = await resolveOperator();
    expect(mockFetch).toHaveBeenCalledTimes(2);
    expect(state).toEqual({ status: "blocked", reason: "OPERATOR_MISSING_IDENTITY" });
  });
});
