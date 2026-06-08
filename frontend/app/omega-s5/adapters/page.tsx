"use client";
import { useContracts } from "@/lib/hooks/useContracts";
import type { ContractEntity } from "@/lib/registries/types-omni";

const KINDS: Array<ContractEntity["contract_kind"]> = [
  "adapter_uniswap_v2","adapter_uniswap_v3","adapter_balancer","adapter_curve",
  "adapter_pancake_v3","adapter_gmx","adapter_synthetix",
];

export default function AdaptersPage() {
  const { data: rows, isLoading, error } = useContracts({ contractKinds: KINDS });

  if (isLoading) {
    return (
      <div>
        <h1 className="text-xl font-semibold">DEX Adapters</h1>
        <p className="text-sm text-muted-foreground">Loading...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div>
        <h1 className="text-xl font-semibold">DEX Adapters</h1>
        <p className="text-sm text-destructive">Error: {error}</p>
      </div>
    );
  }

  return (
    <div>
      <h1 className="text-xl font-semibold">DEX Adapters</h1>
      <p className="text-sm text-muted-foreground">IDEXAdapter — agnostic to chain · polymorphic by contract_kind</p>
      <table className="mt-4 w-full text-sm">
        <thead><tr className="border-b border-border text-left">
          <th className="py-2">Kind</th><th>Chain</th><th>Address</th><th>Status</th>
        </tr></thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.id} className="border-b border-border/50">
              <td className="py-2 font-medium">{r.contract_kind.replace("adapter_","")}</td>
              <td>{r.chain_id}</td>
              <td className="font-mono text-xs">{r.address}</td>
              <td>{r.status}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
