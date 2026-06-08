"use client";

/**
 * SystemGuardBanner — top-of-page visual barrier asserting that the platform
 * is in paper-only state and capital exposure is structurally zero.
 *
 * Doctrine (CLAUDE.md §24 + arbx-mev-ethics-gate):
 *   - LIVE TRADING: forced OFF until A.4 fork validation + A.5 paper-shadow
 *     + A.6 circuit breakers + A.9 GO/NO-GO formal review all PASS.
 *   - PRIVATE RELAY / SUBMIT / BROADCAST: never wired in this codebase; the
 *     relays-client is a no-op stub. Even if a malicious operator flipped a
 *     flag, no broadcast path exists.
 *   - CAPITAL EXPOSURE: $0 by construction — no signer holds funds in paper
 *     mode (Phase 4 onboarding verifies `signer_zero_balance_verified`).
 *
 * R8 fail-honest data flow:
 *   - getRuntimeStatus → engine_loaded inference per strategy
 *   - getReadiness     → flip_blocked / summary counts
 *   - getScannerHeartbeat → 404 ⇒ "no heartbeat (searcher paused?)"
 *
 * Loading: initial render shows skeleton-equivalent (dimmed badges).
 * Unavailable: badge shows the verbatim source state ("Redis unavailable",
 * "PG 503", "edge timeout 5000ms"); we never fabricate a green check.
 *
 * Polls every 15s — aligns with the edge KV 15s TTL on /api/readiness.
 * Cadence is well below the 4s WebSocket polling fallback used by
 * /opportunities, so the two coexist without storm risk.
 */

import * as React from "react";
import { ShieldOff, ShieldAlert, RefreshCwIcon } from "lucide-react";

import { getRuntimeStatus, getReadiness, getScannerHeartbeat } from "@/lib/api-client";
import { useSystemReadiness } from "@/hooks/useSystemReadiness";
import { cn } from "@/lib/utils";

type LoadState =
  | { kind: "loading" }
  | { kind: "ok"; flipBlocked: boolean | null; engineLoaded: boolean | null; heartbeatFresh: boolean | null; postgres: string; redis: string; hbDetail: string | null }
  | { kind: "error"; detail: string };

const POLL_MS = 15_000;

async function fetchAll(): Promise<LoadState> {
  // Run all three in parallel — none blocks the others. allSettled so a single
  // endpoint failure doesn't black out the banner; we degrade gracefully.
  const [rs, rd, hb] = await Promise.allSettled([
    getRuntimeStatus(1),
    getReadiness(),
    getScannerHeartbeat(1),
  ]);

  let flipBlocked: boolean | null = null;
  let engineLoaded: boolean | null = null;
  let postgres = "unavailable";
  let redis = "unavailable";
  let heartbeatFresh: boolean | null = null;
  let hbDetail: string | null = null;

  if (rs.status === "fulfilled" && rs.value.ok) {
    postgres = rs.value.data.source.postgres;
    redis = rs.value.data.source.redis;
    // engine_loaded is "true if any strategy reports engine_loaded=true"
    engineLoaded = rs.value.data.strategies.some((s) => s.engine_loaded);
  } else if (rs.status === "fulfilled" && !rs.value.ok) {
    postgres = "edge_error";
    redis = "edge_error";
  }

  if (rd.status === "fulfilled" && rd.value.ok) {
    flipBlocked = rd.value.data.flip_blocked;
  }

  if (hb.status === "fulfilled" && hb.value.ok) {
    heartbeatFresh = true;
    hbDetail = null;
  } else if (hb.status === "fulfilled" && !hb.value.ok) {
    heartbeatFresh = false;
    hbDetail = hb.value.error.slice(0, 80);
  }

  return { kind: "ok", flipBlocked, engineLoaded, heartbeatFresh, postgres, redis, hbDetail };
}

export function SystemGuardBanner() {
  // R1: Mounted snapshot pattern — initial server render shows neutral
  // "loading" tokens; the first useEffect tick reads real data.
  const [state, setState] = React.useState<LoadState>({ kind: "loading" });
  const readiness = useSystemReadiness();

  React.useEffect(() => {
    let alive = true;

    const tick = async () => {
      try {
        const next = await fetchAll();
        if (alive) setState(next);
      } catch (e) {
        if (alive) {
          setState({
            kind: "error",
            detail: (e as Error).message?.slice(0, 120) ?? "unknown error",
          });
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

  // Hardcoded structural facts (NOT fabricated metrics — these are
  // declarative statements of immutable code state):
  //   - liveOff = true: there is NO live submission path in this binary.
  //   - relayOff = true: relays-client is a no-op stub in paper mode.
  //   - submitOff / broadcastOff = true: no signer.send_transaction wired.
  //   - paperModeOn = true: ARBX_TRADE_MODE=paper enforced in scanner.
  //   - capital = "$0": Phase 4 verified signer_zero_balance.
  //   - a4Blocked = true: fork validation requires RPC_HTTP_1 + EXECUTOR_1.
  //   - a5Blocked = true: paper-shadow runtime not yet executed.
  // These tiles will become dynamic in P2/P3 (BlockersPanel + GoNoGoPanel).
  return (
    <div
      data-slot="system-guard-banner"
      className="border-b border-destructive/20 bg-destructive/5 px-4 py-2 text-xs lg:px-10"
      role="status"
      aria-live="polite"
    >
      <div className="mx-auto flex max-w-[1800px] flex-wrap items-center gap-x-3 gap-y-1.5 text-foreground/90">
        <span className="inline-flex items-center gap-1.5 font-semibold uppercase tracking-wider text-destructive">
          <ShieldOff className="size-3.5" aria-hidden="true" />
          System guard
        </span>

        <GuardTile label="Live" value="OFF" tone="danger" />
        <GuardTile label="Private relay" value="OFF" tone="danger" />
        <GuardTile label="Submit" value="OFF" tone="danger" />
        <GuardTile label="Broadcast" value="OFF" tone="danger" />
        <GuardTile label="Paper" value="ON" tone="safe" />
        <GuardTile label="Capital" value="$0" tone="safe" />
        <GuardTile label="Readiness" value={`${readiness.completedCount}/${readiness.totalCount}`} tone={readiness.allReady ? "safe" : "warning"} />
        <GuardTile label="LIVE lock" value={readiness.allReady ? "REVIEW" : "LOCKED"} tone={readiness.allReady ? "warning" : "danger"} />
        <GuardTile label="GO live" value="NO-GO" tone="danger" />
        <GuardTile label="A.4 fork" value="BLOCKED" tone="warning" />
        <GuardTile label="A.5 paper-shadow" value="BLOCKED" tone="warning" />

        <span className="ml-auto inline-flex items-center gap-2 text-[11px] text-muted-foreground">
          {renderRuntimeStatus(state)}
        </span>
      </div>
    </div>
  );
}

function GuardTile({
  label,
  value,
  tone,
}: {
  label: string;
  value: string;
  tone: "danger" | "warning" | "safe";
}) {
  return (
    <span
      data-slot="system-guard-tile"
      data-tone={tone}
      className={cn(
        "inline-flex items-center gap-1 rounded-md border px-1.5 py-0.5 font-mono text-[11px] uppercase",
        tone === "danger" && "border-destructive/40 bg-destructive/10 text-destructive",
        tone === "warning" && "border-amber-500/40 bg-amber-500/10 text-amber-600 dark:text-amber-300",
        tone === "safe" && "border-emerald-500/40 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300",
      )}
    >
      <span className="font-sans normal-case text-foreground/60">{label}:</span>
      <span className="font-semibold">{value}</span>
    </span>
  );
}

function renderRuntimeStatus(state: LoadState): React.ReactNode {
  if (state.kind === "loading") {
    return (
      <>
        <RefreshCwIcon className="size-3 animate-spin" aria-hidden="true" />
        <span>Loading runtime…</span>
      </>
    );
  }
  if (state.kind === "error") {
    return (
      <>
        <ShieldAlert className="size-3 text-destructive" aria-hidden="true" />
        <span title={state.detail} className="text-destructive">
          Runtime fetch error
        </span>
      </>
    );
  }
  // ok state
  const pgOk = state.postgres === "ok";
  const redisOk = state.redis === "ok";
  const hbLabel =
    state.heartbeatFresh === true
      ? "fresh"
      : state.heartbeatFresh === false
        ? "stale/absent"
        : "unknown";
  return (
    <>
      <span title={`PG: ${state.postgres}`}>PG {pgOk ? "ok" : state.postgres}</span>
      <span>·</span>
      <span title={`Redis: ${state.redis}`}>Redis {redisOk ? "ok" : state.redis}</span>
      <span>·</span>
      <span title={state.hbDetail ?? ""}>HB {hbLabel}</span>
      <span>·</span>
      <span>
        Engine{" "}
        {state.engineLoaded === null
          ? "?"
          : state.engineLoaded
            ? "loaded"
            : "not loaded"}
      </span>
      <span>·</span>
      <span>
        Flip{" "}
        {state.flipBlocked === null
          ? "?"
          : state.flipBlocked
            ? "blocked"
            : "open"}
      </span>
    </>
  );
}
