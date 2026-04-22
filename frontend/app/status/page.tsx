import { getStatus } from "../../lib/api-client";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export default async function StatusPage() {
  const res = await getStatus();

  if (!res.ok) {
    return (
      <>
        <section className="hero">
          <h1>System status</h1>
          <p className="hero__lede">Live operator view of every hot-path service + kill-switch state.</p>
        </section>
        <div className="err">edge unreachable: <code>{res.error}</code></div>
        <p className="muted mt-4">
          This view displays only real edge data. There is no fallback. If the edge is down,
          this page shows the error — it never synthesizes values.
        </p>
      </>
    );
  }

  const s = res.data;
  const overallDot = s.ok ? "dot dot--success" : "dot dot--danger";

  return (
    <>
      <section className="hero">
        <h1>System status</h1>
        <p className="hero__lede">
          Live operator view of every hot-path service + kill-switch state. Kept raw — no synthesis.
        </p>
        <div className="hero__meta">
          <span>env: {s.env}</span>
          <span>api-server: v{s.version}</span>
          <span>as of {new Date(s.ts).toLocaleTimeString()}</span>
        </div>
      </section>

      <section className="grid grid--kpi mb-6">
        <div className="card">
          <div className="card__title">Overall</div>
          <div className="row">
            <span className={overallDot} aria-hidden />
            <span className="card__value">{s.ok ? "OK" : "DEGRADED"}</span>
          </div>
          <div className="card__hint">{Object.keys(s.services).length} services monitored</div>
        </div>
        <div className="card">
          <div className="card__title">Kill-switch</div>
          {s.killswitch ? (
            <>
              <div className="row">
                <span className={s.killswitch.enabled ? "dot dot--danger" : "dot dot--success"} aria-hidden />
                <span className="card__value">{s.killswitch.enabled ? "ARMED" : "disabled"}</span>
              </div>
              <div className="card__hint">
                {s.killswitch.reason ? `reason: ${s.killswitch.reason}` : "no reason set"} · updated{" "}
                {new Date(s.killswitch.updated_at).toLocaleTimeString()}
              </div>
            </>
          ) : (
            <div className="muted">no state returned</div>
          )}
        </div>
        <div className="card">
          <div className="card__title">Paper-mode</div>
          <div className="row">
            <span className="dot dot--success" aria-hidden />
            <span className="card__value">ON</span>
          </div>
          <div className="card__hint">Real capital is not at risk until S9.</div>
        </div>
      </section>

      <h2>Services</h2>
      <div className="tbl-wrap">
        <table className="tbl">
          <thead>
            <tr>
              <th>Service</th>
              <th>Status</th>
              <th>Detail</th>
            </tr>
          </thead>
          <tbody>
            {Object.entries(s.services).map(([name, v]) => (
              <tr key={name}>
                <td className="mono">{name}</td>
                <td>
                  <span className={v.ok ? "badge badge--success" : "badge badge--danger"}>
                    {v.ok ? "UP" : "DOWN"}
                  </span>
                </td>
                <td className="muted">{v.status ? `HTTP ${v.status}` : v.detail ?? "—"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  );
}
