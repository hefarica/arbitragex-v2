/**
 * A-01 — Read-side PII redaction for audit-log rows.
 *
 * The WRITE path already anonymizes `ip_address` at insert time via the
 * mig-070 Postgres helper `arbx_anonymize_ip()::cidr` (guarded by
 * `pii-wireado-recursive.test.ts`). This module is DEFENSE-IN-DEPTH at the
 * READ boundary so that:
 *   - legacy rows written before mig-070 (raw IPs) can never reach a response,
 *   - raw IPs that surface as `target_id` (e.g. an RPC endpoint host) are
 *     collapsed to a /48 (IPv6) or /24 (IPv4) network,
 *   - the operator email carried in `actor` is replaced by a stable hash, so
 *     CSV/JSON exports of the audit log cannot leak it.
 *
 * Scope: ONLY the read serializer. The append-only audit store and the write
 * path are NOT touched (A-01 correction + RULE 00 Zero-Mocks).
 */
import { createHash } from "node:crypto";

const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

function isEmail(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.length >= 3 &&
    value.length < 320 &&
    EMAIL_RE.test(value)
  );
}

/** Stable pseudonym for an actor. Same input → same hash (audit correlation). */
export function hashActor(actor: string): string {
  return "sha256:" + createHash("sha256").update(actor, "utf8").digest("hex").slice(0, 12);
}

/** Expand a (possibly `::`-compressed) IPv6 to 8 full lowercase groups, or null. */
function expandIPv6(addr: string): string[] | null {
  const bare = addr.split("/")[0] ?? addr;
  const halves = bare.split("::");
  if (halves.length > 2) return null;
  if (halves.length === 2) {
    const headStr = halves[0] ?? "";
    const tailStr = halves[1] ?? "";
    const head = headStr ? headStr.split(":") : [];
    const tail = tailStr ? tailStr.split(":") : [];
    const fill = 8 - head.length - tail.length;
    if (fill < 0) return null;
    return [...head, ...Array.from({ length: fill }, () => "0"), ...tail].map((g) =>
      g.padStart(4, "0").toLowerCase(),
    );
  }
  const groups = bare.split(":");
  if (groups.length !== 8) return null;
  return groups.map((g) => g.padStart(4, "0").toLowerCase());
}

/**
 * Collapse any IP (raw or already-CIDR) to its /48 (IPv6) or /24 (IPv4)
 * network. Non-IP strings are returned unchanged so that `target_id` values
 * like pool addresses, chain ids or config keys pass through untouched.
 */
export function anonymizeIpString(raw: string | null | undefined): string | null {
  if (raw == null) return null;
  const s = String(raw).trim();
  if (s === "") return "";

  const beforeSlash = s.split("/")[0] ?? s;
  const v4 = beforeSlash.match(/^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/);
  if (v4) {
    const [a = "0", b = "0", c = "0"] = v4.slice(1, 4);
    if ([a, b, c].every((octet) => Number(octet) <= 255)) {
      return `${a}.${b}.${c}.0/24`;
    }
  }

  if (s.includes(":")) {
    const groups = expandIPv6(s);
    if (groups) {
      const [g0 = "0", g1 = "0", g2 = "0"] = groups;
      return `${g0}:${g1}:${g2}::/48`;
    }
  }

  return s;
}

/**
 * Per-row redactor applied at the read boundary of /admin/audit. Returns a new
 * row object; never mutates the input (the audit store is append-only).
 */
export function redactAuditRow(row: Record<string, unknown>): Record<string, unknown> {
  return {
    ...row,
    ip_address: anonymizeIpString(row.ip_address as string | null | undefined),
    target_id:
      typeof row.target_id === "string"
        ? (anonymizeIpString(row.target_id) ?? row.target_id)
        : row.target_id,
    actor: isEmail(row.actor) ? hashActor(row.actor) : row.actor,
  };
}
