#!/usr/bin/env node
/**
 * npm-audit-gate — a REAL prod-dependency vulnerability gate.
 *
 * Replaces the rigged `npm audit --omit=dev --audit-level=high || true` in
 * .github/workflows/security.yml, whose `|| true` defeated the gate entirely
 * (its own comment said "NEVER add continue-on-error here — that defeats the
 * gate", yet `|| true` did exactly that). This script makes the gate honest:
 *
 *   - It FAILS (exit 1) on any high/critical prod-dependency advisory whose
 *     top-level package is NOT in the documented allowlist.
 *   - Known/triaged advisories are explicitly allowlisted in
 *     .github/npm-audit-allowlist.json, each with a reason + upgrade plan and
 *     its OWN expiry (`{ reason, expires }`), falling back to the global
 *     `expires` for plain-string entries. This mirrors the repo's existing
 *     cargo-audit RUSTSEC_IGNORES discipline (security.yml) — known debt is
 *     VISIBLE and EXPIRING, not silently masked.
 *   - Past an entry's expiry that entry stops suppressing → the gate re-fails,
 *     forcing re-triage / the dependency-upgrade sprint. A single shared expiry
 *     is not enough: the `next` entry carried "Expires: 2026-08-31" inside its
 *     prose for 51 days while still suppressing, because no code read it.
 *
 * Net effect vs `|| true`: a NEW high/critical in any package outside the
 * triaged set (e.g. express, pg, redis, a backend dep) now FAILS the merge,
 * and the accepted frontend-wallet/framework debt is tracked openly with a
 * deadline instead of being invisible forever.
 *
 * Usage (run from repo root, after `npm install`):
 *   node scripts/npm-audit-gate.mjs
 */
import { execSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ALLOWLIST_PATH = path.join(__dirname, "..", ".github", "npm-audit-allowlist.json");

function runAudit() {
  // npm audit exits non-zero when advisories exist; capture stdout regardless.
  try {
    return execSync("npm audit --omit=dev --json", {
      encoding: "utf8",
      maxBuffer: 64 * 1024 * 1024,
      stdio: ["ignore", "pipe", "ignore"],
    });
  } catch (e) {
    if (e.stdout) return e.stdout.toString();
    throw e;
  }
}

function main() {
  const allowlist = JSON.parse(readFileSync(ALLOWLIST_PATH, "utf8"));
  const globalExpires = allowlist.expires; // "YYYY-MM-DD"
  const allow = allowlist.allow || {};

  // Per-entry expiry. A string entry inherits the global date; an object entry
  // carries its own, so one long-running debt cannot hide behind a renewal that
  // was granted for a different package.
  function entryFor(name) {
    const raw = allow[name];
    if (typeof raw === "string") return { reason: raw, expires: globalExpires };
    if (raw && typeof raw === "object" && typeof raw.reason === "string" && typeof raw.expires === "string") {
      return { reason: raw.reason, expires: raw.expires };
    }
    throw new Error(`allowlist entry "${name}" must be a reason string or { reason, expires }`);
  }
  const isExpired = (d) => new Date() > new Date(`${d}T23:59:59Z`);
  const daysLeft = (d) => Math.ceil((new Date(`${d}T23:59:59Z`) - new Date()) / 86400000);

  const audit = JSON.parse(runAudit());
  const vulns = audit.vulnerabilities || {};

  const blocking = [];
  const tracked = [];
  for (const [name, v] of Object.entries(vulns)) {
    if (v.severity !== "high" && v.severity !== "critical") continue;
    const allowed = Object.prototype.hasOwnProperty.call(allow, name);
    const entry = allowed ? entryFor(name) : null;
    if (entry && !isExpired(entry.expires)) {
      tracked.push({ name, severity: v.severity, ...entry });
    } else {
      blocking.push({
        name,
        severity: v.severity,
        expires: entry ? entry.expires : null,
        allowlistedButExpired: Boolean(entry && isExpired(entry.expires)),
      });
    }
  }

  console.log(`npm-audit-gate — prod deps (--omit=dev), high+ blocking`);
  console.log(`allowlist: per-entry expiry (global default ${globalExpires}${isExpired(globalExpires) ? ", ⚠️ EXPIRED" : ""})`);
  if (tracked.length) {
    console.log(`\nTracked (allowlisted, expiring) — ${tracked.length}:`);
    for (const t of tracked) {
      console.log(`  • ${t.severity.padEnd(8)} ${t.name} [expires ${t.expires}, ${daysLeft(t.expires)}d] — ${t.reason}`);
    }
    const cliffs = tracked.map((t) => t.expires).sort();
    console.log(`\nnext expiry cliff: ${cliffs[0]} (${daysLeft(cliffs[0])}d)`);
  }
  if (blocking.length) {
    console.error(`\n❌ BLOCKING high/critical advisories — ${blocking.length}:`);
    for (const b of blocking) {
      console.error(
        `  • ${b.severity.padEnd(8)} ${b.name}` +
          (b.allowlistedButExpired ? `  (allowlist entry EXPIRED ${b.expires})` : "  (NOT allowlisted)"),
      );
    }
    console.error(`\nFix: upgrade the offending package(s), or — if triaged — add them to ${path.relative(process.cwd(), ALLOWLIST_PATH)} with a reason + plan.`);
    process.exit(1);
  }
  console.log(`\n✅ no un-allowlisted high/critical prod advisories`);
}

main();
