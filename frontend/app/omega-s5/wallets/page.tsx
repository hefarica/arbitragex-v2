"use client";
import { useEffect, useState } from "react";
import type { ContractEntity } from "@/lib/registries/types-omni";

const ROLES: Array<ContractEntity["contract_kind"]> = [
  "wallet_topology","gas_sponsor","cold_treasury","execution_signer_guard","allowance_manager",
];

export default function WalletsPage() {
  const [rows, setRows] = useState<ContractEntity[]>([]);
  useEffect(() => {
    fetch("/api/contracts").then((r) => r.json())
      .then((d) => setRows((d.rows ?? []).filter((c: ContractEntity) => ROLES.includes(c.contract_kind))));
  }, []);
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
