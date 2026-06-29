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
 *     .github/npm-audit-allowlist.json, each with a reason + upgrade plan, and
 *     a single shared expiry. This mirrors the repo's existing cargo-audit
 *     RUSTSEC_IGNORES discipline (security.yml) — known debt is VISIBLE and
 *     EXPIRING, not silently masked.
 *   - Past the expiry the allowlist stops suppressing → the gate re-fails,
 *     forcing re-triage / the dependency-upgrade sprint.
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
  const expires = allowlist.expires; // "YYYY-MM-DD"
  const allow = allowlist.allow || {};
  const expired = new Date() > new Date(`${expires}T23:59:59Z`);

  const audit = JSON.parse(runAudit());
  const vulns = audit.vulnerabilities || {};

  const blocking = [];
  const tracked = [];
  for (const [name, v] of Object.entries(vulns)) {
    if (v.severity !== "high" && v.severity !== "critical") continue;
    const allowed = Object.prototype.hasOwnProperty.call(allow, name);
    if (allowed && !expired) {
      tracked.push({ name, severity: v.severity, reason: allow[name] });
    } else {
      blocking.push({ name, severity: v.severity, allowlistedButExpired: allowed && expired });
    }
  }

  console.log(`npm-audit-gate — prod deps (--omit=dev), high+ blocking`);
  console.log(`allowlist expiry: ${expires}${expired ? "  ⚠️ EXPIRED — allowlist no longer suppresses" : ""}`);
  if (tracked.length) {
    console.log(`\nTracked (allowlisted, expiring) — ${tracked.length}:`);
    for (const t of tracked) console.log(`  • ${t.severity.padEnd(8)} ${t.name} — ${t.reason}`);
  }
  if (blocking.length) {
    console.error(`\n❌ BLOCKING high/critical advisories — ${blocking.length}:`);
    for (const b of blocking) {
      console.error(`  • ${b.severity.padEnd(8)} ${b.name}${b.allowlistedButExpired ? "  (allowlist EXPIRED)" : "  (NOT allowlisted)"}`);
    }
    console.error(`\nFix: upgrade the offending package(s), or — if triaged — add them to ${path.relative(process.cwd(), ALLOWLIST_PATH)} with a reason + plan.`);
    process.exit(1);
  }
  console.log(`\n✅ no un-allowlisted high/critical prod advisories`);
}

main();
