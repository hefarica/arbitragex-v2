import { getApiBaseUrl } from "@/lib/api-client";
import Link from "next/link";
import {
  ActivityIcon,
  AlertTriangleIcon,
  ArrowUpRightIcon,
  FlaskConicalIcon,
  GaugeIcon,
  KeyRoundIcon,
  ListChecksIcon,
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
  { href: "/status",             title: "System status",      blurb: "Per-service health, kill-switch state, version metadata.",                          icon: ActivityIcon,      tag: "observe" },
  { href: "/opportunities",      title: "Live opportunities", blurb: "Recent detections from the mempool; filter by chain + strategy.",                  icon: SatelliteDishIcon, tag: "observe" },
  { href: "/executions",         title: "Recent executions",  blurb: "Bundle submissions and outcomes — included, reverted, replaced, dropped.",          icon: ZapIcon,           tag: "observe" },
  { href: "/live-readiness",     title: "Live readiness",     blurb: "GO/NO-GO gate — readiness criteria that must be green before flipping paper-mode off.", icon: ListChecksIcon,  tag: "control" },
  { href: "/risk",               title: "Risk & alerts",      blurb: "Circuit-breaker trips, anomaly events, blacklist hits, kill-switch log.",           icon: AlertTriangleIcon, tag: "observe" },
  { href: "/recon",              title: "Recon & PnL",        blurb: "Window totals, realised PnL, top strategies by adaptive score.",                    icon: GaugeIcon,         tag: "observe" },
  { href: "/paper/history",      title: "Paper history",      blurb: "Paper-trade ledger and drift analysis during the paper-shadow accumulation window.", icon: FlaskConicalIcon,  tag: "observe" },
  { href: "/settings/credentials", title: "Credentials",      blurb: "Inject RPC keys, relay auth, token-safety APIs. Step 1 — the platform is blind without it.", icon: KeyRoundIcon, tag: "control" },
  { href: "/config",             title: "Current config",     blurb: "Loaded application settings. Secrets are redacted.",                                icon: SettingsIcon,      tag: "control" },
  { href: "/killswitch",         title: "Kill-switch",        blurb: "Arm or disable execution at the platform level. Admin token required.",              icon: PowerIcon,         tag: "control" },
];

export default async function Home() {
  const [status, recon] = await Promise.all([getStatus(), getReconSummary(1)]);
  return (
    <>
      {/* Sphere aura — mockup parity: radial glow behind HeroSphere */}
      <div className="sphere-aura" aria-hidden />

      <div className="relative z-10">
        {/* Hero section — mockup parity */}
        <div className="mb-8 max-w-[980px]">
          <div
            className="mb-4 text-[10.5px] uppercase tracking-[0.14em]"
            style={{ color: 'var(--primary)', fontFamily: 'var(--font-data)' }}
          >
            IA OMEGA · OBSERVE → SIMULATE → EXECUTE
          </div>
          <h1 className="mb-6 text-[clamp(2.4rem,4.6vw,4rem)] font-semibold leading-[1.03] tracking-[-0.04em]">
            {"Convergencia estocástica.".split("").map((char, i) => (
              <span key={i} className="char" style={{ animationDelay: `${i * 25}ms` }}>{char === " " ? " " : char}</span>
            ))}
            <br />
            <span style={{ color: 'var(--primary-2)' }}>
              {"Topological Yield".split("").map((char, i) => (
                <span key={i} className="char" style={{ animationDelay: `${(i + 20) * 25}ms` }}>{char === " " ? " " : char}</span>
              ))}
            </span>
            {" en milisegundos.".split("").map((char, i) => (
              <span key={i} className="char" style={{ animationDelay: `${(i + 37) * 25}ms` }}>{char === " " ? " " : char}</span>
            ))}
          </h1>
          <p className="max-w-[64ch] text-base leading-[1.6] text-muted-foreground">
            El motor observa <b className="font-medium text-foreground">50 rutas de Liquidity Manifolds</b> en paralelo,
            resuelve <b className="font-medium text-foreground">Asimetría Topológica</b> bajo <b className="font-medium text-foreground">Temporal Liquidity Superposition</b>,
            y mantiene el capital expuesto en <b className="font-medium text-foreground">$0.00</b> hasta que cada gate
            institucional esté en verde. Doctrina OMEGA: honestidad antes que teatro.
          </p>
        </div>

        <HomeKpiStrip status={status} recon={recon} />

        {/* ProgressRealCard temporarily disabled - verify build stability before re-enabling */}
        {/* <ProgressRealCard /> */}

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
      </div>
    </>
  );
}
