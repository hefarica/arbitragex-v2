"use client";
import { useContracts } from "@/lib/hooks/useContracts";
import type { ContractEntity } from "@/lib/registries/types-omni";

const ROLES: Array<ContractEntity["contract_kind"]> = [
  "wallet_topology","gas_sponsor","cold_treasury","execution_signer_guard","allowance_manager",
];

export default function WalletsPage() {
  const { data: rows, isLoading, error } = useContracts({ contractKinds: ROLES });

  if (isLoading) {
    return (
      <div>
        <h1 className="text-xl font-semibold">Wallet Topology</h1>
        <p className="text-sm text-muted-foreground">Loading...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div>
        <h1 className="text-xl font-semibold">Wallet Topology</h1>
        <p className="text-sm text-destructive">Error: {error}</p>
      </div>
    );
  }

  return (
    <div>
      <h1 className="text-xl font-semibold">Wallet Topology</h1>
      <p className="text-sm text-muted-foreground">Ghost Protocol: ExecutionSigner.balance ≡ 0 (enforced at bytecode)</p>
      <table className="mt-4 w-full text-sm">
        <thead><tr className="border-b border-border text-left">
          <th className="py-2">Role</th><th>Chain</th><th>Address</th><th>Status</th>
        </tr></thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.id} className="border-b border-border/50">
              <td className="py-2 font-medium">{r.contract_kind}</td>
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
