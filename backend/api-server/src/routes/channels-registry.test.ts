/**
 * CHANNELS-REGISTRY (2026-09-26) — wiring-claims-are-checked tests (R10).
 *
 * The registry makes DECLARED-WIRING claims (producer/consumer repo paths).
 * This test makes those claims load-bearing: a path that disappears from the
 * repo (crate renamed, module moved) FAILS here until the registry is edited
 * deliberately in the same PR. Also locks the single source of truth: the
 * /channels runtime surface must report exactly the registry's stream names.
 */

import { describe, expect, it } from "vitest";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { CANONICAL_CHANNEL_REGISTRY } from "./channels-registry.js";
import { DEFAULT_CHANNELS } from "./channels.js";

const __dirname_local = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(__dirname_local, "..", "..", "..", "..");

const VALID_STATUSES = new Set(["active", "wired-starved", "no-producer-observed"]);

describe("CHANNELS-REGISTRY — R10 declared-wiring invariants", () => {
  it("every declared producer/consumer path EXISTS in the repo (claims are checked, not prose)", () => {
    const broken: string[] = [];
    for (const entry of CANONICAL_CHANNEL_REGISTRY) {
      for (const p of [...entry.producer, ...entry.consumers]) {
        if (!existsSync(resolve(REPO_ROOT, p))) {
          broken.push(`${entry.name}: missing path ${p}`);
        }
      }
    }
    expect(broken).toEqual([]);
  });

  it("schema: stream names are arbx: keys, statuses are the R10 vocabulary, impact is non-empty", () => {
    for (const entry of CANONICAL_CHANNEL_REGISTRY) {
      expect(entry.name.startsWith("arbx:"), entry.name).toBe(true);
      expect(VALID_STATUSES.has(entry.status), entry.name).toBe(true);
      expect(entry.producer.length, entry.name).toBeGreaterThan(0);
      expect(entry.consumers.length, entry.name).toBeGreaterThan(0);
      expect(entry.impact.length, entry.name).toBeGreaterThan(10);
    }
  });

  it("R10 survey truth is locked: exactly these streams are starved/never-produced today", () => {
    const byName = new Map(CANONICAL_CHANNEL_REGISTRY.map((e) => [e.name, e]));
    // wired-starved: upstream starvation keeps XLEN at 0 (simulated/executed chain).
    expect(byName.get("arbx:opps:validated")?.status).toBe("wired-starved");
    expect(byName.get("arbx:opps:simulated")?.status).toBe("wired-starved");
    expect(byName.get("arbx:opps:executed")?.status).toBe("wired-starved");
    // declared-never-produced (R10: NO COMPUTADO — jamás "cero evaluado").
    expect(byName.get("arbx:hot:detected")?.status).toBe("no-producer-observed");
    expect(byName.get("arbx:hot:simulated")?.status).toBe("no-producer-observed");
    // active per the 2026-09-26 survey.
    expect(byName.get("arbx:opps:detected")?.status).toBe("active");
  });

  it("single source of truth: /channels DEFAULT list === registry names (same order)", () => {
    expect(DEFAULT_CHANNELS).toEqual(CANONICAL_CHANNEL_REGISTRY.map((e) => e.name));
  });
});
