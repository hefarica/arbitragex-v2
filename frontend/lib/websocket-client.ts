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
 * WO-01 (2026-09-06): adapta el payload del evento `new_opportunity` al shape
 * `HotOpportunityEvent` que consumen los callbacks `onDetected`.
 *
 * Contrato del server (backend/api-server/src/websocket.ts:339
 * `broadcastOpportunity` ← index.ts:1847-1863 LISTEN `opportunities_channel`
 * ← trigger PG `trg_notify_opportunity` AFTER INSERT, payload =
 * `row_to_json(NEW)`): una fila completa de la tabla `opportunities` (escritor
 * canónico: backend/searcher-rs/src/persistence.rs). Campos relevantes:
 *   id (uuid string) · chain_id (number) · strategy_kind (string) ·
 *   detected_at (ISO 8601 string) · status ('detected'|…|'rejected'|'failed')
 *   · expected_profit_usd / net_expected_profit_usd (USD, number|null).
 *
 * RULE 00 / R8 fail-honest — solo se mapean campos con correspondencia 1:1
 * veraz; NUNCA se fabrican valores:
 *   - `status` del row PG ('detected' etc.) NO pertenece al union
 *     "passed" | "failed" del hot stream → se omite.
 *   - `net_profit_wei` / `gas_used` no existen en el row PG (el row trae USD,
 *     no wei) → se omiten.
 *   - `detected_at` ISO se convierte a `detected_at_ms` (epoch ms string); si
 *     no es parseable se omite el campo, jamás se emite "NaN".
 *
 * @returns null cuando el payload no es objeto o carece de `id` uuid string
 *          (payload corrupto → se descarta, R8).
 */
export function adaptNewOpportunityToHotEvent(
  payload: unknown,
): HotOpportunityEvent | null {
  if (typeof payload !== "object" || payload === null) return null;
  const row = payload as Record<string, unknown>;
  if (typeof row.id !== "string" || row.id.length === 0) return null;

  const event: HotOpportunityEvent = { id: row.id };
  if (typeof row.chain_id === "number") {
    event.chain_id = String(row.chain_id);
  } else if (typeof row.chain_id === "string") {
    event.chain_id = row.chain_id;
  }
  if (typeof row.strategy_kind === "string") {
    event.strategy_kind = row.strategy_kind;
  }
  if (typeof row.detected_at === "string") {
    const ms = Date.parse(row.detected_at);
    if (!Number.isNaN(ms)) {
      event.detected_at_ms = String(ms);
    }
  }
  return event;
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

    // WO-01 (2026-09-06): el api-server emite el broadcast insignia
    // `new_opportunity` (PostgreSQL LISTEN opportunities_channel →
    // broadcastOpportunity) al MISMO room `opportunities`; este cliente antes
    // no lo escuchaba — el broadcast llegaba a nadie. Listener ADITIVO:
    // despacha al mismo flujo que `opportunity:detected` con el payload PG
    // adaptado (adaptNewOpportunityToHotEvent); payload corrupto se descarta
    // (R8 fail-honest). Los listeners existentes quedan intactos.
    this.socket.on("new_opportunity", (data: unknown) => {
      const adapted = adaptNewOpportunityToHotEvent(data);
      if (adapted === null) return;
      this.onDetectedCallbacks.forEach((cb) => cb(adapted));
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
