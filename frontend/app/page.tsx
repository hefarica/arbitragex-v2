import Link from "next/link";

type Tile = { href: string; title: string; blurb: string; tag: "observe" | "control" };
const TILES: Tile[] = [
  { href: "/status",        title: "System status",       blurb: "Per-service health, kill-switch state, version metadata.",                       tag: "observe" },
  { href: "/opportunities", title: "Live opportunities",  blurb: "Recent detections from the mempool; filters by chain + strategy.",              tag: "observe" },
  { href: "/executions",    title: "Recent executions",   blurb: "Bundle submissions and outcomes — included, reverted, replaced, dropped.",       tag: "observe" },
  { href: "/risk",          title: "Risk & alerts",       blurb: "Circuit-breaker trips, anomaly events, blacklist hits, kill-switch log.",        tag: "observe" },
  { href: "/recon",         title: "Recon & PnL",         blurb: "Window totals, realised PnL, top strategies by adaptive score.",                 tag: "observe" },
  { href: "/config",        title: "Current config",      blurb: "Loaded application settings. Secrets are redacted.",                             tag: "control" },
  { href: "/killswitch",    title: "Kill-switch",         blurb: "Arm or disable execution at the platform level. Admin token required.",          tag: "control" },
];

export default function Home() {
  return (
    <>
      <section className="hero">
        <h1>Operator console</h1>
        <p className="hero__lede">
          Every view below consumes live edge endpoints. When an upstream is unhealthy
          the page surfaces the error verbatim — it never synthesizes values.
        </p>
        <div className="hero__meta">
          <span>env: <code>{process.env.NEXT_PUBLIC_EDGE_URL}</code></span>
          <span>paper-mode: ON</span>
          <span>sprints: S1–S6 merged</span>
        </div>
      </section>

      <section className="grid grid--2">
        {TILES.map((t) => (
          <Link key={t.href} href={t.href} className="stat">
            <span className={`badge ${t.tag === "control" ? "badge--accent" : "badge--info"}`}>{t.tag}</span>
            <div className="card__value">{t.title}</div>
            <div className="muted text-sm">{t.blurb}</div>
          </Link>
        ))}
      </section>
    </>
  );
}
