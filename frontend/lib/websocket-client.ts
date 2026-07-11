// frontend/lib/websocket-client.ts
// FASE OMEGA — Cliente WebSocket para streaming de oportunidades hot path

import { io, Socket } from "socket.io-client";

export interface HotOpportunity {
  id: string;
  chain_id?: string;
  strategy_kind?: string;
  detected_at_ms?: string;
  timestamp_ms?: string;
  status?: string;
  net_profit_wei?: string;
  gas_used?: string;
  _stream_id?: string;
}

export interface HotOpportunityEvent {
  id: string;
  chain_id?: string;
  strategy_kind?: string;
  detected_at_ms?: string;
  timestamp_ms?: string;
  status?: "passed" | "failed";
  net_profit_wei?: string;
  gas_used?: string;
}

export interface WebSocketClientOptions {
  url: string;
  token: string;
  logger?: Console;
}

/**
 * Cliente WebSocket para recibir oportunidades hot path en tiempo real.
 * Conecta al namespace /ws/hot-opportunities y se suscribe al room 'opportunities'.
 *
 * @example
 * ```typescript
 * const client = new HotOpportunityWebSocket({
 *   url: "ws://localhost:8080",
 *   token: process.env.ARBX_ADMIN_TOKEN!,
 * });
 * client.onDetected((opp) => console.log("Detected:", opp));
 * client.onValidated((opp) => console.log("Validated:", opp));
 * client.connect();
 * ```
 */
export class HotOpportunityWebSocket {
  private socket: Socket | null = null;
  private opts: WebSocketClientOptions;
  private onDetectedCallbacks: ((opp: HotOpportunityEvent) => void)[] = [];
  private onValidatedCallbacks: ((opp: HotOpportunityEvent) => void)[] = [];
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 5;
  private reconnectDelay = 1000;

  constructor(opts: WebSocketClientOptions) {
    this.opts = opts;
  }

  connect(): void {
    this.socket = io(this.opts.url, {
      auth: { token: this.opts.token },
      transports: ["websocket"],
      reconnection: true,
      reconnectionDelay: this.reconnectDelay,
      reconnectionDelayMax: 5000,
    });

    this.socket.on("connect", () => {
      this.opts.logger?.log("[HotOpportunityWebSocket] Connected");
      this.reconnectAttempts = 0;
      // Subscribe to opportunities room
      this.socket?.emit("subscribe:opportunities");
    });

    this.socket.on("disconnect", (reason) => {
      this.opts.logger?.log("[HotOpportunityWebSocket] Disconnected:", reason);
    });

    this.socket.on("connect_error", (err) => {
      this.opts.logger?.error("[HotOpportunityWebSocket] Connection error:", err.message);
      this.reconnectAttempts++;
      if (this.reconnectAttempts >= this.maxReconnectAttempts) {
        this.opts.logger?.error("[HotOpportunityWebSocket] Max reconnection attempts reached");
        this.socket?.disconnect();
      }
    });

    this.socket.on("opportunity:detected", (data: HotOpportunityEvent) => {
      this.onDetectedCallbacks.forEach((cb) => cb(data));
    });

    this.socket.on("opportunity:validated", (data: HotOpportunityEvent) => {
      this.onValidatedCallbacks.forEach((cb) => cb(data));
    });

    this.socket.on("error", (err: { code: string; room?: string }) => {
      this.opts.logger?.error("[HotOpportunityWebSocket] Server error:", err);
    });
  }

  onDetected(callback: (opp: HotOpportunityEvent) => void): () => void {
    this.onDetectedCallbacks.push(callback);
    return () => {
      const idx = this.onDetectedCallbacks.indexOf(callback);
      if (idx > -1) this.onDetectedCallbacks.splice(idx, 1);
    };
  }

  onValidated(callback: (opp: HotOpportunityEvent) => void): () => void {
    this.onValidatedCallbacks.push(callback);
    return () => {
      const idx = this.onValidatedCallbacks.indexOf(callback);
      if (idx > -1) this.onValidatedCallbacks.splice(idx, 1);
    };
  }

  disconnect(): void {
    this.socket?.disconnect();
    this.socket = null;
  }

  isConnected(): boolean {
    return this.socket?.connected ?? false;
  }
}

// React Hook para usar el WebSocket en componentes
import { useEffect, useState, useCallback } from "react";

export interface UseHotOpportunitiesOptions {
  url: string;
  token: string;
  maxDetected?: number;
  maxValidated?: number;
}

export function useHotOpportunities(opts: UseHotOpportunitiesOptions) {
  const [detected, setDetected] = useState<HotOpportunityEvent[]>([]);
  const [validated, setValidated] = useState<HotOpportunityEvent[]>([]);
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState<Error | null>(null);

  useEffect(() => {
    const client = new HotOpportunityWebSocket({
      url: opts.url,
      token: opts.token,
    });

    client.onDetected((opp) => {
      setDetected((prev) => [opp, ...prev].slice(0, opts.maxDetected ?? 100));
    });

    client.onValidated((opp) => {
      setValidated((prev) => [opp, ...prev].slice(0, opts.maxValidated ?? 50));
    });

    try {
      client.connect();
      setConnected(true);
    } catch (e) {
      setError(e as Error);
    }

    return () => {
      client.disconnect();
      setConnected(false);
    };
  }, [opts.url, opts.token, opts.maxDetected, opts.maxValidated]);

  const clearDetected = useCallback(() => setDetected([]), []);
  const clearValidated = useCallback(() => setValidated([]), []);

  return {
    detected,
    validated,
    connected,
    error,
    clearDetected,
    clearValidated,
  };
}
