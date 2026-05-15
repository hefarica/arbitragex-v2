"use client";
import { useEffect, useState } from "react";
import { getValidated } from "@/lib/frontier";
import { ContractsResponseSchema, type ContractEntityRow, type ContractsResponse, type FrontierResult } from "@/lib/schemas";
import { FrontierStateView, type FrontierStateMessage } from "@/components/FrontierStateView";

const KINDS: ReadonlyArray<string> = [
  "adapter_uniswap_v2",
  "adapter_uniswap_v3",
  "adapter_balancer",
  "adapter_curve",
  "adapter_pancake_v3",
  "adapter_gmx",
  "adapter_synthetix",
];

// OMEGA-8 / M5 Fase 6: fail-honest Zod-parsed /api/contracts read.
export default function OmegaS5AdaptersPage() {
  const [rows, setRows] = useState<ContractEntityRow[]>([]);
  const [msg, setMsg] = useState<FrontierStateMessage>({ kind: "loading", resource: "adapters" });

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const r: FrontierResult<ContractsResponse> = await getValidated("/api/contracts", ContractsResponseSchema, {
        withCredentials: true,
      });
      if (cancelled) return;
      if (r.kind === "ok") {
        const filtered = r.data.rows.filter((c) => KINDS.includes(c.contract_kind));
        setRows(filtered);
        if (filtered.length === 0) setMsg({ kind: "empty", resource: "adapters" });
        else setMsg({ kind: "loading", resource: "" });
      } else {
        setMsg({ kind: r.kind, detail: "detail" in r ? r.detail : String(r.kind), resource: "adapters" });
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div>
      <h1 className="text-xl font-semibold">DEX Adapters</h1>
      <p className="text-sm text-muted-foreground">IDEXAdapter — agnostic to chain · polymorphic by contract_kind</p>
      {(msg.kind !== "loading" || rows.length === 0) && <FrontierStateView msg={msg} />}
      {rows.length > 0 && (
        <table className="mt-4 w-full text-sm">
          <thead>
            <tr className="border-b border-border text-left">
              <th className="py-2">Kind</th>
              <th>Chain</th>
              <th>Address</th>
              <th>Status</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr key={r.id} className="border-b border-border/50">
                <td className="py-2 font-medium">{r.contract_kind.replace("adapter_", "")}</td>
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
