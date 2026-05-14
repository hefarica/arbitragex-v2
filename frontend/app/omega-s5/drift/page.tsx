"use client";
import { useOmniDrift } from "@/lib/drift/useOmniDrift";

const SEVERITY_COLOR: Record<string, string> = {
  info: "bg-sky-500/15 text-sky-500",
  warn: "bg-amber-500/15 text-amber-500",
  error: "bg-rose-500/15 text-rose-500",
  critical: "bg-red-600/20 text-red-500",
};

export default function DriftPage() {
  const { observations, worstSeverity, loading, error, refreshedAt } = useOmniDrift(5000);
  return (
    <div>
      <h1 className="text-xl font-semibold">Drift Detection</h1>
      <p className="text-sm text-muted-foreground">PostgreSQL ↔ Redis ↔ searcher-rs ↔ Frontend — unresolved deltas only</p>
      <div className="mt-2 text-xs">
        Worst severity:{" "}
        <span className={`rounded px-2 py-0.5 font-medium ${SEVERITY_COLOR[worstSeverity] ?? ""}`}>
          {worstSeverity}
        </span>{" "}
        · refreshed {new Date(refreshedAt).toLocaleTimeString()}
      </div>
      {error && <div className="mt-2 text-sm text-rose-500">{error}</div>}
      <table className="mt-4 w-full text-sm">
        <thead><tr className="border-b border-border text-left">
          <th className="py-2">Resource</th><th>Chain</th><th>A → B</th><th>Hash A</th><th>Hash B</th><th>Sev</th>
        </tr></thead>
        <tbody>
          {loading ? (
            <tr><td colSpan={6} className="py-4 text-center text-muted-foreground">loading…</td></tr>
          ) : observations.length === 0 ? (
            <tr><td colSpan={6} className="py-4 text-center text-emerald-500">No drift detected.</td></tr>
          ) : observations.map((o) => (
            <tr key={o.id} className="border-b border-border/50">
              <td className="py-2">{o.resource}</td>
              <td>{o.chain_id ?? "—"}</td>
              <td>{o.layer_a} → {o.layer_b}</td>
              <td className="font-mono text-xs">{o.hash_a?.slice(0, 12) ?? "—"}</td>
              <td className="font-mono text-xs">{o.hash_b?.slice(0, 12) ?? "—"}</td>
              <td><span className={`rounded px-2 py-0.5 ${SEVERITY_COLOR[o.severity]}`}>{o.severity}</span></td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
