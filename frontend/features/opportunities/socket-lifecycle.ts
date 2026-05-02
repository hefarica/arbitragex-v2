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
  emit: (event: string, ...args: unknown[]) => unknown;
  disconnect: () => void;
}

export interface OpportunitySocketOptions {
  url: string;
  ioFactory: (url: string, opts?: Record<string, unknown>) => SocketLike;
  onStatus: (status: WsStatus) => void;
  onOpportunity: (opp: Opportunity) => void;
}

export interface OpportunitySocketHandle {
  dispose: () => void;
}

// Connection knobs — match what page.tsx already uses, kept as a single source of truth.
const CONNECT_OPTS = { reconnectionAttempts: 5, timeout: 2000 } as const;

export function createOpportunitySocket(
  opts: OpportunitySocketOptions,
): OpportunitySocketHandle {
  const { url, ioFactory, onStatus, onOpportunity } = opts;

  const socket = ioFactory(url, CONNECT_OPTS);

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
    onOpportunity(opp as Opportunity);
  });

  return {
    dispose: () => socket.disconnect(),
  };
}
