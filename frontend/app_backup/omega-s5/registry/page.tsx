"use client";
import Link from "next/link";

// FE-CRIT-02 fix: link to the real dynamic route /omega-s5/registry/[entity] using the
// CANONICAL REGISTRY_KEYS (singular/snake) from lib/operator/types — the old /registry/*
// prefix + plural slugs had no route tree and 404'd on every card. Only `chain` is wired
// today; the rest land on an honest `registry_not_wired` placeholder (navigable, not 404).
const REGISTRIES = [
  { path: "/omega-s5/registry/rpc",          name: "RPC Endpoints",     channel: "arbx:config:rpcs" },
  { path: "/omega-s5/registry/contract",     name: "Contracts",         channel: "arbx:config:contracts" },
  { path: "/omega-s5/registry/relay",        name: "Private Relays",    channel: "arbx:config:relays" },
  { path: "/omega-s5/registry/risk_gate",    name: "Risk Gates",        channel: "arbx:config:risk" },
  { path: "/omega-s5/registry/capital_gate", name: "Capital Gates",     channel: "arbx:config:capital" },
  { path: "/omega-s5/registry/agent",        name: "Sindicato Agents",  channel: "arbx:config:agents" },
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
