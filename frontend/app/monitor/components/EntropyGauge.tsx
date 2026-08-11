"use client";

import { useEffect, useState } from "react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { ThermometerIcon, TrendingDownIcon, TrendingUpIcon, ActivityIcon } from "lucide-react";
import { useSocketIO } from "../hooks/useSocketIO";

interface EntropyGaugeProps {
  initialValue?: number | null;
}

// Escala de colores basada en nivel de entropía
function getEntropyColor(value: number | null): string {
  if (value === null) return "text-slate-400";
  if (value <= 0.33) return "text-emerald-500"; // Baja entropía = estable
  if (value <= 0.66) return "text-amber-500";   // Media entropía = atención
  return "text-rose-500";                       // Alta entropía = crítico
}

function getEntropyBgColor(value: number | null): string {
  if (value === null) return "bg-slate-400";
  if (value <= 0.33) return "bg-emerald-500";
  if (value <= 0.66) return "bg-amber-500";
  return "bg-rose-500";
}

function getEntropyLabel(value: number | null): string {
  if (value === null) return "SIN DATOS";
  if (value <= 0.33) return "ESTABLE";
  if (value <= 0.66) return "ATENCIÓN";
  return "CRÍTICO";
}

// Componente de barra de progreso circular simplificada
function CircularProgress({ value, size = 120 }: { value: number | null; size?: number }) {
  const strokeWidth = 8;
  const radius = (size - strokeWidth) / 2;
  const circumference = radius * 2 * Math.PI;
  const progress = value !== null ? Math.min(Math.max(value, 0), 1) : 0;
  const dashoffset = circumference - progress * circumference;

  const colorClass = getEntropyColor(value);
  const bgColorClass = getEntropyBgColor(value);

  return (
    <div className="relative" style={{ width: size, height: size }}>
      {/* Círculo de fondo */}
      <svg
        width={size}
        height={size}
        className="-rotate-90 transform"
        aria-label={value !== null ? `Entropía: ${(value * 100).toFixed(1)}%` : "Sin datos de entropía"}
      >
        <circle
          cx={size / 2}
          cy={size / 2}
          r={radius}
          fill="none"
          stroke="currentColor"
          strokeWidth={strokeWidth}
          className="text-muted/20"
        />
        {value !== null && (
          <circle
            cx={size / 2}
            cy={size / 2}
            r={radius}
            fill="none"
            stroke="currentColor"
            strokeWidth={strokeWidth}
            strokeDasharray={circumference}
            strokeDashoffset={dashoffset}
            strokeLinecap="round"
            className={`${colorClass} transition-all duration-500 ease-out`}
          />
        )}
      </svg>

      {/* Valor central */}
      <div className="absolute inset-0 flex flex-col items-center justify-center">
        <span className={`text-2xl font-bold font-mono ${colorClass}`}>
          {value !== null ? `${(value * 100).toFixed(1)}%` : "--"}
        </span>
        <span className="text-[10px] uppercase tracking-wider text-muted-foreground">
          Entropía
        </span>
      </div>

      {/* Indicador de pulso */}
      {value !== null && (
        <div
          className={`absolute -bottom-1 left-1/2 size-2 -translate-x-1/2 rounded-full ${bgColorClass} animate-pulse`}
          aria-hidden="true"
        />
      )}
    </div>
  );
}

// Componente de trend
function TrendIndicator({
  current,
  previous,
}: {
  current: number | null;
  previous: number | null;
}) {
  if (current === null) {
    return (
      <span className="flex items-center gap-1 text-xs text-muted-foreground">
        <ActivityIcon className="size-3" />
        Sin datos
      </span>
    );
  }

  if (previous === null) {
    return (
      <span className="flex items-center gap-1 text-xs text-muted-foreground">
        <ActivityIcon className="size-3" />
        Inicializando...
      </span>
    );
  }

  const diff = current - previous;
  const isIncreasing = diff > 0;
  const isSignificant = Math.abs(diff) > 0.01;

  if (!isSignificant) {
    return (
      <span className="flex items-center gap-1 text-xs text-muted-foreground">
        <ActivityIcon className="size-3" />
        Estable
      </span>
    );
  }

  const Icon = isIncreasing ? TrendingUpIcon : TrendingDownIcon;
  const colorClass = isIncreasing ? "text-rose-500" : "text-emerald-500";

  return (
    <span className={`flex items-center gap-1 text-xs ${colorClass}`}>
      <Icon className="size-3" />
      {isIncreasing ? "+" : ""}
      {(diff * 100).toFixed(1)}%
    </span>
  );
}

export function EntropyGauge({ initialValue = null }: EntropyGaugeProps) {
  const { entropy, isConnected, error } = useSocketIO();

  const [previousEntropy, setPreviousEntropy] = useState<number | null>(null);
  const [history, setHistory] = useState<number[]>([]);

  // Usar valor inicial mientras no hay datos del socket
  const displayEntropy = entropy !== null ? entropy : initialValue;

  // Actualizar historial cuando cambia la entropía
  useEffect(() => {
    if (entropy !== null && entropy !== previousEntropy) {
      setPreviousEntropy(entropy);
      setHistory((prev) => {
        const newHistory = [...prev, entropy];
        // Mantener últimos 20 valores para el mini-gráfico
        return newHistory.slice(-20);
      });
    }
  }, [entropy, previousEntropy]);

  const colorClass = getEntropyColor(displayEntropy);
  const label = getEntropyLabel(displayEntropy);

  return (
    <Card className="relative overflow-hidden">
      <CardHeader className="pb-2">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <ThermometerIcon className="size-5 text-primary" />
            <CardTitle className="text-sm font-medium">Entropía del Sistema</CardTitle>
          </div>
          <div className="flex items-center gap-2">
            <Badge
              variant={
                displayEntropy === null
                  ? "outline"
                  : displayEntropy <= 0.33
                    ? "default"
                    : displayEntropy <= 0.66
                      ? "secondary"
                      : "destructive"
              }
            >
              {label}
            </Badge>
            <div
              className={`size-2 rounded-full ${isConnected ? "bg-emerald-500 animate-pulse" : "bg-rose-500"}`}
              title={isConnected ? "Conectado" : "Desconectado"}
            />
          </div>
        </div>
        <CardDescription>Medida de desorden termodinámico</CardDescription>
      </CardHeader>

      <CardContent className="space-y-4">
        <div className="flex items-center justify-between">
          <CircularProgress value={displayEntropy} />

          <div className="flex-1 pl-6 space-y-3">
            <div>
              <p className="text-xs text-muted-foreground">Estado</p>
              <p className={`text-lg font-semibold ${colorClass}`}>{label}</p>
            </div>

            <div>
              <p className="text-xs text-muted-foreground">Tendencia</p>
              <TrendIndicator current={displayEntropy} previous={previousEntropy} />
            </div>

            <div>
              <p className="text-xs text-muted-foreground">Conexión</p>
              <p className={`text-sm font-medium ${isConnected ? "text-emerald-500" : "text-rose-500"}`}>
                {isConnected ? "Socket.IO Activo" : error ?? "Sin conexión"}
              </p>
            </div>
          </div>
        </div>

        {/* Mini gráfico de historial */}
        {history.length > 1 && (
          <div className="pt-2">
            <p className="text-xs text-muted-foreground mb-2">Historial (últimos 20s)</p>
            <div className="flex items-end gap-0.5 h-12">
              {history.map((value, idx) => {
                const height = Math.max(10, value * 100);
                const barColor =
                  value <= 0.33 ? "bg-emerald-500/60" : value <= 0.66 ? "bg-amber-500/60" : "bg-rose-500/60";
                return (
                  <div
                    key={idx}
                    className={`flex-1 rounded-t-sm ${barColor} transition-all duration-300`}
                    style={{ height: `${height}%` }}
                    title={`${(value * 100).toFixed(1)}%`}
                  />
                );
              })}
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
