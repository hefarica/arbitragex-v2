/**
 * OMEGA-8/M4 Fase 12 — Prometheus metrics naming convention enforcement.
 *
 * Verifies the four mandatory invariants on every metric exported by
 * shared-ts/src/metrics/index.ts:
 *
 *   1. Every name starts with the `arbx_` namespace prefix.
 *   2. Counters end with `_total` (Prometheus convention).
 *   3. Histograms end with a unit suffix (`_seconds`, `_ms`,
 *      `_bytes`, etc.) — never `_total`.
 *   4. Gauges have NO `_total` suffix.
 *
 * These invariants protect dashboards from silent drift. If a metric
 * is renamed or a new metric is added that violates them, this test
 * fails loudly so the operator can update the Grafana boards in
 * lockstep.
 *
 * Reads metrics/index.ts source rather than introspecting registered
 * Counters/Histograms/Gauges to avoid coupling to prom-client internals.
 */

import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname_local = dirname(fileURLToPath(import.meta.url));
const METRICS_SRC = readFileSync(
  resolve(__dirname_local, "../metrics/index.ts"),
  "utf8",
);

type MetricKind = "Counter" | "Gauge" | "Histogram";
interface MetricDecl {
  name: string;
  kind: MetricKind;
}

function extractMetrics(src: string): MetricDecl[] {
  // Match `new Counter({\n  name: "..."` / Gauge / Histogram blocks.
  const out: MetricDecl[] = [];
  const re = /new\s+(Counter|Gauge|Histogram)\(\s*\{\s*name:\s*"([^"]+)"/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(src)) !== null) {
    out.push({ kind: m[1] as MetricKind, name: m[2]! });
  }
  return out;
}

const ALL = extractMetrics(METRICS_SRC);

describe("OMEGA-8/M4 Fase 12 — Prometheus naming conventions", () => {
  it("at least 15 metrics were discovered in shared-ts/src/metrics/index.ts", () => {
    // Smoke test against an empty regex match. Update threshold up as new
    // metrics are added.
    expect(ALL.length).toBeGreaterThanOrEqual(15);
  });

  it("every metric name starts with the 'arbx_' namespace prefix", () => {
    for (const m of ALL) {
      expect(m.name, `${m.kind} ${m.name}`).toMatch(/^arbx_/);
    }
  });

  it("every Counter ends with '_total' (Prom convention)", () => {
    const counters = ALL.filter((m) => m.kind === "Counter");
    expect(counters.length).toBeGreaterThan(0);
    for (const c of counters) {
      expect(c.name, `Counter ${c.name}`).toMatch(/_total$/);
    }
  });

  it("no Gauge ends with '_total' (Prom convention reserves _total for counters)", () => {
    const gauges = ALL.filter((m) => m.kind === "Gauge");
    expect(gauges.length).toBeGreaterThan(0);
    for (const g of gauges) {
      expect(g.name, `Gauge ${g.name}`).not.toMatch(/_total$/);
    }
  });

  it("every Histogram ends with a recognised unit suffix (never _total)", () => {
    const histograms = ALL.filter((m) => m.kind === "Histogram");
    expect(histograms.length).toBeGreaterThan(0);
    const unitRe = /_(seconds|ms|microseconds|nanoseconds|bytes|count)$/;
    for (const h of histograms) {
      expect(h.name, `Histogram ${h.name}`).not.toMatch(/_total$/);
      expect(h.name, `Histogram ${h.name} needs unit suffix`).toMatch(unitRe);
    }
  });

  it("critical OMEGA-7/OMEGA-8 metric names are present (dashboard contract)", () => {
    const names = new Set(ALL.map((m) => m.name));
    const required = [
      "arbx_runtime_ack_broadcast_total",
      "arbx_runtime_ack_broadcast_latency_ms",
      "arbx_selector_invalid_messages_total",
      "arbx_cb_state",
      "arbx_cb_trips_total",
      "arbx_killswitch_enabled",
      "arbx_audit_events_total",
    ];
    for (const r of required) {
      expect(names.has(r), `expected metric ${r}`).toBe(true);
    }
  });
});
