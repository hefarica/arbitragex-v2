/**
 * @vitest-environment node
 *
 * We polyfill a minimal localStorage so the module can be tested without jsdom.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  adminTokenExpiresInMs,
  clearAdminToken,
  DEFAULT_TTL_MS,
  fmtRemaining,
  getAdminToken,
  setAdminToken,
} from "./admin-token";

class MemoryStorage implements Storage {
  private map = new Map<string, string>();
  get length(): number { return this.map.size; }
  clear(): void { this.map.clear(); }
  getItem(k: string): string | null { return this.map.get(k) ?? null; }
  key(i: number): string | null { return Array.from(this.map.keys())[i] ?? null; }
  removeItem(k: string): void { this.map.delete(k); }
  setItem(k: string, v: string): void { this.map.set(k, v); }
}

beforeEach(() => {
  // @ts-expect-error injecting a partial window for the module to find.
  globalThis.window = { localStorage: new MemoryStorage() };
});

afterEach(() => {
  // @ts-expect-error tearing down our shim.
  delete globalThis.window;
  vi.useRealTimers();
});

describe("admin-token storage", () => {
  it("stores and retrieves a token within TTL", () => {
    setAdminToken("secret-abc");
    expect(getAdminToken()).toBe("secret-abc");
  });

  it("clears both keys when expired", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-01-01T00:00:00Z"));
    setAdminToken("secret-abc", 1000); // 1s TTL
    vi.setSystemTime(new Date("2026-01-01T00:00:02Z")); // 2s later
    expect(getAdminToken()).toBe("");
    expect(adminTokenExpiresInMs()).toBe(0);
  });

  it("clearAdminToken wipes both keys", () => {
    setAdminToken("secret-abc");
    expect(getAdminToken()).toBe("secret-abc");
    clearAdminToken();
    expect(getAdminToken()).toBe("");
    expect(adminTokenExpiresInMs()).toBe(0);
  });

  it("default TTL is 8 hours", () => {
    expect(DEFAULT_TTL_MS).toBe(8 * 60 * 60 * 1000);
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-01-01T00:00:00Z"));
    setAdminToken("secret-abc");
    const remaining = adminTokenExpiresInMs();
    expect(remaining).toBeGreaterThan(7 * 60 * 60 * 1000);
    expect(remaining).toBeLessThanOrEqual(DEFAULT_TTL_MS);
  });

  it("ignores corrupted expiry records and clears them", () => {
    // Pre-seed a corrupt expiry value directly.
    window.localStorage.setItem("arbx_admin_token", "secret-abc");
    window.localStorage.setItem("arbx_admin_token_expires_at", "not-a-number");
    expect(getAdminToken()).toBe("");
  });

  it("returns empty string when nothing is stored", () => {
    expect(getAdminToken()).toBe("");
    expect(adminTokenExpiresInMs()).toBe(0);
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
