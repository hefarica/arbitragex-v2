"use client";
import Link from "next/link";

// OMEGA-8 / M5 Fase 7 (P1-OS5-2): registry index links must point to existing
// /omega-s5/registry/[entity] routes — the prior `/registry/...` prefix was a
// 404 because the route only mounts under /omega-s5/registry.
const OMEGA_S5_REGISTRY_BASE = "/omega-s5/registry";

const REGISTRIES: ReadonlyArray<{ entity: string; name: string; channel: string }> = [
  { entity: "rpcs", name: "RPC Endpoints", channel: "arbx:config:rpcs" },
  { entity: "contracts", name: "Contracts", channel: "arbx:config:contracts" },
  { entity: "relays", name: "Private Relays", channel: "arbx:config:relays" },
  { entity: "risk-gates", name: "Risk Gates", channel: "arbx:config:risk" },
  { entity: "capital-gates", name: "Capital Gates", channel: "arbx:config:capital" },
  { entity: "agents", name: "Sindicato Agents", channel: "arbx:config:agents" },
];

export default function OmegaS5RegistryIndexPage() {
  return (
    <div>
      <h1 className="text-xl font-semibold">Canonical Entity Registries</h1>
      <p className="text-sm text-muted-foreground">
        Schema-First. Hot-reload per resource. Idempotent CRUD. 7-Layer Coherence enforced.
      </p>
      <div className="mt-4 grid gap-3 sm:grid-cols-2">
        {REGISTRIES.map((r) => (
          <Link
            key={r.entity}
            href={`${OMEGA_S5_REGISTRY_BASE}/${r.entity}`}
            className="rounded-lg border border-border p-4 hover:bg-muted"
          >
            <div className="text-sm font-medium">{r.name}</div>
            <div className="mt-1 font-mono text-xs text-muted-foreground">{r.channel}:reload</div>
          </Link>
        ))}
      </div>
    </div>
  );
}
