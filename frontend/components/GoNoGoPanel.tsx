"use client";

/**
 * GoNoGoPanel — Large GO/NO-GO indicator with 4 safety check items.
 *
 * Shows:
 *   - Large colored circle with GO / NO-GO text
 *   - 4 check items with ✅ / ❌ status:
 *       1. Paper Shadow 14d completed
 *       2. Vault initialized (Phase 1 onboarding)
 *       3. Kill-switch is OFF
 *       4. All services UP
 *
 * Derives data from /api/v1/readiness/decision plus supporting
 * endpoints for the individual checks.
 *
 * R8 fail-honest: every check item displays the REAL fetched state.
 * When an endpoint is unavailable the check shows ❌ with the error detail.
 */

import * as React from "react";
import { Shield, AlertTriangle, CheckCircle2, XCircle, Loader2 } from "lucide-react";
import {
  getReadinessDecision,
  getOnboardingStatus,
  getStatus,
} from "@/lib/api-client";
import { cn } from "@/lib/utils";

// ─── Types ───────────────────────────────────────────────────────────────────

interface CheckItem {
  label: string;
  passed: boolean;
  detail: string | null;
}

interface GoNoGoData {
  verdict: "GO" | "NO_GO";
  checks: CheckItem[];
  next_action: string;
  generated_at: string;
}

type LoadState =
  | { kind: "loading" }
  | { kind: "ok"; data: GoNoGoData }
  | { kind: "error"; detail: string };

const POLL_MS = 30_000;

// ─── Fetch helper ────────────────────────────────────────────────────────────

async function fetchGoNoGoData(): Promise<LoadState> {
  // Run all three fetches in parallel — no single failure blacks out the panel.
  const [decisionRes, onboardingRes, statusRes] = await Promise.allSettled([
    getReadinessDecision(),
    getOnboardingStatus(),
    getStatus(),
  ]);

  let verdict: "GO" | "NO_GO" = "NO_GO";
  let nextAction = "";
  let generatedAt = new Date().toISOString();

  // Parse decision
  if (decisionRes.status === "fulfilled" && decisionRes.value.ok) {
    const d = decisionRes.value.data;
    verdict = d.verdict === "GO" ? "GO" : "NO_GO";
    nextAction = d.next_action;
    generatedAt = d.generated_at;
  } else if (decisionRes.status === "fulfilled" && !decisionRes.value.ok) {
    return { kind: "error", detail: `Decision API: ${decisionRes.value.error}` };
  } else {
    return { kind: "error", detail: `Decision API failed: ${(decisionRes.reason as Error).message}` };
  }

  // Check 1: Paper Shadow 14d — inferred from decision (go_a5 implies A.5 PASS)
  const paperShadowPassed = decisionRes.status === "fulfilled"
    && decisionRes.value.ok
    && decisionRes.value.data.go_a5;

  // Check 2: Vault initialized (Phase 1 onboarding completed)
  let vaultInitialized = false;
  let vaultDetail: string | null = null;
  if (onboardingRes.status === "fulfilled" && onboardingRes.value.ok) {
    const o = onboardingRes.value.data;
    vaultInitialized = o.phase_1_completed_at !== null && o.phase_1_vault_sealed_healthy;
    vaultDetail = vaultInitialized
      ? `Completed by ${o.phase_1_completed_by} at ${new Date(o.phase_1_completed_at!).toLocaleDateString()}`
      : "Phase 1 not completed";
  } else if (onboardingRes.status === "fulfilled" && !onboardingRes.value.ok) {
    vaultDetail = `Error: ${onboardingRes.value.error}`;
  } else {
    vaultDetail = `Error: ${(onboardingRes.reason as Error).message}`;
  }

  // Check 3: Kill-switch is OFF
  let killswitchOff = false;
  let ksDetail: string | null = null;
  if (statusRes.status === "fulfilled" && statusRes.value.ok) {
    const s = statusRes.value.data;
    killswitchOff = !(s.killswitch?.enabled ?? false);
    ksDetail = s.killswitch?.enabled
      ? `ENABLED: ${s.killswitch.reason ?? "no reason"}`
      : "Kill-switch is disarmed";
  } else if (statusRes.status === "fulfilled" && !statusRes.value.ok) {
    ksDetail = `Error: ${statusRes.value.error}`;
  } else {
    ksDetail = `Error: ${(statusRes.reason as Error).message}`;
  }

  // Check 4: All services UP
  let allServicesUp = false;
  let svcDetail: string | null = null;
  if (statusRes.status === "fulfilled" && statusRes.value.ok) {
    const s = statusRes.value.data;
    const services = Object.entries(s.services);
    const failedServices = services.filter(([, v]) => !v.ok);
    allServicesUp = failedServices.length === 0;
    svcDetail = allServicesUp
      ? `${services.length} services healthy`
      : `${failedServices.length} service(s) down: ${failedServices.map(([k]) => k).join(", ")}`;
  } else if (statusRes.status === "fulfilled" && !statusRes.value.ok) {
    svcDetail = `Error: ${statusRes.value.error}`;
  } else {
    svcDetail = `Error: ${(statusRes.reason as Error).message}`;
  }

  const checks: CheckItem[] = [
    {
      label: "Paper Shadow 14d",
      passed: paperShadowPassed,
      detail: paperShadowPassed
        ? "A.5 paper-shadow PASS"
        : "A.5 paper-shadow not yet completed (need 14 consecutive green days)",
    },
    { label: "Vault initialized", passed: vaultInitialized, detail: vaultDetail },
    { label: "Kill-switch off", passed: killswitchOff, detail: ksDetail },
    { label: "All services UP", passed: allServicesUp, detail: svcDetail },
  ];

  // Override verdict: if any check fails, it's NO-GO
  const anyFailed = checks.some((c) => !c.passed);
  if (anyFailed && verdict === "GO") {
    verdict = "NO_GO";
  }

  return {
    kind: "ok",
    data: {
      verdict,
      checks,
      next_action: nextAction || (anyFailed ? "Resolve failing checks before GO." : "All checks passed."),
      generated_at: generatedAt,
    },
  };
}

// ─── Component ───────────────────────────────────────────────────────────────

export function GoNoGoPanel() {
  const [state, setState] = React.useState<LoadState>({ kind: "loading" });

  React.useEffect(() => {
    let alive = true;

    const tick = async () => {
      try {
        const next = await fetchGoNoGoData();
        if (alive) setState(next);
      } catch (e) {
        if (alive) {
          setState({ kind: "error", detail: (e as Error).message ?? "unknown error" });
        }
      }
    };

    void tick();
    const id = setInterval(tick, POLL_MS);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, []);

  // ─── Loading ─────────────────────────────────────────────────────────────

  if (state.kind === "loading") {
    return (
      <div className="rounded-lg border border-border bg-card p-4 shadow-sm">
        <div className="flex items-center gap-2 text-sm font-semibold text-card-foreground">
          <Loader2 className="size-4 animate-spin text-muted-foreground" aria-hidden="true" />
          GO / NO-GO
        </div>
        <div className="mt-4 flex flex-col items-center gap-3">
          <div className="size-20 animate-pulse rounded-full bg-muted" />
          <div className="h-4 w-32 animate-pulse rounded bg-muted" />
        </div>
        <div className="mt-4 space-y-2">
          {Array.from({ length: 4 }).map((_, i) => (
            <div key={i} className="h-4 w-full animate-pulse rounded bg-muted" />
          ))}
        </div>
      </div>
    );
  }

  // ─── Error ───────────────────────────────────────────────────────────────

  if (state.kind === "error") {
    return (
      <div className="rounded-lg border border-destructive/30 bg-destructive/5 p-4 shadow-sm">
        <div className="flex items-center gap-2 text-sm font-semibold text-destructive">
          <AlertTriangle className="size-4" aria-hidden="true" />
          GO / NO-GO
        </div>
        <div className="mt-4 flex flex-col items-center gap-2">
          <div className="flex size-20 items-center justify-center rounded-full border-4 border-destructive/30 bg-destructive/10">
            <XCircle className="size-10 text-destructive" />
          </div>
          <span className="text-lg font-bold text-destructive">ERROR</span>
        </div>
        <p className="mt-3 text-xs text-destructive/80">{state.detail}</p>
      </div>
    );
  }

  // ─── OK ──────────────────────────────────────────────────────────────────

  const { data } = state;
  const isGo = data.verdict === "GO";
  const allPassed = data.checks.every((c) => c.passed);

  return (
    <div className="rounded-lg border border-border bg-card p-4 shadow-sm">
      {/* Header */}
      <div className="flex items-center gap-2 text-sm font-semibold text-card-foreground">
        <Shield className="size-4 text-muted-foreground" aria-hidden="true" />
        GO / NO-GO
      </div>

      {/* Large indicator */}
      <div className="mt-4 flex flex-col items-center gap-2">
        <div
          className={cn(
            "flex size-24 items-center justify-center rounded-full border-4 transition-colors duration-500",
            isGo
              ? "border-emerald-500 bg-emerald-500/10"
              : allPassed
                ? "border-amber-500 bg-amber-500/10"
                : "border-red-500 bg-red-500/10",
          )}
        >
          {isGo ? (
            <CheckCircle2 className="size-12 text-emerald-500" aria-hidden="true" />
          ) : (
            <XCircle className="size-12 text-red-500" aria-hidden="true" />
          )}
        </div>
        <span
          className={cn(
            "text-2xl font-extrabold tracking-wider",
            isGo
              ? "text-emerald-600 dark:text-emerald-400"
              : "text-red-600 dark:text-red-400",
          )}
        >
          {data.verdict}
        </span>
      </div>

      {/* Check items */}
      <ul className="mt-4 space-y-2">
        {data.checks.map((check) => (
          <li
            key={check.label}
            className="flex items-start gap-2 rounded-md border border-border bg-muted/20 px-2.5 py-2"
            title={check.detail ?? undefined}
          >
            <span className="mt-0.5 shrink-0">
              {check.passed ? (
                <CheckCircle2
                  className="size-4 text-emerald-500"
                  aria-hidden="true"
                />
              ) : (
                <XCircle
                  className="size-4 text-red-500"
                  aria-hidden="true"
                />
              )}
            </span>
            <div className="min-w-0">
              <span
                className={cn(
                  "text-xs font-medium",
                  check.passed
                    ? "text-emerald-700 dark:text-emerald-300"
                    : "text-red-700 dark:text-red-300",
                )}
              >
                {check.passed ? "✅" : "❌"} {check.label}
              </span>
              {check.detail && (
                <p className="mt-0.5 truncate text-[11px] text-muted-foreground">
                  {check.detail}
                </p>
              )}
            </div>
          </li>
        ))}
      </ul>

      {/* Next action */}
      {data.next_action && (
        <p className="mt-3 text-xs text-muted-foreground">
          <strong>Next:</strong> {data.next_action}
        </p>
      )}

      {/* Timestamp */}
      <p className="mt-2 text-[11px] text-muted-foreground">
        Updated: {new Date(data.generated_at).toLocaleTimeString()}
      </p>
    </div>
  );
}
