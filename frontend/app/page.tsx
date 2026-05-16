import { getApiBaseUrl } from "@/lib/api-client";
import Link from "next/link";
import {
  ActivityIcon,
  AlertTriangleIcon,
  ArrowUpRightIcon,
  GaugeIcon,
  PowerIcon,
  SatelliteDishIcon,
  SettingsIcon,
  ZapIcon,
  type LucideIcon,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { HomeKpiStrip } from "@/features/home/HomeKpiStrip";
import { ProgressRealCard } from "@/features/home/ProgressRealCard";
import { getReconSummary, getStatus } from "@/lib/api-client";

export const dynamic = "force-dynamic";
export const revalidate = 0;

type Tile = {
  href: string;
  title: string;
  blurb: string;
  icon: LucideIcon;
  tag: "observe" | "control";
};

const TILES: Tile[] = [
  { href: "/status",        title: "System status",      blurb: "Per-service health, kill-switch state, version metadata.",                          icon: ActivityIcon,      tag: "observe" },
  { href: "/opportunities", title: "Live opportunities", blurb: "Recent detections from the mempool; filter by chain + strategy.",                  icon: SatelliteDishIcon, tag: "observe" },
  { href: "/executions",    title: "Recent executions",  blurb: "Bundle submissions and outcomes — included, reverted, replaced, dropped.",          icon: ZapIcon,           tag: "observe" },
  { href: "/risk",          title: "Risk & alerts",      blurb: "Circuit-breaker trips, anomaly events, blacklist hits, kill-switch log.",           icon: AlertTriangleIcon, tag: "observe" },
  { href: "/recon",         title: "Recon & PnL",        blurb: "Window totals, realised PnL, top strategies by adaptive score.",                    icon: GaugeIcon,         tag: "observe" },
  { href: "/config",        title: "Current config",     blurb: "Loaded application settings. Secrets are redacted.",                                icon: SettingsIcon,      tag: "control" },
  { href: "/killswitch",    title: "Kill-switch",        blurb: "Arm or disable execution at the platform level. Admin token required.",              icon: PowerIcon,         tag: "control" },
];

export default async function Home() {
  // Tolerate-degraded pattern: the home page MUST render an h1 even when
  // upstream calls fail. Promise.all would have rejected the whole render
  // on a single failure, falling through to error.tsx which surfaces a
  // level-2 "Application error" heading — the smoke test (and humans)
  // would then see no level-1 heading at all on /.
  //
  // With allSettled, each tile gracefully degrades:
  //  - getStatus()         → HomeKpiStrip shows "status unavailable" badge
  //  - getReconSummary(1)  → KPI tiles render zero-state, not a crash.
  //
  // Doctrine: degraded != mocked. We surface the real error, we just keep
  // the chrome (header, h1, sidebar) intact so the user can still navigate.
  const [statusSettled, reconSettled] = await Promise.allSettled([
    getStatus(),
    getReconSummary(1),
  ]);

  const status =
    statusSettled.status === "fulfilled"
      ? statusSettled.value
      : { ok: false as const, error: String(statusSettled.reason ?? "status unavailable") };

  const recon =
    reconSettled.status === "fulfilled"
      ? reconSettled.value
      : { ok: false as const, error: String(reconSettled.reason ?? "recon unavailable") };

  return (
    <>
      <div className="mb-8 space-y-2">
        <div className="flex items-center gap-2 text-xs uppercase tracking-widest text-muted-foreground">
          <span className="size-1.5 rounded-full bg-primary" />
          operator console
        </div>
        <h1 data-testid="page-title">ArbitrageX operator console</h1>
        <p className="max-w-2xl text-base text-muted-foreground">
          Every view below consumes live edge endpoints. When an upstream is unhealthy
          the page surfaces the error verbatim — it never synthesizes values.
        </p>
        <div className="flex flex-wrap gap-2 pt-2">
          <Badge variant="info">paper-mode ON</Badge>
          <Badge variant="outline">S1 – S6 merged</Badge>
          <Badge variant="outline" className="font-mono">{getApiBaseUrl()}</Badge>
        </div>
      </div>

      <HomeKpiStrip status={status} recon={recon} />

      <ProgressRealCard />

      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
        {TILES.map((t) => {
          const Icon = t.icon;
          return (
            <Link key={t.href} href={t.href} className="group">
              <Card className="relative h-full overflow-hidden transition-all hover:border-primary/40 hover:shadow-md">
                <div
                  aria-hidden
                  className="pointer-events-none absolute inset-0 opacity-0 transition-opacity group-hover:opacity-100"
                  style={{
                    background:
                      "radial-gradient(600px circle at 0% 0%, color-mix(in oklab, var(--primary) 8%, transparent), transparent 40%)",
                  }}
                />
                <CardHeader>
                  <div className="flex items-center justify-between">
                    <div className="flex size-9 items-center justify-center rounded-md bg-muted text-muted-foreground group-hover:bg-primary/10 group-hover:text-primary">
                      <Icon className="size-4" />
                    </div>
                    <Badge variant={t.tag === "control" ? "default" : "secondary"}>
                      {t.tag}
                    </Badge>
                  </div>
                  <CardTitle className="mt-3 flex items-center gap-1.5">
                    {t.title}
                    <ArrowUpRightIcon className="size-3.5 opacity-0 transition-opacity group-hover:opacity-100" />
                  </CardTitle>
                  <CardDescription>{t.blurb}</CardDescription>
                </CardHeader>
              </Card>
            </Link>
          );
        })}
      </div>
    </>
  );
}
