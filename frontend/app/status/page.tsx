import { getStatus } from "../../lib/api-client";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export default async function StatusPage() {
  const res = await getStatus();

  if (!res.ok) {
    return (
      <section>
        <h1>System Status</h1>
        <p style={{ color: "#f87171" }}>
          edge unreachable: <code>{res.error}</code>
        </p>
        <p style={{ color: "#9ca3af" }}>
          This view displays ONLY real edge data. There is no fallback. If the edge is down,
          this page shows the error — it never synthesizes values.
        </p>
      </section>
    );
  }

  const s = res.data;
  const overallColor = s.ok ? "#22c55e" : "#ef4444";

  return (
    <section>
      <h1>System Status</h1>
      <p>
        <span style={{
          display: "inline-block", width: 10, height: 10, borderRadius: 10,
          background: overallColor, marginRight: 8,
        }} />
        Overall: <strong>{s.ok ? "OK" : "DEGRADED"}</strong> · env: <code>{s.env}</code> · version <code>{s.version}</code>
      </p>

      <h2>Kill switch</h2>
      {s.killswitch ? (
        <p>
          <strong style={{ color: s.killswitch.enabled ? "#f59e0b" : "#22c55e" }}>
            {s.killswitch.enabled ? "ENABLED" : "disabled"}
          </strong>{" "}
          {s.killswitch.reason && <>— reason: <em>{s.killswitch.reason}</em></>}{" "}
          (updated {new Date(s.killswitch.updated_at).toLocaleString()})
        </p>
      ) : (
        <p style={{ color: "#9ca3af" }}>no killswitch state returned</p>
      )}

      <h2>Services</h2>
      <table style={{ borderCollapse: "collapse", width: "100%", maxWidth: 640 }}>
        <thead>
          <tr><th align="left">service</th><th align="left">ok</th><th align="left">detail</th></tr>
        </thead>
        <tbody>
        {Object.entries(s.services).map(([name, v]) => (
          <tr key={name}>
            <td style={{ padding: "6px 8px", borderBottom: "1px solid #1f2937" }}>{name}</td>
            <td style={{ padding: "6px 8px", borderBottom: "1px solid #1f2937" }}>
              <span style={{ color: v.ok ? "#22c55e" : "#ef4444" }}>{v.ok ? "UP" : "DOWN"}</span>
            </td>
            <td style={{ padding: "6px 8px", borderBottom: "1px solid #1f2937", color: "#9ca3af" }}>
              {v.status ? `HTTP ${v.status}` : v.detail ?? "—"}
            </td>
          </tr>
        ))}
        </tbody>
      </table>

      <p style={{ marginTop: 24, color: "#6b7280", fontSize: 13 }}>
        retrieved at {new Date(s.ts).toLocaleString()}
      </p>
    </section>
  );
}
