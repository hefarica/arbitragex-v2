import Link from "next/link";

export default function Home() {
  return (
    <section>
      <h1>ArbitrageX v2 — Operator Console</h1>
      <p style={{ color: "#9ca3af", maxWidth: 720 }}>
        All pages below consume live edge endpoints. When an upstream is unhealthy, the
        corresponding page surfaces the error verbatim — it never synthesizes values.
      </p>

      <h2>Observability</h2>
      <ul>
        <li><Link href="/status">System status</Link> — per-service health + kill-switch state</li>
        <li><Link href="/opportunities">Live opportunities</Link> — recent detections from the mempool</li>
        <li><Link href="/executions">Recent executions</Link> — bundle submissions + outcomes</li>
        <li><Link href="/risk">Risk &amp; alerts</Link> — circuit-breaker trips, anomalies, blacklist hits</li>
        <li><Link href="/recon">Recon &amp; PnL</Link> — window totals + top strategies</li>
      </ul>

      <h2>Control</h2>
      <ul>
        <li><Link href="/config">Current config</Link> — loaded settings (secrets redacted)</li>
        <li><Link href="/killswitch">Kill-switch</Link> — arm/disable (admin token required)</li>
      </ul>

      <p style={{ marginTop: 32, color: "#6b7280", fontSize: 13 }}>
        Edge URL: <code>{process.env.NEXT_PUBLIC_EDGE_URL}</code>
      </p>
    </section>
  );
}
