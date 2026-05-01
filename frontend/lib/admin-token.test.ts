/**
 * @vitest-environment node
 *
 * V-AT-1: admin-token now uses httpOnly cookies via edge endpoints.
 * These tests cover the cookie-reading utilities (getAdminToken,
 * adminTokenExpiresInMs, fmtRemaining) and verify that setAdminToken
 * and clearAdminToken call the correct edge endpoints.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  adminTokenExpiresInMs,
  clearAdminToken,
  DEFAULT_TTL_MS,
  fmtRemaining,
  getAdminToken,
  hasAdminSession,
  setAdminToken,
} from "./admin-token";

// Mock fetch for setAdminToken / clearAdminToken edge calls.
const mockFetch = vi.fn();

beforeEach(() => {
  // @ts-expect-error injecting document.cookie for the cookie-reading functions.
  globalThis.document = { cookie: "" };
  globalThis.fetch = mockFetch;
  mockFetch.mockReset();
});

afterEach(() => {
  // @ts-expect-error tearing down our shim.
  delete globalThis.document;
  vi.useRealTimers();
});

describe("admin-token cookie session", () => {
  it("getAdminToken returns empty when no session cookie exists", () => {
    expect(getAdminToken()).toBe("");
    expect(hasAdminSession()).toBe(false);
  });

  it("getAdminToken returns sentinel when session TTL cookie is valid", () => {
    const future = Date.now() + 3600_000; // 1h from now
    // @ts-expect-error setting cookie
    globalThis.document = { cookie: `arbx_admin_session_ttl=${future}` };
    expect(getAdminToken()).toBe("__session_active__");
    expect(hasAdminSession()).toBe(true);
  });

  it("getAdminToken returns empty when session TTL cookie is expired", () => {
    const past = Date.now() - 1000;
    // @ts-expect-error setting cookie
    globalThis.document = { cookie: `arbx_admin_session_ttl=${past}` };
    expect(getAdminToken()).toBe("");
    expect(hasAdminSession()).toBe(false);
  });

  it("adminTokenExpiresInMs returns remaining time from TTL cookie", () => {
    const future = Date.now() + 3600_000;
    // @ts-expect-error setting cookie
    globalThis.document = { cookie: `arbx_admin_session_ttl=${future}` };
    const remaining = adminTokenExpiresInMs();
    expect(remaining).toBeGreaterThan(3599_000);
    expect(remaining).toBeLessThanOrEqual(3600_000);
  });

  it("adminTokenExpiresInMs returns 0 when no cookie", () => {
    expect(adminTokenExpiresInMs()).toBe(0);
  });

  it("setAdminToken calls edge POST /admin/session", async () => {
    mockFetch.mockResolvedValueOnce({ ok: true, json: () => ({ ok: true }) });
    const result = await setAdminToken("my-secret-token");
    expect(result).toBe(true);
    expect(mockFetch).toHaveBeenCalledTimes(1);
    const [url, opts] = mockFetch.mock.calls[0]!;
    expect(url).toContain("/admin/session");
    expect(opts.method).toBe("POST");
    expect(opts.credentials).toBe("include");
    expect(JSON.parse(opts.body as string)).toEqual({ token: "my-secret-token" });
  });

  it("setAdminToken returns false on failure", async () => {
    mockFetch.mockResolvedValueOnce({ ok: false, status: 401 });
    const result = await setAdminToken("bad-token");
    expect(result).toBe(false);
  });

  it("setAdminToken returns false on network error", async () => {
    mockFetch.mockRejectedValueOnce(new Error("network fail"));
    const result = await setAdminToken("any-token");
    expect(result).toBe(false);
  });

  it("clearAdminToken calls edge POST /admin/session/logout", async () => {
    mockFetch.mockResolvedValueOnce({ ok: true });
    await clearAdminToken();
    expect(mockFetch).toHaveBeenCalledTimes(1);
    const [url, opts] = mockFetch.mock.calls[0]!;
    expect(url).toContain("/admin/session/logout");
    expect(opts.method).toBe("POST");
    expect(opts.credentials).toBe("include");
  });

  it("default TTL is 8 hours", () => {
    expect(DEFAULT_TTL_MS).toBe(8 * 60 * 60 * 1000);
  });
});

describe("fmtRemaining", () => {
  it("formats hours and minutes", () => {
    expect(fmtRemaining(2 * 60 * 60 * 1000 + 15 * 60 * 1000)).toBe("2h 15m");
  });

  it("formats just minutes when under an hour", () => {
    expect(fmtRemaining(45 * 60 * 1000)).toBe("45m");
  });

  it("returns 'expired' for zero or negative", () => {
    expect(fmtRemaining(0)).toBe("expired");
    expect(fmtRemaining(-10)).toBe("expired");
  });
});
