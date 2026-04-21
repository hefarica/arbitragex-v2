import Link from "next/link";

export default function Home() {
  return (
    <section>
      <h1>ArbitrageX v2 — Operator Console</h1>
      <p style={{ color: "#9ca3af" }}>
        This is the Sprint 1 foundation. Real dashboards arrive in Sprint 7. Nothing on this
        page is synthesized — links below consume live edge endpoints.
      </p>
      <ul>
        <li><Link href="/status">System status</Link></li>
      </ul>
      <p style={{ marginTop: 24, color: "#6b7280", fontSize: 13 }}>
        Data source: <code>{process.env.NEXT_PUBLIC_EDGE_URL}</code>
      </p>
    </section>
  );
}
