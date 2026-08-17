/**
 * /settings/credentials — operator's single surface for every external
 * credential the platform needs to function (RPC HTTP/WS per chain,
 * Coingecko, Alchemy Prices, Flashbots signer, BloxRoute, Titan, CEX APIs,
 * GitHub, internal admin/edge tokens).
 *
 * R1 Mounted Snapshot: this page is a Server Component that fetches the
 * masked credential list once at SSR time and hands it to the Client
 * component for interactive editing.
 *
 * MC-CRED-2: it ALSO fetches the Topology Vault snapshot so the dynamic RPC
 * category is hydrated from the server-side SSOT. Previously the category
 * came only from the browser's localStorage store (written by Topology
 * Vault visits) — when that cache was empty/wiped, saved rpc_* rows sat
 * valid in Postgres but had no card to render on, so the operator's
 * validated state "disappeared" on every reload.
 */

import type { Metadata } from "next";
import { getApiBaseUrl } from "@/lib/api-client";
import {
  snapshotToActiveChains,
  type TopologySnapshotEnvelope,
} from "@/lib/topology-snapshot";
import type { ActiveChain } from "@/store/useSystemStore";
import { CredentialsClient, type CredentialsSnapshot } from "./CredentialsClient";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export const metadata: Metadata = {
  title: "Credentials — QuantumX",
  description: "Operator credentials store — RPC, CEX, MEV, prices. Live status per credential.",
};

const EDGE_URL = getApiBaseUrl();

// V-AT-2: SSR has no browser session — authenticate admin-gated reads with
// the runtime token (same pattern as /audit-logs SSR) and bypass the edge
// rate limit like api-client's ssrEdgeTokenHeader.
function ssrAdminHeaders(): Record<string, string> {
  return {
    accept: "application/json",
    ...(process.env.ARBX_ADMIN_TOKEN
      ? { "x-arbx-admin-token": process.env.ARBX_ADMIN_TOKEN }
      : {}),
    ...(process.env.ARBX_EDGE_TOKEN
      ? { "x-arbx-edge-token": process.env.ARBX_EDGE_TOKEN }
      : {}),
  };
}

async function fetchInitial(): Promise<CredentialsSnapshot> {
  try {
    const res = await fetch(`${EDGE_URL}/api/credentials`, {
      headers: ssrAdminHeaders(),
      cache: "no-store",
    });
    if (!res.ok) {
      return { items: [], ts: new Date().toISOString(), error: `HTTP ${res.status}` };
    }
    const data = (await res.json()) as { items?: unknown[]; ts?: string };
    return {
      items: Array.isArray(data.items) ? (data.items as CredentialsSnapshot["items"]) : [],
      ts: data.ts ?? new Date().toISOString(),
      error: null,
    };
  } catch (e) {
    return { items: [], ts: new Date().toISOString(), error: (e as Error).message };
  }
}

/**
 * MC-CRED-2: server-side source for the dynamic RPC category. Any failure is
 * fail-honest — empty chains + the error string surfaced in the client (the
 * client still falls back to persisted rpc_* rows and the local store).
 */
async function fetchTopologyChains(): Promise<{ chains: ActiveChain[]; error: string | null }> {
  try {
    const res = await fetch(`${EDGE_URL}/api/admin/topology/snapshot`, {
      headers: ssrAdminHeaders(),
      cache: "no-store",
    });
    if (!res.ok) {
      return { chains: [], error: `HTTP ${res.status}` };
    }
    const data = (await res.json()) as TopologySnapshotEnvelope;
    if (!data.ok) {
      return { chains: [], error: data.error ?? "topology_snapshot_not_ok" };
    }
    // topology === null + source "empty_vault" is an HONEST empty — no error.
    return { chains: snapshotToActiveChains(data.topology ?? null), error: null };
  } catch (e) {
    return { chains: [], error: (e as Error).message };
  }
}

export default async function CredentialsPage() {
  // Shotgun dispatch: both admin reads go out in parallel (hot-path doctrine).
  const [initial, topology] = await Promise.all([fetchInitial(), fetchTopologyChains()]);
  // MC-CRED-1: never pass an SSR-computed base URL into the client — in the
  // browser it resolved to the docker-internal http://edge:8787 (mixed content).
  // CredentialsClient builds same-origin URLs via getApiBaseUrl() at call time.
  return (
    <CredentialsClient
      initialSnapshot={initial}
      initialActiveChains={topology.chains}
      topologyError={topology.error}
    />
  );
}
