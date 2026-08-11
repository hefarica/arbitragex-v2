"use client";

import { useEffect, useState } from "react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { ServerIcon, DatabaseIcon, CpuIcon, WifiIcon, WifiOffIcon, AlertCircleIcon } from "lucide-react";

// Tipos para estado de servicios
type ServiceStatus = "running" | "degraded" | "down" | "unknown";

interface ServiceState {
  name: string;
  status: ServiceStatus;
  latency?: number;
  lastPing: string;
  version?: string;
}

interface ServiceStatusProps {
  initialServices?: ServiceState[];
}

// Configuración de servicios a monitorear
const DEFAULT_SERVICES: ServiceState[] = [
  {
    name: "searcher-rs",
    status: "unknown",
    lastPing: new Date().toISOString(),
    version: "v2.1.0",
  },
  {
    name: "postgres",
    status: "unknown",
    lastPing: new Date().toISOString(),
  },
  {
    name: "redis",
    status: "unknown",
    lastPing: new Date().toISOString(),
  },
  {
    name: "api-server",
    status: "unknown",
    lastPing: new Date().toISOString(),
  },
];

// Iconos por servicio
const SERVICE_ICONS: Record<string, typeof ServerIcon> = {
  "searcher-rs": CpuIcon,
  postgres: DatabaseIcon,
  redis: DatabaseIcon,
  "api-server": ServerIcon,
};

// Configuración visual por estado
const STATUS_CONFIG: Record<
  ServiceStatus,
  { color: string; bgColor: string; label: string; icon: typeof WifiIcon }
> = {
  running: {
    color: "text-emerald-500",
    bgColor: "bg-emerald-500",
    label: "OPERATIVO",
    icon: WifiIcon,
  },
  degraded: {
    color: "text-amber-500",
    bgColor: "bg-amber-500",
    label: "DEGRADADO",
    icon: AlertCircleIcon,
  },
  down: {
    color: "text-rose-500",
    bgColor: "bg-rose-500",
    label: "CAÍDO",
    icon: WifiOffIcon,
  },
  unknown: {
    color: "text-slate-400",
    bgColor: "bg-slate-400",
    label: "DESCONOCIDO",
    icon: WifiOffIcon,
  },
};

// Componente de indicador individual
function ServiceIndicator({ service }: { service: ServiceState }) {
  const Icon = SERVICE_ICONS[service.name] ?? ServerIcon;
  const config = STATUS_CONFIG[service.status];
  const StatusIcon = config.icon;

  return (
    <div className="flex items-center justify-between rounded-lg border p-3 transition-colors hover:bg-muted/50">
      <div className="flex items-center gap-3">
        <div className={`flex size-9 items-center justify-center rounded-md bg-muted ${config.color}`}>
          <Icon className="size-4" />
        </div>
        <div>
          <p className="font-medium text-sm">{service.name}</p>
          {service.version && (
            <p className="text-xs text-muted-foreground">{service.version}</p>
          )}
        </div>
      </div>

      <div className="flex items-center gap-3">
        {service.latency !== undefined && service.status !== "unknown" && (
          <span className="text-xs font-mono text-muted-foreground">{service.latency}ms</span>
        )}

        <div className="flex items-center gap-1.5">
          <div className={`size-2 rounded-full ${config.bgColor} ${service.status === "running" ? "animate-pulse" : ""}`} />
          <Badge
            variant={service.status === "running" ? "default" : "outline"}
            className={`text-xs ${service.status === "degraded" ? "border-amber-500 text-amber-500" : ""} ${
              service.status === "down" ? "border-rose-500 text-rose-500" : ""
            }`}
          >
            <StatusIcon className="mr-1 size-3" />
            {config.label}
          </Badge>
        </div>
      </div>
    </div>
  );
}

// Hook para verificar salud de servicios
function useServiceHealth() {
  const [services, setServices] = useState<ServiceState[]>(DEFAULT_SERVICES);
  const [isChecking, setIsChecking] = useState(false);

  const checkHealth = async () => {
    if (isChecking) return;
    setIsChecking(true);

    try {
      const base = process.env.NEXT_PUBLIC_EDGE_URL ?? "";

      // Verificar health endpoint
      const startTime = performance.now();
      const res = await fetch(`${base.replace(/\/$/, "")}/health`, {
        headers: { accept: "application/json" },
        cache: "no-store",
      });
      const latency = Math.round(performance.now() - startTime);

      if (res.ok) {
        const data = await res.json();

        setServices((prev) =>
          prev.map((svc) => {
            // Mapear estado del health endpoint a nuestros servicios
            const serviceHealth = data.services?.[svc.name];

            if (serviceHealth) {
              return {
                ...svc,
                status: serviceHealth.status === "healthy" ? "running" : serviceHealth.status === "degraded" ? "degraded" : "down",
                latency: svc.name === "api-server" ? latency : serviceHealth.latency,
                lastPing: new Date().toISOString(),
              };
            }

            // Si no hay datos específicos, marcar como desconocido
            return {
              ...svc,
              status: "unknown",
              lastPing: new Date().toISOString(),
            };
          })
        );
      } else {
        // Health endpoint no disponible
        setServices((prev) =>
          prev.map((svc) => ({
            ...svc,
            status: "unknown",
            lastPing: new Date().toISOString(),
          }))
        );
      }
    } catch {
      // Error de conexión - todos desconocidos
      setServices((prev) =>
        prev.map((svc) => ({
          ...svc,
          status: "unknown",
          lastPing: new Date().toISOString(),
        }))
      );
    } finally {
      setIsChecking(false);
    }
  };

  useEffect(() => {
    // Verificación inicial
    checkHealth();

    // Polling cada 10 segundos
    const interval = setInterval(checkHealth, 10000);

    return () => clearInterval(interval);
  }, []);

  return { services, isChecking, checkHealth };
}

// Componente principal
export function ServiceStatus({ initialServices }: ServiceStatusProps) {
  const { services, isChecking } = useServiceHealth();

  // Calcular resumen
  const runningCount = services.filter((s) => s.status === "running").length;
  const degradedCount = services.filter((s) => s.status === "degraded").length;
  const downCount = services.filter((s) => s.status === "down").length;
  const unknownCount = services.filter((s) => s.status === "unknown").length;

  const overallStatus: ServiceStatus =
    downCount > 0 ? "down" : degradedCount > 0 ? "degraded" : runningCount > 0 ? "running" : "unknown";

  const overallConfig = STATUS_CONFIG[overallStatus];

  return (
    <Card>
      <CardHeader className="pb-2">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <ServerIcon className="size-5 text-primary" />
            <CardTitle className="text-sm font-medium">Estado de Servicios</CardTitle>
          </div>
          <div className="flex items-center gap-2">
            <Badge
              variant={overallStatus === "running" ? "default" : "outline"}
              className={`
                ${overallStatus === "degraded" ? "border-amber-500 text-amber-500" : ""}
                ${overallStatus === "down" ? "border-rose-500 text-rose-500" : ""}
                ${overallStatus === "unknown" ? "border-slate-400 text-slate-400" : ""}
              `}
            >
              <overallConfig.icon className="mr-1 size-3" />
              {overallConfig.label}
            </Badge>
          </div>
        </div>
        <CardDescription>Infraestructura de simulación</CardDescription>
      </CardHeader>

      <CardContent className="space-y-3">
        {/* Resumen */}
        <div className="flex gap-2 text-xs">
          <div className="flex items-center gap-1">
            <div className="size-2 rounded-full bg-emerald-500" />
            <span className="text-muted-foreground">{runningCount} OK</span>
          </div>
          {degradedCount > 0 && (
            <div className="flex items-center gap-1">
              <div className="size-2 rounded-full bg-amber-500" />
              <span className="text-muted-foreground">{degradedCount} DEG</span>
            </div>
          )}
          {downCount > 0 && (
            <div className="flex items-center gap-1">
              <div className="size-2 rounded-full bg-rose-500" />
              <span className="text-muted-foreground">{downCount} DOWN</span>
            </div>
          )}
          {unknownCount > 0 && (
            <div className="flex items-center gap-1">
              <div className="size-2 rounded-full bg-slate-400" />
              <span className="text-muted-foreground">{unknownCount} ?</span>
            </div>
          )}
        </div>

        {/* Lista de servicios */}
        <div className="space-y-2">
          {services.map((service) => (
            <ServiceIndicator key={service.name} service={service} />
          ))}
        </div>

        {/* Estado de verificación */}
        <div className="flex items-center justify-between pt-2 text-xs text-muted-foreground">
          <span>{isChecking ? "Verificando..." : "Actualizado automáticamente"}</span>
          <span>Cada 10s</span>
        </div>
      </CardContent>
    </Card>
  );
}
