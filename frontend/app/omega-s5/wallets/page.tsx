"use client";
import { useEffect, useState } from "react";
import { getValidated } from "@/lib/frontier";
import { ContractsResponseSchema, type ContractEntityRow, type FrontierResult, type ContractsResponse } from "@/lib/schemas";
import { FrontierStateView, type FrontierStateMessage } from "@/components/FrontierStateView";

const ROLES: ReadonlyArray<string> = [
  "wallet_topology",
  "gas_sponsor",
  "cold_treasury",
  "execution_signer_guard",
  "allowance_manager",
];

// OMEGA-8 / M5 Fase 6: replaced raw fetch().then(r=>r.json()) with Zod-parsed
// getValidated() so a 404 or schema drift is surfaced via FrontierStateView,
// not silently rendered as an empty table.
export default function OmegaS5WalletsPage() {
  const [rows, setRows] = useState<ContractEntityRow[]>([]);
  const [msg, setMsg] = useState<FrontierStateMessage>({ kind: "loading", resource: "wallet topology" });

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const r: FrontierResult<ContractsResponse> = await getValidated("/api/contracts", ContractsResponseSchema, {
        withCredentials: true,
      });
      if (cancelled) return;
      if (r.kind === "ok") {
        const filtered = r.data.rows.filter((c) => ROLES.includes(c.contract_kind));
        setRows(filtered);
        if (filtered.length === 0) setMsg({ kind: "empty", resource: "wallet topology" });
        else setMsg({ kind: "loading", resource: "" });
      } else {
        setMsg({ kind: r.kind, detail: "detail" in r ? r.detail : String(r.kind), resource: "wallet topology" });
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div>
      <h1 className="text-xl font-semibold">Wallet Topology</h1>
      <p className="text-sm text-muted-foreground">
        Ghost Protocol: ExecutionSigner.balance ≡ 0 (enforced at bytecode)
      </p>
      {msg.kind !== "loading" || rows.length === 0 ? <FrontierStateView msg={msg} /> : null}
      {rows.length > 0 && (
        <table className="mt-4 w-full text-sm">
          <thead>
            <tr className="border-b border-border text-left">
              <th className="py-2">Role</th>
              <th>Chain</th>
              <th>Address</th>
              <th>Status</th>
            </tr>
          </thead>
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
      )}
    </div>
  );
}
