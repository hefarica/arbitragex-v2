// frontend/components/__tests__/RuntimePostureBar.test.tsx
//
// FE-MASTER · FE-0009 — SSR-branch tests for the posture bar (§34/§35).
//
// Contract-bearing surfaces tested directly:
//   - projectChannel — the pure RealtimeSlice → 7-token projection with
//     precedence (disconnected > error > connecting > stale > polling > live);
//   - ConnectionStateChip / PostureChip — closed vocabularies, §40 hints,
//     distinct chip-class+icon pairs, R8 detail semantics;
//   - killswitchPosture / paperPosture — the §35 tripartite (value / error /
//     absent), never conflating "not served" with an error or a value;
//   - the bar itself at INITIAL store state (every channel connecting,
//     posture null → honest "—"), R1-deterministic.
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";

import {
  CHANNEL_COPY,
  CONNECTION_STATE_COPY,
  ConnectionStateChip,
  PostureChip,
  RuntimePostureBar,
  WS_NATIVE_CHANNELS,
  killswitchPosture,
  paperPosture,
  projectChannel,
  socketChipProps,
  type ConnectionState,
} from "../RuntimePostureBar";
import { REALTIME_CHANNELS, type RealtimeChannelState } from "@/lib/store/realtime-slices";
import type { KillSwitchState, PaperModeState } from "@/lib/schemas";

// ─── store mock: faithful to the slice's INITIAL state (blank channels) ─────
// The bar only reads `channels` + `wsConnected`; renderToStaticMarkup never
// runs effects, so no fetch escapes. Mutable so scenario tests can vary it.

interface MockStore {
  channels: Record<string, RealtimeChannelState>;
  wsConnected: boolean;
}

const store: MockStore = vi.hoisted(() => ({
  channels: {},
  wsConnected: false,
}));

vi.mock("@/lib/store/omni-store", () => ({
  useOmniStore: (sel: (s: MockStore) => unknown) => sel(store),
}));

function blank(): RealtimeChannelState {
  return { transport: "rest", status: "connecting", lastMessageAt: null, lastError: null };
}
for (const id of REALTIME_CHANNELS) store.channels[id] = blank();

// ─── fixtures ────────────────────────────────────────────────────────────────

const ch = (patch: Partial<RealtimeChannelState>): RealtimeChannelState => ({ ...blank(), ...patch });

const KILL_ON: KillSwitchState = {
  enabled: true,
  reason: "operator halt",
  triggered_by: "hefarica",
  updated_at: "2026-08-24T00:00:00Z",
};

const KILL_OFF: KillSwitchState = { ...KILL_ON, enabled: false, reason: null, triggered_by: null };

const PAPER_ON: PaperModeState = {
  enabled: true,
  chain_id: 1,
  source: "explicit",
  confidence: "explicit",
  degraded: false,
  conflict: false,
  updated_at: "2026-08-24T00:00:00Z",
  reasons: [],
  chains: [],
};

const PAPER_OFF: PaperModeState = { ...PAPER_ON, enabled: false };

// ─── projectChannel — the §34 projection ─────────────────────────────────────

describe("projectChannel — RealtimeChannelState → §34 token", () => {
  it("maps every raw status to its token", () => {
    expect(projectChannel("routes", ch({ transport: "ws", status: "live" }))).toBe("LIVE");
    expect(projectChannel("pairs", ch({ status: "connecting" }))).toBe("CONNECTING");
    expect(projectChannel("routes", ch({ status: "stale" }))).toBe("STALE");
    expect(projectChannel("routes", ch({ status: "disconnected" }))).toBe("DISCONNECTED");
    expect(projectChannel("routes", ch({ status: "polling", transport: "rest" }))).toBe("DEGRADED");
    expect(projectChannel("pairs", ch({ status: "polling", transport: "rest" }))).toBe("POLLING");
    expect(projectChannel("quote_anchor", ch({ status: "polling", transport: "rest" }))).toBe("POLLING");
  });

  it("DEGRADED is reserved for WS-native surfaces on REST fallback (polling split)", () => {
    expect(WS_NATIVE_CHANNELS).toEqual(["routes", "runtime_ack"]);
    for (const id of REALTIME_CHANNELS) {
      const token = projectChannel(id, ch({ status: "polling", transport: "rest" }));
      if (WS_NATIVE_CHANNELS.includes(id)) expect(token).toBe("DEGRADED");
      else expect(token).toBe("POLLING");
    }
  });

  it("lastError ⇒ ERROR (cleared only by the next accepted payload)", () => {
    expect(projectChannel("routes", ch({ status: "live", lastError: "schema_reject: x" }))).toBe("ERROR");
  });

  it("precedence: disconnected beats error; error beats stale and connecting", () => {
    expect(projectChannel("routes", ch({ status: "disconnected", lastError: "x" }))).toBe("DISCONNECTED");
    expect(projectChannel("routes", ch({ status: "stale", lastError: "x" }))).toBe("ERROR");
    expect(projectChannel("routes", ch({ status: "connecting", lastError: "x" }))).toBe("ERROR");
  });
});

// ─── socketChipProps — WO-08 R8 honest aggregate for the "socket" chip ───────
//
// The socket chip must never read LIVE while a subsystem chip reads
// CONNECTING/DISCONNECTED: LIVE requires the shared connection up AND every
// subsystem connected (LIVE, or POLLING on a REST-native surface — its
// normal cadence). Any demoted subsystem collapses the chip to that worst
// real state, naming each one in the detail.

describe("socketChipProps — LIVE only with EVERY subsystem connected (WO-08, R8)", () => {
  const allConnected = (): Record<string, RealtimeChannelState> => ({
    routes: ch({ transport: "ws", status: "live", lastMessageAt: "2026-08-24T00:00:00Z" }),
    runtime_ack: ch({ transport: "ws", status: "live", lastMessageAt: "2026-08-24T00:00:00Z" }),
    pairs: ch({ transport: "rest", status: "live", lastMessageAt: "2026-08-24T00:00:00Z" }),
    quote_anchor: ch({ transport: "rest", status: "live", lastMessageAt: "2026-08-24T00:00:00Z" }),
  });

  it("socket transport down ⇒ DISCONNECTED even if channels claim live (precondition)", () => {
    expect(socketChipProps(false, allConnected())).toEqual({
      state: "DISCONNECTED",
      detail: "the single socket.io connection is down",
    });
  });

  it("socket up + every subsystem connected ⇒ LIVE, no detail", () => {
    expect(socketChipProps(true, allConnected())).toEqual({ state: "LIVE", detail: null });
  });

  it("POLLING on a REST-native surface is a connected state (does not demote)", () => {
    const channels = allConnected();
    channels.pairs = ch({ transport: "rest", status: "polling", lastMessageAt: "2026-08-24T00:00:00Z" });
    channels.quote_anchor = ch({ transport: "rest", status: "polling", lastMessageAt: "2026-08-24T00:00:00Z" });
    expect(socketChipProps(true, channels)).toEqual({ state: "LIVE", detail: null });
  });

  it("socket up + subsystems still CONNECTING ⇒ CONNECTING naming them (never LIVE)", () => {
    const channels = allConnected();
    channels.pairs = ch({ status: "connecting" }); // lastMessageAt null — real boot state
    channels.quote_anchor = ch({ status: "connecting" });
    expect(socketChipProps(true, channels)).toEqual({
      state: "CONNECTING",
      detail: "pairs=CONNECTING, quote_anchor=CONNECTING",
    });
  });

  it("a DISCONNECTED subsystem beats CONNECTING (worst-first precedence)", () => {
    const channels = allConnected();
    channels.runtime_ack = ch({ transport: "rest", status: "disconnected" });
    channels.pairs = ch({ status: "connecting" });
    expect(socketChipProps(true, channels).state).toBe("DISCONNECTED");
  });

  it("DEGRADED / STALE / ERROR subsystem tokens demote the aggregate to themselves", () => {
    const degraded = allConnected();
    degraded.routes = ch({ transport: "rest", status: "polling", lastMessageAt: "2026-08-24T00:00:00Z" });
    expect(socketChipProps(true, degraded).state).toBe("DEGRADED");

    const stale = allConnected();
    stale.routes = ch({ transport: "ws", status: "stale", lastMessageAt: "2026-08-24T00:00:00Z" });
    expect(socketChipProps(true, stale).state).toBe("STALE");

    const errored = allConnected();
    errored.routes = ch({ transport: "ws", status: "live", lastError: "schema_reject: x" });
    expect(socketChipProps(true, errored).state).toBe("ERROR");
  });
});

// ─── ConnectionStateChip — closed §34 vocabulary ──────────────────────────────

const STATES: ConnectionState[] = [
  "LIVE",
  "CONNECTING",
  "DEGRADED",
  "POLLING",
  "STALE",
  "DISCONNECTED",
  "ERROR",
];

describe("ConnectionStateChip — §34 closed vocabulary", () => {
  it("renders every token with its label and canonical hint (§40)", () => {
    for (const state of STATES) {
      const html = renderToStaticMarkup(
        React.createElement(ConnectionStateChip, { state }),
      );
      expect(html).toContain(`>${CONNECTION_STATE_COPY[state].label}<`);
      expect(html).toContain(CONNECTION_STATE_COPY[state].hint);
    }
  });

  it("the vocabulary is EXACTLY seven tokens (drift alarm)", () => {
    expect(Object.keys(CONNECTION_STATE_COPY).sort()).toEqual([...STATES].sort());
  });

  it("each token picks a DISTINCT chip class + icon pair", () => {
    const combos = new Set<string>();
    for (const state of STATES) {
      const html = renderToStaticMarkup(
        React.createElement(ConnectionStateChip, { state }),
      );
      const m = html.match(/rounded-full border [^"]*/);
      expect(m, `token ${state} must have a chip class`).not.toBeNull();
      const icon = html.match(/lucide-([a-z0-9-]+)/);
      expect(icon, `token ${state} must have an icon`).not.toBeNull();
      combos.add(`${m![0]!}|${icon![1]!}`);
    }
    expect(combos.size).toBe(STATES.length);
  });

  it("R8: detail null = canonical hint only (no ' — '); a detail rides verbatim", () => {
    const plain = renderToStaticMarkup(
      React.createElement(ConnectionStateChip, { state: "ERROR" }),
    );
    expect(plain.match(/ — /g)?.length ?? 0).toBe(0);
    const withDetail = renderToStaticMarkup(
      React.createElement(ConnectionStateChip, { state: "ERROR", detail: "schema_reject: detector_mask admitted" }),
    );
    expect(withDetail).toContain("schema_reject: detector_mask admitted");
  });
});

// ─── posture derivation — the §35 tripartite ────────────────────────────────

describe("killswitchPosture — value / error / absent", () => {
  it("absent (null, no error) = muted dash, never an error and never a value", () => {
    const p = killswitchPosture(null, null);
    expect(p.label).toBe("KILL SWITCH —");
    expect(p.tone).toBe("muted");
    expect(p.detail).toBeUndefined();
  });

  it("ON = bad with reason detail; OFF = ok", () => {
    expect(killswitchPosture(KILL_ON, null)).toMatchObject({
      tone: "bad",
      label: "KILL SWITCH ON",
      detail: "operator halt",
    });
    const off = killswitchPosture(KILL_OFF, null);
    expect(off).toMatchObject({ tone: "ok", label: "KILL SWITCH OFF" });
    expect(off.detail ?? null).toBeNull(); // optional key: absent or null, never a value
  });

  it("error rides verbatim as '?' — never silently becomes absent", () => {
    const p = killswitchPosture(null, "edge 503");
    expect(p.label).toBe("KILL SWITCH ?");
    expect(p.detail).toBe("edge 503");
  });
});

describe("paperPosture — value / error / absent", () => {
  it("absent = muted dash; ON = warn with source+confidence; OFF = bad (fail-safe posture is prominent)", () => {
    expect(paperPosture(null, null)).toMatchObject({ tone: "muted", label: "PAPER MODE —" });
    expect(paperPosture(PAPER_ON, null)).toMatchObject({
      tone: "warn",
      label: "PAPER MODE ON",
      detail: "source explicit · confidence explicit",
    });
    expect(paperPosture(PAPER_OFF, null)).toMatchObject({
      tone: "bad",
      label: "PAPER MODE OFF",
    });
  });

  it("error rides verbatim as '?'", () => {
    expect(paperPosture(null, "timeout")).toMatchObject({
      label: "PAPER MODE ?",
      detail: "timeout",
    });
  });

  it("PostureChip renders label + hint verbatim with the detail appended", () => {
    const p = paperPosture(PAPER_ON, null);
    const html = renderToStaticMarkup(React.createElement(PostureChip, { ...p }));
    expect(html).toContain("PAPER MODE ON");
    expect(html).toContain("source explicit · confidence explicit");
  });
});

// ─── the bar at INITIAL state (what SSR + first client render produce) ───────

describe("RuntimePostureBar — initial render (R1)", () => {
  it("renders both posture dash chips, the socket chip and all four channels CONNECTING", () => {
    const html = renderToStaticMarkup(React.createElement(RuntimePostureBar));
    expect(html).toContain("KILL SWITCH —");
    expect(html).toContain("PAPER MODE —");
    expect(html).toContain(">socket<");
    expect(html).toContain("DISCONNECTED"); // socket at initial wsConnected=false
    for (const id of REALTIME_CHANNELS) {
      expect(html).toContain(`>${CHANNEL_COPY[id].label}<`);
    }
    // 4 channel chips + 1 socket chip, all CONNECTING at initial state.
    expect(html.match(/>CONNECTING</g)?.length).toBe(REALTIME_CHANNELS.length);
  });

  it("a channel with an error renders ERROR and the error string rides in its title", () => {
    store.channels.routes = ch({ transport: "ws", status: "live", lastError: "schema_reject: tick.funnel" });
    const html = renderToStaticMarkup(React.createElement(RuntimePostureBar));
    expect(html).toContain(">ERROR<");
    expect(html).toContain("schema_reject: tick.funnel");
    store.channels.routes = blank();
  });

  it("WO-08: socket chip aggregates subsystems — no LIVE socket over CONNECTING channels", () => {
    // Real production state observed by informe /goal §5: socket.io transport
    // up while pairs/quote_anchor never left `connecting`. The socket chip must
    // show the degraded truth and name the subsystems in its title.
    store.wsConnected = true;
    store.channels.routes = ch({ transport: "ws", status: "live" });
    store.channels.runtime_ack = ch({ transport: "ws", status: "live" });
    const html = renderToStaticMarkup(React.createElement(RuntimePostureBar));
    expect(html).toContain("pairs=CONNECTING, quote_anchor=CONNECTING");

    // All subsystems connected → socket chip back to LIVE (detail null).
    store.channels.pairs = ch({ transport: "rest", status: "live", lastMessageAt: "2026-08-24T00:00:00Z" });
    store.channels.quote_anchor = ch({ transport: "rest", status: "live", lastMessageAt: "2026-08-24T00:00:00Z" });
    const healthy = renderToStaticMarkup(React.createElement(RuntimePostureBar));
    expect(healthy).not.toContain("not connected");
    expect(healthy.match(/>LIVE</g)?.length).toBe(REALTIME_CHANNELS.length + 1); // socket + 4 channels

    // restore the shared mutable mock for the tests that follow
    store.wsConnected = false;
    for (const id of REALTIME_CHANNELS) store.channels[id] = blank();
  });

  it("two renders of the same state are byte-identical (no clock, no random)", () => {
    const a = renderToStaticMarkup(React.createElement(RuntimePostureBar));
    const b = renderToStaticMarkup(React.createElement(RuntimePostureBar));
    expect(a).toBe(b);
  });

  it("carries role=status with an aria-label", () => {
    const html = renderToStaticMarkup(React.createElement(RuntimePostureBar));
    expect(html).toContain('role="status"');
    expect(html).toContain('aria-label="Runtime posture"');
  });
});
