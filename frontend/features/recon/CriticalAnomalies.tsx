import { CheckCircle2Icon } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import type { ReconSummary } from "@/lib/api-client";
import { fmtDateTime } from "@/lib/formatters";

export function CriticalAnomalies({ rows }: { rows: ReconSummary["critical_anomalies_24h"] }) {
  if (rows.length === 0) {
    return (
      <Card className="py-14">
        <CardContent className="flex flex-col items-center gap-3 text-center">
          <div className="flex size-12 items-center justify-center rounded-full bg-success/10 text-success">
            <CheckCircle2Icon className="size-6" />
          </div>
          <div className="text-lg font-medium">None.</div>
        </CardContent>
      </Card>
    );
  }

  return (
    <div className="space-y-4">
      {rows.map((a, i) => (
        <Card key={i} className="border-destructive/40">
          <CardHeader>
            <div className="flex flex-wrap items-center gap-2">
              <Badge variant="destructive">{a.event_type}</Badge>
              <span className="text-xs text-muted-foreground font-mono">{a.source_service}</span>
              <span className="ml-auto text-xs text-muted-foreground">{fmtDateTime(a.created_at)}</span>
            </div>
          </CardHeader>
          <CardContent>
            <pre className="overflow-auto rounded-md border bg-muted/40 p-3 text-xs font-mono">
              {JSON.stringify(a.payload, null, 2)}
            </pre>
          </CardContent>
        </Card>
      ))}
    </div>
  );
}
