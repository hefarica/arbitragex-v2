// frontend/features/opportunities/socket-lifecycle.ts

export type WsStatus = "CONNECTING" | "LIVE" | "STALE";

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
  off: (event: string, handler?: (...args: unknown[]) => void) => unknown;
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

  // Stable listeners so we can remove them on dispose (not removeAllListeners,
  // which would nuke Socket.IO internal handlers).
  const onConnect = () => {
    onStatus("LIVE");
    socket.emit("subscribe:opportunities");
  };
  const onDisconnect = () => onStatus("STALE");
  const onConnectError = () => onStatus("STALE");
  const onNewOpportunity = (opp: unknown) => onOpportunity(opp as Opportunity);

  socket.on("connect", onConnect);
  socket.on("disconnect", onDisconnect);
  socket.on("connect_error", onConnectError);
  socket.on("new_opportunity", onNewOpportunity);

  return {
    dispose: () => {
      // PERF (2026-08-10): disconnect() alone does NOT remove user listeners;
      // across reconnects/cleanup they accumulate and retain closures. Off first.
      socket.off("connect", onConnect);
      socket.off("disconnect", onDisconnect);
      socket.off("connect_error", onConnectError);
      socket.off("new_opportunity", onNewOpportunity);
      socket.disconnect();
    },
  };
}
