"use client";

/**
 * useWebSocket — generic typed WebSocket hook with auto-reconnect.
 *
 * Architecture:
 *   - Wraps the native WebSocket API with automatic reconnection.
 *   - Returns a typed messages ring buffer (last 100) + connection status.
 *   - JSON parsing is handled internally; malformed messages are dropped
 *     with a console warning (not a fatal error).
 *   - Auto-reconnect uses a fixed 5s backoff to avoid reconnect storms.
 *   - Cleanup on unmount: closes the socket and clears all timers.
 *
 * R1 compliant: all WebSocket access is inside useEffect; initial render
 *   is pure useState(initial) — identical to the SSR snapshot.
 * R8 fail-honest: connection status is surfaced verbatim; we never fake
 *   "connected" when the socket is not open.
 */

import { useEffect, useRef, useState, useCallback } from "react";

export type WsConnectionStatus =
  | "connecting"
  | "open"
  | "closed"
  | "error"
  | "reconnecting";

export interface UseWebSocketOptions {
  /** Auto-reconnect on close/error. Default: true. */
  autoReconnect?: boolean;
  /** Reconnect backoff in ms. Default: 5000. */
  reconnectBackoffMs?: number;
  /** Max messages to keep in the ring buffer. Default: 100. */
  maxMessages?: number;
  /** Optional subprotocols for the WebSocket constructor. */
  protocols?: string | string[];
}

export interface UseWebSocketResult<T> {
  /** Last N messages received (most recent first). */
  messages: T[];
  /** Current WebSocket connection status. */
  status: WsConnectionStatus;
  /** Send a message to the server (no-op when not open). */
  send: (data: string | object) => void;
  /** Manually close the connection. */
  close: () => void;
  /** Latest error message, if any. */
  lastError: string | null;
}

const DEFAULT_MAX_MESSAGES = 100;
const DEFAULT_RECONNECT_BACKOFF_MS = 5000;

/**
 * Generic WebSocket hook with auto-reconnect and typed message buffer.
 *
 * @param url — WebSocket URL (e.g. "wss://api.example.com/ws"). Pass null to keep the socket closed.
 * @param options — Configuration for reconnect, buffer size, protocols.
 *
 * @example
 * ```tsx
 * const { messages, status, send } = useWebSocket<MyEventType>("wss://api.example.com/ws");
 * ```
 */
export function useWebSocket<T = unknown>(
  url: string | null,
  options: UseWebSocketOptions = {},
): UseWebSocketResult<T> {
  const {
    autoReconnect = true,
    reconnectBackoffMs = DEFAULT_RECONNECT_BACKOFF_MS,
    maxMessages = DEFAULT_MAX_MESSAGES,
    protocols,
  } = options;

  const [messages, setMessages] = useState<T[]>([]);
  const [status, setStatus] = useState<WsConnectionStatus>("connecting");
  const [lastError, setLastError] = useState<string | null>(null);

  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const aliveRef = useRef(true);
  const maxMessagesRef = useRef(maxMessages);
  const urlRef = useRef(url);

  // Keep refs in sync without triggering re-renders.
  useEffect(() => {
    maxMessagesRef.current = maxMessages;
    urlRef.current = url;
  }, [maxMessages, url]);

  const closeSocket = useCallback(() => {
    const ws = wsRef.current;
    if (ws) {
      // Suppress the onclose handler so it doesn't trigger reconnect
      // when we're doing an intentional close.
      ws.onclose = null;
      ws.onerror = null;
      ws.onmessage = null;
      ws.onopen = null;
      if (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING) {
        ws.close();
      }
      wsRef.current = null;
    }
  }, []);

  const connect = useCallback(() => {
    if (typeof window === "undefined") return;
    if (!urlRef.current) return;
    if (!aliveRef.current) return;

    // Close any existing socket first.
    closeSocket();

    setStatus("connecting");
    setLastError(null);

    try {
      const ws = protocols
        ? new WebSocket(urlRef.current, protocols)
        : new WebSocket(urlRef.current);

      wsRef.current = ws;

      ws.onopen = () => {
        if (!aliveRef.current) {
          ws.close();
          return;
        }
        setStatus("open");
        setLastError(null);
      };

      ws.onmessage = (event: MessageEvent) => {
        if (!aliveRef.current) return;

        let parsed: T;
        try {
          parsed =
            typeof event.data === "string"
              ? (JSON.parse(event.data) as T)
              : (event.data as T);
        } catch {
          // Malformed JSON — drop with a warning.
          // eslint-disable-next-line no-console
          console.warn("[useWebSocket] dropped malformed JSON message:", event.data?.slice(0, 200));
          return;
        }

        setMessages((prev) => {
          const next = [parsed, ...prev];
          if (next.length > maxMessagesRef.current) {
            return next.slice(0, maxMessagesRef.current);
          }
          return next;
        });
      };

      ws.onerror = () => {
        if (!aliveRef.current) return;
        setStatus("error");
        setLastError("WebSocket error occurred");
      };

      ws.onclose = () => {
        if (!aliveRef.current) return;
        setStatus("closed");

        if (autoReconnect && aliveRef.current) {
          setStatus("reconnecting");
          reconnectTimerRef.current = setTimeout(() => {
            if (aliveRef.current) {
              connect();
            }
          }, reconnectBackoffMs);
        }
      };
    } catch (err) {
      if (!aliveRef.current) return;
      setStatus("error");
      setLastError((err as Error).message ?? "WebSocket construction failed");

      if (autoReconnect && aliveRef.current) {
        setStatus("reconnecting");
        reconnectTimerRef.current = setTimeout(() => {
          if (aliveRef.current) {
            connect();
          }
        }, reconnectBackoffMs);
      }
    }
  }, [autoReconnect, reconnectBackoffMs, closeSocket]);

  // Main effect: connect when url changes, cleanup on unmount.
  useEffect(() => {
    aliveRef.current = true;

    if (url) {
      connect();
    } else {
      setStatus("closed");
    }

    return () => {
      aliveRef.current = false;
      if (reconnectTimerRef.current) {
        clearTimeout(reconnectTimerRef.current);
        reconnectTimerRef.current = null;
      }
      closeSocket();
    };
  }, [url, connect, closeSocket]);

  const send = useCallback(
    (data: string | object) => {
      const ws = wsRef.current;
      if (!ws || ws.readyState !== WebSocket.OPEN) {
        // eslint-disable-next-line no-console
        console.warn("[useWebSocket] send() called but socket is not open");
        return;
      }
      const payload = typeof data === "string" ? data : JSON.stringify(data);
      ws.send(payload);
    },
    [],
  );

  const close = useCallback(() => {
    if (reconnectTimerRef.current) {
      clearTimeout(reconnectTimerRef.current);
      reconnectTimerRef.current = null;
    }
    closeSocket();
    setStatus("closed");
  }, [closeSocket]);

  return { messages, status, send, close, lastError };
}
