import { getRiskAlerts } from "../../lib/api-client";

export const dynamic = "force-dynamic";
export const revalidate = 0;

const SEV_COLOR: Record<string, string> = {
  critical: "#ef4444",
  warning: "#f59e0b",
  info: "#60a5fa",
};

export default async function RiskPage() {
  const res = await getRiskAlerts(24);

  if (!res.ok) {
    return (
      <section>
        <h1>Risk & alerts</h1>
        <p style={{ color: "#f87171" }}>edge error: <code>{res.error}</code></p>
      </section>
    );
  }

  const { alerts, window_hours, killswitch, ts } = res.data;

  return (
    <section>
      <h1>Risk & alerts</h1>
      <p style={{ color: "#9ca3af" }}>
        Last {window_hours}h · snapshot {new Date(ts).toLocaleString()}
      </p>

      {killswitch && (
        <div style={{ padding: 12, borderRadius: 6,
                      background: killswitch.enabled ? "#2a1010" : "#0a2010",
                      border: `1px solid ${killswitch.enabled ? "#ef4444" : "#22c55e"}`,
                      marginBottom: 16 }}>
          <strong style={{ color: killswitch.enabled ? "#ef4444" : "#22c55e" }}>
            Kill-switch: {killswitch.enabled ? "ARMED" : "disabled"}
          </strong>
          {killswitch.reason && <> — reason: <em>{killswitch.reason}</em></>}
          <div style={{ color: "#9ca3af", fontSize: 12 }}>
            updated {new Date(killswitch.updated_at).toLocaleString()}
            {killswitch.triggered_by && <> · by <code>{killswitch.triggered_by}</code></>}
          </div>
        </div>
      )}

      <h2>Recent events</h2>
      {alerts.length === 0 ? (
        <p style={{ color: "#9ca3af" }}>no warnings or criticals in the last {window_hours}h.</p>
      ) : (
        <table style={{ borderCollapse: "collapse", width: "100%", fontSize: 13 }}>
          <thead>
            <tr>
              {["time", "severity", "type", "source", "details"].map((h) => (
                <th key={h} align="left" style={{ padding: "4px 8px", borderBottom: "1px solid #1f2937" }}>
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {alerts.map((a) => (
              <tr key={a.id}>
                <td style={{ padding: "4px 8px", borderBottom: "1px solid #111827", color: "#9ca3af" }}>
                  {new Date(a.created_at).toLocaleTimeString()}
                </td>
                <td style={{ padding: "4px 8px", borderBottom: "1px solid #111827",
                             color: SEV_COLOR[a.severity] ?? "#e7e9eb" }}>
                  {a.severity}
                </td>
                <td style={{ padding: "4px 8px", borderBottom: "1px solid #111827" }}>{a.event_type}</td>
                <td style={{ padding: "4px 8px", borderBottom: "1px solid #111827" }}>{a.source_service}</td>
                <td style={{ padding: "4px 8px", borderBottom: "1px solid #111827", fontFamily: "monospace",
                             color: "#9ca3af", maxWidth: 400, overflow: "hidden", textOverflow: "ellipsis",
                             whiteSpace: "nowrap" }}>
                  {JSON.stringify(a.payload).slice(0, 120)}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}
