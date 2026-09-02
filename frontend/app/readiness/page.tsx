import { InfoIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { FocusOnMount } from "@/components/focus-on-mount";
import { PageHeader } from "@/components/page-header";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { getReadinessBlockers, getReadinessDecision } from "@/lib/api-client";

export const dynamic = "force-dynamic";
export const revalidate = 0;

// RULE 00 (Zero Mocks) — this page previously rendered a hardcoded
// `gateStatus` object ("static/mock gate status for now"). It now derives the
// gate view from the REAL backend aggregates: /api/readiness/blockers and
// /api/readiness/decision (same source of truth as /live-readiness). When the
// edge is unreachable the page says so honestly instead of fabricating gates.

function severityVariant(sev: string): "success" | "warning" | "destructive" {
  if (sev === "critical" || sev === "high") return "destructive";
  if (sev === "medium") return "warning";
  return "success";
}

export default async function ReadinessPage() {
  const [blockersRes, decisionRes] = await Promise.all([
    getReadinessBlockers(),
    getReadinessDecision(),
  ]);

  const fetchFailed = !blockersRes.ok && !decisionRes.ok;
  const blockers = blockersRes.ok ? blockersRes.data.blockers : [];
  const summary = blockersRes.ok ? blockersRes.data.summary : null;
  const decision = decisionRes.ok ? decisionRes.data : null;

  const verdict = decision?.verdict ?? (blockersRes.ok ? blockersRes.data.overall_status : "unavailable");
  const totalPending =
    summary != null ? summary.critical + summary.high + summary.medium + summary.low : null;

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
          <AlertTitle>Fuente de verdad: backend</AlertTitle>
          <AlertDescription>
            Estado derivado de <code>/api/readiness/blockers</code> y{" "}
            <code>/api/readiness/decision</code> — los mismos agregados que alimentan{" "}
            <code>/live-readiness</code>. Doctrina Zero-Mocks: sin datos fabricados.
          </AlertDescription>
        </Alert>
      </FocusOnMount>

      {fetchFailed && (
        <Alert variant="destructive" className="mb-6">
          <InfoIcon />
          <AlertTitle>Backend no disponible</AlertTitle>
          <AlertDescription>
            No se pudo obtener <code>/api/readiness/blockers</code> ni{" "}
            <code>/api/readiness/decision</code>. R8 fail-honest: no se fabrican gates.
          </AlertDescription>
        </Alert>
      )}

      {/* Overall verdict card */}
      <div className="mb-6">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-3">
              Overall Status
              <Badge variant={decision?.go_live ? "success" : "destructive"}>
                {String(verdict).toUpperCase()}
              </Badge>
              {totalPending != null && (
                <Badge variant={totalPending === 0 ? "success" : "warning"}>
                  {totalPending === 0 ? "0 PENDING" : `${totalPending} PENDING`}
                </Badge>
              )}
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-2">
            {decision != null ? (
              <>
                <p className="text-sm text-muted-foreground">
                  Fase: <span className="font-mono text-foreground">{decision.phase}</span>
                  {" · "}
                  Capital expuesto:{" "}
                  <span className="font-mono text-foreground">
                    ${decision.capital_exposure_usd.toFixed(2)}
                  </span>
                  {" · "}
                  Live trading:{" "}
                  <span className="font-mono text-foreground">
                    {decision.live_trading ? "ON" : "OFF"}
                  </span>
                </p>
                <p className="text-sm text-muted-foreground">
                  Siguiente acción:{" "}
                  <span className="font-mono text-foreground">{decision.next_action}</span>
                </p>
              </>
            ) : (
              <p className="text-sm text-muted-foreground">
                Veredicto de decisión no disponible desde el backend.
              </p>
            )}
          </CardContent>
        </Card>
      </div>

      {/* Blockers grid — real, from the API */}
      {blockers.length === 0 && !fetchFailed ? (
        <Card>
          <CardContent className="pt-6">
            <p className="text-sm text-muted-foreground">
              {blockersRes.ok
                ? "Sin blockers reportados por el backend — 0 pendientes."
                : "No se pudieron cargar los blockers."}
            </p>
          </CardContent>
        </Card>
      ) : (
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {blockers.map((b) => (
            <Card key={b.id}>
              <CardHeader className="pb-3">
                <CardTitle className="flex items-center justify-between text-base gap-2">
                  <span className="min-w-0 flex-1 truncate" title={b.title}>
                    {b.title}
                  </span>
                  <Badge variant={severityVariant(b.severity)}>{b.severity}</Badge>
                </CardTitle>
              </CardHeader>
              <CardContent className="pt-0 space-y-1">
                <p className="text-xs font-mono text-muted-foreground uppercase tracking-wide">
                  {b.category} · {b.status}
                </p>
                <p className="text-sm text-muted-foreground">{b.description}</p>
                <p className="text-xs text-muted-foreground pt-1 border-t border-border/50">
                  Acción: <span className="text-foreground">{b.required_action}</span>
                </p>
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </>
  );
}
