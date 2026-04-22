import { AlertCircleIcon, RadarIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { PageHeader } from "@/components/page-header";
import { getOpportunitiesLive } from "@/lib/api-client";

export const dynamic = "force-dynamic";
export const revalidate = 0;

const fmtMoney = (n: number | null) => (n == null ? "—" : `$${n.toFixed(2)}`);
const fmtPct = (n: number | null) => (n == null ? "—" : `${n.toFixed(2)}%`);

export default async function OpportunitiesPage() {
  const res = await getOpportunitiesLive(100);

  return (
    <>
      <PageHeader
        title="Live opportunities"
        lede="Recent candidates detected from the mempool. Nothing here is synthesized; if the scanner has no RPC, the list stays empty — that's honest."
      />

      {!res.ok ? (
        <Alert variant="destructive">
          <AlertCircleIcon />
          <AlertTitle>edge error</AlertTitle>
          <AlertDescription>
            <code className="font-mono text-xs">{res.error}</code>
          </AlertDescription>
        </Alert>
      ) : res.data.count === 0 ? (
        <EmptyState />
      ) : (
        <>
          <div className="mb-4 flex flex-wrap gap-4 text-xs uppercase tracking-widest text-muted-foreground/70">
            <span>{res.data.count} rows</span>
            <span>window: {res.data.window}</span>
            <span>snapshot {new Date(res.data.ts).toLocaleTimeString()}</span>
          </div>
          <Card className="py-0">
            <CardContent className="p-0">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Detected</TableHead>
                    <TableHead>Chain</TableHead>
                    <TableHead>Strategy</TableHead>
                    <TableHead>Pair</TableHead>
                    <TableHead className="text-right">Est. PnL</TableHead>
                    <TableHead className="text-right">ROI</TableHead>
                    <TableHead>Status</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {res.data.items.map((o) => (
                    <TableRow key={o.id}>
                      <TableCell className="text-muted-foreground">
                        {new Date(o.detected_at).toLocaleTimeString()}
                      </TableCell>
                      <TableCell>{o.chain_id}</TableCell>
                      <TableCell><Badge variant="info">{o.strategy_kind}</Badge></TableCell>
                      <TableCell>{o.pair_symbol ?? "—"}</TableCell>
                      <TableCell className="text-right font-mono tabular-nums">{fmtMoney(o.expected_profit_usd)}</TableCell>
                      <TableCell className="text-right font-mono tabular-nums">{fmtPct(o.roi_pct)}</TableCell>
                      <TableCell><Badge variant="outline">{o.status}</Badge></TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </CardContent>
          </Card>
        </>
      )}
    </>
  );
}

function EmptyState() {
  return (
    <Card className="py-14">
      <CardContent className="flex flex-col items-center gap-3 text-center">
        <div className="flex size-12 items-center justify-center rounded-full bg-muted text-muted-foreground">
          <RadarIcon className="size-6" />
        </div>
        <div className="text-lg font-medium">No opportunities in flight.</div>
        <p className="max-w-md text-sm text-muted-foreground">
          Either the scanner has not connected to an RPC yet, or the market currently
          has nothing worth executing. Check <a className="underline underline-offset-4" href="/status">system status</a>.
        </p>
      </CardContent>
    </Card>
  );
}
