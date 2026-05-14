"use client";
import Link from "next/link";

const REGISTRIES = [
  { path: "/registry/rpcs",          name: "RPC Endpoints",     channel: "arbx:config:rpcs" },
  { path: "/registry/contracts",     name: "Contracts",         channel: "arbx:config:contracts" },
  { path: "/registry/relays",        name: "Private Relays",    channel: "arbx:config:relays" },
  { path: "/registry/risk-gates",    name: "Risk Gates",        channel: "arbx:config:risk" },
  { path: "/registry/capital-gates", name: "Capital Gates",     channel: "arbx:config:capital" },
  { path: "/registry/agents",        name: "Sindicato Agents",  channel: "arbx:config:agents" },
];

export default function RegistryIndexPage() {
  return (
    <div>
      <h1 className="text-xl font-semibold">Canonical Entity Registries</h1>
      <p className="text-sm text-muted-foreground">
        Schema-First. Hot-reload per resource. Idempotent CRUD. 7-Layer Coherence enforced.
      </p>
      <div className="mt-4 grid gap-3 sm:grid-cols-2">
        {REGISTRIES.map((r) => (
          <Link key={r.path} href={r.path} className="rounded-lg border border-border p-4 hover:bg-muted">
            <div className="text-sm font-medium">{r.name}</div>
            <div className="mt-1 font-mono text-xs text-muted-foreground">{r.channel}:reload</div>
          </Link>
        ))}
      </div>
    </div>
  );
}
