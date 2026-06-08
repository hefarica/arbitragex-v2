"use client";
import { useContracts } from "@/lib/hooks/useContracts";
import { useFeatureManifest } from "@/lib/hooks/useFeatureManifest";

export default function CorePage() {
  const { data: cores, isLoading: coresLoading, error: coresError } = useContracts({
    contractKind: "resolution_core",
  });
  const { feature: manifest, isLoading: manifestLoading, error: manifestError } = useFeatureManifest({
    featureKey: "omega.s5.core",
  });

  const isLoading = coresLoading || manifestLoading;
  const error = coresError || manifestError;

  if (isLoading) {
    return (
      <div>
        <h1 className="text-xl font-semibold">ResolutionCore + Holonomic Decoder</h1>
        <p className="text-sm text-muted-foreground">Loading...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div>
        <h1 className="text-xl font-semibold">ResolutionCore + Holonomic Decoder</h1>
        <p className="text-sm text-destructive">Error: {error}</p>
      </div>
    );
  }

  return (
    <div>
      <h1 className="text-xl font-semibold">ResolutionCore + Holonomic Decoder</h1>
      <p className="text-sm text-muted-foreground">UUPS proxy · Thermodynamic Balance Check post-execution · Yul decoder −40% gas</p>
      {manifest && (
        <div className="mt-2 text-xs text-muted-foreground">
          Manifest hash: <code>{manifest.state_hash.slice(0, 16)}…</code>
        </div>
      )}
      <table className="mt-4 w-full text-sm">
        <thead><tr className="border-b border-border text-left">
          <th className="py-2">Chain</th><th>Address</th><th>Impl</th><th>Version</th>
        </tr></thead>
        <tbody>
          {cores.map((r) => (
            <tr key={r.id} className="border-b border-border/50">
              <td className="py-2">{r.chain_id}</td>
              <td className="font-mono text-xs">{r.address}</td>
              <td className="font-mono text-xs">{r.implementation ?? "—"}</td>
              <td>{r.abi_version}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
