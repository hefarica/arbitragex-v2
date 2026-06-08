"use client";
import { useContracts } from "@/lib/hooks/useContracts";

export default function FactoryPage() {
  const { data: rows, isLoading, error } = useContracts({ contractKind: "factory" });

  if (isLoading) {
    return (
      <div>
        <h1 className="text-xl font-semibold">DeterministicFactory Deployments</h1>
        <p className="text-sm text-muted-foreground">Loading...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div>
        <h1 className="text-xl font-semibold">DeterministicFactory Deployments</h1>
        <p className="text-sm text-destructive">Error: {error}</p>
      </div>
    );
  }

  return (
    <div>
      <h1 className="text-xl font-semibold">DeterministicFactory Deployments</h1>
      <p className="text-sm text-muted-foreground">CREATE2 — salt = keccak(OMEGA_DOMAIN, chainId, version, label)</p>
      <table className="mt-4 w-full text-sm">
        <thead><tr className="border-b border-border text-left">
          <th className="py-2">Chain</th><th>Address</th><th>Salt</th><th>Verified</th><th>Hash</th>
        </tr></thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.id} className="border-b border-border/50">
              <td className="py-2">{r.chain_id}</td>
              <td className="font-mono text-xs">{r.address}</td>
              <td className="font-mono text-xs">{r.salt?.slice(0, 18) ?? "—"}…</td>
              <td>{r.verified ? "✅" : "—"}</td>
              <td className="font-mono text-xs">{r.config_hash?.slice(0, 12) ?? "—"}…</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
