/**
 * A-01 — Unit tests for the read-side audit PII redactor.
 *
 * Guards: raw/legacy IPs are collapsed to /48 or /24; the operator email in
 * `actor` is hashed stably; non-IP / non-email fields pass through untouched.
 * The append-only write path is not exercised here (it has its own guards:
 * registry-engine.pii.test.ts, pii-wireado-recursive.test.ts).
 */
import { describe, it, expect } from "vitest";
import { anonymizeIpString, hashActor, redactAuditRow } from "./audit-redact.js";

describe("A-01 anonymizeIpString", () => {
  it("collapses a raw IPv4 to its /24 network", () => {
    expect(anonymizeIpString("203.0.113.45")).toBe("203.0.113.0/24");
  });

  it("normalizes an already-CIDR IPv4 to /24", () => {
    expect(anonymizeIpString("203.0.113.45/32")).toBe("203.0.113.0/24");
  });

  it("collapses a full 8-group IPv6 to /48", () => {
    expect(anonymizeIpString("2001:0db8:abcd:1234:5678:9abc:def0:1111")).toBe(
      "2001:0db8:abcd::/48",
    );
  });

  it("expands a compressed (::) IPv6 before collapsing to /48", () => {
    expect(anonymizeIpString("2001:db8:1::2")).toBe("2001:0db8:0001::/48");
  });

  it("preserves an already-/48 IPv6 in normalized form", () => {
    expect(anonymizeIpString("2001:db8:abcd::/48")).toBe("2001:0db8:abcd::/48");
  });

  it("leaves non-IP strings untouched (pool address, config key)", () => {
    expect(anonymizeIpString("0xabcdef0123456789abcdef0123456789abcdef01")).toBe(
      "0xabcdef0123456789abcdef0123456789abcdef01",
    );
    expect(anonymizeIpString("chain_id=1")).toBe("chain_id=1");
    expect(anonymizeIpString("capital_usd")).toBe("capital_usd");
  });

  it("passes null/empty through", () => {
    expect(anonymizeIpString(null)).toBeNull();
    expect(anonymizeIpString("")).toBe("");
  });
});

describe("A-01 hashActor", () => {
  it("is stable — same email yields the same hash", () => {
    expect(hashActor("beticosa1@gmail.com")).toBe(hashActor("beticosa1@gmail.com"));
  });

  it("produces a sha256: prefix + 12 lowercase hex chars", () => {
    expect(hashActor("a@b.com")).toMatch(/^sha256:[0-9a-f]{12}$/);
  });

  it("differs for different emails", () => {
    expect(hashActor("a@b.com")).not.toBe(hashActor("c@d.com"));
  });
});

describe("A-01 redactAuditRow", () => {
  const base = {
    id: "00000000-0000-0000-0000-000000000001",
    actor: "beticosa1@gmail.com",
    action: "auth.login_ok",
    target_kind: "rpc_endpoint",
    target_id: "2001:0db8:abcd:1234:5678:9abc:def0:1111",
    before_state: null,
    after_state: null,
    ip_address: "203.0.113.45",
    user_agent: null,
    trace_id: "t-1",
    created_at: "2026-01-01T00:00:00.000Z",
  };

  it("redacts an email actor, a raw ip_address, and a raw-IP target_id together", () => {
    const r = redactAuditRow(base);
    expect(r.actor).toMatch(/^sha256:[0-9a-f]{12}$/);
    expect(r.ip_address).toBe("203.0.113.0/24");
    expect(r.target_id).toBe("2001:0db8:abcd::/48");
  });

  it("leaves a non-email actor and a non-IP target_id untouched", () => {
    const r = redactAuditRow({ ...base, actor: "system", target_id: "0xabc123" });
    expect(r.actor).toBe("system");
    expect(r.target_id).toBe("0xabc123");
  });

  it("preserves an already-anonymized ip_address", () => {
    const r = redactAuditRow({ ...base, ip_address: "2001:0db8:0001::/48" });
    expect(r.ip_address).toBe("2001:0db8:0001::/48");
  });

  it("does not mutate the input row (append-only store integrity)", () => {
    const snapshot = { ...base };
    redactAuditRow(base);
    expect(base).toEqual(snapshot);
  });
});
