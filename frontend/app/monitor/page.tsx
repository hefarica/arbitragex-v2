import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import {
  ActivityIcon,
  BrainCircuitIcon,
  NetworkIcon,
  RadioIcon,
  ServerIcon,
  DatabaseIcon,
  LayersIcon,
  TrendingDownIcon,
  TrendingUpIcon,
  MinusIcon,
} from "lucide-react";
import { EntropyGauge } from "./components/EntropyGauge";
import { ServiceStatus } from "./components/ServiceStatus";
import { getApiBaseUrl } from "@/lib/api-client";

export const dynamic = "force-dynamic";
export const revalidate = 0;

// Tipos para el estado del sistema. Entropy/convergence son null cuando el
// edge no sirve ese campo (RULE 00 Zero-Mocks: NO fabricar telemetría).
interface MathGuardianState {
  status: "pass" | "fail" | "degraded" | "unavailable";
  lastCheck: string | null;
  entropyThreshold: number | null;
  convergenceRate: number | null;
}

interface TopologyState {
  activeChains: string[];
  totalManifolds: number;
  loopResolutions: number;
  orthogonalEquilibriums: number;
}

// Server Component - fetch inicial de datos desde /api/status (JSON).
// El edge NO sirve guardian/topology/entropy hoy → todos llegan como
// null/empty y la UI los muestra como NOT_AVAILABLE (R8 fail-honest).
// Nunca inyectamos defaults 0.92/0.42/["ethereum",...] disfrazados de vivo.
async function getInitialMetrics(): Promise<{
  guardian: MathGuardianState;
  topology: TopologyState;
  entropy: number | null;
}> {
  try {
    const base = getApiBaseUrl();
    const res = await fetch(`${base.replace(/\/$/, "")}/api/status`, {
      headers: { accept: "application/json" },
      cache: "no-store",
      next: { revalidate: 0 },
    });

    if (res.ok) {
      const data = await res.json();
      // El edge sirve { services, killswitch, ... }. Los campos de "guardian"
      // y "entropy" NO existen en el contrato real → siempre null aquí.
      return {
        guardian: {
          status: "unavailable",
          lastCheck: null,
          entropyThreshold: null,
          convergenceRate: null,
        },
        topology: {
          activeChains: [],
          totalManifolds: 0,
          loopResolutions: 0,
          orthogonalEquilibriums: 0,
        },
        entropy: null,
      };
    }
  } catch {
    // fall-through to unavailable
  }

  return {
    guardian: {
      status: "unavailable",
      lastCheck: null,
      entropyThreshold: null,
      convergenceRate: null,
    },
    topology: {
      activeChains: [],
      totalManifolds: 0,
      loopResolutions: 0,
      orthogonalEquilibriums: 0,
    },
    entropy: null,
  };
}

// Componente Math Guardian Status
function MathGuardianCard({ guardian }: { guardian: MathGuardianState }) {
  const statusConfig = {
    pass: { color: "bg-emerald-500", text: "PASS", icon: TrendingUpIcon },
    fail: { color: "bg-rose-500", text: "FAIL", icon: TrendingDownIcon },
    degraded: { color: "bg-amber-500", text: "DEGRADED", icon: MinusIcon },
    unavailable: { color: "bg-slate-400", text: "NOT_AVAILABLE", icon: MinusIcon },
  };

  const config = statusConfig[guardian.status];
  const Icon = config.icon;

  return (
    <Card className="relative overflow-hidden">
      <CardHeader className="pb-2">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <BrainCircuitIcon className="size-5 text-primary" />
            <CardTitle className="text-sm font-medium">Math Guardian</CardTitle>
          </div>
          <Badge
            variant={guardian.status === "pass" ? "default" : "outline"}
            className={guardian.status === "degraded" ? "border-amber-500 text-amber-500" : ""}
          >
            <Icon className="mr-1 size-3" />
            {config.text}
          </Badge>
        </div>
        <CardDescription>Validación de invariantes topológicas</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="flex items-center gap-3">
          <div className={`size-3 rounded-full ${config.color}`} />
          <span className="text-sm text-muted-foreground">
            Última verificación:{" "}
            {guardian.lastCheck
              ? new Date(guardian.lastCheck).toLocaleTimeString()
              : "NOT_AVAILABLE"}
          </span>
        </div>
        <Separator />
        <div className="grid grid-cols-2 gap-4">
          <div className="space-y-1">
            <p className="text-xs text-muted-foreground">Umbral de Entropía</p>
            <p className="text-lg font-mono font-semibold">
              {guardian.entropyThreshold != null
                ? `${(guardian.entropyThreshold * 100).toFixed(1)}%`
                : "NOT_AVAILABLE"}
            </p>
          </div>
          <div className="space-y-1">
            <p className="text-xs text-muted-foreground">Tasa de Convergencia</p>
            <p className="text-lg font-mono font-semibold">
              {guardian.convergenceRate != null
                ? `${(guardian.convergenceRate * 100).toFixed(1)}%`
                : "NOT_AVAILABLE"}
            </p>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

// Componente Topology Status
function TopologyCard({ topology }: { topology: TopologyState }) {
  const chainLabels: Record<string, string> = {
    ethereum: "ETH",
    arbitrum: "ARB",
    base: "BASE",
  };

  return (
    <Card>
      <CardHeader className="pb-2">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <NetworkIcon className="size-5 text-primary" />
            <CardTitle className="text-sm font-medium">Topología de Red</CardTitle>
          </div>
          <Badge variant="outline">{topology.activeChains.length} cadenas</Badge>
        </div>
        <CardDescription>Estado de variedades de liquidez</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="flex gap-2">
          {topology.activeChains.length > 0 ? (
            topology.activeChains.map((chain) => (
              <Badge key={chain} variant="secondary" className="font-mono">
                {chainLabels[chain] ?? chain.toUpperCase()}
              </Badge>
            ))
          ) : (
            <span className="text-sm font-mono text-muted-foreground">NOT_AVAILABLE</span>
          )}
        </div>
        <Separator />
        <div className="grid grid-cols-3 gap-4">
          <div className="space-y-1">
            <p className="text-xs text-muted-foreground">Variedades</p>
            <p className="text-lg font-mono font-semibold">{topology.totalManifolds}</p>
          </div>
          <div className="space-y-1">
            <p className="text-xs text-muted-foreground">Res. Holonómicas</p>
            <p className="text-lg font-mono font-semibold">{topology.loopResolutions}</p>
          </div>
          <div className="space-y-1">
            <p className="text-xs text-muted-foreground">Equilibrios Ort.</p>
            <p className="text-lg font-mono font-semibold">{topology.orthogonalEquilibriums}</p>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

// Componente de métricas en tiempo real (Client Component wrapper)
import { MetricsStream } from "./components/MetricsStream";

export default async function MonitorPage() {
  const initial = await getInitialMetrics();

  return (
    <div className="min-h-screen bg-background p-6 lg:p-8">
      {/* Header */}
      <div className="mb-8 space-y-2">
        <div className="flex items-center gap-2 text-xs uppercase tracking-widest text-muted-foreground">
          <RadioIcon className="size-4 animate-pulse text-primary" />
          Observatorio en Tiempo Real
        </div>
        <h1 className="text-3xl font-bold tracking-tight">SISTEMA OMEGA: MONITOR</h1>
        <p className="max-w-2xl text-base text-muted-foreground">
          Panel de observación de métricas matemáticas y estado de convergencia estocástica.
          Los datos se actualizan vía Socket.IO desde el núcleo de simulación.
        </p>
      </div>

      {/* Grid principal de métricas */}
      <div className="grid gap-6 lg:grid-cols-3">
        {/* Math Guardian */}
        <MathGuardianCard guardian={initial.guardian} />

        {/* Entropy Gauge - Client Component con streaming */}
        <EntropyGauge initialValue={initial.entropy} />

        {/* Service Status */}
        <ServiceStatus />
      </div>

      <Separator className="my-8" />

      {/* Segunda fila: Topología y métricas detalladas */}
      <div className="grid gap-6 lg:grid-cols-2">
        <TopologyCard topology={initial.topology} />

        {/* Métricas en tiempo real - Client Component */}
        <MetricsStream />
      </div>

      <Separator className="my-8" />

      {/* Footer informativo */}
      <div className="flex items-center justify-between text-xs text-muted-foreground">
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-1.5">
            <ActivityIcon className="size-3.5" />
            <span>Socket.IO: same-origin</span>
          </div>
          <div className="flex items-center gap-1.5">
            <ServerIcon className="size-3.5" />
            <span>Edge: proxied (same-origin)</span>
          </div>
        </div>
        <div className="flex items-center gap-1.5">
          <LayersIcon className="size-3.5" />
          <span>v2.0.0-omega</span>
        </div>
      </div>
    </div>
  );
}
