"use client";
import { useEffect, useState } from "react";
import { getValidated } from "@/lib/frontier";
import {
  ContractsResponseSchema,
  FeatureManifestResponseSchema,
  type ContractEntityRow,
  type FeatureManifestEntry,
} from "@/lib/schemas";
import { FrontierStateView, type FrontierStateMessage } from "@/components/FrontierStateView";

// OMEGA-8 / M5 Fase 6: replaced two raw fetches with getValidated() so a 404
// on either endpoint surfaces honestly instead of "manifest hash: undefined…".
export default function OmegaS5CorePage() {
  const [cores, setCores] = useState<ContractEntityRow[]>([]);
  const [manifest, setManifest] = useState<FeatureManifestEntry | null>(null);
  const [msg, setMsg] = useState<FrontierStateMessage>({ kind: "loading", resource: "resolution_core" });
  const [manifestMsg, setManifestMsg] = useState<FrontierStateMessage | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const c = await getValidated("/api/contracts", ContractsResponseSchema, { withCredentials: true });
      if (cancelled) return;
      if (c.kind === "ok") {
        const filtered = c.data.rows.filter((r) => r.contract_kind === "resolution_core");
        setCores(filtered);
        if (filtered.length === 0) setMsg({ kind: "empty", resource: "resolution_core" });
        else setMsg({ kind: "loading", resource: "" });
      } else {
        setMsg({ kind: c.kind, detail: "detail" in c ? c.detail : String(c.kind), resource: "resolution_core" });
      }

      const m = await getValidated("/api/system/feature_manifest", FeatureManifestResponseSchema, {
        withCredentials: true,
      });
      if (cancelled) return;
      if (m.kind === "ok") {
        const features = m.data.features ?? m.data.rows ?? [];
        const omegaCore = features.find((f) => f.feature_key === "omega.s5.core") ?? null;
        setManifest(omegaCore);
        if (!omegaCore) setManifestMsg({ kind: "empty", resource: "feature manifest" });
      } else {
        setManifestMsg({
          kind: m.kind,
          detail: "detail" in m ? m.detail : String(m.kind),
          resource: "feature manifest",
        });
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div>
      <h1 className="text-xl font-semibold">ResolutionCore + Holonomic Decoder</h1>
      <p className="text-sm text-muted-foreground">
        UUPS proxy · Thermodynamic Balance Check post-execution · Yul decoder −40% gas
      </p>
      {manifest?.state_hash && (
        <div className="mt-2 text-xs text-muted-foreground">
          Manifest hash: <code>{manifest.state_hash.slice(0, 16)}…</code>
        </div>
      )}
      {manifestMsg && <FrontierStateView msg={manifestMsg} />}
      {(msg.kind !== "loading" || cores.length === 0) && <FrontierStateView msg={msg} />}
      {cores.length > 0 && (
        <table className="mt-4 w-full text-sm">
          <thead>
            <tr className="border-b border-border text-left">
              <th className="py-2">Chain</th>
              <th>Address</th>
              <th>Impl</th>
              <th>Version</th>
            </tr>
          </thead>
          <tbody>
            {cores.map((r) => {
              const impl = (r as ContractEntityRow & { implementation?: string | null }).implementation ?? null;
              const abi = (r as ContractEntityRow & { abi_version?: string }).abi_version ?? "";
              return (
                <tr key={r.id} className="border-b border-border/50">
                  <td className="py-2">{r.chain_id}</td>
                  <td className="font-mono text-xs">{r.address}</td>
                  <td className="font-mono text-xs">{impl ?? "—"}</td>
                  <td>{abi}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
    </div>
  );
}
