import { AlertCircleIcon, ZapIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { PageHeader } from "@/components/page-header";
import { getExecutionsRecent, type ExecutionRow } from "@/lib/api-client";

export const dynamic = "force-dynamic";
export const revalidate = 0;

const fmtMoney = (n: number | null) => (n == null ? "—" : `$${n.toFixed(2)}`);

function statusVariant(s: ExecutionRow["status"]): "success" | "destructive" | "warning" | "info" | "outline" {
  if (s === "included") return "success";
  if (s === "reverted") return "destructive";
  if (s === "dropped" || s === "replaced") return "warning";
  if (s === "submitted") return "info";
  return "outline";
}

export default async function ExecutionsPage() {
  const res = await getExecutionsRecent(100);

  return (
    <>
      <PageHeader
        title="Recent executions"
        lede="Bundle submissions and their outcomes. Paper-mode is ON until S9; real relays only record submitted / not_implemented until the safety rail is flipped."
      />

      {!res.ok ? (
        <Alert variant="destructive">
          <AlertCircleIcon />
          <AlertTitle>edge error</AlertTitle>
          <AlertDescription><code className="font-mono text-xs">{res.error}</code></AlertDescription>
        </Alert>
      ) : res.data.count === 0 ? (
        <Card className="py-14">
          <CardContent className="flex flex-col items-center gap-3 text-center">
            <div className="flex size-12 items-center justify-center rounded-full bg-muted text-muted-foreground">
              <ZapIcon className="size-6" />
            </div>
            <div className="text-lg font-medium">No executions yet.</div>
            <p className="max-w-md text-sm text-muted-foreground">
              This table fills when simulated opportunities reach the relay submission path.
            </p>
          </CardContent>
        </Card>
      ) : (
        <>
          <div className="mb-4 flex flex-wrap gap-4 text-xs uppercase tracking-widest text-muted-foreground/70">
            <span>{res.data.count} rows</span>
            <span>snapshot {new Date(res.data.ts).toLocaleTimeString()}</span>
          </div>
          <Card className="py-0">
            <CardContent className="p-0">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Submitted</TableHead>
                    <TableHead>Chain</TableHead>
                    <TableHead>Strategy</TableHead>
                    <TableHead>Pair</TableHead>
                    <TableHead>Relay</TableHead>
                    <TableHead>Status</TableHead>
                    <TableHead>Tx</TableHead>
                    <TableHead className="text-right">Block</TableHead>
                    <TableHead className="text-right">PnL</TableHead>
                    <TableHead>Error</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {res.data.items.map((e) => (
                    <TableRow key={e.id}>
                      <TableCell className="text-muted-foreground">{new Date(e.submitted_at).toLocaleTimeString()}</TableCell>
                      <TableCell>{e.chain_id}</TableCell>
                      <TableCell>{e.strategy_kind}</TableCell>
                      <TableCell>{e.pair_symbol ?? "—"}</TableCell>
                      <TableCell><Badge variant="outline">{e.relay_name}</Badge></TableCell>
                      <TableCell><Badge variant={statusVariant(e.status)}>{e.status}</Badge></TableCell>
                      <TableCell className="font-mono text-xs text-muted-foreground">{e.tx_hash ? `${e.tx_hash.slice(0, 10)}…` : "—"}</TableCell>
                      <TableCell className="text-right font-mono tabular-nums">{e.block_included ?? "—"}</TableCell>
                      <TableCell className="text-right font-mono tabular-nums">{fmtMoney(e.actual_profit_usd)}</TableCell>
                      <TableCell className="max-w-[240px] truncate text-muted-foreground">{e.error_message ?? ""}</TableCell>
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
