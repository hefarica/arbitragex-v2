"use client";

/**
 * =============================================================================
 * OMEGA Runtime ACK WSS Bridge — Frontend Consumer (PR-2)
 * =============================================================================
 *
 * Closes the dark-channel decoherence opened by PR #73 (backend WSS room
 * `runtime_ack`). Subscribes to the `runtime_ack` room emitted by api-server
 * post-INSERT and bridges the broadcast into the `useActionState` machine
 * (WAITING_RUNTIME_ACK → VERIFIED transition).
 *
 * Zod schema is ISOMORPHIC 1:1 with the backend RuntimeAckSchema at
 * `backend/api-server/src/routes/system-manifest.ts:22-37` (single source of
 * truth lives there; this is a frontier mirror).
 *
 * Invariants:
 *   I-1 Bijection event_id — strict client-side filter on event_id match.
 *   I-2 POST-INSERT only   — backend guarantees emit after INSERT success.
 *   I-3 Schema conservation — Zod fields/regex/enums identical to backend.
 *
 * Risk Gates:
 *   RG-1 Anti-Mock Schema     — safeParse Fail-Closed on malformed payloads.
 *   RG-2 Anti-Correlation Drift — ignore payloads whose event_id differs.
 *   RG-3 Anti-Stale UI        — heartbeat watchdog 60s → status='error'.
 *
 * Doctrine: Conservación de Información cruzando fronteras.
 *
 * @module statemachine/useRuntimeAckSocket
 */

import { useEffect, useRef, useState, useCallback } from "react";
import { io as ioReal, type Socket } from "socket.io-client";
import { z } from "zod";

// ---------------------------------------------------------------------------
// Zod schema — ISOMORPHIC 1:1 with backend RuntimeAckSchema
// system-manifest.ts:22-37
// ---------------------------------------------------------------------------

const HEX64 = /^[0-9a-f]{64}$/;

export const RuntimeAckBroadcastSchema = z.object({
  event_id: z.string().uuid(),
  resource: z.string().min(1).max(64),
  chain_id: z.number().int().positive().nullable(),
  idempotency_key: z.string().min(1).max(256),
  config_hash_before: z.string().regex(HEX64).nullable(),
  config_hash_after: z.string().regex(HEX64),
  worker_id: z.string().min(1).max(128),
  layer: z.enum([
    "api",
    "persistence",
    "redis_pubsub",
    "searcher_rs",
    "arc_swap",
    "frontend_refresh",
    "readiness",
    "audit",
  ]),
  status: z.enum([
    "pending",
    "received",
    "applied",
    "rejected",
    "timeout",
    "failed",
  ]),
  latency_ms: z.number().int().nonnegative().nullable().optional(),
  error: z.string().max(2000).nullable().optional(),
});

export type RuntimeAckBroadcast = z.infer<typeof RuntimeAckBroadcastSchema>;

export type RuntimeAckSocketStatus =
  | "idle"
  | "connecting"
  | "connected"
  | "reconnecting"
  | "disconnected"
  | "error";

// ---------------------------------------------------------------------------
// Socket abstraction (factory-injectable for testability — pattern reused
// from frontend/features/opportunities/socket-lifecycle.ts).
// ---------------------------------------------------------------------------

export interface RuntimeAckSocketLike {
  on: (event: string, handler: (...args: unknown[]) => void) => unknown;
  emit: (event: string, ...args: unknown[]) => unknown;
  disconnect: () => void;
}

export type RuntimeAckIoFactory = (
  url: string,
  opts?: Record<string, unknown>,
) => RuntimeAckSocketLike;

// ---------------------------------------------------------------------------
// Pure factory — fully testable without React.
// ---------------------------------------------------------------------------

export interface CreateRuntimeAckSocketOptions {
  url: string;
  eventId: string;
  ioFactory: RuntimeAckIoFactory;
  authToken?: string;
  /** Default 30000ms — matches backend POST timeout window. */
  timeoutMs?: number;
  /** Default 60000ms — RG-3 stale watchdog. */
  heartbeatStaleMs?: number;
  onStatus: (status: RuntimeAckSocketStatus) => void;
  onAck: (payload: RuntimeAckBroadcast) => void;
  onNack: (reason: string, payload?: RuntimeAckBroadcast) => void;
}

export interface RuntimeAckSocketHandle {
  dispose: () => void;
}

const ACK_STATUSES_OK: ReadonlyArray<RuntimeAckBroadcast["status"]> = [
  "applied",
  "received",
];

const CONNECT_OPTS = {
  reconnectionAttempts: Infinity,
  reconnectionDelay: 1000,
  reconnectionDelayMax: 30000,
  randomizationFactor: 0.2,
  timeout: 5000,
} as const;

export function createRuntimeAckSocket(
  opts: CreateRuntimeAckSocketOptions,
): RuntimeAckSocketHandle {
  const {
    url,
    eventId,
    ioFactory,
    authToken,
    timeoutMs = 30000,
    heartbeatStaleMs = 60000,
    onStatus,
    onAck,
    onNack,
  } = opts;

  let settled = false;
  let timeoutHandle: ReturnType<typeof setTimeout> | null = null;
  let watchdogHandle: ReturnType<typeof setTimeout> | null = null;

  const clearTimers = (): void => {
    if (timeoutHandle !== null) {
      clearTimeout(timeoutHandle);
      timeoutHandle = null;
    }
    if (watchdogHandle !== null) {
      clearTimeout(watchdogHandle);
      watchdogHandle = null;
    }
  };

  const armWatchdog = (): void => {
    if (watchdogHandle !== null) clearTimeout(watchdogHandle);
    watchdogHandle = setTimeout(() => {
      if (!settled) {
        onStatus("error");
      }
    }, heartbeatStaleMs);
  };

  // C4 auth payload (same triple-channel pattern as opportunities socket).
  //
  // V-AT-1 / OMEGA-8 M5 Phase 1: When no explicit ephemeral token is provided,
  // `withCredentials` makes Socket.IO send the httpOnly admin session cookie
  // in the handshake (server-side gating in PR #73 accepts cookie OR header).
  // The token NEVER comes from `NEXT_PUBLIC_*` env (would be baked into the
  // public bundle).
  const connectOpts: Record<string, unknown> = {
    ...CONNECT_OPTS,
    withCredentials: true,
  };
  if (authToken && authToken.length > 0) {
    connectOpts["auth"] = { token: authToken };
    connectOpts["query"] = { token: authToken };
    connectOpts["extraHeaders"] = { "x-arbx-admin-token": authToken };
  }

  // Surface server-side authz failure honestly (Fail-Honest R8) — when the
  // backend emits `error { code:'unauthorized', room:'runtime_ack' }` the UI
  // must NOT silently retry; it must transition to error so the operator can
  // log in / refresh session.
  // (Wiring deferred to socket.on("error") below.)

  onStatus("connecting");
  const socket = ioFactory(url, connectOpts);

  socket.on("connect", () => {
    onStatus("connected");
    socket.emit("subscribe:runtime_ack");
    armWatchdog();
  });

  socket.on("disconnect", () => {
    if (!settled) onStatus("disconnected");
  });

  socket.on("connect_error", () => {
    if (!settled) onStatus("reconnecting");
  });

  socket.on("reconnect_attempt", () => {
    if (!settled) onStatus("reconnecting");
  });

  // M3 P0-2 / M5 Fase X: backend WSS room emits
  //   error { code: 'unauthorized', room: 'runtime_ack' }
  // when the session cookie/token does not pass server-side gating. Surface
  // it fail-honest — no silent retry, no degraded "live" UI.
  socket.on("error", (raw: unknown) => {
    if (settled) return;
    let reason = "unauthorized";
    if (raw && typeof raw === "object") {
      const obj = raw as Record<string, unknown>;
      if (typeof obj["code"] === "string") reason = obj["code"] as string;
      if (typeof obj["message"] === "string") reason = obj["message"] as string;
    }
    settled = true;
    clearTimers();
    onStatus("error");
    onNack(reason);
  });

  socket.on("runtime_ack", (raw: unknown) => {
    armWatchdog();
    const parsed = RuntimeAckBroadcastSchema.safeParse(raw);
    if (!parsed.success) {
      // RG-1 Anti-Mock Schema: Fail-Closed silently. The persistence layer
      // is authoritative; we do not let malformed WSS payloads drive UI.
      return;
    }
    if (parsed.data.event_id !== eventId) {
      // RG-2 Anti-Correlation Drift: this broadcast belongs to a different
      // mutation; ignore. Bijection invariant I-1 enforced here.
      return;
    }
    if (settled) return;
    settled = true;
    clearTimers();
    if (ACK_STATUSES_OK.includes(parsed.data.status)) {
      onAck(parsed.data);
    } else {
      onNack(parsed.data.status, parsed.data);
    }
  });

  timeoutHandle = setTimeout(() => {
    if (!settled) {
      settled = true;
      clearTimers();
      onNack("timeout");
    }
  }, timeoutMs);

  return {
    dispose: () => {
      settled = true;
      clearTimers();
      try {
        socket.disconnect();
      } catch {
        // disconnect must never throw; swallow defensively.
      }
    },
  };
}

// ---------------------------------------------------------------------------
// React hook wrapper.
// ---------------------------------------------------------------------------

export interface UseRuntimeAckSocketOptions {
  /** Null disables connection (use when state is not WAITING_RUNTIME_ACK). */
  eventId: string | null;
  wssUrl?: string;
  authToken?: string;
  timeoutMs?: number;
  heartbeatStaleMs?: number;
  onAck: (payload: RuntimeAckBroadcast) => void;
  onNack: (reason: string, payload?: RuntimeAckBroadcast) => void;
  /** Factory injection point. Defaults to socket.io-client's `io`. */
  ioFactory?: RuntimeAckIoFactory;
}

export interface UseRuntimeAckSocketReturn {
  status: RuntimeAckSocketStatus;
  reconnect: () => void;
}

function resolveWssUrl(provided: string | undefined): string {
  if (provided && provided.length > 0) return provided;
  const envUrl = process.env.NEXT_PUBLIC_WSS_URL;
  if (envUrl && envUrl.length > 0) return envUrl;
  return "";
}

/**
 * Resolve the WSS auth token.
 *
 * SECURITY (V-AT-1 / OMEGA-8 M5 Phase 1): The admin token MUST NEVER be
 * baked into the public bundle via `NEXT_PUBLIC_*` envs. This resolver
 * accepts ONLY an explicit, ephemeral, caller-supplied token (e.g., a
 * short-lived runtime token derived from the httpOnly admin session cookie).
 *
 * When no token is provided, return "" and let the socket connect with
 * `withCredentials` so the httpOnly admin session cookie travels in the
 * Socket.IO handshake. Backend gating (PR #73 `runtime_ack` room) accepts
 * either header-bearer or cookie-session and emits
 * `error { code:'unauthorized' }` on failure — the hook surfaces it to UI.
 */
function resolveAuthToken(provided: string | undefined): string {
  if (provided && provided.length > 0) return provided;
  return "";
}

const defaultIoFactory: RuntimeAckIoFactory = (url, factoryOpts) => {
  return ioReal(url, factoryOpts) as unknown as RuntimeAckSocketLike;
};

export function useRuntimeAckSocket(
  options: UseRuntimeAckSocketOptions,
): UseRuntimeAckSocketReturn {
  const [status, setStatus] = useState<RuntimeAckSocketStatus>("idle");
  const [reconnectTick, setReconnectTick] = useState(0);

  const onAckRef = useRef(options.onAck);
  const onNackRef = useRef(options.onNack);
  onAckRef.current = options.onAck;
  onNackRef.current = options.onNack;

  const { eventId, wssUrl, authToken, timeoutMs, heartbeatStaleMs, ioFactory } =
    options;

  useEffect(() => {
    if (eventId === null) {
      setStatus("idle");
      return;
    }
    const url = resolveWssUrl(wssUrl);
    if (url.length === 0) {
      setStatus("error");
      return;
    }
    const handle = createRuntimeAckSocket({
      url,
      eventId,
      ioFactory: ioFactory ?? defaultIoFactory,
      authToken: resolveAuthToken(authToken),
      timeoutMs,
      heartbeatStaleMs,
      onStatus: setStatus,
      onAck: (payload) => onAckRef.current(payload),
      onNack: (reason, payload) => onNackRef.current(reason, payload),
    });
    return () => {
      handle.dispose();
    };
  }, [eventId, wssUrl, authToken, timeoutMs, heartbeatStaleMs, ioFactory, reconnectTick]);

  const reconnect = useCallback(() => {
    setReconnectTick((t) => t + 1);
  }, []);

  return { status, reconnect };
}
