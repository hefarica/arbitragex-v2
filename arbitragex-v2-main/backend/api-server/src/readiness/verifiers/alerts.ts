import { promises as fs } from "node:fs";
import path from "node:path";
import type { ReadinessItem } from "../types.js";

const DEFAULT_REPO = "/repo";
const DEFAULT_PROM = process.env["PROMETHEUS_INTERNAL_URL"] ?? "http://prometheus:9090";
const DEFAULT_AM = process.env["ALERTMANAGER_INTERNAL_URL"] ?? "http://alertmanager:9093";
const MIN_RULES = 5;

/**
 * ALERTS — Alert rules + Alertmanager wired and reachable.
 *
 * Three-layer verification:
 *
 *   1. monitoring/alerts.rules.yml has ≥MIN_RULES alert definitions.
 *   2. Prometheus has loaded the rules (api/v1/rules returns ≥MIN_RULES).
 *   3. Alertmanager /api/v2/status returns 2xx (the dispatcher is up).
 *
 * Note: this gate does NOT require an alert to have fired through to
 * PagerDuty/Telegram — that is operationally hard to verify automatically.
 * What it DOES require is the full plumbing: rules in repo + loaded by
 * Prometheus + Alertmanager reachable. A real fire is exercised via the
 * weekly `KillSwitchActivated` smoke test (operator ops drill).
 */
export async function verifyAlerts(opts?: {
  repo?: string;
  promUrl?: string;
  amUrl?: string;
  minRules?: number;
  timeoutMs?: number;
  now?: () => Date;
}): Promise<ReadinessItem> {
  const repo = opts?.repo ?? DEFAULT_REPO;
  const prom = opts?.promUrl ?? DEFAULT_PROM;
  const am = opts?.amUrl ?? DEFAULT_AM;
  const minRules = opts?.minRules ?? MIN_RULES;
  const timeout = opts?.timeoutMs ?? 3000;
  const verified_at = (opts?.now ?? (() => new Date()))().toISOString();
  const base = {
    id: "ALERTS",
    group: "operations" as const,
    label: "Alert rules wired (rules.yml + Prometheus + Alertmanager)",
    verified_at,
  };

  // Layer 1: rules.yml exists + has ≥minRules entries.
  const rulesPath = path.join(repo, "monitoring/alerts.rules.yml");
  let rules_in_file = 0;
  try {
    const body = await fs.readFile(rulesPath, "utf8");
    rules_in_file = (body.match(/^\s*-\s*alert:\s/gm) ?? []).length;
  } catch {
    return {
      ...base,
      status: "red",
      reason: `${rulesPath} missing — no alert rules to load`,
    };
  }
  if (rules_in_file < minRules) {
    return {
      ...base,
      status: "yellow",
      reason: `${rulesPath} has only ${rules_in_file} alerts (minimum ${minRules})`,
      evidence: { kind: "file", ref: "monitoring/alerts.rules.yml" },
    };
  }

  // Layer 2: Prometheus loaded the rules.
  const ctrl1 = new AbortController();
  const t1 = setTimeout(() => ctrl1.abort(), timeout);
  let rules_loaded = 0;
  try {
    const r = await fetch(`${prom}/api/v1/rules`, { signal: ctrl1.signal });
    if (!r.ok) throw new Error(`prom http ${r.status}`);
    const j = (await r.json()) as {
      data?: { groups?: Array<{ rules?: Array<{ type?: string }> }> };
    };
    for (const g of j.data?.groups ?? []) {
      for (const ru of g.rules ?? []) {
        if (ru.type === "alerting") rules_loaded += 1;
      }
    }
  } catch (e) {
    return {
      ...base,
      status: "yellow",
      reason: `${rules_in_file} rules in repo, but Prometheus ${prom}/api/v1/rules unreachable: ${(e as Error).message.slice(0, 80)}`,
    };
  } finally {
    clearTimeout(t1);
  }
  if (rules_loaded < minRules) {
    return {
      ...base,
      status: "yellow",
      reason: `${rules_in_file} alerts in file but Prometheus reports only ${rules_loaded} loaded`,
    };
  }

  // Layer 3: Alertmanager status.
  const ctrl2 = new AbortController();
  const t2 = setTimeout(() => ctrl2.abort(), timeout);
  let am_ok = false;
  try {
    const r = await fetch(`${am}/api/v2/status`, { signal: ctrl2.signal });
    am_ok = r.ok;
  } catch {
    am_ok = false;
  } finally {
    clearTimeout(t2);
  }
  if (!am_ok) {
    return {
      ...base,
      status: "yellow",
      reason: `${rules_loaded} rules loaded in Prometheus but Alertmanager ${am}/api/v2/status unreachable`,
    };
  }

  return {
    ...base,
    status: "green",
    reason: `${rules_loaded} alert rules loaded in Prometheus + Alertmanager status reachable`,
    evidence: { kind: "endpoint", ref: `${prom}/api/v1/rules + ${am}/api/v2/status` },
  };
}
