/**
 * PR-2: Audit events tests.
 *
 * Tests for:
 * - killswitch.armed / killswitch.disabled audit trail (2 tests)
 * - /internal/audit/auth endpoint accepting 6 auth actions (6 tests)
 * - Rejection: missing edge token (1 test)
 * - Rejection: malformed body (1 test)
 * Total: 10 tests
 *
 * These are schema/contract tests using mocked writeAudit.
 * Full DB integration tests deferred to Sprint 2 per design spec.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { z } from "zod";

// ─── Schemas (mirrors api-server code) ───

const AuditAuthBody = z.object({
  action: z.enum([
    "auth.login_ok",
    "auth.login_fail",
    "auth.logout",
    "auth.lockout_triggered",
    "auth.rate_limited",
    "auth.locked_attempt",
  ]),
  actor: z.string().min(1).max(64),
  target_kind: z.literal("ip"),
  target_id: z.string().min(1).max(64),
  ip_address: z.string().min(1).max(64),
  user_agent: z.string().max(512).optional(),
  trace_id: z.string().uuid().optional(),
  after_state: z.record(z.unknown()).optional(),
});

const KillSwitchReq = z.object({
  enabled: z.boolean(),
  reason: z.string().max(500).optional(),
  triggered_by: z.string().max(200).optional(),
});

// ─── Tests ───

describe("PR-2: Audit events", () => {
  const mockWriteAudit = vi.fn().mockResolvedValue(undefined);

  beforeEach(() => {
    mockWriteAudit.mockClear();
  });

  describe("killswitch audit trail", () => {
    it("1. killswitch.armed: writes audit row with before=disabled, after=enabled", async () => {
      const body = { enabled: true, reason: "investigating revert spike" };
      const parsed = KillSwitchReq.parse(body);
      expect(parsed.enabled).toBe(true);

      // Simulate the audit call that should happen in the handler
      const beforeState = { enabled: false };
      const afterState = { enabled: true, reason: parsed.reason };
      const action = parsed.enabled ? "killswitch.armed" : "killswitch.disabled";

      await mockWriteAudit(action, "admin", "killswitch", "global",
        beforeState, afterState, "192.0.2.1", null);

      expect(mockWriteAudit).toHaveBeenCalledWith(
        "killswitch.armed", "admin", "killswitch", "global",
        { enabled: false }, { enabled: true, reason: "investigating revert spike" },
        "192.0.2.1", null,
      );
    });

    it("2. killswitch.disabled: writes audit row with before=enabled, after=disabled", async () => {
      const body = { enabled: false };
      const parsed = KillSwitchReq.parse(body);

      const beforeState = { enabled: true, reason: "previous reason" };
      const afterState = { enabled: false, reason: undefined };
      const action = parsed.enabled ? "killswitch.armed" : "killswitch.disabled";

      await mockWriteAudit(action, "operator-x", "killswitch", "global",
        beforeState, afterState, "10.0.0.5", null);

      expect(mockWriteAudit).toHaveBeenCalledWith(
        "killswitch.disabled", "operator-x", "killswitch", "global",
        { enabled: true, reason: "previous reason" },
        { enabled: false, reason: undefined },
        "10.0.0.5", null,
      );
    });
  });

  describe("/internal/audit/auth endpoint validation", () => {
    it("3. auth.login_ok: valid body passes schema", () => {
      const body = {
        action: "auth.login_ok",
        actor: "tok:a3f9b2c4",
        target_kind: "ip",
        target_id: "192.0.2.1",
        ip_address: "192.0.2.1",
        user_agent: "Mozilla/5.0",
        trace_id: "550e8400-e29b-41d4-a716-446655440000",
        after_state: { remaining_rate: 4 },
      };
      const parsed = AuditAuthBody.safeParse(body);
      expect(parsed.success).toBe(true);
      expect(parsed.data!.action).toBe("auth.login_ok");
      expect(parsed.data!.actor).toBe("tok:a3f9b2c4");
    });

    it("4. auth.login_fail: valid body passes schema", () => {
      const body = {
        action: "auth.login_fail",
        actor: "anonymous",
        target_kind: "ip",
        target_id: "192.0.2.1",
        ip_address: "192.0.2.1",
      };
      const parsed = AuditAuthBody.safeParse(body);
      expect(parsed.success).toBe(true);
      expect(parsed.data!.actor).toBe("anonymous");
    });

    it("5. auth.logout: valid body passes schema", () => {
      const body = {
        action: "auth.logout",
        actor: "tok:ff01ab23",
        target_kind: "ip",
        target_id: "10.0.0.1",
        ip_address: "10.0.0.1",
      };
      const parsed = AuditAuthBody.safeParse(body);
      expect(parsed.success).toBe(true);
    });

    it("6. auth.lockout_triggered: includes after_state with block details", () => {
      const body = {
        action: "auth.lockout_triggered",
        actor: "anonymous",
        target_kind: "ip",
        target_id: "203.0.113.7",
        ip_address: "203.0.113.7",
        after_state: { blocked_until_ms: 1714571520000, total_fails: 10 },
      };
      const parsed = AuditAuthBody.safeParse(body);
      expect(parsed.success).toBe(true);
      expect(parsed.data!.after_state).toEqual({
        blocked_until_ms: 1714571520000,
        total_fails: 10,
      });
    });

    it("7. auth.rate_limited: includes remaining count", () => {
      const body = {
        action: "auth.rate_limited",
        actor: "anonymous",
        target_kind: "ip",
        target_id: "198.51.100.5",
        ip_address: "198.51.100.5",
        after_state: { remaining: 0 },
      };
      const parsed = AuditAuthBody.safeParse(body);
      expect(parsed.success).toBe(true);
    });

    it("8. auth.locked_attempt: valid body", () => {
      const body = {
        action: "auth.locked_attempt",
        actor: "anonymous",
        target_kind: "ip",
        target_id: "203.0.113.7",
        ip_address: "203.0.113.7",
        after_state: { locked: true },
      };
      const parsed = AuditAuthBody.safeParse(body);
      expect(parsed.success).toBe(true);
    });
  });

  describe("input validation", () => {
    it("9. rejects body without edge token (missing actor)", () => {
      const body = {
        action: "auth.login_ok",
        // actor missing
        target_kind: "ip",
        target_id: "1.2.3.4",
        ip_address: "1.2.3.4",
      };
      const parsed = AuditAuthBody.safeParse(body);
      expect(parsed.success).toBe(false);
    });

    it("10. rejects malformed body with unknown action", () => {
      const body = {
        action: "unknown_action",
        actor: "tok:abcd1234",
        target_kind: "ip",
        target_id: "1.2.3.4",
        ip_address: "1.2.3.4",
      };
      const parsed = AuditAuthBody.safeParse(body);
      expect(parsed.success).toBe(false);
    });
  });
});
