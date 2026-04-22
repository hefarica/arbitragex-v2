import { getRiskAlerts, type RiskAlertRow } from "../../lib/api-client";

export const dynamic = "force-dynamic";
export const revalidate = 0;

function sevBadge(s: RiskAlertRow["severity"]): string {
  switch (s) {
    case "critical": return "badge badge--danger";
    case "warning":  return "badge badge--warning";
    case "info":     return "badge badge--info";
  }
}

export default async function RiskPage() {
  const res = await getRiskAlerts(24);

  if (!res.ok) {
    return (
      <>
        <section className="hero">
          <h1>Risk &amp; alerts</h1>
          <p className="hero__lede">Circuit-breakers, anomalies, blacklist hits, kill-switch log.</p>
        </section>
        <div className="err">edge error: <code>{res.error}</code></div>
      </>
    );
  }

  const { alerts, window_hours, killswitch, ts } = res.data;

  return (
    <>
      <section className="hero">
        <h1>Risk &amp; alerts</h1>
        <p className="hero__lede">
          Active kill-switch state and every risk event recorded by services in the last {window_hours}h.
        </p>
        <div className="hero__meta">
          <span>window: last {window_hours}h</span>
          <span>snapshot {new Date(ts).toLocaleTimeString()}</span>
        </div>
      </section>

      {killswitch && (
        <div className={killswitch.enabled ? "banner banner--bad" : "banner banner--ok"}>
          <div className="row">
            <span className={killswitch.enabled ? "dot dot--danger" : "dot dot--success"} aria-hidden />
            <span className="banner__title">
              Kill-switch: {killswitch.enabled ? "ARMED — executions blocked" : "disabled — executions permitted"}
            </span>
          </div>
          {killswitch.reason && <div>reason: <em>{killswitch.reason}</em></div>}
          <div className="banner__meta">
            updated {new Date(killswitch.updated_at).toLocaleString()}
            {killswitch.triggered_by && <> · by <code>{killswitch.triggered_by}</code></>}
          </div>
        </div>
      )}

      <h2>Recent events</h2>
      {alerts.length === 0 ? (
        <div className="empty">
          <strong>No warnings or criticals.</strong>
          <div className="muted mt-2">Nothing of concern logged in the last {window_hours}h.</div>
        </div>
      ) : (
        <div className="tbl-wrap">
          <table className="tbl">
            <thead>
              <tr>
                <th>Time</th>
                <th>Severity</th>
                <th>Type</th>
                <th>Source</th>
                <th>Details</th>
              </tr>
            </thead>
            <tbody>
              {alerts.map((a) => (
                <tr key={a.id}>
                  <td className="muted">{new Date(a.created_at).toLocaleTimeString()}</td>
                  <td><span className={sevBadge(a.severity)}>{a.severity}</span></td>
                  <td>{a.event_type}</td>
                  <td className="mono">{a.source_service}</td>
                  <td className="mono truncate">{JSON.stringify(a.payload).slice(0, 200)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </>
  );
}
