import { statfs as defaultStatfs } from "node:fs/promises";
import type { ReadinessItem } from "../types.js";

type StatFsSample = {
  bsize: number;
  blocks: number;
  bfree: number;
  bavail: number;
};

/**
 * G-DISK-1 — Host disk usage below critical threshold.
 *
 * Origin: 2026-08-29 production incident — the VPS hit 100% disk
 * (150G/150G; 59.6GB of it docker build cache) and PostgreSQL crash-looped
 * for hours ("No space left on device" PANIC on a pg_logical checkpoint).
 * Nothing surfaced it: prometheus scraped only app /metrics (no
 * node-exporter), alerts.rules.yml had no disk rule, and the frontend had
 * no disk surface. This gate lands disk health in the readiness blockers
 * panel (readiness_check derives blockers automatically).
 *
 * Measurement: statfs on the deployment partition. Inside the api-server
 * container statfs("/") reflects the host's single partition (/dev/sda1
 * overlay) — that is the deployment reality, not an approximation.
 * Override the probe path with ARBX_DISK_PATH when a different mount
 * matters.
 *
 * Thresholds (documented alert policy, env-overridable):
 *   ARBX_DISK_WARN_PCT (default 85) → yellow at/above
 *   ARBX_DISK_CRIT_PCT (default 95) → red at/above
 * usedPct = (blocks - bfree) / blocks * 100 — matches df Use%.
 *
 * Honesty contract (R8): if statfs throws (unsupported platform, EINVAL,
 * Windows dev) the item is yellow with the exact error surfaced — never a
 * fabricated percentage, never a crash of verifyAll. Malformed thresholds
 * are yellow naming the env var (mirrors risk-circuit-breakers env
 * handling). blocks=0 is a degenerate statfs result → yellow.
 */
export async function verifyGDISK1(opts?: {
  now?: () => Date;
  statfs?: (path: string) => Promise<StatFsSample>;
  path?: string;
}): Promise<ReadinessItem> {
  const target = opts?.path ?? process.env["ARBX_DISK_PATH"] ?? "/";
  const verified_at = (opts?.now ?? (() => new Date()))().toISOString();
  const base = {
    id: "G-DISK-1",
    group: "operations" as const,
    label: "Host disk usage below critical threshold",
    doctrine: "safe-production-observability",
    verified_at,
  };

  // Read at call time (not module load) so operator env changes and tests
  // both take effect without a process restart.
  const warnRaw = process.env["ARBX_DISK_WARN_PCT"] ?? "85";
  const critRaw = process.env["ARBX_DISK_CRIT_PCT"] ?? "95";
  const warnPct = Number(warnRaw);
  const critPct = Number(critRaw);

  // Threshold validation BEFORE any I/O: malformed operator config must not
  // reach statfs, and the reason must name the offending env var.
  if (!Number.isFinite(warnPct) || warnPct <= 0 || warnPct >= 100) {
    return {
      ...base,
      status: "yellow",
      reason: `ARBX_DISK_WARN_PCT="${warnRaw}" is malformed (expected 0 < pct < 100) — disk usage not evaluated`,
    };
  }
  if (!Number.isFinite(critPct) || critPct <= 0 || critPct >= 100) {
    return {
      ...base,
      status: "yellow",
      reason: `ARBX_DISK_CRIT_PCT="${critRaw}" is malformed (expected 0 < pct < 100) — disk usage not evaluated`,
    };
  }
  if (warnPct >= critPct) {
    return {
      ...base,
      status: "yellow",
      reason: `ARBX_DISK_WARN_PCT=${warnPct} must be < ARBX_DISK_CRIT_PCT=${critPct} — inverted thresholds, disk usage not evaluated`,
    };
  }

  let st: StatFsSample;
  try {
    st = await (opts?.statfs ?? defaultStatfs)(target);
  } catch (e) {
    return {
      ...base,
      status: "yellow",
      reason: `disk usage could not be measured: ${(e as Error).message}`,
    };
  }

  const evidence = { kind: "shell" as const, ref: `statfs(${target})` };

  if (st.blocks === 0) {
    return {
      ...base,
      status: "yellow",
      reason: "degenerate statfs result (blocks=0) — disk usage could not be computed",
      evidence,
    };
  }

  const usedPct = ((st.blocks - st.bfree) / st.blocks) * 100;
  const freeGb = (st.bfree * st.bsize) / 1024 ** 3;

  if (usedPct >= critPct) {
    return {
      ...base,
      status: "red",
      reason: `disk usage ${usedPct.toFixed(1)}% at/above crit ${critPct}% — ${freeGb.toFixed(1)} GB free · 2026-08-29: disk 100% crash-looped postgres for hours`,
      evidence,
    };
  }
  if (usedPct >= warnPct) {
    return {
      ...base,
      status: "yellow",
      reason: `disk usage ${usedPct.toFixed(1)}% at/above warn ${warnPct}% (crit ${critPct}%) — ${freeGb.toFixed(1)} GB free`,
      evidence,
    };
  }
  return {
    ...base,
    status: "green",
    reason: `disk usage ${usedPct.toFixed(1)}% — ${freeGb.toFixed(1)} GB free (below warn ${warnPct}%)`,
    evidence,
  };
}
