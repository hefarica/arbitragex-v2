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
import { CredentialsClient, type CredentialsSnapshot } from "./CredentialsClient";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export const metadata: Metadata = {
  title: "Credentials — ArbitrageX v2",
  description: "Operator credentials store — RPC, CEX, MEV, prices. Live status per credential.",
};

const EDGE_URL = process.env["NEXT_PUBLIC_EDGE_URL"] ?? "http://localhost:8787";

async function fetchInitial(): Promise<CredentialsSnapshot> {
  try {
    const res = await fetch(`${EDGE_URL}/api/credentials`, {
      headers: { accept: "application/json" },
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
  return <CredentialsClient initialSnapshot={initial} edgeUrl={EDGE_URL} />;
}
