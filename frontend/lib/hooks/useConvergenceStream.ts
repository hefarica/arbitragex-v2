"use client";
// frontend/lib/hooks/useConvergenceStream.ts
//
// WebSocket stream for SED convergence telemetry.
//
// Architecture:
//   - Connects to Socket.IO room "convergence" via event "subscribe:convergence"
//   - Server emits → "convergence_signal" (ConvergenceSignal payload)
//   - Client emits → "subscribe:convergence" (on connect)
//   - C4 fix compliant: admin token gate on WS handshake via getAdminToken()
//   - Failover: WS → STALE (no HTTP polling fallback — convergence is WS-only)
//   - R8 fail-honest: status "STALE" is surfaced when disconnected

import { useEffect, useState, useRef, startTransition } from "react";
import { io, type Socket } from "socket.io-client";
import { getAdminToken } from "@/lib/admin-token";
import { getWsBaseUrl } from "@/lib/api-client";

// ─── Canonical type mirror ────────────────────────────────────────────────────
// Must stay in sync with ConvergenceSignal in the Rust backend (sed-core).

export interface EntropySnapshot {
  mempool_tx_per_sec: number;
  mempool_avg_gas_price_gwei: number;
  mempool_entropy_score: number;
  reserve_divergence_max: number;
}

export interface ConvergenceSignal {
  entropy_snapshot: EntropySnapshot;
  pipeline_latency_ms: number;
  opportunities_detected: number;
  simulations_run: number;
  simulations_success: number;
  timestamp: string;
  schema_version: number;
}

export type ConvergenceStatus = "CONNECTING" | "LIVE" | "STALE";

export interface UseConvergenceStreamResult {
  signal: ConvergenceSignal | null;
  status: ConvergenceStatus;
}

// ─── Configuration constants ──────────────────────────────────────────────────
const MAX_WS_ERRORS = 3;

// ─── Public API ───────────────────────────────────────────────────────────────
export function useConvergenceStream(): UseConvergenceStreamResult {
  const [signal, setSignal] = useState<ConvergenceSignal | null>(null);
  const [status, setStatus] = useState<ConvergenceStatus>("CONNECTING");
  const errorCountRef = useRef(0);

  // ─── WebSocket lifecycle (R1: entire body runs after mount) ──────────────
  useEffect(() => {
    if (typeof window === "undefined") return;

    const wsUrl = getWsBaseUrl();

    // C4 fix (audit 2026-05-10): the backend WS gateway rejects every
    // handshake without a valid admin token. Read the token from the
    // operator's existing admin session and pass it to the socket auth.
    // When no admin session exists, skip the WS connection entirely and
    // go to STALE — attempting an authless connection produces noisy 401s.
    const adminToken = getAdminToken();
    if (!adminToken) {
      setStatus("STALE");
      return;
    }

    const socket: Socket = io(wsUrl, {
      auth: { token: adminToken },
      transports: ["websocket", "polling"],
    });

    socket.on("connect", () => {
      setStatus("LIVE");
      errorCountRef.current = 0;
      socket.emit("subscribe:convergence");
    });

    socket.on("convergence_signal", (data: ConvergenceSignal) => {
      startTransition(() => {
        setSignal(data);
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

  return { signal, status };
}
