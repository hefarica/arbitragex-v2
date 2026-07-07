import { describe, it, expect, beforeEach } from "vitest";
import {
  hitAdminSession,
  isLockedOut,
  recordAuthFailure,
  recordAuthSuccess,
  __resetAdminSessionState,
} from "./admin-session-limits.js";

describe("admin session rate-limit (5/min/IP)", () => {
  beforeEach(() => __resetAdminSessionState());

  it("allows the first 5 attempts, denies the 6th", () => {
    const ip = "1.2.3.4";
    for (let i = 1; i <= 5; i++) {
      const r = hitAdminSession(ip);
      expect(r.ok).toBe(true);
      expect(r.remaining).toBe(5 - i);
    }
    const denied = hitAdminSession(ip);
    expect(denied.ok).toBe(false);
    expect(denied.remaining).toBe(0);
  });

  it("rolls over after the 60s window elapses", () => {
    const ip = "1.2.3.4";
    const t0 = 1_000_000;
    for (let i = 0; i < 5; i++) hitAdminSession(ip, t0);
    expect(hitAdminSession(ip, t0).ok).toBe(false);
    // Advance past the window.
    expect(hitAdminSession(ip, t0 + 60_001).ok).toBe(true);
  });

  it("tracks IPs independently", () => {
    for (let i = 0; i < 5; i++) hitAdminSession("a.a.a.a");
    expect(hitAdminSession("a.a.a.a").ok).toBe(false);
    expect(hitAdminSession("b.b.b.b").ok).toBe(true);
  });
});

describe("admin session 401 lockout (10 fails → 15min block)", () => {
  beforeEach(() => __resetAdminSessionState());

  it("does not lock out below threshold", () => {
    const ip = "1.2.3.4";
    for (let i = 0; i < 9; i++) recordAuthFailure(ip);
    expect(isLockedOut(ip)).toBe(false);
  });

  it("locks out at exactly the 10th consecutive failure", () => {
    const ip = "1.2.3.4";
    for (let i = 0; i < 10; i++) recordAuthFailure(ip);
    expect(isLockedOut(ip)).toBe(true);
  });

  it("releases the lock after 15 minutes", () => {
    const ip = "1.2.3.4";
    const t0 = 1_000_000;
    for (let i = 0; i < 10; i++) recordAuthFailure(ip, t0);
    expect(isLockedOut(ip, t0)).toBe(true);
    expect(isLockedOut(ip, t0 + 15 * 60_000 + 1)).toBe(false);
  });

  it("recordAuthSuccess clears any failure counter", () => {
    const ip = "1.2.3.4";
    for (let i = 0; i < 9; i++) recordAuthFailure(ip);
    recordAuthSuccess(ip);
    // After success, a fresh series of 9 fails must not trigger lockout.
    for (let i = 0; i < 9; i++) recordAuthFailure(ip);
    expect(isLockedOut(ip)).toBe(false);
  });
});
