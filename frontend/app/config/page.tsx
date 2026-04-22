import { Fragment } from "react";
import { getConfigCurrent } from "../../lib/api-client";

export const dynamic = "force-dynamic";
export const revalidate = 0;

function KV({ obj }: { obj: Record<string, unknown> }) {
  return (
    <dl className="kv">
      {Object.entries(obj).map(([k, v]) => (
        <Fragment key={k}>
          <dt>{k}</dt>
          <dd>{String(v)}</dd>
        </Fragment>
      ))}
    </dl>
  );
}

export default async function ConfigPage() {
  const res = await getConfigCurrent();

  if (!res.ok) {
    return (
      <>
        <section className="hero">
          <h1>Current config</h1>
          <p className="hero__lede">Loaded application settings — secrets redacted at the edge.</p>
        </section>
        <div className="err">edge error: <code>{res.error}</code></div>
      </>
    );
  }

  const c = res.data;
  const paperOn = c.execution["paper_mode"] === true;

  return (
    <>
      <section className="hero">
        <h1>Current config</h1>
        <p className="hero__lede">
          Loaded from <code>configs/app.toml</code> at service boot. Secrets are never
          rendered here — to rotate keys see the <code>rotate-secrets</code> runbook.
        </p>
        {paperOn && (
          <div className="hero__meta mt-2">
            <span className="badge badge--success"><span className="dot dot--success" /> paper-mode ON</span>
          </div>
        )}
      </section>

      <div className="grid grid--2 mb-6">
        <div className="card">
          <h2 className="card__title mt-0">System</h2>
          <KV obj={c.system} />
        </div>
        <div className="card">
          <h2 className="card__title mt-0">Execution</h2>
          <KV obj={c.execution} />
        </div>
        <div className="card">
          <h2 className="card__title mt-0">Risk</h2>
          <KV obj={c.risk} />
        </div>
        <div className="card">
          <h2 className="card__title mt-0">Scoring</h2>
          <KV obj={c.scoring} />
        </div>
      </div>

      <h2>Chains</h2>
      <div className="tbl-wrap mb-6">
        <table className="tbl">
          <thead><tr><th>chain_id</th><th>name</th><th>enabled</th></tr></thead>
          <tbody>
            {c.chains.map((ch) => (
              <tr key={ch.chain_id}>
                <td>{ch.chain_id}</td>
                <td>{ch.name}</td>
                <td>
                  <span className={ch.enabled ? "badge badge--success" : "badge"}>
                    {ch.enabled ? "yes" : "no"}
                  </span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <h2>Relays</h2>
      <div className="tbl-wrap mb-6">
        <table className="tbl">
          <thead><tr><th>name</th><th>enabled</th><th>chains</th><th>endpoint</th></tr></thead>
          <tbody>
            {c.relays.map((r) => (
              <tr key={r.name}>
                <td>{r.name}</td>
                <td>
                  <span className={r.enabled ? "badge badge--success" : "badge"}>
                    {r.enabled ? "yes" : "no"}
                  </span>
                </td>
                <td className="num">{r.chains.join(", ")}</td>
                <td className="mono">{r.endpoint || "—"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <h2>Circuit breakers</h2>
      <div className="tbl-wrap">
        <table className="tbl">
          <thead><tr><th>name</th><th className="num">threshold</th><th className="num">window_ms</th><th className="num">cooldown_ms</th></tr></thead>
          <tbody>
            {c.circuit_breakers.map((cb) => (
              <tr key={cb.name}>
                <td><span className="badge badge--accent">{cb.name}</span></td>
                <td className="num">{cb.threshold}</td>
                <td className="num">{cb.window_ms}</td>
                <td className="num">{cb.cooldown_ms}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  );
}
