"use client";
import { useCapitalGates } from "@/lib/hooks/useCapitalGates";

export default function OperatorPage() {
  const { data: gates, isLoading, error } = useCapitalGates();

  if (isLoading) {
    return (
      <div>
        <h1 className="text-xl font-semibold">Operator Parametrization</h1>
        <p className="text-sm text-muted-foreground">Loading...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div>
        <h1 className="text-xl font-semibold">Operator Parametrization</h1>
        <p className="text-sm text-destructive">Error: {error}</p>
      </div>
    );
  }

  const global = gates.find((g) => g.scope === "global");

  return (
    <div>
      <h1 className="text-xl font-semibold">Operator Parametrization</h1>
      <p className="text-sm text-muted-foreground">
        Capital cap and risk envelopes — only the Operator can raise above $0.00 via cryptographic signature.
      </p>
      {global && (
        <div className="mt-4 rounded-lg border border-border p-4">
          <div className="text-xs text-muted-foreground">Global Capital Cap</div>
          <div className="text-2xl font-semibold">${global.capital_cap_usd.toFixed(2)} USD</div>
          <div className="mt-2 text-xs">Status: <span className="font-medium">{global.status}</span></div>
          <div className="text-xs">Hash: <code className="font-mono">{global.config_hash.slice(0, 16)}…</code></div>
        </div>
      )}
      <table className="mt-4 w-full text-sm">
        <thead><tr className="border-b border-border text-left">
          <th className="py-2">Scope</th><th>Name</th><th>Cap USD</th><th>Max Drawdown</th><th>Status</th>
        </tr></thead>
        <tbody>
          {gates.map((g) => (
            <tr key={g.id} className="border-b border-border/50">
              <td className="py-2">{g.scope}{g.scope_ref ? `:${g.scope_ref}` : ""}</td>
              <td>{g.name}</td>
              <td>${g.capital_cap_usd.toFixed(2)}</td>
              <td>{g.max_drawdown_pct.toFixed(2)}%</td>
              <td>{g.status}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
