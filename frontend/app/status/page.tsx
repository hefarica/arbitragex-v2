import { AlertCircleIcon, CheckCircle2Icon, ShieldCheckIcon, ShieldOffIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { PageHeader } from "@/components/page-header";
import { getStatus } from "@/lib/api-client";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export default async function StatusPage() {
  const res = await getStatus();

  if (!res.ok) {
    return (
      <>
        <PageHeader
          title="System status"
          lede="Live operator view of every hot-path service + kill-switch state."
        />
        <Alert variant="destructive">
          <AlertCircleIcon />
          <AlertTitle>edge unreachable</AlertTitle>
          <AlertDescription>
            <code className="rounded bg-destructive/10 px-1.5 py-0.5 font-mono text-xs">{res.error}</code>
            <p className="mt-2">
              This view displays only real edge data. There is no fallback. If the edge is down,
              this page shows the error — it never synthesizes values.
            </p>
          </AlertDescription>
        </Alert>
      </>
    );
  }

  const s = res.data;
  const ksArmed = s.killswitch?.enabled ?? false;

  return (
    <>
      <PageHeader
        title="System status"
        lede="Live operator view of every hot-path service + kill-switch state."
        meta={[`env: ${s.env}`, `api-server v${s.version}`, `as of ${new Date(s.ts).toLocaleTimeString()}`]}
      />

      <div className="mb-8 grid gap-4 sm:grid-cols-3">
        <Kpi
          title="Overall"
          value={s.ok ? "OK" : "DEGRADED"}
          tone={s.ok ? "success" : "danger"}
          hint={`${Object.keys(s.services).length} services monitored`}
          icon={s.ok ? <CheckCircle2Icon className="size-4" /> : <AlertCircleIcon className="size-4" />}
        />
        <Kpi
          title="Kill-switch"
          value={ksArmed ? "ARMED" : "disabled"}
          tone={ksArmed ? "danger" : "success"}
          hint={s.killswitch?.reason ?? "no reason set"}
          icon={ksArmed ? <ShieldOffIcon className="size-4" /> : <ShieldCheckIcon className="size-4" />}
        />
        <Kpi
          title="Paper-mode"
          value="ON"
          tone="success"
          hint="Real capital is not at risk until S9."
          icon={<ShieldCheckIcon className="size-4" />}
        />
      </div>

      <h2>Services</h2>
      <Card className="py-0">
        <CardContent className="p-0">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Service</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Detail</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {Object.entries(s.services).map(([name, v]) => (
                <TableRow key={name}>
                  <TableCell className="font-mono text-sm">{name}</TableCell>
                  <TableCell>
                    <Badge variant={v.ok ? "success" : "destructive"}>
                      <span
                        className={`size-1.5 rounded-full ${v.ok ? "bg-success" : "bg-destructive"}`}
                        aria-hidden
                      />
                      {v.ok ? "UP" : "DOWN"}
                    </Badge>
                  </TableCell>
                  <TableCell className="text-muted-foreground">
                    {v.status ? `HTTP ${v.status}` : (v.detail ?? "—")}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </>
  );
}

function Kpi({
  title, value, tone, hint, icon,
}: { title: string; value: string; tone: "success" | "warning" | "danger" | "info"; hint?: string; icon?: React.ReactNode }) {
  const toneClasses: Record<typeof tone, string> = {
    success: "text-success",
    warning: "text-warning",
    danger: "text-destructive",
    info: "text-info",
  };
  return (
    <Card>
      <CardHeader>
        <div className={`flex items-center gap-2 text-xs uppercase tracking-widest ${toneClasses[tone]}`}>
          {icon}
          <span>{title}</span>
        </div>
        <CardTitle className="mt-2 text-3xl font-medium tracking-tight">{value}</CardTitle>
        {hint && <CardDescription>{hint}</CardDescription>}
      </CardHeader>
    </Card>
  );
}
