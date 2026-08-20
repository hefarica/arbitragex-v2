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
//   - Fallback: after MAX_WS_ERRORS consecutive connect_error events the hook
//     silently degrades to HTTP polling of the edge REST snapshot every
//     POLL_INTERVAL_MS (same shape, minus seq).
//   - R1 compliant: all WS/DOM access inside useEffect; initial render is the
//     (optional) SSR-provided snapshot.
//   - R8 fail-honest: empty `prices` = feed absent — rendered as such by the
//     caller, NEVER padded with fabricated values.
//
// Pure state transitions live in `applyPriceEvent` so they are unit-testable
// without a socket (see usePricesStream.test.ts).

import { useEffect, useRef, useState, useCallback, startTransition } from "react";
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

/** Pure transition: snapshot & update both replace the full map (server never
 * sends deltas), rotating the previous map for direction tracking. Invalid
 * payloads are rejected by returning the previous state (R8 — no partial
 * garbage). Exported for unit tests. */
export function applyPriceEvent(prev: PricesState, evt: unknown): PricesState {
  if (typeof evt !== "object" || evt === null) return prev;
  const e = evt as Partial<PricesEvent>;
  if (typeof e.chain_id !== "number" || e.prices === null || typeof e.prices !== "object") {
    return prev;
  }
  // Defensive re-validation: drop non-finite / non-positive entries (the Rust
  // reader already filters these — belt and suspenders on the boundary).
  const clean: Record<string, number> = {};
  for (const [sym, v] of Object.entries(e.prices)) {
    if (typeof v === "number" && Number.isFinite(v) && v > 0) {
      clean[sym.toUpperCase()] = v;
    }
  }
  return {
    prices: clean,
    prevPrices: prev.prices,
    ts: typeof e.ts === "string" ? e.ts : prev.ts,
    ttlSecs: typeof e.ttl_secs === "number" ? e.ttl_secs : null,
    seq: typeof e.seq === "number" ? e.seq : prev.seq,
  };
}

const MAX_WS_ERRORS = 3;
const POLL_INTERVAL_MS = 4_000;

// ── EDGE-HARD-1 silence watchdog ─────────────────────────────────────────────
// The price stream has a WRITTEN liveness contract: the Rust writers persist
// prices on fixed cadences (price_worker 30s + DexScreener 15s), so a healthy
// room emits a frame at least every ~35s. A connected-but-silent socket means
// something died silently (server bridge, Redis pub/sub, room routing) — the
// transport-level ping/pong CANNOT see this. Recovery ladder: force a
// reconnect (fresh snapshot on subscribe) up to MAX_FORCED_RECONNECTS times;
// if silence persists, degrade to HTTP polling which self-refreshes forever.
export const SILENCE_RECONNECT_MS = 90_000;
export const MAX_FORCED_RECONNECTS = 3;

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

export interface UsePricesStreamResult {
  state: PricesState;
  status: PricesStatus;
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
    initial ? applyPriceEvent(EMPTY_PRICES_STATE, initial) : EMPTY_PRICES_STATE,
  );
  const [status, setStatus] = useState<PricesStatus>("CONNECTING");

  const errorCountRef = useRef(0);
  const usingPollingRef = useRef(false);
  const pollingTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const chainIdRef = useRef(chainId);

  const applyEvent = useCallback((evt: unknown) => {
    startTransition(() => {
      setState((prev) => applyPriceEvent(prev, evt));
    });
  }, []);

  useEffect(() => {
    if (typeof window === "undefined") return;

    // Chain changed mid-session (or no chain yet): drop stale state honestly.
    if (chainId === null) {
      setState(EMPTY_PRICES_STATE);
      setStatus("CONNECTING");
      return;
    }
    chainIdRef.current = chainId;
    // Reset per-chain state so symbols from a previous chain never bleed in.
    setState(EMPTY_PRICES_STATE);
    setStatus("CONNECTING");

    const handle = { dispose: () => {} };

    // ── HTTP polling fallback (edge REST snapshot) ──────────────────────────
    const startPolling = () => {
      if (usingPollingRef.current) return;
      usingPollingRef.current = true;
      setStatus("POLLING");
      const poll = async () => {
        try {
          const chain = chainIdRef.current;
          if (chain === null) return;
          const res = await fetch(`${edgeUrl}/api/prices/live?chain_id=${chain}`, {
            headers: { accept: "application/json" },
            signal: AbortSignal.timeout(POLL_INTERVAL_MS),
            cache: "no-store",
          });
          if (!res.ok) return;
          applyEvent(await res.json());
        } catch {
          // Swallow — status badge already shows "POLLING" (degraded, honest).
        }
      };
      poll();
      pollingTimerRef.current = setInterval(poll, POLL_INTERVAL_MS);
    };

    // ── WebSocket lifecycle (R1: entire body runs after mount) ─────────────
    const socket = io(getWsBaseUrl(), {
      transports: ["websocket", "polling"],
      reconnectionDelayMax: 5_000,
    });
    handle.dispose = () => socket.close();

    // EDGE-HARD-1: last-frame clock + forced-reconnect budget. Arm on
    // connect (snapshot expected within ~1s); every frame re-arms it.
    let lastFrameAt: number | null = null;
    let forcedReconnects = 0;
    let watchdogTimer: ReturnType<typeof setInterval> | null = null;
    const armWatchdog = () => {
      lastFrameAt = Date.now();
    };
    socket.on("connect", () => {
      socket.emit("subscribe:prices", { chain_id: chainIdRef.current });
      armWatchdog();
      if (watchdogTimer === null) {
        watchdogTimer = setInterval(() => {
          if (usingPollingRef.current) return;
          const action = silenceAction(lastFrameAt, forcedReconnects, Date.now());
          if (action === "ok") return;
          if (action === "degrade") {
            console.warn("[PricesStream] silent >" + SILENCE_RECONNECT_MS + "ms after " + forcedReconnects + " forced reconnects — degrading to polling");
            handle.dispose();
            startPolling();
            return;
          }
          forcedReconnects += 1;
          console.warn("[PricesStream] silent >" + SILENCE_RECONNECT_MS + "ms — forcing reconnect (" + forcedReconnects + "/" + MAX_FORCED_RECONNECTS + ")");
          // Full teardown+reconnect: a fresh connect re-subscribes and pulls a
          // fresh snapshot, recovering from ANY silent server/room failure.
          socket.disconnect();
          socket.connect();
        }, 15_000);
      }
    });
    socket.on("prices:snapshot", (evt: unknown) => {
      if (usingPollingRef.current) return; // degraded — ignore WS noise
      setStatus("LIVE");
      armWatchdog();
      forcedReconnects = 0; // a frame proves the path is alive again
      applyEvent(evt);
    });
    socket.on("prices:update", (evt: unknown) => {
      if (usingPollingRef.current) return;
      setStatus("LIVE");
      armWatchdog();
      forcedReconnects = 0;
      applyEvent(evt);
    });
    socket.on("prices:error", () => {
      // Server-side read failure — honest STALE until the next good frame.
      if (!usingPollingRef.current) setStatus("STALE");
    });
    socket.on("connect_error", () => {
      if (usingPollingRef.current) return;
      setStatus("STALE");
      errorCountRef.current += 1;
      if (errorCountRef.current >= MAX_WS_ERRORS) {
        handle.dispose();
        startPolling();
      }
    });

    return () => {
      handle.dispose();
      if (watchdogTimer !== null) {
        clearInterval(watchdogTimer);
      }
      if (pollingTimerRef.current !== null) {
        clearInterval(pollingTimerRef.current);
        pollingTimerRef.current = null;
      }
      usingPollingRef.current = false;
      errorCountRef.current = 0;
    };
  }, [chainId, edgeUrl, applyEvent]);

  return { state, status };
}
