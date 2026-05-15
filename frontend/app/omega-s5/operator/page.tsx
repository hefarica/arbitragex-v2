"use client";
import { useEffect, useState } from "react";
import { getValidated } from "@/lib/frontier";
import { CapitalGatesResponseSchema, type CapitalGateRow, type FrontierResult, type CapitalGatesResponse } from "@/lib/schemas";
import { FrontierStateView, type FrontierStateMessage } from "@/components/FrontierStateView";

// OMEGA-8 / M5 Fase 6: Zod-parsed capital-gates with fail-honest discriminated
// union — no more silent empty tables when the endpoint is unimplemented.
export default function OmegaS5OperatorPage() {
  const [gates, setGates] = useState<CapitalGateRow[]>([]);
  const [msg, setMsg] = useState<FrontierStateMessage>({ kind: "loading", resource: "capital gates" });

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const r: FrontierResult<CapitalGatesResponse> = await getValidated(
        "/api/capital-gates",
        CapitalGatesResponseSchema,
        { withCredentials: true },
      );
      if (cancelled) return;
      if (r.kind === "ok") {
        setGates(r.data.rows);
        if (r.data.rows.length === 0) setMsg({ kind: "empty", resource: "capital gates" });
        else setMsg({ kind: "loading", resource: "" });
      } else {
        setMsg({ kind: r.kind, detail: "detail" in r ? r.detail : String(r.kind), resource: "capital gates" });
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const global = gates.find((g) => g.scope === "global");
  return (
    <div>
      <h1 className="text-xl font-semibold">Operator Parametrization</h1>
      <p className="text-sm text-muted-foreground">
        Capital cap and risk envelopes — only the Operator can raise above $0.00 via cryptographic signature.
      </p>
      {(msg.kind !== "loading" || gates.length === 0) && <FrontierStateView msg={msg} />}
      {global && (
        <div className="mt-4 rounded-lg border border-border p-4">
          <div className="text-xs text-muted-foreground">Global Capital Cap</div>
          <div className="text-2xl font-semibold">${global.capital_cap_usd.toFixed(2)} USD</div>
          <div className="mt-2 text-xs">Status: <span className="font-medium">{global.status}</span></div>
          <div className="text-xs">Hash: <code className="font-mono">{global.config_hash.slice(0, 16)}…</code></div>
        </div>
      )}
      {gates.length > 0 && (
        <table className="mt-4 w-full text-sm">
          <thead>
            <tr className="border-b border-border text-left">
              <th className="py-2">Scope</th>
              <th>Name</th>
              <th>Cap USD</th>
              <th>Max Drawdown</th>
              <th>Status</th>
            </tr>
          </thead>
          <tbody>
            {gates.map((g) => (
              <tr key={g.id} className="border-b border-border/50">
                <td className="py-2">
                  {g.scope}
                  {g.scope_ref ? `:${g.scope_ref}` : ""}
                </td>
                <td>{g.name}</td>
                <td>${g.capital_cap_usd.toFixed(2)}</td>
                <td>{g.max_drawdown_pct.toFixed(2)}%</td>
                <td>{g.status}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
