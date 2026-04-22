import { AlertCircleIcon, ShieldCheckIcon, ShieldOffIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { PageHeader } from "@/components/page-header";
import { getRiskAlerts, type RiskAlertRow } from "@/lib/api-client";

export const dynamic = "force-dynamic";
export const revalidate = 0;

function sevVariant(s: RiskAlertRow["severity"]): "destructive" | "warning" | "info" {
  if (s === "critical") return "destructive";
  if (s === "warning") return "warning";
  return "info";
}

export default async function RiskPage() {
  const res = await getRiskAlerts(24);

  if (!res.ok) {
    return (
      <>
        <PageHeader title="Risk & alerts" lede="Circuit-breakers, anomalies, blacklist hits, kill-switch log." />
        <Alert variant="destructive">
          <AlertCircleIcon />
          <AlertTitle>edge error</AlertTitle>
          <AlertDescription><code className="font-mono text-xs">{res.error}</code></AlertDescription>
        </Alert>
      </>
    );
  }

  const { alerts, window_hours, killswitch, ts } = res.data;

  return (
    <>
      <PageHeader
        title="Risk & alerts"
        lede={`Active kill-switch state and every risk event recorded by services in the last ${window_hours}h.`}
        meta={[`window: last ${window_hours}h`, `snapshot ${new Date(ts).toLocaleTimeString()}`]}
      />

      {killswitch && (
        <Alert variant={killswitch.enabled ? "destructive" : "success"} className="mb-6">
          {killswitch.enabled ? <ShieldOffIcon /> : <ShieldCheckIcon />}
          <AlertTitle>
            Kill-switch: {killswitch.enabled ? "ARMED — executions blocked" : "disabled — executions permitted"}
          </AlertTitle>
          <AlertDescription>
            {killswitch.reason && <div>reason: <em>{killswitch.reason}</em></div>}
            <div className="text-xs">
              updated {new Date(killswitch.updated_at).toLocaleString()}
              {killswitch.triggered_by && <> · by <code className="font-mono">{killswitch.triggered_by}</code></>}
            </div>
          </AlertDescription>
        </Alert>
      )}

      <h2>Recent events</h2>
      {alerts.length === 0 ? (
        <Card className="py-14">
          <CardContent className="flex flex-col items-center gap-3 text-center">
            <div className="flex size-12 items-center justify-center rounded-full bg-success/10 text-success">
              <ShieldCheckIcon className="size-6" />
            </div>
            <div className="text-lg font-medium">All clear.</div>
            <p className="max-w-md text-sm text-muted-foreground">
              No warnings or criticals logged in the last {window_hours}h.
            </p>
          </CardContent>
        </Card>
      ) : (
        <Card className="py-0">
          <CardContent className="p-0">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Time</TableHead>
                  <TableHead>Severity</TableHead>
                  <TableHead>Type</TableHead>
                  <TableHead>Source</TableHead>
                  <TableHead>Details</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {alerts.map((a) => (
                  <TableRow key={a.id}>
                    <TableCell className="text-muted-foreground">{new Date(a.created_at).toLocaleTimeString()}</TableCell>
                    <TableCell><Badge variant={sevVariant(a.severity)}>{a.severity}</Badge></TableCell>
                    <TableCell>{a.event_type}</TableCell>
                    <TableCell className="font-mono text-xs">{a.source_service}</TableCell>
                    <TableCell className="max-w-[360px] truncate font-mono text-xs text-muted-foreground">
                      {JSON.stringify(a.payload).slice(0, 200)}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}
    </>
  );
}
