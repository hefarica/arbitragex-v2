import type { Metadata } from "next";
import { cookies } from "next/headers";
import { TopologyVaultClient, type TopologyInitialSnapshot } from "./TopologyVaultClient";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export const metadata: Metadata = {
  title: "Topology Vault — QuantumX",
  description: "Operator control plane for RPC/WSS topology mutations.",
};

const BROWSER_EDGE_URL = process.env["NEXT_PUBLIC_EDGE_URL"] ?? "http://localhost:8787";
const SERVER_EDGE_URL = process.env["INTERNAL_EDGE_URL"] ?? BROWSER_EDGE_URL;

async function buildAdminCookieHeader(): Promise<string | undefined> {
  const cookieStore = await cookies();
  const names = ["arbx_admin_session", "arbx_admin_session_ttl"];
  const parts = names
    .map((name) => {
      const value = cookieStore.get(name)?.value;
      return value ? `${name}=${value}` : null;
    })
    .filter((part): part is string => Boolean(part));

  return parts.length > 0 ? parts.join("; ") : undefined;
}

async function fetchInitial(): Promise<TopologyInitialSnapshot> {
  try {
    const cookieHeader = await buildAdminCookieHeader();
    const res = await fetch(`${SERVER_EDGE_URL}/api/admin/topology/snapshot`, {
      headers: {
        accept: "application/json",
        ...(cookieHeader ? { cookie: cookieHeader } : {}),
      },
      cache: "no-store",
    });
    const data = (await res.json().catch(() => ({}))) as {
      ok?: boolean;
      topology?: TopologyInitialSnapshot["topology"];
      source?: string;
      error?: string;
    };
    if (!res.ok || !data.ok) {
      return { topology: null, source: "error", error: data.error ?? `HTTP ${res.status}` };
    }
    return {
      topology: data.topology ?? null,
      source: data.source ?? "vault",
      error: null,
    };
  } catch (e) {
    return { topology: null, source: "error", error: (e as Error).message };
  }
}

export default async function TopologyVaultPage() {
  const initial = await fetchInitial();
  return <TopologyVaultClient initialSnapshot={initial} edgeUrl={BROWSER_EDGE_URL.replace(/\/$/, "")} />;
}
