// frontend/features/opportunities/socket-lifecycle.ts

import { z } from "zod";

export type WsStatus = "CONNECTING" | "LIVE" | "STALE";

// OMEGA-8 / M5 Fase 8 (P1-WS-1): every WSS frontier must parse with Zod
// before reaching React state — no `as Opportunity` casts. Schema kept
// permissive (passthrough on string ids, optional fields) because the
// scanner pipeline emits enriched fields incrementally (A.8 confidence
// scoring) — but the *shape* of the row we render is locked.
export const WsOpportunitySchema = z
  .object({
    id: z.string().min(1),
    timestamp: z.union([z.number(), z.string()]),
    route: z.string(),
    expected_profit_usd: z.number(),
    net_roi_pct: z.number(),
    score: z.number(),
  })
  .passthrough();

export interface Opportunity {
  id: string;
  timestamp: number | string;
  route: string;
  expected_profit_usd: number;
  net_roi_pct: number;
  score: number;
}

export interface SocketLike {
  on: (event: string, handler: (...args: unknown[]) => void) => unknown;
  emit: (event: string, ...args: unknown[]) => unknown;
  disconnect: () => void;
}

export interface OpportunitySocketOptions {
  url: string;
  ioFactory: (url: string, opts?: Record<string, unknown>) => SocketLike;
  onStatus: (status: WsStatus) => void;
  onOpportunity: (opp: Opportunity) => void;
  /**
   * C4 fix (audit 2026-05-10): backend `setupWebSocketGateway` rejects every
   * handshake without an admin token. Frontend MUST pass the operator's
   * admin token in the auth payload. Three protocol-supported channels are
   * accepted by the backend:
   *
   *   1. `auth: { token }` — preferred for socket.io v3+
   *   2. `query: { token }` — browser fallback when auth is unavailable
   *   3. `extraHeaders: { 'x-arbx-admin-token': token }` — tooling/curl
   *
   * Pass an empty string to skip auth (the backend will reject and the
   * caller should degrade to polling). The constructor never crashes on
   * missing tokens — connection failure surfaces via `onStatus("STALE")`.
   */
  authToken?: string;
}

export interface OpportunitySocketHandle {
  dispose: () => void;
}

// Connection knobs — match what page.tsx already uses, kept as a single source of truth.
const CONNECT_OPTS = { reconnectionAttempts: 5, timeout: 2000 } as const;

export function createOpportunitySocket(
  opts: OpportunitySocketOptions,
): OpportunitySocketHandle {
  const { url, ioFactory, onStatus, onOpportunity, authToken } = opts;

  // C4: assemble the auth payload the backend `extractHandshakeToken` expects.
  // Use ALL three transport channels for compatibility (auth + query + header).
  // Empty token → omit entirely so the backend can return its standard
  // "unauthorized" error rather than seeing an empty-string masquerade.
  const connectOpts: Record<string, unknown> = { ...CONNECT_OPTS };
  if (authToken && authToken.length > 0) {
    connectOpts["auth"] = { token: authToken };
    connectOpts["query"] = { token: authToken };
    connectOpts["extraHeaders"] = { "x-arbx-admin-token": authToken };
  }

  const socket = ioFactory(url, connectOpts);

  socket.on("connect", () => {
    onStatus("LIVE");
    socket.emit("subscribe:opportunities");
  });

  socket.on("disconnect", () => {
    onStatus("STALE");
  });

  socket.on("connect_error", () => {
    onStatus("STALE");
  });

  socket.on("new_opportunity", (opp: unknown) => {
    // OMEGA-8 / M5 Fase 8 (P1-WS-1): safeParse Fail-Closed — a malformed
    // payload MUST NOT reach React state. Drop silently (count via a side
    // channel later) but never cast.
    const parsed = WsOpportunitySchema.safeParse(opp);
    if (!parsed.success) {
      if (typeof console !== "undefined" && typeof console.warn === "function") {
        console.warn(
          "[opportunities-ws] dropped malformed payload:",
          parsed.error.issues.slice(0, 2).map((i) => `${i.path.join(".")}: ${i.message}`).join("; "),
        );
      }
      return;
    }
    onOpportunity(parsed.data);
  });

  return {
    dispose: () => socket.disconnect(),
  };
}
