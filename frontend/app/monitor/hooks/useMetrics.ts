"use client";

import { useEffect, useRef, useState, useCallback } from "react";

// Tipos para métricas
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

interface UseMetricsOptions {
  initialEntropy?: number;
  wsUrl?: string;
  restUrl?: string;
  reconnectInterval?: number;
  maxReconnectAttempts?: number;
}

interface UseMetricsReturn {
  entropy: number;
  convergenceRate: number;
  topology: MetricsData["topology"];
  isConnected: boolean;
  isUsingFallback: boolean;
  error: string | null;
  lastUpdate: Date | null;
}

// Constantes de configuración
const DEFAULT_WS_URL = process.env.NEXT_PUBLIC_WS_URL ? process.env.NEXT_PUBLIC_WS_URL.replace('http', 'ws') + '/ws/metrics' : 'ws://[WS_URL]/ws/metrics';
const DEFAULT_REST_URL = process.env.NEXT_PUBLIC_WS_URL ? process.env.NEXT_PUBLIC_WS_URL + '/metrics/entropy' : 'http://[WS_URL]/metrics/entropy';
const DEFAULT_RECONNECT_INTERVAL = 5000;
const DEFAULT_MAX_RECONNECT_ATTEMPTS = 5;

export function useMetrics(options: UseMetricsOptions = {}): UseMetricsReturn {
  const {
    initialEntropy = 0,
    wsUrl = DEFAULT_WS_URL,
    restUrl = DEFAULT_REST_URL,
    reconnectInterval = DEFAULT_RECONNECT_INTERVAL,
    maxReconnectAttempts = DEFAULT_MAX_RECONNECT_ATTEMPTS,
  } = options;

  // Estados
  const [entropy, setEntropy] = useState<number>(initialEntropy);
  const [convergenceRate, setConvergenceRate] = useState<number>(0);
  const [topology, setTopology] = useState<MetricsData["topology"]>(undefined);
  const [isConnected, setIsConnected] = useState<boolean>(false);
  const [isUsingFallback, setIsUsingFallback] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const [lastUpdate, setLastUpdate] = useState<Date | null>(null);

  // Refs para manejo de WebSocket
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectAttemptsRef = useRef<number>(0);
  const reconnectTimerRef = useRef<NodeJS.Timeout | null>(null);
  const fallbackTimerRef = useRef<NodeJS.Timeout | null>(null);
  const isMountedRef = useRef<boolean>(true);

  // Fallback a REST polling
  const startFallbackPolling = useCallback(() => {
    if (!isMountedRef.current) return;

    setIsUsingFallback(true);
    setError("WebSocket no disponible, usando polling REST");

    const poll = async () => {
      if (!isMountedRef.current) return;

      try {
        const res = await fetch(restUrl, {
          headers: { accept: "application/json" },
          cache: "no-store",
        });

        if (res.ok) {
          const data = await res.json();

          if (isMountedRef.current) {
            setEntropy(data.entropy ?? 0);
            setConvergenceRate(data.convergenceRate ?? 0);
            if (data.topology) {
              setTopology(data.topology);
            }
            setLastUpdate(new Date());
            setError(null);
          }
        } else {
          if (isMountedRef.current) {
            setError(`HTTP ${res.status}: ${res.statusText}`);
          }
        }
      } catch (err) {
        if (isMountedRef.current) {
          setError(err instanceof Error ? err.message : "Error de conexión");
        }
      }

      // Programar siguiente poll
      if (isMountedRef.current) {
        fallbackTimerRef.current = setTimeout(poll, reconnectInterval);
      }
    };

    poll();
  }, [restUrl, reconnectInterval]);

  // Conectar WebSocket
  const connectWebSocket = useCallback(() => {
    if (!isMountedRef.current) return;

    // Limpiar fallback si existe
    if (fallbackTimerRef.current) {
      clearTimeout(fallbackTimerRef.current);
      fallbackTimerRef.current = null;
    }

    setIsUsingFallback(false);

    try {
      const ws = new WebSocket(wsUrl);
      wsRef.current = ws;

      ws.onopen = () => {
        if (!isMountedRef.current) return;

        setIsConnected(true);
        setError(null);
        reconnectAttemptsRef.current = 0;
      };

      ws.onmessage = (event) => {
        if (!isMountedRef.current) return;

        try {
          const data: MetricsData = JSON.parse(event.data);

          setEntropy(data.entropy ?? 0);
          setConvergenceRate(data.convergenceRate ?? 0);
          if (data.topology) {
            setTopology(data.topology);
          }
          setLastUpdate(new Date());
          setError(null);
        } catch (err) {
          console.error("Error parsing WebSocket message:", err);
        }
      };

      ws.onerror = () => {
        if (!isMountedRef.current) return;

        setIsConnected(false);
        setError("Error de conexión WebSocket");
      };

      ws.onclose = () => {
        if (!isMountedRef.current) return;

        setIsConnected(false);
        wsRef.current = null;

        // Intentar reconectar si no excedimos el máximo
        if (reconnectAttemptsRef.current < maxReconnectAttempts) {
          reconnectAttemptsRef.current += 1;
          reconnectTimerRef.current = setTimeout(connectWebSocket, reconnectInterval);
        } else {
          // Máximo de intentos alcanzado, usar fallback
          startFallbackPolling();
        }
      };
    } catch (err) {
      if (!isMountedRef.current) return;

      setError(err instanceof Error ? err.message : "Error al crear WebSocket");
      startFallbackPolling();
    }
  }, [wsUrl, reconnectInterval, maxReconnectAttempts, startFallbackPolling]);

  // Efecto de conexión inicial
  useEffect(() => {
    isMountedRef.current = true;
    connectWebSocket();

    return () => {
      isMountedRef.current = false;

      // Limpiar timers
      if (reconnectTimerRef.current) {
        clearTimeout(reconnectTimerRef.current);
      }
      if (fallbackTimerRef.current) {
        clearTimeout(fallbackTimerRef.current);
      }

      // Cerrar WebSocket
      if (wsRef.current) {
        wsRef.current.close();
        wsRef.current = null;
      }
    };
  }, [connectWebSocket]);

  return {
    entropy,
    convergenceRate,
    topology,
    isConnected,
    isUsingFallback,
    error,
    lastUpdate,
  };
}
