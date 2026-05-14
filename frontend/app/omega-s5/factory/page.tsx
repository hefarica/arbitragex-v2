"use client";
import { useEffect, useState } from "react";
import type { ContractEntity } from "@/lib/registries/types-omni";

export default function FactoryPage() {
  const [rows, setRows] = useState<ContractEntity[]>([]);
  useEffect(() => {
    fetch("/api/contracts?contract_kind=factory")
      .then((r) => r.json())
      .then((d) => setRows((d.rows ?? []).filter((c: ContractEntity) => c.contract_kind === "factory")));
  }, []);
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
              <td className="font-mono text-xs">{r.salt.slice(0, 18)}…</td>
              <td>{r.verified ? "✅" : "—"}</td>
              <td className="font-mono text-xs">{r.config_hash.slice(0, 12)}…</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
