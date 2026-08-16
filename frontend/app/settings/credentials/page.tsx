/**
 * /settings/credentials — operator's single surface for every external
 * credential the platform needs to function (RPC HTTP/WS per chain,
 * Coingecko, Alchemy Prices, Flashbots signer, BloxRoute, Titan, CEX APIs,
 * GitHub, internal admin/edge tokens).
 *
 * R1 Mounted Snapshot: this page is a Server Component that fetches the
 * masked credential list once at SSR time and hands it to the Client
 * component for interactive editing.
 */

import type { Metadata } from "next";
import { getApiBaseUrl } from "@/lib/api-client";
import { CredentialsClient, type CredentialsSnapshot } from "./CredentialsClient";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export const metadata: Metadata = {
  title: "Credentials — QuantumX",
  description: "Operator credentials store — RPC, CEX, MEV, prices. Live status per credential.",
};

const EDGE_URL = getApiBaseUrl();

async function fetchInitial(): Promise<CredentialsSnapshot> {
  try {
    const res = await fetch(`${EDGE_URL}/api/credentials`, {
      headers: {
        accept: "application/json",
        // V-AT-2: SSR has no browser session — authenticate the admin-gated
        // list with the runtime token (same pattern as /audit-logs SSR) and
        // bypass the edge rate limit like api-client's ssrEdgeTokenHeader.
        ...(process.env.ARBX_ADMIN_TOKEN
          ? { "x-arbx-admin-token": process.env.ARBX_ADMIN_TOKEN }
          : {}),
        ...(process.env.ARBX_EDGE_TOKEN
          ? { "x-arbx-edge-token": process.env.ARBX_EDGE_TOKEN }
          : {}),
      },
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

export default async function CredentialsPage() {
  const initial = await fetchInitial();
  // MC-CRED-1: never pass an SSR-computed base URL into the client — in the
  // browser it resolved to the docker-internal http://edge:8787 (mixed content).
  // CredentialsClient builds same-origin URLs via getApiBaseUrl() at call time.
  return <CredentialsClient initialSnapshot={initial} />;
}
