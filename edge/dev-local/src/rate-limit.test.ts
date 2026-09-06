/**
 * EDGE-AUDIT-BUCKET-01 — unit tests for the classified rate limiter.
 *
 * The invariants that keep NR-0000 fixed WITHOUT weakening the public
 * posture (fail-closed on absent env, separate keyspaces, audit floor).
 */
import { describe, expect, it } from "vitest";

import { classifyRateLimit, createRateLimiter } from "./rate-limit.js";

const SECRETS = { edgeToken: "edge-secret", auditToken: "audit-secret" };

describe("classifyRateLimit", () => {
  it("internal SSR token ⇒ exempt", () => {
    expect(classifyRateLimit({ "x-arbx-edge-token": "edge-secret" }, SECRETS)).toBe("exempt");
  });

  it("audit token ⇒ audit bucket", () => {
    expect(classifyRateLimit({ "x-arbx-audit-token": "audit-secret" }, SECRETS)).toBe("audit");
  });

  it("no headers ⇒ public", () => {
    expect(classifyRateLimit({}, SECRETS)).toBe("public");
  });

  it("wrong token value ⇒ public (never a partial grant)", () => {
    expect(classifyRateLimit({ "x-arbx-audit-token": "wrong" }, SECRETS)).toBe("public");
    expect(classifyRateLimit({ "x-arbx-edge-token": "wrong" }, SECRETS)).toBe("public");
  });

  it("EDGE_AUDIT_TOKEN unset ⇒ audit header ignored (fail-closed)", () => {
    const noAuditEnv = { edgeToken: "edge-secret", auditToken: "" };
    expect(classifyRateLimit({ "x-arbx-audit-token": "audit-secret" }, noAuditEnv)).toBe("public");
  });

  it("duplicated header (string[]) never matches a secret", () => {
    expect(
      classifyRateLimit({ "x-arbx-audit-token": ["audit-secret", "audit-secret"] }, SECRETS),
    ).toBe("public");
  });
});

describe("createRateLimiter", () => {
  const mkLimiter = () => {
    let t = 1_000_000;
    return createRateLimiter({
      publicMax: 3,
      auditMax: 5,
      now: () => t,
    });
  };

  it("public bucket 429s at its own max with correct remaining math", () => {
    const rl = createRateLimiter({ publicMax: 3, auditMax: 5 });
    expect(rl.check("public", "1.2.3.4")).toMatchObject({ ok: true, remaining: 2 });
    expect(rl.check("public", "1.2.3.4")).toMatchObject({ ok: true, remaining: 1 });
    expect(rl.check("public", "1.2.3.4")).toMatchObject({ ok: true, remaining: 0 });
    expect(rl.check("public", "1.2.3.4")).toMatchObject({ ok: false, remaining: 0 });
  });

  it("audit traffic does NOT consume the public bucket (NR-0000 core)", () => {
    const rl = createRateLimiter({ publicMax: 2, auditMax: 10 });
    // Drain the AUDIT bucket past the public max.
    for (let i = 0; i < 5; i++) expect(rl.check("audit", "9.9.9.9").ok).toBe(true);
    // Same IP on the public class still has its full quota.
    expect(rl.check("public", "9.9.9.9")).toMatchObject({ ok: true, remaining: 1 });
    expect(rl.check("public", "9.9.9.9")).toMatchObject({ ok: true, remaining: 0 });
    expect(rl.check("public", "9.9.9.9")).toMatchObject({ ok: false, remaining: 0 });
  });

  it("audit bucket is bounded — the auditor is rate-limited too, never a bypass", () => {
    const rl = createRateLimiter({ publicMax: 2, auditMax: 4 });
    for (let i = 0; i < 4; i++) expect(rl.check("audit", "8.8.8.8").ok).toBe(true);
    expect(rl.check("audit", "8.8.8.8")).toMatchObject({ ok: false, remaining: 0 });
  });

  it("exempt consumes nothing and never 429s", () => {
    const rl = createRateLimiter({ publicMax: 1, auditMax: 1 });
    for (let i = 0; i < 10; i++) {
      expect(rl.check("exempt", "7.7.7.7")).toEqual({ klass: "exempt", ok: true, remaining: null });
    }
    // Public quota untouched by the exempt traffic above.
    expect(rl.check("public", "7.7.7.7").ok).toBe(true);
  });

  it("window elapse resets the counter (same semantics as the original hit())", () => {
    let t = 1_000_000;
    const rl = createRateLimiter({ publicMax: 1, auditMax: 1, windowMs: 60_000, now: () => t });
    expect(rl.check("public", "6.6.6.6").ok).toBe(true);
    expect(rl.check("public", "6.6.6.6").ok).toBe(false);
    t += 60_001;
    expect(rl.check("public", "6.6.6.6").ok).toBe(true);
  });

  it("distinct IPs get independent public buckets", () => {
    const rl = createRateLimiter({ publicMax: 1, auditMax: 1 });
    expect(rl.check("public", "5.5.5.5").ok).toBe(true);
    expect(rl.check("public", "4.4.4.4").ok).toBe(true);
  });

  it("decision carries its class so telemetry can distinguish buckets", () => {
    const rl = mkLimiter();
    expect(rl.check("audit", "3.3.3.3").klass).toBe("audit");
    expect(rl.check("public", "3.3.3.3").klass).toBe("public");
    expect(rl.check("exempt", "3.3.3.3").klass).toBe("exempt");
  });
});
