"use client";

import { useEffect, useRef, useState, useCallback } from "react";
import { io, Socket } from "socket.io-client";
import { getWsBaseUrl } from "@/lib/api-client";

// Tipos para métricas del sistema
interface MetricsData {
  entropy: number;
  convergenceRate: number;
  timestamp: string;
  topology?: {
    activeChains: string[];
    totalManifolds: number;
    loopResolutions: number;
    orthogonalEquilibriums: number;
  };
}

// ── WS-CONTRACT-METRICS-01 (audit 2026-09-29) ─────────────────────────────
// Real wire shape of `metrics:initial` / `metrics:entropy`
// (backend/api-server/src/routes/health.ts:540 and :564). `calculateEntropy`
// returns nulls on purpose when there is no data, so the nullables here are the
// server's own R8 contract — not defensive noise.
interface MetricsEntropyPayload {
  entropy?: number | null;
  raw_entropy?: number | null;
  service_health_factor?: number | null;
  delta?: number | null;
  has_data?: boolean;
  /** Only present on `metrics:initial`. */
  convergence?: { rate?: number; target?: number; variance?: number };
  /**
   * Only present on `metrics:initial`. HONEST NOTE: `getTopologyMetrics` keys are
   * `manifolds_observed` / `loops_resolved` / `decoherence_rate`, NOT the camelCase
   * fields below, and the service route that would fill `activeChains` is not
   * mounted (see /monitor page notes). Typed as optional-unknown so a consumer must
   * check before reading; do NOT populate it with invented values.
   */
  topology?: MetricsData["topology"];
  /** Unix seconds (the server sends `Math.floor(Date.now()/1000)`). */
  timestamp?: number;
}

// Tipos para telemetría de convergencia
interface ConvergenceTelemetry {
  iteration: number;
  residual: number;
  delta: number;
  timestamp: string;
}

// Tipos para eventos de salud de servicios
interface ServiceHealthData {
  services: Record<string, {
    status: "healthy" | "degraded" | "unhealthy";
    latency?: number;
    lastCheck: string;
  }>;
  overall: "healthy" | "degraded" | "unhealthy";
}

interface UseSocketIOOptions {
  wsUrl?: string;
  reconnectInterval?: number;
  maxReconnectAttempts?: number;
}

interface UseSocketIOReturn {
  // Métricas
  entropy: number | null;
  convergenceRate: number | null;
  topology: MetricsData["topology"] | null;

  // Telemetría de convergencia
  convergenceHistory: ConvergenceTelemetry[];

  // Salud de servicios
  serviceHealth: ServiceHealthData | null;

  // Estado de conexión
  isConnected: boolean;
  isConnecting: boolean;
  error: string | null;
  lastUpdate: Date | null;

  // Acciones
  reconnect: () => void;
}

// Constantes de configuración
const DEFAULT_WS_URL = getWsBaseUrl();
const DEFAULT_RECONNECT_INTERVAL = 5000;
const DEFAULT_MAX_RECONNECT_ATTEMPTS = 5;

// Rooms de Socket.IO a suscribirse
const SOCKET_ROOMS = ["metrics", "convergence", "telemetry"];

export function useSocketIO(options: UseSocketIOOptions = {}): UseSocketIOReturn {
  const {
    wsUrl = DEFAULT_WS_URL,
    reconnectInterval = DEFAULT_RECONNECT_INTERVAL,
    maxReconnectAttempts = DEFAULT_MAX_RECONNECT_ATTEMPTS,
  } = options;

  // Estados de datos
  const [entropy, setEntropy] = useState<number | null>(null);
  const [convergenceRate, setConvergenceRate] = useState<number | null>(null);
  const [topology, setTopology] = useState<MetricsData["topology"] | null>(null);
  const [convergenceHistory, setConvergenceHistory] = useState<ConvergenceTelemetry[]>([]);
  const [serviceHealth, setServiceHealth] = useState<ServiceHealthData | null>(null);

  // Estados de conexión
  const [isConnected, setIsConnected] = useState<boolean>(false);
  const [isConnecting, setIsConnecting] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const [lastUpdate, setLastUpdate] = useState<Date | null>(null);

  // Refs para manejo de Socket.IO
  const socketRef = useRef<Socket | null>(null);
  const reconnectAttemptsRef = useRef<number>(0);
  const reconnectTimerRef = useRef<NodeJS.Timeout | null>(null);
  const isMountedRef = useRef<boolean>(true);

  // Limpiar timers y socket
  const cleanup = useCallback(() => {
    if (reconnectTimerRef.current) {
      clearTimeout(reconnectTimerRef.current);
      reconnectTimerRef.current = null;
    }

    if (socketRef.current) {
      socketRef.current.removeAllListeners();
      socketRef.current.close();
      socketRef.current = null;
    }
  }, []);

  // Conectar Socket.IO
  const connect = useCallback(() => {
    if (!isMountedRef.current) return;

    // Limpiar conexión anterior si existe
    cleanup();

    setIsConnecting(true);
    setError(null);

    try {
      const socket = io(wsUrl, {
        transports: ["websocket"],
        reconnection: false, // Manejamos reconexión manualmente
        timeout: 10000,
      });

      socketRef.current = socket;

      // Evento de conexión exitosa
      socket.on("connect", () => {
        if (!isMountedRef.current) return;

        setIsConnected(true);
        setIsConnecting(false);
        setError(null);
        reconnectAttemptsRef.current = 0;

        // Suscribirse a rooms
        SOCKET_ROOMS.forEach((room) => {
          socket.emit("subscribe", room);
        });
      });

      // Evento de desconexión
      socket.on("disconnect", (reason) => {
        if (!isMountedRef.current) return;

        setIsConnected(false);

        // Reconectar automáticamente si no fue desconexión manual
        if (reason !== "io client disconnect") {
          if (reconnectAttemptsRef.current < maxReconnectAttempts) {
            reconnectAttemptsRef.current += 1;
            setError(`Desconectado. Reintentando (${reconnectAttemptsRef.current}/${maxReconnectAttempts})...`);

            reconnectTimerRef.current = setTimeout(() => {
              if (isMountedRef.current) {
                connect();
              }
            }, reconnectInterval);
          } else {
            setError("Máximo de intentos de reconexión alcanzado");
            setIsConnecting(false);
          }
        }
      });

      // Evento de error de conexión
      socket.on("connect_error", (err) => {
        if (!isMountedRef.current) return;

        setIsConnected(false);
        setError(`Error de conexión: ${err.message}`);

        // Intentar reconectar
        if (reconnectAttemptsRef.current < maxReconnectAttempts) {
          reconnectAttemptsRef.current += 1;

          reconnectTimerRef.current = setTimeout(() => {
            if (isMountedRef.current) {
              connect();
            }
          }, reconnectInterval);
        } else {
          setIsConnecting(false);
        }
      });

      // ── WS-CONTRACT-METRICS-01 (audit 2026-09-29) ────────────────────────
      // This hook used to listen for a bare `metrics` event and for
      // `entropy:update`. The server emits NEITHER: api-server/routes/health.ts
      // sends `metrics:initial` (L540), `metrics:entropy` (L564, every 5 s) and
      // `metrics:error` (L551/L573). So MetricsStream and EntropyGauge — both
      // live components of /monitor — were wired to a channel that never fires:
      // a rename that landed on the producer only. Payload fields below are read
      // with the server's real names (`convergence.rate`, not `convergenceRate`)
      // and R8 applies: a missing field stays null, never a fabricated 0.
      const applyMetricsPayload = (data: MetricsEntropyPayload) => {
        if (!isMountedRef.current) return;

        setEntropy(data.entropy ?? null);
        setConvergenceRate(data.convergence?.rate ?? null);
        if (data.topology) setTopology(data.topology);
        setLastUpdate(
          typeof data.timestamp === "number" ? new Date(data.timestamp * 1000) : new Date(),
        );
      };

      socket.on("metrics:initial", applyMetricsPayload);

      // Entropy refresh every 5 s (no convergence/topology in this payload).
      socket.on("metrics:entropy", applyMetricsPayload);

      // Transport/RPC failure while computing metrics. Surfaces as the hook's
      // error so the panel can show the absence instead of looking frozen.
      socket.on("metrics:error", (data: { error?: string; detail?: string }) => {
        if (!isMountedRef.current) return;
        setError(`${data.error ?? "metrics_error"}${data.detail ? `: ${data.detail}` : ""}`);
      });

      // NOT WIRED — declared here for auditability, do NOT invent a producer:
      //   `convergence:telemetry` — the server emits `convergence_signal`
      //     (websocket.ts:548) and the frontend already consumes it in
      //     lib/hooks/useConvergenceStream.ts; this hook's history array is a
      //     second, incompatible shape.
      //   `services:health` — no producer anywhere in the backend (0 hits); the
      //     live path for service health is REST (GET /api/status).

      // Evento genérico para datos de telemetría
      socket.on("telemetry", (data: Record<string, unknown>) => {
        if (!isMountedRef.current) return;

        // Procesar datos de telemetría según su tipo
        if (data.type === "entropy" && typeof data.value === "number") {
          setEntropy(data.value);
        }
        if (data.type === "convergence" && typeof data.rate === "number") {
          setConvergenceRate(data.rate);
        }

        setLastUpdate(new Date());
      });
    } catch (err) {
      if (!isMountedRef.current) return;

      setError(err instanceof Error ? err.message : "Error al crear conexión Socket.IO");
      setIsConnecting(false);

      // Intentar reconectar
      if (reconnectAttemptsRef.current < maxReconnectAttempts) {
        reconnectAttemptsRef.current += 1;

        reconnectTimerRef.current = setTimeout(() => {
          if (isMountedRef.current) {
            connect();
          }
        }, reconnectInterval);
      }
    }
  }, [wsUrl, reconnectInterval, maxReconnectAttempts, cleanup]);

  // Reconexión manual
  const reconnect = useCallback(() => {
    reconnectAttemptsRef.current = 0;
    connect();
  }, [connect]);

  // Efecto de conexión inicial
  useEffect(() => {
    isMountedRef.current = true;
    connect();

    return () => {
      isMountedRef.current = false;
      cleanup();
    };
  }, [connect, cleanup]);

  return {
    entropy,
    convergenceRate,
    topology,
    convergenceHistory,
    serviceHealth,
    isConnected,
    isConnecting,
    error,
    lastUpdate,
    reconnect,
  };
}
