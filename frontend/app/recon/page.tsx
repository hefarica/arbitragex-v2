import { getReconSummary } from "../../lib/api-client";

export const dynamic = "force-dynamic";
export const revalidate = 0;

function pct(n: number | null): string { return n == null ? "—" : (n * 100).toFixed(2) + "%"; }
function usd(n: number | null): string { return n == null ? "—" : "$" + n.toFixed(2); }
function ms(n: number | null): string  { return n == null ? "—" : n.toFixed(0) + " ms"; }

export default async function ReconPage() {
  const res = await getReconSummary(1);

  if (!res.ok) {
    return (
      <>
        <section className="hero">
          <h1>Recon &amp; PnL</h1>
          <p className="hero__lede">Realised PnL, adaptive strategy scores, critical anomalies.</p>
        </section>
        <div className="err">edge error: <code>{res.error}</code></div>
      </>
    );
  }

  const s = res.data;

  return (
    <>
      <section className="hero">
        <h1>Recon &amp; PnL</h1>
        <p className="hero__lede">
          Realised PnL and adaptive strategy scoring — both populated by the recon learning loop
          (S6) as executions are reconciled against their pre-trade simulation.
        </p>
        <div className="hero__meta">
          <span>window: last {s.window_hours}h</span>
          <span>snapshot {new Date(s.ts).toLocaleTimeString()}</span>
        </div>
      </section>

      <section className="grid grid--kpi mb-6">
        <div className="card">
          <div className="card__title">Attempts</div>
          <div className="card__value num">{s.totals.total}</div>
          <div className="card__hint">in last {s.window_hours}h</div>
        </div>
        <div className="card">
          <div className="card__title">Included</div>
          <div className="card__value num">{s.totals.included}</div>
          <div className="card__hint">blocks confirmed</div>
        </div>
        <div className="card">
          <div className="card__title">Reverted</div>
          <div className="card__value num">{s.totals.reverted}</div>
          <div className="card__hint">on-chain reverts</div>
        </div>
        <div className="card">
          <div className="card__title">Revert rate</div>
          <div className="card__value num">{pct(s.revert_rate)}</div>
          <div className="card__hint">lower is better</div>
        </div>
        <div className="card">
          <div className="card__title">Avg PnL (incl.)</div>
          <div className="card__value num">{usd(s.totals.avg_pnl_included_usd)}</div>
          <div className="card__hint">per included tx</div>
        </div>
        <div className="card">
          <div className="card__title">Avg confirm latency</div>
          <div className="card__value num">{ms(s.totals.avg_confirm_latency_ms)}</div>
          <div className="card__hint">submit → block</div>
        </div>
      </section>

      <h2>Top strategies</h2>
      {s.top_strategies.length === 0 ? (
        <div className="empty">
          <strong>No strategy scores yet.</strong>
          <div className="muted mt-2">
            The recon learning loop populates this after the first aggregation window closes.
          </div>
        </div>
      ) : (
        <div className="tbl-wrap">
          <table className="tbl">
            <thead>
              <tr>
                <th>Strategy</th>
                <th>Chain</th>
                <th className="num">Samples</th>
                <th className="num">Success</th>
                <th className="num">Revert</th>
                <th className="num">Avg PnL</th>
                <th className="num">Score</th>
              </tr>
            </thead>
            <tbody>
              {s.top_strategies.map((row, i) => (
                <tr key={`${row.strategy_kind}-${row.chain_id}-${i}`}>
                  <td><span className="badge badge--info">{row.strategy_kind}</span></td>
                  <td>{row.chain_id}</td>
                  <td className="num">{row.sample_count}</td>
                  <td className="num">{pct(row.success_rate)}</td>
                  <td className="num">{pct(row.revert_rate)}</td>
                  <td className="num">{usd(row.avg_profit_usd)}</td>
                  <td className="num"><strong>{row.score != null ? row.score.toFixed(2) : "—"}</strong></td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      <h2>Critical anomalies (last 24h)</h2>
      {s.critical_anomalies_24h.length === 0 ? (
        <div className="empty">
          <span className="dot dot--success" aria-hidden /> <strong>None.</strong>
        </div>
      ) : (
        <div className="col gap-4">
          {s.critical_anomalies_24h.map((a, i) => (
            <div key={i} className="card card--accent">
              <div className="row">
                <span className="badge badge--danger">{a.event_type}</span>
                <span className="muted text-xs">{a.source_service}</span>
                <span className="faint text-xs">{new Date(a.created_at).toLocaleString()}</span>
              </div>
              <pre className="mt-2">{JSON.stringify(a.payload, null, 2)}</pre>
            </div>
          ))}
        </div>
      )}
    </>
  );
}
