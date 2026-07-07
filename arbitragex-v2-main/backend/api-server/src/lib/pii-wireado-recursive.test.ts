/**
 * OMEGA-8 / CAPA5-F10 — PII wireado recursive guard (post-M3 extension).
 *
 * Verifies that across the ENTIRE backend tree (TS + Rust + SQL), any
 * INSERT into `audit_log` or `audit_event` that references `ip_address`
 * or `user_agent` is wrapped with the migration-053 helpers
 * (`arbx_anonymize_ip()` and `arbx_hash_user_agent()`).
 *
 * Strategy: walk backend/, find all source files containing `INSERT INTO
 * audit_log` or `INSERT INTO audit_event`, and assert the helper presence
 * invariant on each. This is a code-level defense matching the CI
 * `.github/workflows/omega8-pii-gates.yml` grep gates, but executed
 * inside the vitest suite so it runs locally pre-commit too.
 *
 * R8 fail-honest: the test reports the violating file + table; it never
 * masks a regression.
 */

import { describe, it, expect } from "vitest";
import { readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";

// Resolve backend/ root relative to this file (backend/api-server/src/lib/).
const BACKEND_ROOT = path.resolve(__dirname, "../../../../");

function walk(dir: string, files: string[] = []): string[] {
  for (const entry of readdirSync(dir)) {
    if (entry === "node_modules" || entry === "target" || entry === "dist") continue;
    const full = path.join(dir, entry);
    const st = statSync(full);
    if (st.isDirectory()) {
      walk(full, files);
    } else if (/\.(ts|rs|sql)$/.test(entry) && !/\.(test|spec)\.ts$/.test(entry)) {
      files.push(full);
    }
  }
  return files;
}

const sourceFiles = walk(BACKEND_ROOT);

function findInsertersOf(table: "audit_log" | "audit_event"): string[] {
  const needle = `INSERT INTO ${table}`;
  return sourceFiles.filter((f) => {
    try {
      return readFileSync(f, "utf8").includes(needle);
    } catch {
      return false;
    }
  });
}

describe("OMEGA-8/CAPA5-F10: PII wireado recursive guard", () => {
  it("every backend file inserting into audit_log + referencing ip_address wraps it via arbx_anonymize_ip", () => {
    const violations: string[] = [];
    for (const f of findInsertersOf("audit_log")) {
      const src = readFileSync(f, "utf8");
      if (src.includes("ip_address") && !src.includes("arbx_anonymize_ip")) {
        violations.push(path.relative(BACKEND_ROOT, f));
      }
    }
    expect(violations).toEqual([]);
  });

  it("every backend file inserting into audit_log + referencing user_agent wraps it via arbx_hash_user_agent", () => {
    const violations: string[] = [];
    for (const f of findInsertersOf("audit_log")) {
      const src = readFileSync(f, "utf8");
      if (src.includes("user_agent") && !src.includes("arbx_hash_user_agent")) {
        violations.push(path.relative(BACKEND_ROOT, f));
      }
    }
    expect(violations).toEqual([]);
  });

  it("every backend file inserting into audit_event + referencing ip_address wraps it via arbx_anonymize_ip", () => {
    const violations: string[] = [];
    for (const f of findInsertersOf("audit_event")) {
      const src = readFileSync(f, "utf8");
      if (src.includes("ip_address") && !src.includes("arbx_anonymize_ip")) {
        violations.push(path.relative(BACKEND_ROOT, f));
      }
    }
    expect(violations).toEqual([]);
  });

  it("every backend file inserting into audit_event + referencing user_agent wraps it via arbx_hash_user_agent", () => {
    const violations: string[] = [];
    for (const f of findInsertersOf("audit_event")) {
      const src = readFileSync(f, "utf8");
      if (src.includes("user_agent") && !src.includes("arbx_hash_user_agent")) {
        violations.push(path.relative(BACKEND_ROOT, f));
      }
    }
    expect(violations).toEqual([]);
  });

  it("at least 2 wireado anchors exist (registry-engine.ts + index.ts)", () => {
    const auditLogInserters = findInsertersOf("audit_log").map((p) => path.basename(p));
    const auditEventInserters = findInsertersOf("audit_event").map((p) => path.basename(p));
    expect(auditLogInserters).toContain("index.ts");
    expect(auditEventInserters).toContain("registry-engine.ts");
  });
});
