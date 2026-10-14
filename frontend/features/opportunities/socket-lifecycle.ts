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
   * FRONT-04 fix (2026-09-24): the original C4 comment claimed the backend
   * `setupWebSocketGateway` "rejects every handshake without an admin token"
   * — that was FALSE. The gateway is PUBLIC by design (websocket.ts:391-393
   * "Public: allow connection without token..."; WS-POLL-1). The token, when
   * present, only elevates the socket to `runtimeAckAllowed`. This consumer
   * still passes the token through all three channels for that elevation,
   * but MUST NOT assume anonymous connections are rejected.
   *
   *   1. `auth: { token }` — preferred for socket.io v3+
   *   2. `query: { token }` — browser fallback when auth is unavailable
   *   3. `extraHeaders: { 'x-arbx-admin-token': token }` — tooling/curl
   *
   * Empty token → omit entirely (public connection; runtime_ack commands
   * stay unprivileged — the server enforces the real authorization).
   */
  authToken?: string;
  /**
   * G-PRICE-1 (v4 t5): patada opcional del feed de precios. Mismo criterio
   * que onStreamEvent en el parche v4: si el llamador no lo pasa, no se
   * registra listener y el socket observado es idéntico al de antes
   * (0 listeners extra). NOTA: el diff v4 citaba un `onStreamEvent` previo
   * que NO existe en esta base — solo se agregan las líneas marcadas `+`.
   */
  onPriceTick?: (tick: unknown) => void;
  onPricesSnapshot?: (payload: unknown) => void;
}

export interface OpportunitySocketHandle {
  dispose: () => void;
}

// Connection knobs — match what page.tsx already uses, kept as a single source of truth.
const CONNECT_OPTS = { reconnectionAttempts: 5, timeout: 2000 } as const;

export function createOpportunitySocket(
  opts: OpportunitySocketOptions,
): OpportunitySocketHandle {
  const { url, ioFactory, onStatus, onOpportunity, authToken, onPriceTick, onPricesSnapshot } = opts;

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
    // G-PRICE-1 (v4 t5): sala pública de precios — subscribe SIN payload.
    // El dispatcher α11 del servidor lo rutea al brazo v4 (websocket.ts);
    // el brazo legacy per-chain (prices-stream.ts) hace yield de payloads null.
    socket.emit("subscribe:prices");
  };
  const onDisconnect = () => onStatus("STALE");
  const onConnectError = () => onStatus("STALE");
  const onNewOpportunity = (opp: unknown) => onOpportunity(opp as Opportunity);
  const onPrice = (tick: unknown) => onPriceTick?.(tick);
  const onSnapshot = (payload: unknown) => onPricesSnapshot?.(payload);

  socket.on("connect", onConnect);
  socket.on("disconnect", onDisconnect);
  socket.on("connect_error", onConnectError);
  socket.on("new_opportunity", onNewOpportunity);
  if (onPriceTick) socket.on("prices:tick", onPrice);
  if (onPricesSnapshot) socket.on("prices:snapshot", onSnapshot);

  return {
    dispose: () => {
      // PERF (2026-08-10): disconnect() alone does NOT remove user listeners;
      // across reconnects/cleanup they accumulate and retain closures. Off first.
      socket.off("connect", onConnect);
      socket.off("disconnect", onDisconnect);
      socket.off("connect_error", onConnectError);
      socket.off("new_opportunity", onNewOpportunity);
      if (onPriceTick) socket.off("prices:tick", onPrice);
      if (onPricesSnapshot) socket.off("prices:snapshot", onSnapshot);
      socket.disconnect();
    },
  };
}
