"use client";
import { useEffect, useState } from "react";
import type { ContractEntity, FeatureManifestEntry } from "@/lib/registries/types-omni";

export default function CorePage() {
  const [cores, setCores] = useState<ContractEntity[]>([]);
  const [manifest, setManifest] = useState<FeatureManifestEntry | null>(null);
  useEffect(() => {
    fetch("/api/contracts").then((r) => r.json())
      .then((d) => setCores((d.rows ?? []).filter((c: ContractEntity) => c.contract_kind === "resolution_core")));
    fetch("/api/system/feature_manifest").then((r) => r.json())
      .then((d) => setManifest(d.features.find((f: FeatureManifestEntry) => f.feature_key === "omega.s5.core") ?? null));
  }, []);
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
