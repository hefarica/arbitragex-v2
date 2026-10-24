"use client";
// frontend/lib/hooks/usePricesStream.ts
//
// G-PRICE-1 — exchange-style snapshot+push USD price stream.
//
// Architecture (mirrors useOpportunitiesStream / FE-1):
//   - Connects to the api-server WS gateway (direct, RULE 02 — never via Edge).
//   - Emits `subscribe:prices` with `{ chain_id }`; the server immediately
//     replies `prices:snapshot` (full map) and then pushes `prices:update`
//     (full-map replace) every time a Rust writer persists prices.
//   - Fallback: MAX_WS_ERRORS consecutive connect_error events — or a handshake
//     that never completes — degrade to HTTP polling of the edge REST snapshot
//     every POLL_INTERVAL_MS (same shape, minus seq). Degradation is NOT
//     permanent: while polling, the lifecycle probes the WebSocket again every
//     POLL_PROBE_MS and returns to it as soon as a probe delivers an accepted
//     frame. Every degradation, poll failure and probe outcome is logged with
//     its reason — nothing is swallowed.
//   - EDGE-HARD-1 watchdog supervises TWO DISTINCT states, each with its own
//     deadline and budget, sampled every WATCHDOG_TICK_MS:
//       · "connected but silent"   → SILENCE_RECONNECT_MS / MAX_FORCED_RECONNECTS
//       · "handshake pending"      → PENDING_ATTEMPT_DEADLINE_MS, with NO
//                                    disconnect while the attempt is in flight
//     It is armed OUTSIDE `connect`, so a socket that never connects is still
//     covered, and it never tears down a handshake that is still in flight.
//   - P0-b: every effect run owns a generation. A late frame — or a late poll
//     response — from a superseded generation (previous chain) is dropped by the
//     generation guard and by the chain check inside `applyPriceEvent`.
//   - R1 compliant: all WS/DOM access inside useEffect; initial render is the
//     (optional) SSR-provided snapshot.
//   - R8 fail-honest: empty `prices` = feed absent — rendered as such by the
//     caller, NEVER padded with fabricated values.
//
// Pure state transitions live in `applyPriceEvent` so they are unit-testable
// without a socket (see usePricesStream.test.ts).

import { useEffect, useRef, useState, startTransition } from "react";
import { io } from "socket.io-client";
import { getWsBaseUrl } from "@/lib/api-client";

/** Server wire shape — must match api-server `PricesSnapshot` (prices-stream.ts). */
export interface PricesEvent {
  chain_id: number;
  prices: Record<string, number>;
  count: number;
  ttl_secs: number | null;
  ts: string;
  seq: number;
}

export type PricesStatus = "CONNECTING" | "LIVE" | "STALE" | "POLLING";

export interface PricesState {
  prices: Record<string, number>;
  /** Previous map — lets the ticker paint per-symbol direction (▲/▼). */
  prevPrices: Record<string, number>;
  ts: string | null;
  ttlSecs: number | null;
  seq: number;
}

export const EMPTY_PRICES_STATE: PricesState = {
  prices: {},
  prevPrices: {},
  ts: null,
  ttlSecs: null,
  seq: 0,
};

/** P0-b boundary gate — the ONLY place a wire frame is validated. Returns the
 * cleaned price map, or `null` when the frame must be rejected.
 *
 * `expectedChainId` (null = no chain filter) is the chain guard: a frame that
 * belongs to a DIFFERENT chain is rejected, so a response arriving after the
 * active chain changed can never be written into the new chain's state. */
function framePrices(
  evt: unknown,
  expectedChainId: number | null,
): Record<string, number> | null {
  if (typeof evt !== "object" || evt === null) return null;
  const e = evt as Partial<PricesEvent>;
  if (typeof e.chain_id !== "number") return null;
  if (expectedChainId !== null && e.chain_id !== expectedChainId) return null;
  if (e.prices === null || typeof e.prices !== "object") return null;
  // Defensive re-validation: drop non-finite / non-positive entries (the Rust
  // reader already filters these — belt and suspenders on the boundary).
  const clean: Record<string, number> = {};
  for (const [sym, v] of Object.entries(e.prices)) {
    if (typeof v === "number" && Number.isFinite(v) && v > 0) {
      clean[sym.toUpperCase()] = v;
    }
  }
  return clean;
}

/** True only for a frame that `applyPriceEvent` would actually ACCEPT for
 * `chainId`. Liveness is credited on this predicate — never on arrival. */
export function acceptPriceFrame(evt: unknown, chainId: number | null): boolean {
  return framePrices(evt, chainId) !== null;
}

/** Pure transition: snapshot & update both replace the full map (server never
 * sends deltas), rotating the previous map for direction tracking. Invalid
 * payloads are rejected by returning the previous state (R8 — no partial
 * garbage). Exported for unit tests.
 *
 * `expectedChainId` defaults to null (no chain filter) so existing callers keep
 * their exact behaviour; the hook always passes the active chain. */
export function applyPriceEvent(
  prev: PricesState,
  evt: unknown,
  expectedChainId: number | null = null,
): PricesState {
  const clean = framePrices(evt, expectedChainId);
  if (clean === null) return prev;
  const e = evt as Partial<PricesEvent>;
  return {
    prices: clean,
    prevPrices: prev.prices,
    ts: typeof e.ts === "string" ? e.ts : prev.ts,
    ttlSecs: typeof e.ttl_secs === "number" ? e.ttl_secs : null,
    seq: typeof e.seq === "number" ? e.seq : prev.seq,
  };
}

/** Consecutive `connect_error` budget before degrading (§2 defect 4). */
export const MAX_WS_ERRORS = 3;
/** REST poll cadence (fallback loop). */
export const POLL_INTERVAL_MS = 4_000;

// REST poll deadline. The original code used `AbortSignal.timeout(POLL_INTERVAL_MS)`
// — the deadline was IDENTICAL to the cadence, so a snapshot slower than the
// interval was aborted exactly when the next request started, and the bare
// `catch {}` swallowed it: with a REST latency ≥ 4s that is a silent no-op, ZERO
// updates while the badge reads "POLLING". The two values are now different by
// construction and every failure is logged instead of swallowed.
//
// This is NOT required to stay below POLL_INTERVAL_MS: overlap is already
// prevented by the `pollInFlight` guard, so the deadline no longer has to fit
// inside one cadence. Keeping it below the interval would abort snapshots that
// could have completed between 3s and 4s — a self-inflicted failure. 8s gives a
// slow-but-healthy response room to land while still bounding a hung one.
//
// Exported so the separation is checkable as an EXPRESSION:
//   POLL_TIMEOUT_MS !== POLL_INTERVAL_MS
export const POLL_TIMEOUT_MS = 8_000;

// ── EDGE-HARD-1 watchdog ─────────────────────────────────────────────────────
// The price stream has a WRITTEN liveness contract: the Rust writers persist
// prices on fixed cadences (price_worker 30s + DexScreener 15s), so a healthy
// room emits a frame at least every ~35s. A connected-but-silent socket means
// something died silently (server bridge, Redis pub/sub, room routing) — the
// transport-level ping/pong CANNOT see this. Recovery ladder: force a
// reconnect (fresh snapshot on subscribe) up to MAX_FORCED_RECONNECTS times;
// if silence persists, degrade to HTTP polling. Polling is not a terminus: the
// probe loop below returns to the WebSocket when the transport is healthy again.
export const SILENCE_RECONNECT_MS = 90_000;
export const MAX_FORCED_RECONNECTS = 3;

/** How often the watchdog evaluates the stream (contract §1 P0-a pin 1).
 * A tick is a SAMPLING period, not a deadline: `setInterval` guarantees neither
 * exact execution nor that two ticks are one deadline apart. */
export const WATCHDOG_TICK_MS = 15_000;
/** socket.io's per-ATTEMPT connect timeout, passed EXPLICITLY to `io()` (main
 * socket and probe). Socket.IO's own default is 20_000 ms; declaring it keeps the
 * library's attempt window and ours from drifting apart silently, and makes the
 * invariant below checkable as an expression rather than assumed. */
export const CONNECT_TIMEOUT_MS = 20_000;
/** Deadline for ONE handshake (reconnect) ATTEMPT, measured from the moment it
 * started (contract §1 P0-a pin 2). At the limit the watchdog could otherwise
 * close an attempt socket.io still considers alive — hence BOTH invariants, which
 * are checkable as expressions:
 *   PENDING_ATTEMPT_DEADLINE_MS > WATCHDOG_TICK_MS        (≥1 full tick of grace)
 *   PENDING_ATTEMPT_DEADLINE_MS >= CONNECT_TIMEOUT_MS + 5_000  (≥5s of margin over
 *                                                              the library's own window)
 * Deliberately NOT a reuse of SILENCE_RECONNECT_MS (pin 5): a connected-but-silent
 * stream and an in-flight handshake are two different clocks by design. */
export const PENDING_ATTEMPT_DEADLINE_MS = 25_000;
/** While polling, how often to probe the WebSocket for the return path. */
const POLL_PROBE_MS = 60_000;

export type SilenceAction = "ok" | "reconnect" | "degrade";

/** Pure decision for the watchdog — exported for unit tests. */
export function silenceAction(
  lastFrameAtMs: number | null,
  forcedReconnects: number,
  nowMs: number,
  silenceMs: number = SILENCE_RECONNECT_MS,
  maxForced: number = MAX_FORCED_RECONNECTS,
): SilenceAction {
  if (lastFrameAtMs === null) return "ok"; // no frame yet — connect flow owns it
  const silentFor = nowMs - lastFrameAtMs;
  if (silentFor < silenceMs) return "ok";
  if (forcedReconnects < maxForced) return "reconnect";
  return "degrade";
}

// ── P0-a: two-phase watchdog decision ────────────────────────────────────────
// `silenceAction` above is the pure silence ladder and is retained verbatim for
// its existing contract. It cannot express the defect it used to be embedded in:
// it knows nothing about a HANDSHAKE IN FLIGHT, so a forced reconnect that has
// not completed yet looks exactly like a connected-but-silent stream and gets
// torn down again on the next tick. `watchdogAction` adds the phase, and with it
// the guard: while a handshake is pending the only reachable transitions are
// "wait" and "declare it dead" — never "disconnect".
export type PricesStreamPhase = "connecting" | "connected" | "polling";

export interface WatchdogSnapshot {
  phase: PricesStreamPhase;
  /** Timestamp of the last ACCEPTED frame, or null if none since connecting. */
  lastFrameAt: number | null;
  /** When the current handshake attempt started; null once connected. */
  attemptStartedAt: number | null;
  /** Forced reconnects already spent on the current silence episode. */
  forcedReconnects: number;
}

export type WatchdogDecision =
  | { kind: "idle" }
  | { kind: "reconnect" }
  | { kind: "degrade"; reason: "silence" | "never-connected" };

/** Pure two-phase watchdog — exported for unit tests. */
export function watchdogAction(
  s: WatchdogSnapshot,
  nowMs: number,
  silenceMs: number = SILENCE_RECONNECT_MS,
  maxForced: number = MAX_FORCED_RECONNECTS,
  pendingAttemptDeadlineMs: number = PENDING_ATTEMPT_DEADLINE_MS,
): WatchdogDecision {
  // Already degraded — the poll probe owns recovery from here.
  if (s.phase === "polling") return { kind: "idle" };

  // HANDSHAKE PENDING (P0-a guard). "reconnect" is unreachable in this branch,
  // so the watchdog cannot call disconnect() over an in-flight handshake. A
  // handshake that has not completed within its own deadline is dead, and
  // degradation to polling is the honest response — this is also what covers a
  // socket that never connects at all.
  if (s.phase === "connecting") {
    if (s.attemptStartedAt === null) return { kind: "idle" };
    if (nowMs - s.attemptStartedAt >= pendingAttemptDeadlineMs) {
      return { kind: "degrade", reason: "never-connected" };
    }
    return { kind: "idle" };
  }

  // CONNECTED: supervise silence, with its own deadline and its own budget.
  if (s.lastFrameAt === null) return { kind: "idle" };
  if (nowMs - s.lastFrameAt < silenceMs) return { kind: "idle" };
  if (s.forcedReconnects < maxForced) return { kind: "reconnect" };
  return { kind: "degrade", reason: "silence" };
}

export interface UsePricesStreamResult {
  state: PricesState;
  status: PricesStatus;
}

// ── Ruta A — injectable lifecycle factory ────────────────────────────────────
// Precedent of the house: frontend/features/opportunities/socket-lifecycle.ts
// (ioFactory injection + a handle with dispose()). Same shape here, carrying the
// four guards this hook needed:
//
//   1. GENERATION GUARD (P0-b) — every write is gated by `isCurrent()`. A frame,
//      poll response or timer belonging to a superseded effect run (previous
//      chain) can never write into the active run's state.
//   2. PHASE SEPARATION (P0-a) — "connected but silent" and "handshake pending"
//      are different phases with different deadlines and budgets; the watchdog
//      never disconnects a handshake that is still in flight.
//   3. SINGLE CHOKE POINT — `enterPolling()` is the only path into degradation.
//      Both triggers (watchdog + connect_error budget) funnel through it, so the
//      live socket is closed at most once and no two mechanisms race.
//   4. RETURN PATH — while polling, a probe re-attempts the WebSocket every
//      POLL_PROBE_MS; the first accepted probe frame ends the degradation.
//
// Zero new dependencies: socket factory, clock, fetch and logger are all
// injected, so the whole lifecycle is unit-testable in the `node` environment.

export interface PricesSocketLike {
  on(event: string, handler: (...args: unknown[]) => void): unknown;
  emit(event: string, ...args: unknown[]): unknown;
  disconnect(): void;
  connect(): void;
  close(): void;
}

/** The subset of `Response` the poll path needs. */
export interface PricesFetchResponse {
  ok: boolean;
  status: number;
  json: () => Promise<unknown>;
}

export interface PricesLifecycleLog {
  /** Human-readable line, always prefixed with [PricesStream]. */
  message: string;
  detail?: unknown;
}

export interface PricesLifecycleDeps {
  /** WS gateway URL (RULE 02 — api-server direct, never via Edge). */
  url: string;
  /** Edge base URL for the REST fallback (RULE 02 — REST via Edge). */
  edgeUrl: string;
  /** Active chain. Frames from any other chain are rejected (P0-b). */
  chainId: number;
  /** P0-b generation guard: false ⇒ this effect run was superseded, drop writes. */
  isCurrent: () => boolean;
  ioFactory: (url: string, opts?: Record<string, unknown>) => PricesSocketLike;
  now: () => number;
  fetchImpl: (url: string, init: RequestInit) => Promise<PricesFetchResponse>;
  onStatus: (status: PricesStatus) => void;
  /** Called ONLY with frames that passed validation for `chainId`. */
  onFrame: (evt: unknown) => void;
  /** Defaults to `console.warn`/`console.error` — injectable for tests. */
  log?: (level: "warn" | "error", entry: PricesLifecycleLog) => void;
}

export interface PricesLifecycleHandle {
  dispose: () => void;
  /** Current phase — exposed for diagnostics and tests. */
  phase: () => PricesStreamPhase;
}

/** Default logger: production observability without a logging dependency.
 * `console.warn` is already how this module reported watchdog events. */
function defaultLog(level: "warn" | "error", entry: PricesLifecycleLog): void {
  const fn = level === "error" ? console.error : console.warn;
  if (entry.detail === undefined) fn(entry.message);
  else fn(entry.message, entry.detail);
}

function describeError(err: unknown): string {
  if (err instanceof Error) return err.name + ": " + err.message;
  return String(err);
}

/** Adapter so the real socket.io client satisfies `PricesSocketLike` without
 * leaking socket.io's wide overload set into the lifecycle contract. */
function realIoFactory(url: string, opts?: Record<string, unknown>): PricesSocketLike {
  const factory = io as unknown as (
    u: string,
    o?: Record<string, unknown>,
  ) => PricesSocketLike;
  return factory(url, opts);
}

export function createPricesLifecycle(
  deps: PricesLifecycleDeps,
): PricesLifecycleHandle {
  const {
    url,
    edgeUrl,
    chainId,
    isCurrent,
    ioFactory,
    now,
    fetchImpl,
    onStatus,
    onFrame,
  } = deps;
  const log = deps.log ?? defaultLog;

  let disposed = false;
  let phase: PricesStreamPhase = "connecting";
  let socket: PricesSocketLike | null = null;
  /** Silence clock: timestamp of the last ACCEPTED frame. */
  let frameClockAt: number | null = null;
  /** Handshake clock: when the CURRENT attempt started (null once connected). */
  let attemptStartedAt: number | null = now();
  let forcedReconnects = 0;
  /** Consecutive connect_error count — reset by a completed handshake or an
   * accepted frame, i.e. exactly what "consecutive" means in this file. */
  let consecutiveConnectErrors = 0;

  let watchdogTimer: ReturnType<typeof setInterval> | null = null;
  let pollTimer: ReturnType<typeof setInterval> | null = null;
  let probeTimer: ReturnType<typeof setInterval> | null = null;
  let pollInFlight = false;
  // F-RV1: whether the CURRENTLY ACTIVE socket reopens itself after a drop. The
  // main socket does (socket.io default); a promoted probe does NOT, because it
  // was created with `reconnection: false`. Without this flag the disconnect
  // handler would assume a retry that nobody ever starts, and the phase clock
  // would supervise a handshake that does not exist.
  let activeSocketSelfReconnects = true;
  let probe: { socket: PricesSocketLike; deadline: ReturnType<typeof setTimeout> } | null = null;
  /** In-flight poll requests, aborted on dispose so a late response can never
   * resolve into a superseded run (P0-b, second half). */
  const inFlight = new Set<AbortController>();

  /** The generation guard: every write in this factory goes through `alive()`. */
  const alive = () => !disposed && isCurrent();

  const stopWatchdog = () => {
    if (watchdogTimer !== null) {
      clearInterval(watchdogTimer);
      watchdogTimer = null;
    }
  };

  // ── Frame acceptance: the ONLY place liveness is credited ─────────────────
  function acceptSocketFrame(
    evt: unknown,
    source: "ws" | "probe",
    from: PricesSocketLike,
  ): boolean {
    if (!alive()) return false;
    // The frame must come from the socket that currently owns the stream: after
    // a degradation `socket` is null, so a late frame from the closed socket is
    // dropped here (and a stale probe can never promote itself).
    if (source === "ws" && socket !== from) return false;
    if (source === "probe" && (probe === null || probe.socket !== from)) return false;
    if (!acceptPriceFrame(evt, chainId)) {
      // A frame that is invalid, or belongs to another chain, is NOT evidence of
      // liveness: it is dropped and reported, and it neither marks LIVE nor
      // re-arms the watchdog budget.
      log("warn", {
        message: "[PricesStream] rejected frame — not applied, no liveness credit",
        detail: { source, chainId },
      });
      return false;
    }
    // Accepted: apply the state transition FIRST, then credit liveness.
    onFrame(evt);
    if (source === "probe") {
      promoteProbe(from);
      return true;
    }
    frameClockAt = now();
    attemptStartedAt = null;
    forcedReconnects = 0; // an accepted frame proves the path is alive again
    consecutiveConnectErrors = 0;
    phase = "connected";
    onStatus("LIVE");
    return true;
  }

  // ── Polling ───────────────────────────────────────────────────────────────
  const pollOnce = async (): Promise<void> => {
    if (!alive() || phase !== "polling" || pollInFlight) return;
    pollInFlight = true;
    const ctrl = new AbortController();
    inFlight.add(ctrl);
    let timedOut = false;
    const deadline = setTimeout(() => {
      timedOut = true;
      ctrl.abort();
    }, POLL_TIMEOUT_MS);
    try {
      const res = await fetchImpl(
        edgeUrl + "/api/prices/live?chain_id=" + chainId,
        {
          headers: { accept: "application/json" },
          signal: ctrl.signal,
          cache: "no-store",
        },
      );
      if (!alive()) return;
      if (!res.ok) {
        // Observable, not swallowed: the badge says POLLING, the log says why
        // that POLLING is producing no updates.
        log("warn", {
          message: "[PricesStream] poll rejected — HTTP " + res.status,
          detail: { chainId, status: res.status },
        });
        return;
      }
      const body = await res.json();
      if (!alive()) return;
      if (!acceptPriceFrame(body, chainId)) {
        log("warn", {
          message: "[PricesStream] poll returned a frame that failed validation",
          detail: { chainId },
        });
        return;
      }
      onFrame(body);
    } catch (err) {
      // Aborted by dispose/disconnect ⇒ not a failure, stay quiet. Anything else
      // is logged with its cause: the previous bare `catch {}` made a dead
      // fallback indistinguishable from a healthy one.
      if (!alive()) return;
      log("warn", {
        message: timedOut
          ? "[PricesStream] poll timed out after " + POLL_TIMEOUT_MS + "ms — no update this tick"
          : "[PricesStream] poll failed — no update this tick",
        detail: { chainId, timedOut, error: describeError(err) },
      });
    } finally {
      clearTimeout(deadline);
      inFlight.delete(ctrl);
      pollInFlight = false;
    }
  };

  const stopPolling = () => {
    if (pollTimer !== null) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
  };

  const startPolling = () => {
    if (pollTimer !== null) return;
    onStatus("POLLING");
    void pollOnce();
    pollTimer = setInterval(() => {
      void pollOnce();
    }, POLL_INTERVAL_MS);
  };

  // ── Return path: probe the WS while degraded ──────────────────────────────
  const destroyProbe = () => {
    if (probe === null) return;
    const current = probe;
    probe = null;
    clearTimeout(current.deadline);
    current.socket.close();
  };

  const startProbeLoop = () => {
    if (probeTimer !== null) return;
    probeTimer = setInterval(() => {
      if (!alive() || phase !== "polling" || probe !== null) return;
      // One-shot probe: `reconnection: false` so a failed probe cannot leave a
      // manager churning in the background. A promoted probe stays
      // non-self-reconnecting on purpose — recovery is owned by exactly ONE
      // loop (this one), which is what keeps the mechanisms from racing.
      const probeSocket = ioFactory(url, {
        transports: ["websocket", "polling"],
        reconnection: false,
        timeout: CONNECT_TIMEOUT_MS,
      });
      const deadline = setTimeout(() => {
        if (probe !== null && probe.socket === probeSocket) {
          log("warn", {
            message: "[PricesStream] WS probe did not complete in " + PENDING_ATTEMPT_DEADLINE_MS + "ms — staying on polling",
            detail: { chainId },
          });
          destroyProbe();
        }
      }, PENDING_ATTEMPT_DEADLINE_MS);
      probe = { socket: probeSocket, deadline };
      bindSocket(probeSocket, "probe");
      probeSocket.connect();
    }, POLL_PROBE_MS);
  };

  const promoteProbe = (promoted: PricesSocketLike) => {
    if (!alive() || phase !== "polling") return;
    if (probe === null || probe.socket !== promoted) {
      promoted.close();
      return;
    }
    const current = probe;
    probe = null;
    clearTimeout(current.deadline);
    if (probeTimer !== null) {
      clearInterval(probeTimer);
      probeTimer = null;
    }
    stopPolling();
    socket = promoted;
    // F-RV1: the probe was built with `reconnection: false` — it will not reopen
    // itself, so recovery for this socket is ours to start (see the disconnect
    // handler below).
    activeSocketSelfReconnects = false;
    frameClockAt = now();
    attemptStartedAt = null;
    forcedReconnects = 0;
    consecutiveConnectErrors = 0;
    phase = "connected";
    log("warn", {
      message: "[PricesStream] WebSocket healthy again — leaving polling",
      detail: { chainId },
    });
    onStatus("LIVE");
    startWatchdog();
  };

  // ── Degradation choke point ───────────────────────────────────────────────
  const enterPolling = (reason: "silence" | "never-connected" | "connect-errors") => {
    if (!alive() || phase === "polling") return;
    phase = "polling";
    stopWatchdog();
    const closing = socket;
    socket = null;
    if (closing !== null) closing.close(); // closed at most once, by construction
    log("warn", {
      message: "[PricesStream] degrading to HTTP polling (" + reason + ")",
      detail: { chainId, reason },
    });
    startPolling();
    startProbeLoop();
  };

  // ── Socket wiring ─────────────────────────────────────────────────────────
  function bindSocket(target: PricesSocketLike, source: "ws" | "probe"): void {
    target.on("connect", () => {
      if (!alive()) return;
      if (source === "probe") {
        // Handshake done — but a probe is NOT liveness yet: it stays on polling
        // until it delivers an accepted frame (acceptSocketFrame → promoteProbe).
        target.emit("subscribe:prices", { chain_id: chainId });
        return;
      }
      target.emit("subscribe:prices", { chain_id: chainId });
      // The handshake clock is replaced by the silence clock, which is what the
      // watchdog supervises from here on. `forcedReconnects` is deliberately NOT
      // reset here: a reconnect that never delivers a frame must still exhaust
      // its budget, otherwise connect-without-frames would loop forever.
      phase = "connected";
      attemptStartedAt = null;
      frameClockAt = now();
      consecutiveConnectErrors = 0;
      startWatchdog();
    });
    target.on("prices:snapshot", (evt: unknown) => {
      acceptSocketFrame(evt, source, target);
    });
    target.on("prices:update", (evt: unknown) => {
      acceptSocketFrame(evt, source, target);
    });
    target.on("prices:error", (payload: unknown) => {
      if (!alive() || source === "probe") return;
      // The payload carries the server-side read failure — report it instead of
      // discarding it, so "STALE" is explainable.
      log("warn", {
        message: "[PricesStream] server reported a price read failure (prices:error)",
        detail: { chainId, payload },
      });
      if (phase !== "polling") onStatus("STALE");
    });
    target.on("disconnect", (reason: unknown) => {
      // Only the ACTIVE socket is supervised, and by IDENTITY — not by `source`.
      // A PROMOTED probe keeps source === "probe" while BEING the active socket,
      // so a source-based guard would leave it completely unattended (F-RV1).
      if (!alive() || target !== socket) return;
      // Respect the LOCAL intent of the close: degrading and cleanup already own
      // the status, so they must not be overwritten by a spurious STALE.
      if (phase === "polling") return;
      // Unexpected drop. The phase goes back to "connecting" with a fresh attempt
      // clock, so the watchdog supervises the HANDSHAKE (its own deadline)
      // instead of tearing it down.
      phase = "connecting";
      attemptStartedAt = now();
      onStatus("STALE");
      log("warn", {
        message: "[PricesStream] socket disconnected — awaiting reconnect",
        detail: { chainId, reason },
      });
      // F-RV1: the main socket retries on its own, but a promoted probe was built
      // with `reconnection: false` and will NOT reopen itself. The phase above
      // now claims a pending attempt, so one must ACTUALLY exist — otherwise the
      // watchdog supervises a handshake nobody started until the deadline expires.
      if (!activeSocketSelfReconnects) target.connect();
    });
    target.on("connect_error", (err: unknown) => {
      if (!alive() || source === "probe") return;
      consecutiveConnectErrors += 1;
      // A failed attempt is a handshake phase, not a silent stream.
      if (phase !== "polling") {
        phase = "connecting";
        attemptStartedAt = now();
        onStatus("STALE");
      }
      log("warn", {
        message:
          "[PricesStream] connect_error (" +
          consecutiveConnectErrors +
          "/" +
          MAX_WS_ERRORS +
          " consecutive)",
        detail: { chainId, error: describeError(err) },
      });
      if (consecutiveConnectErrors >= MAX_WS_ERRORS) enterPolling("connect-errors");
    });
  }

  // ── Watchdog ──────────────────────────────────────────────────────────────
  function startWatchdog(): void {
    if (watchdogTimer !== null) return;
    // Armed from the caller, OUTSIDE `connect` (see hook): a socket that never
    // connects is supervised by the same clock, and its "connecting" phase is
    // what turns patience into degradation.
    watchdogTimer = setInterval(() => {
      if (!alive() || phase === "polling") return;
      const decision = watchdogAction(
        {
          phase,
          lastFrameAt: frameClockAt,
          attemptStartedAt,
          forcedReconnects,
        },
        now(),
      );
      if (decision.kind === "idle") return;
      if (decision.kind === "degrade") {
        enterPolling(decision.reason);
        return;
      }
      // "reconnect" is only reachable from the CONNECTED phase — that is the
      // P0-a guard. Mark the attempt BEFORE tearing down, so the next tick sees
      // a pending handshake and leaves it alone.
      forcedReconnects += 1;
      const target = socket;
      phase = "connecting";
      attemptStartedAt = now();
      log("warn", {
        message:
          "[PricesStream] silent >" +
          SILENCE_RECONNECT_MS +
          "ms — forcing reconnect (" +
          forcedReconnects +
          "/" +
          MAX_FORCED_RECONNECTS +
          ")",
        detail: { chainId },
      });
      if (target !== null) {
        target.disconnect();
        target.connect();
      }
    }, WATCHDOG_TICK_MS);
  }

  // ── Boot ──────────────────────────────────────────────────────────────────
  socket = ioFactory(url, {
    transports: ["websocket", "polling"],
    reconnectionDelayMax: 5_000,
    timeout: CONNECT_TIMEOUT_MS,
  });
  bindSocket(socket, "ws");
  startWatchdog();

  return {
    phase: () => phase,
    dispose: () => {
      if (disposed) return;
      disposed = true;
      stopWatchdog();
      stopPolling();
      if (probeTimer !== null) {
        clearInterval(probeTimer);
        probeTimer = null;
      }
      destroyProbe();
      const closing = socket;
      socket = null;
      if (closing !== null) closing.close();
      for (const ctrl of inFlight) ctrl.abort();
      inFlight.clear();
    },
  };
}

/**
 * @param chainId  chain to stream (e.g. 1). `null` = don't connect (ticker
 *                hidden — honest, not a fabricated feed).
 * @param edgeUrl  edge base URL for the polling fallback (RULE 02 REST→Edge).
 * @param initial  optional SSR snapshot (R1) — same shape as PricesEvent.
 */
export function usePricesStream(
  chainId: number | null,
  edgeUrl: string,
  initial?: PricesEvent | null,
): UsePricesStreamResult {
  const [state, setState] = useState<PricesState>(() =>
    initial ? applyPriceEvent(EMPTY_PRICES_STATE, initial, chainId) : EMPTY_PRICES_STATE,
  );
  const [status, setStatus] = useState<PricesStatus>("CONNECTING");

  // ── P0-b · GENERATION GUARD ────────────────────────────────────────────────
  // Every effect run owns a generation number. A write is allowed only while the
  // run that produced it is still the CURRENT one, so a late frame — or a late
  // REST poll response — from a superseded run (previous chain) can never reach
  // this component's state. The chain check inside `framePrices` is the second,
  // independent half of the same gate.
  const generationRef = useRef(0);

  useEffect(() => {
    // Claim this run's generation BEFORE anything can write.
    const generation = ++generationRef.current;
    const isCurrent = () => generationRef.current === generation;

    if (typeof window === "undefined") return;

    if (chainId === null) {
      // No chain ⇒ don't connect. Honest: the caller hides the ticker instead of
      // being shown a fabricated feed (R8).
      setState(EMPTY_PRICES_STATE);
      setStatus("CONNECTING");
      return;
    }

    // Reset per-chain state so symbols from a previous chain never bleed in.
    setState(EMPTY_PRICES_STATE);
    setStatus("CONNECTING");

    // The whole lifecycle — socket, silence reconnect, degradation, polling,
    // probe return path, abort of in-flight polls — lives in the factory. The
    // hook only supplies the environment and the two generation-gated sinks.
    const lifecycle = createPricesLifecycle({
      url: getWsBaseUrl(), // RULE 02 — WS goes straight to the api-server
      edgeUrl, // RULE 02 — REST fallback goes through the Edge
      chainId,
      isCurrent,
      ioFactory: realIoFactory,
      now: () => Date.now(),
      fetchImpl: (url, init) => fetch(url, init),
      // Only an ACCEPTED (valid + own-chain) frame credits LIVE: the factory calls
      // this after `onFrame`, never on arrival (contract §2 defect 2).
      onStatus: (next) => {
        if (!isCurrent()) return;
        setStatus(next);
      },
      onFrame: (evt) => {
        if (!isCurrent()) return;
        startTransition(() => {
          setState((prev) => applyPriceEvent(prev, evt, chainId));
        });
      },
    });

    return () => {
      lifecycle.dispose();
      // Invalidate this generation the instant it is torn down, so nothing that
      // was already in flight can write before the next run claims its own.
      if (generationRef.current === generation) generationRef.current += 1;
    };
  }, [chainId, edgeUrl]);

  return { state, status };
}
