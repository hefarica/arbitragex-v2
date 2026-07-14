import { InfoIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { FocusOnMount } from "@/components/focus-on-mount";
import { PageHeader } from "@/components/page-header";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export default async function ReadinessPage() {
  // Static/mock gate status for now — this page is under development
  const gateStatus = {
    gates: [
      { name: "CI/CD Pipeline", status: "green", description: "All checks passing" },
      { name: "Security Audit", status: "yellow", description: "In progress — 4 items remaining" },
      { name: "Simulation Parity", status: "green", description: "Core simulators validated" },
      { name: "Paper Trade", status: "green", description: "Paper mode active and verified" },
      { name: "Live Trading", status: "red", description: "Blocked — awaiting final review" },
      { name: "Kill Switch", status: "green", description: "Redis-based killswitch operational" },
    ],
    overall: "blocked",
  };

  return (
    <>
      <PageHeader
        title="Readiness Dashboard"
        lede="Live readiness gate status and deployment preparation."
        showRefresh
      />

      <FocusOnMount>
        <Alert variant="default" className="mb-6">
          <InfoIcon />
          <AlertTitle>Módulo en desarrollo</AlertTitle>
          <AlertDescription>
            El panel de readiness completo estará disponible próximamente.
            Esta vista muestra el estado actual de los gates de preparación para deployment.
          </AlertDescription>
        </Alert>
      </FocusOnMount>

      <div className="mb-6">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-3">
              Overall Status
              <Badge
                variant={gateStatus.overall === "green" ? "success" : gateStatus.overall === "yellow" ? "warning" : "destructive"}
              >
                {gateStatus.overall.toUpperCase()}
              </Badge>
            </CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-sm text-muted-foreground">
              Live trading remains blocked until all gates achieve green status.
              Current blockers: Security audit completion, final operator sign-off.
            </p>
          </CardContent>
        </Card>
      </div>

      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
        {gateStatus.gates.map((gate) => (
          <Card key={gate.name}>
            <CardHeader className="pb-3">
              <CardTitle className="flex items-center justify-between text-base">
                {gate.name}
                <Badge
                  variant={
                    gate.status === "green"
                      ? "success"
                      : gate.status === "yellow"
                      ? "warning"
                      : "destructive"
                  }
                >
                  {gate.status}
                </Badge>
              </CardTitle>
            </CardHeader>
            <CardContent className="pt-0">
              <p className="text-sm text-muted-foreground">{gate.description}</p>
            </CardContent>
          </Card>
        ))}
      </div>
    </>
  );
}
