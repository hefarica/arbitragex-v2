import { getExecutionsRecent, type ExecutionRow } from "../../lib/api-client";

export const dynamic = "force-dynamic";
export const revalidate = 0;

function statusBadge(s: ExecutionRow["status"]): string {
  switch (s) {
    case "included":  return "badge badge--success";
    case "reverted":  return "badge badge--danger";
    case "dropped":   return "badge badge--warning";
    case "replaced":  return "badge badge--warning";
    case "submitted": return "badge badge--info";
    default:          return "badge";
  }
}

function fmtMoney(n: number | null): string {
  return n == null ? "—" : `$${n.toFixed(2)}`;
}

export default async function ExecutionsPage() {
  const res = await getExecutionsRecent(100);

  return (
    <>
      <section className="hero">
        <h1>Recent executions</h1>
        <p className="hero__lede">
          Bundle submissions and their outcomes. Paper-mode is ON until S9; real relays only
          record <em>submitted</em>/<em>not_implemented</em> until the safety rail is flipped.
        </p>
      </section>

      {!res.ok ? (
        <div className="err">edge error: <code>{res.error}</code></div>
      ) : res.data.count === 0 ? (
        <div className="empty">
          <strong>No executions yet.</strong>
          <div className="muted mt-2">
            This table fills when simulated opportunities reach the relay submission path.
          </div>
        </div>
      ) : (
        <>
          <div className="hero__meta mb-4">
            <span>{res.data.count} rows</span>
            <span>snapshot {new Date(res.data.ts).toLocaleTimeString()}</span>
          </div>
          <div className="tbl-wrap">
            <table className="tbl">
              <thead>
                <tr>
                  <th>Submitted</th>
                  <th>Chain</th>
                  <th>Strategy</th>
                  <th>Pair</th>
                  <th>Relay</th>
                  <th>Status</th>
                  <th>Tx</th>
                  <th className="num">Block</th>
                  <th className="num">PnL</th>
                  <th>Error</th>
                </tr>
              </thead>
              <tbody>
                {res.data.items.map((e) => (
                  <tr key={e.id}>
                    <td className="muted">{new Date(e.submitted_at).toLocaleTimeString()}</td>
                    <td>{e.chain_id}</td>
                    <td>{e.strategy_kind}</td>
                    <td>{e.pair_symbol ?? "—"}</td>
                    <td><span className="badge">{e.relay_name}</span></td>
                    <td><span className={statusBadge(e.status)}>{e.status}</span></td>
                    <td className="mono">{e.tx_hash ? `${e.tx_hash.slice(0, 10)}…` : "—"}</td>
                    <td className="num">{e.block_included ?? "—"}</td>
                    <td className="num">{fmtMoney(e.actual_profit_usd)}</td>
                    <td className="muted truncate">{e.error_message ?? ""}</td>
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
