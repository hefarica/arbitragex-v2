"use client";
import { useCrucibleStatus } from "@/lib/hooks/useCrucibleStatus";

export default function CruciblePage() {
  const { data: rows, isLoading, error } = useCrucibleStatus();

  if (isLoading) {
    return (
      <div>
        <h1 className="text-xl font-semibold">Crucible Survival Tracker</h1>
        <p className="text-sm text-muted-foreground">Loading...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div>
        <h1 className="text-xl font-semibold">Crucible Survival Tracker</h1>
        <p className="text-sm text-destructive">Error: {error}</p>
      </div>
    );
  }

  const ok = rows.every((r) => r.success_rate_pct >= 95 && r.runtime_hours >= 72 && r.doctrinal_reverts === 0);

  return (
    <div>
      <h1 className="text-xl font-semibold">Crucible Survival Tracker</h1>
      <p className="text-sm text-muted-foreground">
        Holesky · Arb Sepolia · Polygon Amoy — must hold ≥95% success across ≥72h with 0 non-doctrinal reverts
      </p>
      <div className={`mt-2 inline-block rounded px-3 py-1 text-sm font-medium ${ok ? "bg-emerald-500/15 text-emerald-500" : "bg-amber-500/15 text-amber-500"}`}>
        {ok ? "CRUCIBLE PASSED" : "CRUCIBLE IN PROGRESS"}
      </div>
      <table className="mt-4 w-full text-sm">
        <thead><tr className="border-b border-border text-left">
          <th className="py-2">Network</th><th>Success</th><th>Runtime</th><th>Reverts</th><th>Cap USD</th>
        </tr></thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.chain_id} className="border-b border-border/50">
              <td className="py-2">{r.network}</td>
              <td>{r.success_rate_pct.toFixed(2)}% ({r.resolutions_success}/{r.resolutions_total})</td>
              <td>{r.runtime_hours.toFixed(1)} h</td>
              <td>{r.doctrinal_reverts}</td>
              <td>${r.capital_cap_usd.toFixed(2)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
