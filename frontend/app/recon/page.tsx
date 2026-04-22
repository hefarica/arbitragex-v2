import { AlertCircleIcon, CheckCircle2Icon, TrendingUpIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { PageHeader } from "@/components/page-header";
import { getReconSummary } from "@/lib/api-client";

export const dynamic = "force-dynamic";
export const revalidate = 0;

const pct = (n: number | null) => (n == null ? "—" : (n * 100).toFixed(2) + "%");
const usd = (n: number | null) => (n == null ? "—" : "$" + n.toFixed(2));
const ms  = (n: number | null) => (n == null ? "—" : n.toFixed(0) + " ms");

export default async function ReconPage() {
  const res = await getReconSummary(1);

  if (!res.ok) {
    return (
      <>
        <PageHeader title="Recon & PnL" lede="Realised PnL, adaptive strategy scores, critical anomalies." />
        <Alert variant="destructive">
          <AlertCircleIcon />
          <AlertTitle>edge error</AlertTitle>
          <AlertDescription><code className="font-mono text-xs">{res.error}</code></AlertDescription>
        </Alert>
      </>
    );
  }

  const s = res.data;

  return (
    <>
      <PageHeader
        title="Recon & PnL"
        lede="Realised PnL and adaptive strategy scoring — both populated by the recon learning loop (S6) as executions are reconciled against their pre-trade simulation."
        meta={[`window: last ${s.window_hours}h`, `snapshot ${new Date(s.ts).toLocaleTimeString()}`]}
      />

      <div className="mb-8 grid gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-6">
        <Kpi title="Attempts"            value={String(s.totals.total)}                hint={`in last ${s.window_hours}h`} />
        <Kpi title="Included"             value={String(s.totals.included)}             hint="blocks confirmed" tone="success" />
        <Kpi title="Reverted"             value={String(s.totals.reverted)}             hint="on-chain reverts" tone="danger" />
        <Kpi title="Revert rate"          value={pct(s.revert_rate)}                     hint="lower is better" />
        <Kpi title="Avg PnL (incl.)"      value={usd(s.totals.avg_pnl_included_usd)}    hint="per included tx" />
        <Kpi title="Avg confirm latency"  value={ms(s.totals.avg_confirm_latency_ms)}   hint="submit → block" />
      </div>

      <h2>Top strategies</h2>
      {s.top_strategies.length === 0 ? (
        <Card className="py-14">
          <CardContent className="flex flex-col items-center gap-3 text-center">
            <div className="flex size-12 items-center justify-center rounded-full bg-muted text-muted-foreground">
              <TrendingUpIcon className="size-6" />
            </div>
            <div className="text-lg font-medium">No strategy scores yet.</div>
            <p className="max-w-md text-sm text-muted-foreground">
              The recon learning loop populates this after the first aggregation window closes.
            </p>
          </CardContent>
        </Card>
      ) : (
        <Card className="py-0">
          <CardContent className="p-0">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Strategy</TableHead>
                  <TableHead>Chain</TableHead>
                  <TableHead className="text-right">Samples</TableHead>
                  <TableHead className="text-right">Success</TableHead>
                  <TableHead className="text-right">Revert</TableHead>
                  <TableHead className="text-right">Avg PnL</TableHead>
                  <TableHead className="text-right">Score</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {s.top_strategies.map((row, i) => (
                  <TableRow key={`${row.strategy_kind}-${row.chain_id}-${i}`}>
                    <TableCell><Badge variant="info">{row.strategy_kind}</Badge></TableCell>
                    <TableCell>{row.chain_id}</TableCell>
                    <TableCell className="text-right font-mono tabular-nums">{row.sample_count}</TableCell>
                    <TableCell className="text-right font-mono tabular-nums">{pct(row.success_rate)}</TableCell>
                    <TableCell className="text-right font-mono tabular-nums">{pct(row.revert_rate)}</TableCell>
                    <TableCell className="text-right font-mono tabular-nums">{usd(row.avg_profit_usd)}</TableCell>
                    <TableCell className="text-right font-mono tabular-nums font-semibold">
                      {row.score != null ? row.score.toFixed(2) : "—"}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}

      <h2>Critical anomalies (last 24h)</h2>
      {s.critical_anomalies_24h.length === 0 ? (
        <Card className="py-14">
          <CardContent className="flex flex-col items-center gap-3 text-center">
            <div className="flex size-12 items-center justify-center rounded-full bg-success/10 text-success">
              <CheckCircle2Icon className="size-6" />
            </div>
            <div className="text-lg font-medium">None.</div>
          </CardContent>
        </Card>
      ) : (
        <div className="space-y-4">
          {s.critical_anomalies_24h.map((a, i) => (
            <Card key={i} className="border-destructive/40">
              <CardHeader>
                <div className="flex flex-wrap items-center gap-2">
                  <Badge variant="destructive">{a.event_type}</Badge>
                  <span className="text-xs text-muted-foreground font-mono">{a.source_service}</span>
                  <span className="ml-auto text-xs text-muted-foreground">
                    {new Date(a.created_at).toLocaleString()}
                  </span>
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
      )}
    </>
  );
}

function Kpi({ title, value, hint, tone }: { title: string; value: string; hint?: string; tone?: "success" | "danger" | "warning" | "info" }) {
  const toneRing: Record<string, string> = {
    success: "bg-success/10 text-success",
    danger: "bg-destructive/10 text-destructive",
    warning: "bg-warning/15 text-warning",
    info: "bg-info/10 text-info",
  };
  return (
    <Card>
      <CardHeader>
        <div className={`inline-flex w-fit items-center gap-2 rounded-md px-2 py-0.5 text-[11px] uppercase tracking-widest ${tone ? toneRing[tone] : "bg-muted text-muted-foreground"}`}>
          {title}
        </div>
        <CardTitle className="mt-2 text-2xl font-medium tracking-tight font-mono tabular-nums">{value}</CardTitle>
        {hint && <CardDescription>{hint}</CardDescription>}
      </CardHeader>
    </Card>
  );
}
