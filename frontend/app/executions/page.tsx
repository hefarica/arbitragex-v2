import { getExecutionsRecent } from "../../lib/api-client";

export const dynamic = "force-dynamic";
export const revalidate = 0;

const STATUS_COLOR: Record<string, string> = {
  included: "#22c55e",
  reverted: "#ef4444",
  dropped: "#f59e0b",
  replaced: "#f59e0b",
  submitted: "#60a5fa",
  not_implemented: "#9ca3af",
};

export default async function ExecutionsPage() {
  const res = await getExecutionsRecent(100);

  if (!res.ok) {
    return (
      <section>
        <h1>Recent executions</h1>
        <p style={{ color: "#f87171" }}>edge error: <code>{res.error}</code></p>
      </section>
    );
  }

  const { items, count, ts } = res.data;

  return (
    <section>
      <h1>Recent executions</h1>
      <p style={{ color: "#9ca3af" }}>
        {count} row{count === 1 ? "" : "s"} · snapshot at {new Date(ts).toLocaleString()}
      </p>
      {count === 0 ? (
        <p style={{ color: "#9ca3af" }}>
          No executions yet. Paper-mode is ON until S9; this table fills when
          simulated opportunities reach the relay submission path.
        </p>
      ) : (
        <table style={{ borderCollapse: "collapse", width: "100%", fontSize: 13 }}>
          <thead>
            <tr>
              {["submitted", "chain", "strategy", "pair", "relay", "status", "tx", "block", "PnL (USD)", "err"]
                .map((h) => (
                  <th key={h} align={h === "PnL (USD)" ? "right" : "left"}
                      style={{ padding: "4px 8px", borderBottom: "1px solid #1f2937" }}>
                    {h}
                  </th>
                ))}
            </tr>
          </thead>
          <tbody>
            {items.map((e) => (
              <tr key={e.id}>
                <td style={{ padding: "4px 8px", borderBottom: "1px solid #111827", color: "#9ca3af" }}>
                  {new Date(e.submitted_at).toLocaleTimeString()}
                </td>
                <td style={{ padding: "4px 8px", borderBottom: "1px solid #111827" }}>{e.chain_id}</td>
                <td style={{ padding: "4px 8px", borderBottom: "1px solid #111827" }}>{e.strategy_kind}</td>
                <td style={{ padding: "4px 8px", borderBottom: "1px solid #111827" }}>{e.pair_symbol ?? "—"}</td>
                <td style={{ padding: "4px 8px", borderBottom: "1px solid #111827" }}>{e.relay_name}</td>
                <td style={{ padding: "4px 8px", borderBottom: "1px solid #111827",
                             color: STATUS_COLOR[e.status] ?? "#e7e9eb" }}>{e.status}</td>
                <td style={{ padding: "4px 8px", borderBottom: "1px solid #111827", fontFamily: "monospace", color: "#9ca3af" }}>
                  {e.tx_hash ? `${e.tx_hash.slice(0, 10)}…` : "—"}
                </td>
                <td style={{ padding: "4px 8px", borderBottom: "1px solid #111827" }}>
                  {e.block_included ?? "—"}
                </td>
                <td align="right" style={{ padding: "4px 8px", borderBottom: "1px solid #111827" }}>
                  {e.actual_profit_usd != null ? e.actual_profit_usd.toFixed(2) : "—"}
                </td>
                <td style={{ padding: "4px 8px", borderBottom: "1px solid #111827", color: "#f87171" }}>
                  {e.error_message ? e.error_message.slice(0, 40) : ""}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}
