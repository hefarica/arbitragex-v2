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
  const [status, recon] = await Promise.all([getStatus(), getReconSummary(1)]);
  return (
    <>
      <div className="mb-8 space-y-2">
        <div className="flex items-center gap-2 text-xs uppercase tracking-widest text-muted-foreground">
          <span className="size-1.5 rounded-full bg-primary" />
          operator console
        </div>
        <h1>ArbitrageX operator console</h1>
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
