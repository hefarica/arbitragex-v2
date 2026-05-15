"use client";
import { useEffect, useState } from "react";
import { getValidated } from "@/lib/frontier";
import { ContractsResponseSchema, type ContractEntityRow } from "@/lib/schemas";
import { FrontierStateView, type FrontierStateMessage } from "@/components/FrontierStateView";

// OMEGA-8 / M5 Fase 6: Zod-parsed /api/contracts (factory filter) with
// fail-honest discriminated union.
export default function OmegaS5FactoryPage() {
  const [rows, setRows] = useState<ContractEntityRow[]>([]);
  const [msg, setMsg] = useState<FrontierStateMessage>({ kind: "loading", resource: "factories" });

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const r = await getValidated("/api/contracts?contract_kind=factory", ContractsResponseSchema, {
        withCredentials: true,
      });
      if (cancelled) return;
      if (r.kind === "ok") {
        const filtered = r.data.rows.filter((c) => c.contract_kind === "factory");
        setRows(filtered);
        if (filtered.length === 0) setMsg({ kind: "empty", resource: "factories" });
        else setMsg({ kind: "loading", resource: "" });
      } else {
        setMsg({ kind: r.kind, detail: "detail" in r ? r.detail : String(r.kind), resource: "factories" });
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div>
      <h1 className="text-xl font-semibold">DeterministicFactory Deployments</h1>
      <p className="text-sm text-muted-foreground">CREATE2 — salt = keccak(OMEGA_DOMAIN, chainId, version, label)</p>
      {(msg.kind !== "loading" || rows.length === 0) && <FrontierStateView msg={msg} />}
      {rows.length > 0 && (
        <table className="mt-4 w-full text-sm">
          <thead>
            <tr className="border-b border-border text-left">
              <th className="py-2">Chain</th>
              <th>Address</th>
              <th>Salt</th>
              <th>Verified</th>
              <th>Hash</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => {
              const salt = (r as ContractEntityRow & { salt?: string }).salt ?? "";
              const verified = (r as ContractEntityRow & { verified?: boolean }).verified ?? false;
              const configHash = (r as ContractEntityRow & { config_hash?: string }).config_hash ?? "";
              return (
                <tr key={r.id} className="border-b border-border/50">
                  <td className="py-2">{r.chain_id}</td>
                  <td className="font-mono text-xs">{r.address}</td>
                  <td className="font-mono text-xs">{salt.slice(0, 18)}…</td>
                  <td>{verified ? "✅" : "—"}</td>
                  <td className="font-mono text-xs">{configHash.slice(0, 12)}…</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
    </div>
  );
}
