import { getReconSummary } from "../../lib/api-client";

export const dynamic = "force-dynamic";
export const revalidate = 0;

function pct(n: number | null): string { return n == null ? "—" : (n * 100).toFixed(2) + "%"; }
function usd(n: number | null): string { return n == null ? "—" : "$" + n.toFixed(2); }

export default async function ReconPage() {
  const res = await getReconSummary(1);

  if (!res.ok) {
    return (
      <section>
        <h1>Recon & PnL</h1>
        <p style={{ color: "#f87171" }}>edge error: <code>{res.error}</code></p>
      </section>
    );
  }

  const s = res.data;

  return (
    <section>
      <h1>Recon & PnL</h1>
      <p style={{ color: "#9ca3af" }}>
        Window: last {s.window_hours}h · {new Date(s.ts).toLocaleString()}
      </p>

      <h2>Totals</h2>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(5, 1fr)", gap: 12, marginBottom: 20 }}>
        {[
          { label: "attempts", val: String(s.totals.total) },
          { label: "included", val: String(s.totals.included) },
          { label: "reverted", val: String(s.totals.reverted) },
          { label: "revert rate", val: pct(s.revert_rate) },
          { label: "avg PnL (incl.)", val: usd(s.totals.avg_pnl_included_usd) },
        ].map((x) => (
          <div key={x.label} style={{ padding: 12, border: "1px solid #1f2937", borderRadius: 6 }}>
            <div style={{ fontSize: 11, color: "#9ca3af", textTransform: "uppercase" }}>{x.label}</div>
            <div style={{ fontSize: 22, marginTop: 4 }}>{x.val}</div>
          </div>
        ))}
      </div>

      <h2>Top strategies (latest scoring window)</h2>
      {s.top_strategies.length === 0 ? (
        <p style={{ color: "#9ca3af" }}>
          No strategy scores yet. Recon learning loop populates this after the first
          aggregation window closes (default: 5 min).
        </p>
      ) : (
        <table style={{ borderCollapse: "collapse", width: "100%", fontSize: 13, marginBottom: 20 }}>
          <thead>
            <tr>
              {["strategy", "chain", "samples", "success", "revert", "avg PnL", "score"].map((h) => (
                <th key={h} align="left" style={{ padding: "4px 8px", borderBottom: "1px solid #1f2937" }}>
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {s.top_strategies.map((row, i) => (
              <tr key={`${row.strategy_kind}-${row.chain_id}-${i}`}>
                <td style={{ padding: "4px 8px", borderBottom: "1px solid #111827" }}>{row.strategy_kind}</td>
                <td style={{ padding: "4px 8px", borderBottom: "1px solid #111827" }}>{row.chain_id}</td>
                <td style={{ padding: "4px 8px", borderBottom: "1px solid #111827" }}>{row.sample_count}</td>
                <td style={{ padding: "4px 8px", borderBottom: "1px solid #111827" }}>{pct(row.success_rate)}</td>
                <td style={{ padding: "4px 8px", borderBottom: "1px solid #111827" }}>{pct(row.revert_rate)}</td>
                <td style={{ padding: "4px 8px", borderBottom: "1px solid #111827" }}>{usd(row.avg_profit_usd)}</td>
                <td style={{ padding: "4px 8px", borderBottom: "1px solid #111827", fontWeight: 600 }}>
                  {row.score != null ? row.score.toFixed(2) : "—"}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      <h2>Critical anomalies (last 24h)</h2>
      {s.critical_anomalies_24h.length === 0 ? (
        <p style={{ color: "#22c55e" }}>none.</p>
      ) : (
        <ul>
          {s.critical_anomalies_24h.map((a, i) => (
            <li key={i} style={{ color: "#ef4444" }}>
              <strong>{a.event_type}</strong> — {a.source_service} · {new Date(a.created_at).toLocaleString()}
              <pre style={{ color: "#9ca3af", fontSize: 12, overflow: "auto" }}>
                {JSON.stringify(a.payload, null, 2)}
              </pre>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
