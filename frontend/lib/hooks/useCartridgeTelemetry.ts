"use client";
// frontend/lib/hooks/useCartridgeTelemetry.ts
//
// FASE OMEGA — live cartridge telemetry stream (Strategy Forge UI).
//
// Architecture (mirrors useConvergenceStream — RULE 02: WebSocket DIRECT to the
// api-server, NEVER via the edge):
//   - Connects to Socket.IO via NEXT_PUBLIC_WS_URL (api-server :8080)
//   - Client emits "subscribe:telemetry" on connect
//   - Server emits "telemetry" for every `log_quantum` the Rust cartridges publish
//     to the Redis channel `arbx:cartridge:telemetry`
//   - Admin-token gate on the WS handshake (same as every other room)
//   - R8 fail-honest: status "STALE" when disconnected; never fabricates data

import { useEffect, useState, useRef, useCallback, startTransition } from "react";
import { io, type Socket } from "socket.io-client";
import { getAdminToken } from "@/lib/admin-token";

// ─── Type mirror ──────────────────────────────────────────────────────────────
// Matches the Rust `log_quantum` payload and the api-server CartridgeTelemetry
// interface (websocket.ts). Open shape: cartridges may emit arbitrary fields.
export interface CartridgeTelemetry {
  cartridge_id?: string;
  level?: string;
  message?: string;
  ts?: number;
  [k: string]: unknown;
}

export type TelemetryStatus = "CONNECTING" | "LIVE" | "STALE";

export interface UseCartridgeTelemetryResult {
  /** Rolling buffer of the most recent telemetry messages (oldest first, newest last). */
  messages: CartridgeTelemetry[];
  /** The single most recent message, or null before any arrives. */
  latest: CartridgeTelemetry | null;
  status: TelemetryStatus;
  /** Clear the in-memory buffer (e.g. a "clear log" button in the Forge UI). */
  clear: () => void;
}

// ─── Configuration constants ──────────────────────────────────────────────────
const MAX_WS_ERRORS = 3;
// Bound the in-memory buffer — telemetry is a high-rate stream (one per log_quantum).
const MAX_BUFFER = 200;

// ─── Public API ───────────────────────────────────────────────────────────────
export function useCartridgeTelemetry(): UseCartridgeTelemetryResult {
  const [messages, setMessages] = useState<CartridgeTelemetry[]>([]);
  const [status, setStatus] = useState<TelemetryStatus>("CONNECTING");
  const errorCountRef = useRef(0);

  const clear = useCallback(() => setMessages([]), []);

  // ─── WebSocket lifecycle (R1: entire body runs after mount) ──────────────
  useEffect(() => {
    if (typeof window === "undefined") return;

    const wsUrl = process.env.NEXT_PUBLIC_WS_URL ?? "http://localhost:3000";

    // Admin-token gate: the backend WS gateway rejects every handshake without a
    // valid admin token. When no admin session exists, go to STALE rather than
    // spamming 401s with an authless connection.
    const adminToken = getAdminToken();
    if (!adminToken) {
      setStatus("STALE");
      return;
    }

    // query.token (alongside auth.token) lets the WS pass the edge /socket.io
    // upgrade gate (browsers can't set WS headers; auth.token is in the engine.io
    // handshake, not the raw upgrade — the edge reads ?token). See
    // useRouteDiscoveryTelemetry for the full rationale.
    const socket: Socket = io(wsUrl, {
      auth: { token: adminToken },
      query: { token: adminToken },
      transports: ["websocket", "polling"],
    });

    socket.on("connect", () => {
      setStatus("LIVE");
      errorCountRef.current = 0;
      socket.emit("subscribe:telemetry");
    });

    socket.on("telemetry", (data: CartridgeTelemetry) => {
      startTransition(() => {
        setMessages((prev) => {
          // Drop the oldest entries once the buffer is full (ring-buffer semantics).
          const base =
            prev.length >= MAX_BUFFER ? prev.slice(prev.length - MAX_BUFFER + 1) : prev;
          return [...base, data];
        });
      });
    });

    socket.on("connect_error", () => {
      errorCountRef.current += 1;
      if (errorCountRef.current >= MAX_WS_ERRORS) {
        setStatus("STALE");
        socket.disconnect();
      }
    });

    socket.on("disconnect", () => {
      setStatus("STALE");
    });

    return () => {
      socket.disconnect();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const latest = messages.length > 0 ? messages[messages.length - 1]! : null;
  return { messages, latest, status, clear };
}
