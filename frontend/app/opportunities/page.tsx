import { getOpportunitiesLive } from "../../lib/api-client";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export default async function OpportunitiesPage() {
  const res = await getOpportunitiesLive(100);

  if (!res.ok) {
    return (
      <section>
        <h1>Live opportunities</h1>
        <p style={{ color: "#f87171" }}>edge error: <code>{res.error}</code></p>
        <p style={{ color: "#9ca3af" }}>
          If searcher-rs has no RPC configured, this list stays empty —
          that is honest, not an error. Check <a href="/status">/status</a>.
        </p>
      </section>
    );
  }

  const { items, count, ts } = res.data;

  return (
    <section>
      <h1>Live opportunities</h1>
      <p style={{ color: "#9ca3af" }}>
        {count} row{count === 1 ? "" : "s"} · snapshot at {new Date(ts).toLocaleString()}
      </p>
      {count === 0 ? (
        <p style={{ color: "#9ca3af" }}>
          No opportunities in flight. Either the scanner has not connected to an RPC yet, or
          the market currently has nothing worth executing.
        </p>
      ) : (
        <table style={{ borderCollapse: "collapse", width: "100%", fontSize: 13 }}>
          <thead>
            <tr>
              <th align="left" style={{ padding: "4px 8px", borderBottom: "1px solid #1f2937" }}>detected</th>
              <th align="left" style={{ padding: "4px 8px", borderBottom: "1px solid #1f2937" }}>chain</th>
              <th align="left" style={{ padding: "4px 8px", borderBottom: "1px solid #1f2937" }}>strategy</th>
              <th align="left" style={{ padding: "4px 8px", borderBottom: "1px solid #1f2937" }}>pair</th>
              <th align="right" style={{ padding: "4px 8px", borderBottom: "1px solid #1f2937" }}>est. PnL (USD)</th>
              <th align="right" style={{ padding: "4px 8px", borderBottom: "1px solid #1f2937" }}>ROI %</th>
              <th align="left" style={{ padding: "4px 8px", borderBottom: "1px solid #1f2937" }}>status</th>
            </tr>
          </thead>
          <tbody>
            {items.map((o) => (
              <tr key={o.id}>
                <td style={{ padding: "4px 8px", borderBottom: "1px solid #111827", color: "#9ca3af" }}>
                  {new Date(o.detected_at).toLocaleTimeString()}
                </td>
                <td style={{ padding: "4px 8px", borderBottom: "1px solid #111827" }}>{o.chain_id}</td>
                <td style={{ padding: "4px 8px", borderBottom: "1px solid #111827" }}>{o.strategy_kind}</td>
                <td style={{ padding: "4px 8px", borderBottom: "1px solid #111827" }}>{o.pair_symbol ?? "—"}</td>
                <td align="right" style={{ padding: "4px 8px", borderBottom: "1px solid #111827" }}>
                  {o.expected_profit_usd != null ? o.expected_profit_usd.toFixed(2) : "—"}
                </td>
                <td align="right" style={{ padding: "4px 8px", borderBottom: "1px solid #111827" }}>
                  {o.roi_pct != null ? o.roi_pct.toFixed(2) : "—"}
                </td>
                <td style={{ padding: "4px 8px", borderBottom: "1px solid #111827" }}>{o.status}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}
