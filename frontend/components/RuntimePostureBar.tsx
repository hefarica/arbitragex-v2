"use client";

/**
 * =============================================================================
 * RuntimePostureBar — FE-0009 (FE-MASTER §34/§35, P2-STORE-RT)
 * =============================================================================
 *
 * The always-visible posture strip in the root layout: per-channel connection
 * states over the RealtimeSlice (FE-0008 owns the writes; this bar only
 * READS), plus the runtime posture values that have REAL wires today:
 *
 *   killswitch  GET /api/status → StatusResponse.killswitch (edge-routed)
 *   paper mode  GET /api/paper-mode/state → canonical PaperModeState
 *
 * RULE 00 — nothing here invents a state: the seven-token vocabulary is a
 * PROJECTION of the slice's raw truth (transport/status/lastError), and the
 * projection itself is a pure exported function pinned by tests.
 *
 * R1 — SSR renders the store's INITIAL state (every channel `connecting`,
 * posture values null → honest "—") which is byte-identical to the first
 * client render; every nondeterministic touch (fetch, timers) lives in
 * useEffect. No age math at render time — freshness detail would need
 * Date.now() and belongs to the certifying caller.
 */

// SSR-test support (repo pattern, cf. ResourceHealth/ProvenanceBadge): the
// node test transformer's classic JSX path needs the React namespace.
import * as React from "react";
import { useEffect, useState } from "react";

import {
  AlertTriangle,
  CheckCircle2,
  Clock3,
  Loader2,
  RefreshCw,
  ScrollText,
  ShieldAlert,
  ShieldCheck,
  Unplug,
  XCircle,
  type LucideIcon,
} from "lucide-react";

import { getPaperModeState, getStatus } from "@/lib/api-client";
import { useOmniStore } from "@/lib/store/omni-store";
import {
  REALTIME_CHANNELS,
  type RealtimeChannelId,
  type RealtimeChannelState,
} from "@/lib/store/realtime-slices";
import type { KillSwitchState, PaperModeState } from "@/lib/schemas";

// ─── §34 connection vocabulary (7 tokens, closed) ───────────────────────────

export type ConnectionState =
  | "LIVE"
  | "CONNECTING"
  | "DEGRADED"
  | "POLLING"
  | "STALE"
  | "DISCONNECTED"
  | "ERROR";

/** Channels whose NATIVE contract is a WS room (a REST loop is a fallback). */
export const WS_NATIVE_CHANNELS: readonly RealtimeChannelId[] = [
  "routes",
  "runtime_ack",
];

/**
 * Pure projection RealtimeChannelState → §34 token. Precedence (tested):
 *   disconnected > error > connecting > stale > polling > live
 * `polling` splits by native contract: a WS-native surface polling REST is
 * DEGRADED (the room is down); a REST-native surface polling is just POLLING
 * (its normal cadence — pairs/anchor have no room to lose).
 */
export function projectChannel(
  id: RealtimeChannelId,
  ch: RealtimeChannelState,
): ConnectionState {
  if (ch.status === "disconnected") return "DISCONNECTED";
  if (ch.lastError !== null) return "ERROR";
  if (ch.status === "connecting") return "CONNECTING";
  if (ch.status === "stale") return "STALE";
  if (ch.status === "polling") {
    return WS_NATIVE_CHANNELS.includes(id) ? "DEGRADED" : "POLLING";
  }
  return "LIVE";
}

/**
 * WO-08 (informe /goal §5) — R8 honest aggregate for the "socket" chip. The
 * chip used to read `wsConnected` alone, so the bar showed a green LIVE
 * socket while its own subsystem chips honestly read CONNECTING. LIVE now
 * requires the shared connection up AND every subsystem connected: LIVE, or
 * POLLING on a REST-native surface (pairs/anchor — their normal snapshot
 * cadence). Any subsystem connecting/disconnected/stale/degraded/error
 * demotes the chip to that worst real state, naming each one — never a
 * green socket over grey channels. Precedence mirrors projectChannel:
 * disconnected > error > connecting > stale > degraded.
 */
export function socketChipProps(
  wsConnected: boolean,
  channels: Record<RealtimeChannelId, RealtimeChannelState>,
): { state: ConnectionState; detail: string | null } {
  if (!wsConnected) {
    return {
      state: "DISCONNECTED",
      detail: "the single socket.io connection is down",
    };
  }
  const precedence: readonly ConnectionState[] = [
    "DISCONNECTED",
    "ERROR",
    "CONNECTING",
    "STALE",
    "DEGRADED",
  ];
  let worst: ConnectionState | null = null;
  const demoted: string[] = [];
  for (const id of REALTIME_CHANNELS) {
    const token = projectChannel(id, channels[id]);
    // LIVE, and POLLING on a REST-native surface, are the connected states.
    if (token === "LIVE" || token === "POLLING") continue;
    demoted.push(`${id}=${token}`);
    if (worst === null || precedence.indexOf(token) < precedence.indexOf(worst)) {
      worst = token;
    }
  }
  if (worst === null) return { state: "LIVE", detail: null };
  return { state: worst, detail: demoted.join(", ") };
}

type Tone = { icon: LucideIcon; chip: string; text: string };

const ok = (): Tone => ({
  icon: CheckCircle2,
  chip: "border-primary/30 bg-primary/15 text-primary",
  text: "text-primary",
});
const warn = (icon: LucideIcon): Tone => ({
  icon,
  chip: "border-warning/30 bg-warning/15 text-warning",
  text: "text-warning",
});
const info = (): Tone => ({
  icon: RefreshCw,
  chip: "border-info/30 bg-info/15 text-info",
  text: "text-info",
});
const bad = (icon: LucideIcon): Tone => ({
  icon,
  chip: "border-destructive/30 bg-destructive/15 text-destructive",
  text: "text-destructive",
});
const muted = (icon: LucideIcon): Tone => ({
  icon,
  chip: "border-border bg-muted/70 text-muted-foreground",
  text: "text-muted-foreground",
});

const CONNECTION_TONES: Record<ConnectionState, Tone> = {
  LIVE: ok(),
  CONNECTING: muted(Loader2),
  DEGRADED: warn(AlertTriangle),
  POLLING: info(),
  STALE: warn(Clock3),
  DISCONNECTED: muted(Unplug),
  ERROR: bad(XCircle),
};

/**
 * §34 canonical copy. Hint discipline: no apostrophes (HTML-escaped in the
 * title) and no " — " — that separator is RESERVED for a caller detail.
 */
export const CONNECTION_STATE_COPY: Record<
  ConnectionState,
  { label: string; hint: string }
> = {
  LIVE: {
    label: "LIVE",
    hint: "transport delivering accepted payloads",
  },
  CONNECTING: {
    label: "CONNECTING",
    hint: "mounted, awaiting the first accepted payload",
  },
  DEGRADED: {
    label: "DEGRADED",
    hint: "WS-native surface serving through the REST fallback loop (the room is down)",
  },
  POLLING: {
    label: "POLLING",
    hint: "REST-native surface serving on its snapshot cadence (no WS room exists to lose)",
  },
  STALE: {
    label: "STALE",
    hint: "transport nominally up but no accepted payload within the 3x cadence budget",
  },
  DISCONNECTED: {
    label: "DISCONNECTED",
    hint: "no transport: pre-connect, teardown, or a passive channel whose only transport is down",
  },
  ERROR: {
    label: "ERROR",
    hint: "last delivery attempt failed and nothing has been accepted since (cleared on the next accepted payload)",
  },
};

/** Per-channel label + the surface the operator recognizes (§35). */
export const CHANNEL_COPY: Record<RealtimeChannelId, { label: string; hint: string }> = {
  routes: {
    label: "routes",
    hint: "route_discovery WS room with a REST tick fallback",
  },
  runtime_ack: {
    label: "runtime_ack",
    hint: "runtime_ack WS room (admin capability); passive recorder with no REST fallback by design",
  },
  pairs: {
    label: "pairs",
    hint: "pairs snapshot over REST (no WS room)",
  },
  quote_anchor: {
    label: "quote_anchor",
    hint: "quote anchor snapshot over REST (no WS room)",
  },
};

// ─── chips (pure, exported for direct testing) ──────────────────────────────

interface ChipProps {
  state: ConnectionState;
  detail?: string | null;
}

/** One §34 connection chip — label + icon, hint via title, detail appended. */
export function ConnectionStateChip({ state, detail = null }: ChipProps) {
  const tone = CONNECTION_TONES[state];
  const copy = CONNECTION_STATE_COPY[state];
  const hint = detail ? `${copy.hint} — ${detail}` : copy.hint;
  const Icon = tone.icon;
  return (
    <span
      title={hint}
      className={`inline-flex items-center gap-1 rounded-full border px-2 py-0.5 font-mono text-[10px] font-semibold uppercase tracking-[0.12em] ${tone.chip}`}
    >
      <Icon size={11} strokeWidth={2.4} className={tone.text} aria-hidden />
      {copy.label}
    </span>
  );
}

interface PostureChipProps {
  tone: "ok" | "warn" | "bad" | "muted";
  label: string;
  hint: string;
  detail?: string | null;
}

const POSTURE_TONES: Record<PostureChipProps["tone"], Tone> = {
  ok: { icon: ShieldCheck, chip: "border-primary/30 bg-primary/15 text-primary", text: "text-primary" },
  warn: { icon: ScrollText, chip: "border-warning/30 bg-warning/15 text-warning", text: "text-warning" },
  bad: { icon: ShieldAlert, chip: "border-destructive/30 bg-destructive/15 text-destructive", text: "text-destructive" },
  muted: { icon: ShieldCheck, chip: "border-border bg-muted/70 text-muted-foreground", text: "text-muted-foreground" },
};

/** One §35 posture value chip (killswitch / paper mode). */
export function PostureChip({ tone, label, hint, detail = null }: PostureChipProps) {
  const t = POSTURE_TONES[tone];
  const full = detail ? `${hint} — ${detail}` : hint;
  const Icon = t.icon;
  return (
    <span
      title={full}
      className={`inline-flex items-center gap-1 rounded-full border px-2 py-0.5 font-mono text-[10px] font-semibold uppercase tracking-[0.12em] ${t.chip}`}
    >
      <Icon size={11} strokeWidth={2.4} className={t.text} aria-hidden />
      {label}
    </span>
  );
}

// ─── posture derivation (pure — the §35 tripartite: value / error / absent) ─

/** Killswitch chip props: ON (bad) / OFF (ok) / not-served (muted) / error (bad). */
export function killswitchPosture(
  k: KillSwitchState | null,
  err: string | null,
): PostureChipProps {
  if (err !== null) {
    return {
      tone: "bad",
      label: "KILL SWITCH ?",
      hint: "killswitch state could not be served",
      detail: err,
    };
  }
  if (k === null) {
    return {
      tone: "muted",
      label: "KILL SWITCH —",
      hint: "killswitch state not served yet (absence is a state, not an error)",
    };
  }
  if (k.enabled) {
    return {
      tone: "bad",
      label: "KILL SWITCH ON",
      hint: "global execution halt is engaged; the terminus refuses new executions",
      detail: k.reason ?? k.triggered_by,
    };
  }
  return {
    tone: "ok",
    label: "KILL SWITCH OFF",
    hint: "execution terminus is not halted by the kill switch",
  };
}

/** Paper-mode chip props: ON (warn) / OFF (bad) / not-served (muted) / error (bad). */
export function paperPosture(
  p: PaperModeState | null,
  err: string | null,
): PostureChipProps {
  if (err !== null) {
    return {
      tone: "bad",
      label: "PAPER MODE ?",
      hint: "paper-mode state could not be served",
      detail: err,
    };
  }
  if (p === null) {
    return {
      tone: "muted",
      label: "PAPER MODE —",
      hint: "paper-mode state not served yet (absence is a state, not an error)",
    };
  }
  const detail = `source ${p.source} · confidence ${p.confidence}`;
  if (p.enabled) {
    return {
      tone: "warn",
      label: "PAPER MODE ON",
      hint: "executions resolve against the paper ledger; no broadcast leaves the terminus",
      detail,
    };
  }
  return {
    tone: "bad",
    label: "PAPER MODE OFF",
    hint: "the paper safety gate is disabled at config level; the execution terminus is no longer paper-gated",
    detail,
  };
}

// ─── the bar ────────────────────────────────────────────────────────────────

/** Posture values poll slowly — they change on operator action, not per tick. */
const POSTURE_POLL_MS = 60_000;

interface PostureData {
  killswitch: KillSwitchState | null;
  killswitchError: string | null;
  paper: PaperModeState | null;
  paperError: string | null;
}

const POSTURE_INITIAL: PostureData = {
  killswitch: null,
  killswitchError: null,
  paper: null,
  paperError: null,
};

export function RuntimePostureBar() {
  const channels = useOmniStore((s) => s.channels);
  const wsConnected = useOmniStore((s) => s.wsConnected);
  const [posture, setPosture] = useState<PostureData>(POSTURE_INITIAL);

  useEffect(() => {
    let alive = true;
    const poll = async () => {
      const status = await getStatus();
      const paper = await getPaperModeState();
      if (!alive) return;
      // R8: an error is never silently turned into "no data" — it rides
      // verbatim and the chip shows the failure, not a fabricated value.
      if (status.ok) {
        setPosture((p) => ({ ...p, killswitch: status.data.killswitch, killswitchError: null }));
      } else {
        setPosture((p) => ({ ...p, killswitchError: status.error }));
      }
      if (paper.ok) {
        setPosture((p) => ({ ...p, paper: paper.data, paperError: null }));
      } else {
        setPosture((p) => ({ ...p, paperError: paper.error }));
      }
    };
    void poll();
    const timer = setInterval(() => void poll(), POSTURE_POLL_MS);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, []);

  return (
    <div
      role="status"
      aria-label="Runtime posture"
      className="flex flex-wrap items-center gap-x-3 gap-y-1 border-b border-border/60 bg-muted/30 px-4 py-1 font-mono text-[10px] lg:px-10"
    >
      <PostureChip {...killswitchPosture(posture.killswitch, posture.killswitchError)} />
      <PostureChip {...paperPosture(posture.paper, posture.paperError)} />
      <span
        className="inline-flex items-center gap-1"
        title="the single socket.io connection shared by every WS channel"
      >
        <span className="text-muted-foreground/80">socket</span>
        <ConnectionStateChip {...socketChipProps(wsConnected, channels)} />
      </span>
      {REALTIME_CHANNELS.map((id) => {
        const ch = channels[id];
        return (
          <span key={id} className="inline-flex items-center gap-1" title={CHANNEL_COPY[id].hint}>
            <span className="text-muted-foreground/80">{CHANNEL_COPY[id].label}</span>
            <ConnectionStateChip
              state={projectChannel(id, ch)}
              detail={ch.lastError}
            />
          </span>
        );
      })}
    </div>
  );
}

export default RuntimePostureBar;
