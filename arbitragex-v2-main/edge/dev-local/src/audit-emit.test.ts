import { describe, it, expect, vi, beforeEach } from "vitest";
import { emitAuditEvent, tokenFingerprint } from "./audit-emit.js";

describe("audit-emit", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  describe("tokenFingerprint", () => {
    it("returns tok: prefix with 8 hex chars", () => {
      const fp = tokenFingerprint("test-token-123");
      expect(fp).toMatch(/^tok:[0-9a-f]{8}$/);
    });

    it("is deterministic", () => {
      expect(tokenFingerprint("same")).toBe(tokenFingerprint("same"));
    });

    it("differs for different tokens", () => {
      expect(tokenFingerprint("token-a")).not.toBe(tokenFingerprint("token-b"));
    });
  });

  describe("emitAuditEvent", () => {
    it("calls fetch with correct URL, headers, and body on success", async () => {
      const mockFetch = vi.fn().mockResolvedValue({ ok: true, status: 204 });
      vi.stubGlobal("fetch", mockFetch);

      await emitAuditEvent("http://api:8080", "edge-tok-123", {
        action: "auth.login_ok",
        actor: "tok:a3f9b2c4",
        ipAddress: "192.0.2.1",
        userAgent: "Mozilla/5.0",
        traceId: "550e8400-e29b-41d4-a716-446655440000",
      });

      expect(mockFetch).toHaveBeenCalledOnce();
      const [url, opts] = mockFetch.mock.calls[0]!;
      expect(url).toBe("http://api:8080/internal/audit/auth");
      expect(opts.method).toBe("POST");
      expect(opts.headers["x-arbx-edge-token"]).toBe("edge-tok-123");
      const body = JSON.parse(opts.body);
      expect(body.action).toBe("auth.login_ok");
      expect(body.actor).toBe("tok:a3f9b2c4");
      expect(body.target_kind).toBe("ip");
      expect(body.ip_address).toBe("192.0.2.1");
    });

    it("logs to console.error on timeout but does not throw", async () => {
      const mockFetch = vi.fn().mockImplementation(() => {
        return new Promise((_, reject) => {
          setTimeout(() => reject(Object.assign(new Error("aborted"), { name: "AbortError" })), 10);
        });
      });
      vi.stubGlobal("fetch", mockFetch);
      const errSpy = vi.spyOn(console, "error").mockImplementation(() => {});

      // Should NOT throw
      await emitAuditEvent("http://api:8080", "tok", {
        action: "auth.login_fail",
        actor: "anonymous",
        ipAddress: "10.0.0.1",
      });

      expect(errSpy).toHaveBeenCalledOnce();
      const logged = JSON.parse(errSpy.mock.calls[0]![0] as string);
      expect(logged.event).toBe("audit.emit_failed");
      expect(logged.reason).toBe("timeout");
    });

    it("logs to console.error on 5xx but does not throw", async () => {
      const mockFetch = vi.fn().mockResolvedValue({ ok: false, status: 500 });
      vi.stubGlobal("fetch", mockFetch);
      const errSpy = vi.spyOn(console, "error").mockImplementation(() => {});

      await emitAuditEvent("http://api:8080", "tok", {
        action: "auth.lockout_triggered",
        actor: "anonymous",
        ipAddress: "203.0.113.7",
        afterState: { blocked_until_ms: 9999, total_fails: 10 },
      });

      expect(errSpy).toHaveBeenCalledOnce();
      const logged = JSON.parse(errSpy.mock.calls[0]![0] as string);
      expect(logged.event).toBe("audit.emit_failed");
      expect(logged.reason).toBe("http_500");
    });
  });
});
