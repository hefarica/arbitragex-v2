import { getOpportunitiesLive } from "../../lib/api-client";

export const dynamic = "force-dynamic";
export const revalidate = 0;

function fmtMoney(n: number | null): string {
  return n == null ? "—" : `$${n.toFixed(2)}`;
}
function fmtPct(n: number | null): string {
  return n == null ? "—" : `${n.toFixed(2)}%`;
}

export default async function OpportunitiesPage() {
  const res = await getOpportunitiesLive(100);

  return (
    <>
      <section className="hero">
        <h1>Live opportunities</h1>
        <p className="hero__lede">
          Recent candidates detected from the mempool. Nothing here is synthesized;
          if the scanner has no RPC, the list stays empty — that's honest.
        </p>
      </section>

      {!res.ok ? (
        <div className="err">edge error: <code>{res.error}</code></div>
      ) : res.data.count === 0 ? (
        <div className="empty">
          <strong>No opportunities in flight.</strong>
          <div className="muted mt-2">
            Either the scanner has not connected to an RPC yet, or the market currently has
            nothing worth executing. Check <a href="/status">system status</a>.
          </div>
        </div>
      ) : (
        <>
          <div className="hero__meta mb-4">
            <span>{res.data.count} rows</span>
            <span>window: {res.data.window}</span>
            <span>snapshot {new Date(res.data.ts).toLocaleTimeString()}</span>
          </div>
          <div className="tbl-wrap">
            <table className="tbl">
              <thead>
                <tr>
                  <th>Detected</th>
                  <th>Chain</th>
                  <th>Strategy</th>
                  <th>Pair</th>
                  <th className="num">Est. PnL</th>
                  <th className="num">ROI</th>
                  <th>Status</th>
                </tr>
              </thead>
              <tbody>
                {res.data.items.map((o) => (
                  <tr key={o.id}>
                    <td className="muted">{new Date(o.detected_at).toLocaleTimeString()}</td>
                    <td>{o.chain_id}</td>
                    <td><span className="badge badge--info">{o.strategy_kind}</span></td>
                    <td>{o.pair_symbol ?? "—"}</td>
                    <td className="num">{fmtMoney(o.expected_profit_usd)}</td>
                    <td className="num">{fmtPct(o.roi_pct)}</td>
                    <td><span className="badge">{o.status}</span></td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}
    </>
  );
}
