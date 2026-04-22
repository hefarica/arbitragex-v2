import { Fragment } from "react";
import { AlertCircleIcon, ShieldCheckIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { PageHeader } from "@/components/page-header";
import { getConfigCurrent } from "@/lib/api-client";

export const dynamic = "force-dynamic";
export const revalidate = 0;

function KV({ obj }: { obj: Record<string, unknown> }) {
  return (
    <dl className="grid grid-cols-[max-content_1fr] gap-x-6 gap-y-1.5 text-sm">
      {Object.entries(obj).map(([k, v]) => (
        <Fragment key={k}>
          <dt className="text-muted-foreground">{k}</dt>
          <dd className="font-mono text-[13px]">{String(v)}</dd>
        </Fragment>
      ))}
    </dl>
  );
}

export default async function ConfigPage() {
  const res = await getConfigCurrent();

  if (!res.ok) {
    return (
      <>
        <PageHeader title="Current config" lede="Loaded application settings — secrets redacted at the edge." showRefresh />
        <Alert variant="destructive">
          <AlertCircleIcon />
          <AlertTitle>edge error</AlertTitle>
          <AlertDescription><code className="font-mono text-xs">{res.error}</code></AlertDescription>
        </Alert>
      </>
    );
  }

  const c = res.data;
  const paperOn = c.execution["paper_mode"] === true;

  return (
    <>
      <PageHeader
        title="Current config"
        lede="Loaded from configs/app.toml at service boot. Secrets are never rendered here — to rotate keys see the rotate-secrets runbook."
        showRefresh
      />

      {paperOn && (
        <Alert variant="success" className="mb-6">
          <ShieldCheckIcon />
          <AlertTitle>Paper-mode is ON</AlertTitle>
          <AlertDescription>Executions stop at simulation; the relay pipeline is active but capital is not at risk.</AlertDescription>
        </Alert>
      )}

      <div className="mb-6 grid gap-4 lg:grid-cols-2">
        <Card>
          <CardHeader><CardTitle>System</CardTitle></CardHeader>
          <CardContent><KV obj={c.system} /></CardContent>
        </Card>
        <Card>
          <CardHeader><CardTitle>Execution</CardTitle></CardHeader>
          <CardContent><KV obj={c.execution} /></CardContent>
        </Card>
        <Card>
          <CardHeader><CardTitle>Risk</CardTitle></CardHeader>
          <CardContent><KV obj={c.risk} /></CardContent>
        </Card>
        <Card>
          <CardHeader><CardTitle>Scoring</CardTitle></CardHeader>
          <CardContent><KV obj={c.scoring} /></CardContent>
        </Card>
      </div>

      <h2>Chains</h2>
      <Card className="mb-6 py-0">
        <CardContent className="p-0">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>chain_id</TableHead>
                <TableHead>name</TableHead>
                <TableHead>enabled</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {c.chains.map((ch) => (
                <TableRow key={ch.chain_id}>
                  <TableCell className="font-mono tabular-nums">{ch.chain_id}</TableCell>
                  <TableCell>{ch.name}</TableCell>
                  <TableCell>
                    <Badge variant={ch.enabled ? "success" : "outline"}>{ch.enabled ? "yes" : "no"}</Badge>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>

      <h2>Relays</h2>
      <Card className="mb-6 py-0">
        <CardContent className="p-0">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>name</TableHead>
                <TableHead>enabled</TableHead>
                <TableHead>chains</TableHead>
                <TableHead>endpoint</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {c.relays.map((r) => (
                <TableRow key={r.name}>
                  <TableCell>{r.name}</TableCell>
                  <TableCell>
                    <Badge variant={r.enabled ? "success" : "outline"}>{r.enabled ? "yes" : "no"}</Badge>
                  </TableCell>
                  <TableCell className="font-mono tabular-nums">{r.chains.join(", ")}</TableCell>
                  <TableCell className="font-mono text-xs text-muted-foreground">{r.endpoint || "—"}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>

      <h2>Circuit breakers</h2>
      <Card className="py-0">
        <CardContent className="p-0">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>name</TableHead>
                <TableHead className="text-right">threshold</TableHead>
                <TableHead className="text-right">window_ms</TableHead>
                <TableHead className="text-right">cooldown_ms</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {c.circuit_breakers.map((cb) => (
                <TableRow key={cb.name}>
                  <TableCell><Badge variant="default">{cb.name}</Badge></TableCell>
                  <TableCell className="text-right font-mono tabular-nums">{cb.threshold}</TableCell>
                  <TableCell className="text-right font-mono tabular-nums">{cb.window_ms}</TableCell>
                  <TableCell className="text-right font-mono tabular-nums">{cb.cooldown_ms}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </>
  );
}
