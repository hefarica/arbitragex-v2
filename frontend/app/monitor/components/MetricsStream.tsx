"use client";

import * as React from "react"; // SSR-test classic JSX runtime (house convention, see ThemeOverrideSelect)
import { useEffect, useState } from "react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  ActivityIcon,
  RefreshCwIcon,
  WifiIcon,
  WifiOffIcon,
  TrendingUpIcon,
  TrendingDownIcon,
  MinusIcon,
  BarChart3Icon,
} from "lucide-react";
import { useSocketIO } from "../hooks/useSocketIO";

// Componente de gráfico de líneas simple para historial de convergencia
function ConvergenceChart({ data }: { data: Array<{ iteration: number; residual: number; delta: number }> }) {
  if (data.length === 0) {
    return (
      <div className="flex h-32 items-center justify-center text-xs text-muted-foreground">
        Esperando datos de telemetría...
      </div>
    );
  }

  const maxResidual = Math.max(...data.map((d) => d.residual), 0.001);
  const maxDelta = Math.max(...data.map((d) => Math.abs(d.delta)), 0.001);

  return (
    <div className="space-y-4">
      {/* Gráfico de Residual */}
      <div>
        <p className="mb-2 text-xs font-medium text-muted-foreground">Residual de Convergencia</p>
        <div className="flex h-16 items-end gap-0.5">
          {data.slice(-30).map((point, idx) => {
            const height = Math.max(5, (point.residual / maxResidual) * 100);
            const colorClass =
              point.residual < 0.01
                ? "bg-emerald-500"
                : point.residual < 0.1
                  ? "bg-amber-500"
                  : "bg-rose-500";
            return (
              <div
                key={idx}
                className={`flex-1 rounded-t-sm ${colorClass} transition-all duration-300`}
                style={{ height: `${height}%` }}
                title={`Iteración ${point.iteration}: ${point.residual.toExponential(3)}`}
              />
            );
          })}
        </div>
      </div>

      {/* Gráfico de Delta */}
      <div>
        <p className="mb-2 text-xs font-medium text-muted-foreground">Variación Delta</p>
        <div className="relative h-16">
          <div className="absolute inset-x-0 top-1/2 h-px bg-border" />
          <div className="flex h-full items-center gap-0.5">
            {data.slice(-30).map((point, idx) => {
              const isPositive = point.delta >= 0;
              const magnitude = Math.abs(point.delta);
              const normalizedHeight = Math.min((magnitude / maxDelta) * 50, 50);

              return (
                <div
                  key={idx}
                  className="flex flex-1 flex-col items-center justify-center"
                >
                  <div
                    className={`w-full ${isPositive ? "bg-emerald-500/70" : "bg-rose-500/70"} transition-all duration-300`}
                    style={{
                      height: `${normalizedHeight}%`,
                      marginTop: isPositive ? 0 : `${50 - normalizedHeight}%`,
                      marginBottom: isPositive ? `${50 - normalizedHeight}%` : 0,
                    }}
                    title={`Iteración ${point.iteration}: ${point.delta > 0 ? "+" : ""}${point.delta.toExponential(3)}`}
                  />
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
}

// Componente de estado de servicios en tiempo real
function RealtimeServiceStatus({
  serviceHealth,
}: {
  serviceHealth: {
    services: Record<
      string,
      {
        status: "healthy" | "degraded" | "unhealthy";
        latency?: number;
        lastCheck: string;
      }
    >;
    overall: "healthy" | "degraded" | "unhealthy";
  } | null;
}) {
  if (!serviceHealth) {
    return (
      <div className="flex h-20 items-center justify-center text-xs text-muted-foreground">
        Sin datos de salud de servicios
      </div>
    );
  }

  const services = Object.entries(serviceHealth.services);

  return (
    <div className="space-y-2">
      {services.map(([name, info]) => {
        const statusConfig = {
          healthy: { color: "bg-emerald-500", label: "OK" },
          degraded: { color: "bg-amber-500", label: "DEG" },
          unhealthy: { color: "bg-rose-500", label: "ERR" },
        };
        const config = statusConfig[info.status];

        return (
          <div key={name} className="flex items-center justify-between rounded-md border p-2">
            <div className="flex items-center gap-2">
              <div className={`size-2 rounded-full ${config.color} animate-pulse`} />
              <span className="text-sm font-medium">{name}</span>
            </div>
            <div className="flex items-center gap-2">
              {info.latency !== undefined && (
                <span className="text-xs font-mono text-muted-foreground">{info.latency}ms</span>
              )}
              <Badge variant="outline" className="text-xs">
                {config.label}
              </Badge>
            </div>
          </div>
        );
      })}
    </div>
  );
}

// Componente de indicador de tendencia
function TrendIndicator({
  current,
  previous,
  label,
}: {
  current: number | null;
  previous: number | null;
  label: string;
}) {
  if (current === null) {
    return (
      <div className="space-y-1">
        <p className="text-xs text-muted-foreground">{label}</p>
        <p className="text-lg font-mono font-semibold">--</p>
      </div>
    );
  }

  if (previous === null) {
    return (
      <div className="space-y-1">
        <p className="text-xs text-muted-foreground">{label}</p>
        <p className="text-lg font-mono font-semibold">{current.toFixed(4)}</p>
      </div>
    );
  }

  const diff = current - previous;
  const isSignificant = Math.abs(diff) > 0.0001;

  const Icon = diff > 0 ? TrendingUpIcon : diff < 0 ? TrendingDownIcon : MinusIcon;
  const colorClass =
    diff > 0 ? "text-rose-500" : diff < 0 ? "text-emerald-500" : "text-muted-foreground";

  return (
    <div className="space-y-1">
      <p className="text-xs text-muted-foreground">{label}</p>
      <div className="flex items-center gap-2">
        <p className="text-lg font-mono font-semibold">{current.toFixed(4)}</p>
        {isSignificant && (
          <span className={`flex items-center gap-0.5 text-xs ${colorClass}`}>
            <Icon className="size-3" />
            {Math.abs(diff).toExponential(2)}
          </span>
        )}
      </div>
    </div>
  );
}

export function MetricsStream() {
  const {
    entropy,
    convergenceRate,
    convergenceHistory,
    serviceHealth,
    isConnected,
    isConnecting,
    error,
    lastUpdate,
    reconnect,
  } = useSocketIO();

  // Estado local para valores anteriores (para calcular tendencias)
  const [prevEntropy, setPrevEntropy] = useState<number | null>(null);
  const [prevConvergence, setPrevConvergence] = useState<number | null>(null);

  // Actualizar valores anteriores cuando cambian
  useEffect(() => {
    if (entropy !== null && entropy !== prevEntropy) {
      setPrevEntropy(entropy);
    }
  }, [entropy, prevEntropy]);

  useEffect(() => {
    if (convergenceRate !== null && convergenceRate !== prevConvergence) {
      setPrevConvergence(convergenceRate);
    }
  }, [convergenceRate, prevConvergence]);

  // Calcular estado general
  const overallStatus = isConnected
    ? "connected"
    : isConnecting
      ? "connecting"
      : error
        ? "error"
        : "disconnected";

  const statusConfig = {
    connected: { color: "bg-emerald-500", icon: WifiIcon, label: "Conectado" },
    connecting: { color: "bg-amber-500", icon: ActivityIcon, label: "Conectando..." },
    error: { color: "bg-rose-500", icon: WifiOffIcon, label: "Error" },
    disconnected: { color: "bg-slate-400", icon: WifiOffIcon, label: "Desconectado" },
  };

  const config = statusConfig[overallStatus];
  const StatusIcon = config.icon;

  return (
    <Card className="relative overflow-hidden">
      <CardHeader className="pb-2">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <BarChart3Icon className="size-5 text-primary" />
            <CardTitle className="text-sm font-medium">Stream de Métricas</CardTitle>
          </div>
          <div className="flex items-center gap-2">
            <Badge
              variant={isConnected ? "default" : "outline"}
              className={`
                ${overallStatus === "connecting" ? "border-amber-500 text-amber-500" : ""}
                ${overallStatus === "error" || overallStatus === "disconnected" ? "border-rose-500 text-rose-500" : ""}
              `}
            >
              <StatusIcon className="mr-1 size-3" />
              {config.label}
            </Badge>
            {!isConnected && (
              <Button
                variant="ghost"
                size="icon"
                aria-label="Reconectar stream de métricas"
                className="size-6"
                onClick={reconnect}
              >
                <RefreshCwIcon className="size-3" />
              </Button>
            )}
          </div>
        </div>
        <CardDescription>Telemetría en tiempo real vía Socket.IO</CardDescription>
      </CardHeader>

      <CardContent className="space-y-6">
        {/* Indicadores principales */}
        <div className="grid grid-cols-2 gap-4">
          <TrendIndicator
            current={entropy}
            previous={prevEntropy}
            label="Entropía Actual"
          />
          <TrendIndicator
            current={convergenceRate}
            previous={prevConvergence}
            label="Tasa de Convergencia"
          />
        </div>

        {/* Estado de conexión detallado */}
        <div className="rounded-lg border p-3">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <div className={`size-2 rounded-full ${config.color} ${isConnected ? "animate-pulse" : ""}`} />
              <span className="text-sm font-medium">Socket.IO</span>
            </div>
            <span className="text-xs text-muted-foreground">
              {lastUpdate ? `Última actualización: ${lastUpdate.toLocaleTimeString()}` : "Sin datos"}
            </span>
          </div>
          {error && (
            <p className="mt-2 text-xs text-rose-500">{error}</p>
          )}
        </div>

        {/* Gráficos de convergencia */}
        <div>
          <p className="mb-3 text-xs font-medium text-muted-foreground">
            Historial de Convergencia ({convergenceHistory.length} puntos)
          </p>
          <ConvergenceChart data={convergenceHistory} />
        </div>

        {/* Estado de servicios en tiempo real */}
        <div>
          <p className="mb-3 text-xs font-medium text-muted-foreground">Estado de Servicios</p>
          <RealtimeServiceStatus serviceHealth={serviceHealth} />
        </div>
      </CardContent>
    </Card>
  );
}
